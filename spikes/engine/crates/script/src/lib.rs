//! Spike B (guide Prompt 5): rquickjs running a script against a toy document API, natively and in
//! wasm32. Spike quality; ScriptRuntime is Prompt 25 and the `execute` doc API is Prompt 26.
//!
//! The script sees one global, `doc`, with `create(kind, parent) -> id` and `set(id, key, value)`.
//! Calls are recorded as ops and returned only when the script finishes: a script that throws or is
//! stopped changes nothing (one transaction, as Prompt 26 will apply it).
//!
//! Limits: a memory limit, a stack limit (mandatory on wasm32, see rquickjs's README) and an
//! interrupt budget counted in interrupt-handler calls (QuickJS polls it about every 10,000
//! bytecode instructions), so a stopped script stops at the same point on every machine. No clock.

// The wasm32 exports below need `#[unsafe(no_mangle)]`; they contain no unsafe code.
#![allow(unsafe_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Exception, Function, Runtime, Value};

pub use vellum_wasm::vellum_format_version;

/// One recorded document call.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Create { id: u32, kind: String, parent: u32 },
    SetNumber { id: u32, key: String, value: f64 },
    SetText { id: u32, key: String, value: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// Interrupt-handler calls before the script is stopped.
    pub interrupt_budget: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            stack_bytes: 256 * 1024,
            interrupt_budget: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    Interrupted,
    OutOfMemory,
    /// A JS exception, as its message (and stack, when there is one).
    Exception(String),
    /// The runtime could not be created.
    Setup(String),
}

/// 100 creates and 400 sets: four properties on each of 100 nodes. TEST DATA.
pub const SCRIPT_500: &str = r##"
const frame = doc.create("frame", 0);
doc.set(frame, "name", "Cards");
doc.set(frame, "w", 1200);
doc.set(frame, "h", 800);
doc.set(frame, "fill", "#ffffff");
for (let i = 0; i < 99; i++) {
  const card = doc.create(i % 3 === 0 ? "text" : "rect", frame);
  doc.set(card, "x", 24 + (i % 10) * 112);
  doc.set(card, "y", 24 + Math.floor(i / 10) * 72);
  doc.set(card, "w", 96);
  doc.set(card, i % 3 === 0 ? "text" : "fill", i % 3 === 0 ? "Card " + i : "#2563eb");
}
"##;

/// A runtime and context with the limits applied. Reusable across scripts.
pub struct Engine {
    runtime: Runtime,
    context: Context,
    polls: Rc<Cell<u64>>,
    budget: u64,
}

impl Engine {
    pub fn new(limits: &Limits) -> Result<Self, ScriptError> {
        let setup = |e: rquickjs::Error| ScriptError::Setup(e.to_string());
        let runtime = Runtime::new().map_err(setup)?;
        runtime.set_memory_limit(limits.memory_bytes);
        runtime.set_max_stack_size(limits.stack_bytes);
        let polls = Rc::new(Cell::new(0u64));
        let budget = limits.interrupt_budget;
        let counter = polls.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            counter.set(counter.get() + 1);
            counter.get() > budget
        })));
        let context = Context::full(&runtime).map_err(setup)?;
        Ok(Self {
            runtime,
            context,
            polls,
            budget,
        })
    }

    /// Runs `src` with a fresh `doc` and returns its ops, or why it stopped.
    pub fn run(&self, src: &str) -> Result<Vec<Op>, ScriptError> {
        self.polls.set(0);
        let ops = Rc::new(RefCell::new(Vec::<Op>::new()));
        let result = self.context.with(|ctx| -> Result<(), ScriptError> {
            install_doc(&ctx, ops.clone()).map_err(|e| ScriptError::Setup(e.to_string()))?;
            // Each run gets its own scope, so a kept engine does not see the last run's bindings.
            let scoped = format!("(() => {{\n{src}\n}})()");
            ctx.eval::<Value, _>(scoped)
                .catch(&ctx)
                .map(drop)
                .map_err(|caught| self.classify(caught))
        });
        while self.runtime.is_job_pending() {
            if self.runtime.execute_pending_job().is_err() {
                break;
            }
        }
        // Collect the stopped script's garbage now, so the next run starts under the memory limit.
        self.runtime.run_gc();
        // The context keeps `doc` (and so the log) alive until the next run replaces it.
        result.map(|()| std::mem::take(&mut *ops.borrow_mut()))
    }

    fn classify(&self, caught: CaughtError<'_>) -> ScriptError {
        let message = caught.to_string();
        if self.polls.get() > self.budget {
            ScriptError::Interrupted
        } else if message.contains("out of memory")
            || matches!(caught, CaughtError::Error(rquickjs::Error::Allocation))
        {
            ScriptError::OutOfMemory
        } else {
            ScriptError::Exception(message)
        }
    }
}

/// One-shot: a new engine, one script.
pub fn run(src: &str, limits: &Limits) -> Result<Vec<Op>, ScriptError> {
    Engine::new(limits)?.run(src)
}

fn install_doc<'js>(ctx: &Ctx<'js>, ops: Rc<RefCell<Vec<Op>>>) -> rquickjs::Result<()> {
    let doc = rquickjs::Object::new(ctx.clone())?;
    let created = Rc::new(Cell::new(0u32));

    let (log, next) = (ops.clone(), created.clone());
    let create = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, kind: String, parent: u32| -> rquickjs::Result<u32> {
            if parent > next.get() {
                return Err(Exception::throw_range(&ctx, "unknown parent"));
            }
            next.set(next.get() + 1);
            let id = next.get();
            log.borrow_mut().push(Op::Create { id, kind, parent });
            Ok(id)
        },
    )?;
    doc.set("create", create)?;

    let (log, known) = (ops, created);
    let set = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, id: u32, key: String, value: Value<'js>| -> rquickjs::Result<()> {
            if id == 0 || id > known.get() {
                return Err(Exception::throw_range(&ctx, "unknown node"));
            }
            let op = if let Some(n) = value.as_number() {
                Op::SetNumber { id, key, value: n }
            } else if let Some(s) = value.as_string() {
                Op::SetText {
                    id,
                    key,
                    value: s.to_string()?,
                }
            } else {
                return Err(Exception::throw_type(
                    &ctx,
                    "value must be a number or a string",
                ));
            };
            log.borrow_mut().push(op);
            Ok(())
        },
    )?;
    doc.set("set", set)?;
    ctx.globals().set("doc", doc)
}

thread_local! {
    static WARM: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn op_count(result: Result<Vec<Op>, ScriptError>) -> u32 {
    match result {
        Ok(ops) => ops.len() as u32,
        Err(e) => {
            LAST_ERROR.with_borrow_mut(|last| *last = format!("{e:?}"));
            0
        }
    }
}

/// wasm32 export: the last error as UTF-8, `script_last_error_len` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn script_last_error_ptr() -> *const u8 {
    LAST_ERROR.with_borrow(|last| last.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn script_last_error_len() -> usize {
    LAST_ERROR.with_borrow(|last| last.len())
}

/// wasm32 export: runs `SCRIPT_500` on a new engine; returns the op count (0 on error).
#[unsafe(no_mangle)]
pub extern "C" fn script_run_500_cold() -> u32 {
    op_count(run(SCRIPT_500, &Limits::default()))
}

/// wasm32 export: runs `SCRIPT_500` on an engine kept between calls; returns the op count.
#[unsafe(no_mangle)]
pub extern "C" fn script_run_500_warm() -> u32 {
    WARM.with_borrow_mut(|warm| {
        let engine = match warm {
            Some(engine) => engine,
            None => warm.insert(Engine::new(&Limits::default()).expect("engine")),
        };
        op_count(engine.run(SCRIPT_500))
    })
}
