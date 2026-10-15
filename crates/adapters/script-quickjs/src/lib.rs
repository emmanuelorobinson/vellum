//! Synchronous QuickJS sandbox (ADR-005). Each run owns a fresh realm and runtime.
//! Only copied JSON data crosses the allowlisted `doc` API. No jobs are executed.
mod allocator;
mod math;
mod source;
mod values;

use allocator::LimitedAllocator;
use rquickjs::{
    CatchResultExt, CaughtError, Context, Ctx, Exception, Function, Runtime, Value,
    context::intrinsic,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use vellum_core::ports::{
    ScriptApi, ScriptError, ScriptLimits, ScriptOutput, ScriptRuntime, ScriptValue,
};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const BOOTSTRAP: &str = include_str!("bootstrap.js");

/// Inject a monotonic millisecond clock (including on WASM) and a deterministic 32-bit seed.
/// The clock never enters the JS realm; successful runs do not depend on wall time.
pub struct QuickJsRuntime {
    clock: Rc<dyn Fn() -> u64>,
    seed: u32,
}
impl QuickJsRuntime {
    pub fn new(clock: impl Fn() -> u64 + 'static, seed: u32) -> Self {
        Self {
            clock: Rc::new(clock),
            seed,
        }
    }
}

#[derive(Default)]
struct State {
    fatal: RefCell<Option<ScriptError>>,
    oom: Rc<Cell<bool>>,
    ops: Cell<u32>,
    nodes: Cell<u32>,
    bytes: Cell<usize>,
}
impl State {
    fn fail(&self, error: ScriptError) {
        self.fatal.borrow_mut().get_or_insert(error);
    }
    fn error(&self) -> Option<ScriptError> {
        if self.oom.get() {
            Some(ScriptError::MemoryLimit)
        } else {
            self.fatal.borrow().clone()
        }
    }
    fn charge(&self, n: usize) -> Result<(), ScriptError> {
        let Some(left) = self.bytes.get().checked_sub(n) else {
            self.fail(ScriptError::MemoryLimit);
            return Err(ScriptError::MemoryLimit);
        };
        self.bytes.set(left);
        Ok(())
    }
    fn throw(&self, ctx: &Ctx<'_>, error: ScriptError) -> rquickjs::Error {
        self.fail(error);
        Exception::throw_message(ctx, "Script stopped")
    }
}
impl ScriptRuntime for QuickJsRuntime {
    fn run(
        &self,
        src: &str,
        api: Rc<dyn ScriptApi>,
        limits: ScriptLimits,
    ) -> Result<ScriptOutput, ScriptError> {
        if limits.time_ms == 0 {
            return Err(ScriptError::Timeout);
        }
        let memory = usize::try_from(u64::from(limits.memory_mb) * 1024 * 1024)
            .map_err(|_| ScriptError::MemoryLimit)?;
        if memory == 0 || src.len() > memory / 4 {
            return Err(ScriptError::MemoryLimit);
        }
        let started = (self.clock)();
        source::check(src)?;
        if (self.clock)().saturating_sub(started) >= u64::from(limits.time_ms) {
            return Err(ScriptError::Timeout);
        }
        let state = Rc::new(State::default());
        state.bytes.set((memory / 4).min(1024 * 1024));
        let rt = Runtime::new_with_alloc(LimitedAllocator::new(memory, state.oom.clone()))
            .map_err(|_| ScriptError::MemoryLimit)?;
        // The runtime's bounded allocator is the sole memory limit: its failure flag survives
        // a script catching an OOM. An additional engine limit could throw before that flag is set.
        rt.set_max_stack_size(256 * 1024);
        let ctx = Context::custom::<(
            intrinsic::Eval,
            intrinsic::Json,
            intrinsic::MapSet,
            intrinsic::Promise,
        )>(&rt)
        .map_err(|_| ScriptError::MemoryLimit)?;
        let active = Rc::new(Cell::new(false));
        let start = Rc::new(Cell::new(started));
        let (s, on, began, clock) = (
            state.clone(),
            active.clone(),
            start.clone(),
            self.clock.clone(),
        );
        rt.set_interrupt_handler(Some(Box::new(move || {
            if on.get() && clock().saturating_sub(began.get()) >= u64::from(limits.time_ms) {
                s.fail(ScriptError::Timeout)
            }
            s.error().is_some()
        })));
        let s = state.clone();
        rt.set_promise_hook(Some(Box::new(move |_, _, _, _| {
            s.fail(ScriptError::Forbidden {
                what: "async or import".into(),
            })
        })));
        let result = ctx.with(|ctx| -> Result<ScriptOutput, ScriptError> {
            let describe = RefCell::new(None);
            let result = (|| -> rquickjs::Result<ScriptOutput> {
                let names = api.functions();
                let mut unique = std::collections::BTreeSet::new();
                for name in names {
                    if name.is_empty()
                        || !name.bytes().enumerate().all(|(i, c)| {
                            c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit())
                        })
                        || matches!(*name, "__proto__" | "constructor" | "prototype")
                        || !unique.insert(*name)
                    {
                        return Err(state.throw(
                            &ctx,
                            ScriptError::Api {
                                message: "Invalid or duplicate script API function name".into(),
                            },
                        ));
                    }
                }
                let name_json = serde_json::to_string(names).expect("string list");
                let names: Value = ctx.eval(format!("({name_json})"))?;
                let s = state.clone();
                let deny = Function::new(ctx.clone(), move |what: String| {
                    s.fail(if what == "memory" {
                        ScriptError::MemoryLimit
                    } else {
                        ScriptError::Forbidden { what }
                    });
                })?;
                let seed = Cell::new(self.seed);
                let random = Function::new(ctx.clone(), move || {
                    // LCG with explicit u32 wrap: identical on native and WASM, including seed zero.
                    let n = seed.get().wrapping_mul(1664525).wrapping_add(1013904223);
                    seed.set(n);
                    f64::from(n) / 4294967296.0
                })?;
                let s = state.clone();
                let api = api.clone();
                let clock = self.clock.clone();
                let began = start.clone();
                let call = Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'_>, name: String, json: String| -> rquickjs::Result<String> {
                        let operation = (|| -> Result<String, ScriptError> {
                            if clock().saturating_sub(began.get()) >= u64::from(limits.time_ms) {
                                return Err(ScriptError::Timeout);
                            }
                            if let Some(error) = s.error() {
                                return Err(error);
                            }
                            if !api.functions().contains(&name.as_str()) {
                                return Err(ScriptError::Forbidden {
                                    what: "unknown API".into(),
                                });
                            }
                            if s.ops.get() >= limits.max_ops {
                                return Err(ScriptError::OpLimit);
                            }
                            s.charge(json.len())?;
                            let args = values::decode(&json)?;
                            let ScriptValue::List(args) = args else {
                                return Err(ScriptError::Api {
                                    message: "Expected argument list".into(),
                                });
                            };
                            let cost = api.node_cost(&name, &args)?;
                            if clock().saturating_sub(began.get()) >= u64::from(limits.time_ms) {
                                return Err(ScriptError::Timeout);
                            }
                            let nodes = s
                                .nodes
                                .get()
                                .checked_add(cost)
                                .filter(|n| *n <= limits.max_nodes)
                                .ok_or(ScriptError::NodeLimit)?;
                            s.ops.set(s.ops.get() + 1);
                            s.nodes.set(nodes);
                            let value = api.call(&name, &args)?;
                            if clock().saturating_sub(began.get()) >= u64::from(limits.time_ms) {
                                return Err(ScriptError::Timeout);
                            }
                            let result = values::encode(&value, s.bytes.get())?;
                            s.charge(result.len())?;
                            Ok(result)
                        })();
                        operation.map_err(|error| s.throw(&ctx, error))
                    },
                )?;
                math::install(&ctx)?;
                let setup: Function = ctx.eval(BOOTSTRAP)?;
                let bridge: rquickjs::Object =
                    setup.call((call, deny, random, names, state.bytes.get()))?;
                let encode: Function = bridge.get("encode")?;
                describe.replace(Some(bridge.get::<_, Function>("describe")?));
                if (self.clock)().saturating_sub(started) >= u64::from(limits.time_ms) {
                    return Err(state.throw(&ctx, ScriptError::Timeout));
                }
                active.set(true);
                let value: Value =
                    ctx.eval(format!("(function(){{\"use strict\";\n{src}\n}})()"))?;
                if let Some(error) = state.error() {
                    return Err(state.throw(&ctx, error));
                }
                let json: String = encode.call((value,))?;
                state.charge(json.len()).map_err(|e| state.throw(&ctx, e))?;
                let value = values::decode(&json).map_err(|e| state.throw(&ctx, e))?;
                Ok(ScriptOutput {
                    value,
                    logs: vec![],
                })
            })();
            // Decode exceptions under the same interrupt budget; never execute queued jobs.
            let result = result
                .catch(&ctx)
                .map_err(|error| classify(error, describe.borrow().as_ref()));
            if active.get()
                && (self.clock)().saturating_sub(start.get()) >= u64::from(limits.time_ms)
            {
                state.fail(ScriptError::Timeout)
            }
            state.error().map_or(result, Err)
        });
        if rt.is_job_pending() {
            return Err(state.error().unwrap_or(ScriptError::Forbidden {
                what: "async".into(),
            }));
        }
        result
    }
}
fn classify<'js>(error: CaughtError<'js>, describe: Option<&Function<'js>>) -> ScriptError {
    match error {
        CaughtError::Error(rquickjs::Error::Allocation) => ScriptError::MemoryLimit,
        CaughtError::Exception(exception) => {
            let details = describe
                .and_then(|describe| describe.call::<_, String>((exception.into_object(),)).ok())
                .and_then(|json| serde_json::from_str::<(bool, String)>(&json).ok());
            let (syntax, message) = details.unwrap_or((false, "Script threw an error".into()));
            if syntax {
                ScriptError::Syntax { message }
            } else {
                ScriptError::Runtime { message }
            }
        }
        // Arbitrary thrown objects are not coerced (their getters/toString may run user code).
        CaughtError::Value(_) => ScriptError::Runtime {
            message: "Script threw a value".into(),
        },
        CaughtError::Error(error) => ScriptError::Runtime {
            message: error.to_string(),
        },
    }
}
