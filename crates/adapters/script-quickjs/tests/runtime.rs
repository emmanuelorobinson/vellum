use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};
use vellum_core::ports::{ScriptApi, ScriptError, ScriptLimits, ScriptRuntime, ScriptValue};
use vellum_script_quickjs::QuickJsRuntime;

const LIMITS: ScriptLimits = ScriptLimits {
    time_ms: 200,
    max_ops: 10,
    max_nodes: 5,
    memory_mb: 8,
};

#[derive(Default)]
struct Api {
    calls: RefCell<Vec<(String, Vec<ScriptValue>)>>,
}
impl ScriptApi for Api {
    fn functions(&self) -> &[&str] {
        &["echo", "insert", "copy", "refuse"]
    }
    fn node_cost(&self, function: &str, args: &[ScriptValue]) -> Result<u32, ScriptError> {
        if matches!(function, "insert" | "copy") {
            Ok(match args.first() {
                Some(ScriptValue::Number(n)) => *n as u32,
                _ => 1,
            })
        } else {
            Ok(0)
        }
    }
    fn call(&self, function: &str, args: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
        self.calls.borrow_mut().push((function.into(), args.into()));
        if function == "refuse" {
            return Err(ScriptError::Api {
                message: "refused".into(),
            });
        }
        Ok(args.first().cloned().unwrap_or_default())
    }
}
fn runtime() -> QuickJsRuntime {
    let start = Instant::now();
    QuickJsRuntime::new(move || start.elapsed().as_millis() as u64, 42)
}
#[test]
fn runtime_contract() {
    vellum_contracts::suites::script::run(&runtime());
}
#[test]
fn values_and_explicit_return_cross_as_copies() {
    let api = Rc::new(Api::default());
    let out = runtime()
        .run(
            "const x = {a:[1,true,null,'ok']}; const y = doc.echo(x); y.a[0]=9; return [x,y];",
            api.clone(),
            LIMITS,
        )
        .unwrap();
    assert!(matches!(out.value, ScriptValue::List(ref xs) if xs.len() == 2 && xs[0] != xs[1]));
    assert_eq!(api.calls.borrow().len(), 1);
}
#[test]
fn limits_are_sticky_and_stop_further_calls_even_when_caught() {
    for (code, limits, error, admitted) in [
        (
            "for(let i=0;i<4;i++){try{doc.echo(i)}catch{}}",
            ScriptLimits {
                max_ops: 2,
                ..LIMITS
            },
            ScriptError::OpLimit,
            2,
        ),
        (
            "doc.insert(2); try{doc.copy(4)}catch{}; try{doc.echo(0)}catch{}",
            LIMITS,
            ScriptError::NodeLimit,
            1,
        ),
        (
            "try{doc.refuse()}catch{}; try{doc.echo(0)}catch{}",
            LIMITS,
            ScriptError::Api {
                message: "refused".into(),
            },
            1,
        ),
    ] {
        let api = Rc::new(Api::default());
        assert_eq!(runtime().run(code, api.clone(), limits), Err(error));
        assert_eq!(api.calls.borrow().len(), admitted);
    }
}
#[test]
fn zero_budgets_do_not_mean_unlimited() {
    assert_eq!(
        runtime().run(
            "return 1",
            Rc::new(Api::default()),
            ScriptLimits {
                time_ms: 0,
                ..LIMITS
            }
        ),
        Err(ScriptError::Timeout)
    );
    assert_eq!(
        runtime().run(
            "return 1",
            Rc::new(Api::default()),
            ScriptLimits {
                memory_mb: 0,
                ..LIMITS
            }
        ),
        Err(ScriptError::MemoryLimit)
    );
    assert_eq!(
        runtime().run(
            "doc.echo(0)",
            Rc::new(Api::default()),
            ScriptLimits {
                max_ops: 0,
                ..LIMITS
            }
        ),
        Err(ScriptError::OpLimit)
    );
    assert_eq!(
        runtime().run(
            "doc.insert(1)",
            Rc::new(Api::default()),
            ScriptLimits {
                max_nodes: 0,
                ..LIMITS
            }
        ),
        Err(ScriptError::NodeLimit)
    );
}
#[test]
fn escape_corpus() {
    vellum_testkit::script::check_escapes(&runtime());
}
#[test]
fn seeded_runs_and_fresh_realms() {
    let rt = runtime();
    let code = "doc.echo(Math.random()); return [Math.random(), Math.random()]";
    let a = Rc::new(Api::default());
    let b = Rc::new(Api::default());
    assert_eq!(
        rt.run(code, a.clone(), LIMITS),
        rt.run(code, b.clone(), LIMITS)
    );
    assert_eq!(*a.calls.borrow(), *b.calls.borrow());
    assert_ne!(
        rt.run("return Math.random()", Rc::new(Api::default()), LIMITS),
        QuickJsRuntime::new(|| 0, 43).run("return Math.random()", Rc::new(Api::default()), LIMITS)
    );
    rt.run(
        "globalThis.privateValue = 1; return 0",
        Rc::new(Api::default()),
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        rt.run(
            "return typeof privateValue",
            Rc::new(Api::default()),
            LIMITS
        )
        .unwrap()
        .value,
        ScriptValue::Text("undefined".into())
    );
}
#[test]
fn syntax_runtime_and_host_errors_remain_distinct() {
    assert!(matches!(
        runtime().run("const =", Rc::new(Api::default()), LIMITS),
        Err(ScriptError::Syntax { .. })
    ));
    assert!(matches!(
        runtime().run(
            "throw new Error('failure')",
            Rc::new(Api::default()),
            LIMITS
        ),
        Err(ScriptError::Runtime { .. })
    ));
    assert!(matches!(
        runtime().run("doc.missing()", Rc::new(Api::default()), LIMITS),
        Err(ScriptError::Runtime { .. })
    ));
}
#[test]
fn injected_clock_enforces_timeout_without_exposing_time() {
    let clock = Cell::new(0u64);
    let rt = QuickJsRuntime::new(
        move || {
            let t = clock.get();
            clock.set(t + 1);
            t
        },
        1,
    );
    assert_eq!(
        rt.run("while(true){}", Rc::new(Api::default()), LIMITS),
        Err(ScriptError::Timeout)
    );
}
#[test]
fn wall_time_interrupts_loop_promptly() {
    let start = Instant::now();
    assert_eq!(
        runtime().run(
            "while(true){}",
            Rc::new(Api::default()),
            ScriptLimits {
                time_ms: 20,
                ..LIMITS
            }
        ),
        Err(ScriptError::Timeout)
    );
    assert!(start.elapsed().as_millis() < 1000);
}

#[test]
fn caught_out_of_memory_still_fails_and_never_calls_the_host() {
    let api = Rc::new(Api::default());
    assert_eq!(
        runtime().run(
            "try{const xs=[];for(;;)xs.push(new Array(100000).fill(1))}catch{};doc.echo(1)",
            api.clone(),
            LIMITS
        ),
        Err(ScriptError::MemoryLimit)
    );
    assert!(api.calls.borrow().is_empty());
}
#[test]
fn diagnostics_do_not_invoke_user_getters_or_string_conversion() {
    let api = Rc::new(Api::default());
    let result=runtime().run("const e=new Error('original');Object.defineProperty(e,'message',{get(){doc.echo('message');return 'bad'}});Object.defineProperty(e,'constructor',{get(){doc.echo('constructor');return SyntaxError}});throw e",api.clone(),LIMITS);
    assert!(matches!(result, Err(ScriptError::Runtime { .. })));
    assert!(api.calls.borrow().is_empty());
}
#[test]
fn slow_host_call_stops_before_the_next_call() {
    struct Slow {
        calls: Cell<u32>,
        now: Rc<Cell<u64>>,
    }
    impl ScriptApi for Slow {
        fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
            Ok(0)
        }
        fn functions(&self) -> &[&str] {
            &["slow"]
        }
        fn call(&self, _: &str, _: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
            self.calls.set(self.calls.get() + 1);
            self.now.set(self.now.get() + 30);
            Ok(ScriptValue::Null)
        }
    }
    let now = Rc::new(Cell::new(0));
    let api = Rc::new(Slow {
        calls: Cell::new(0),
        now: now.clone(),
    });
    let rt = QuickJsRuntime::new(move || now.get(), 42);
    assert_eq!(
        rt.run(
            "doc.slow();doc.slow()",
            api.clone(),
            ScriptLimits {
                time_ms: 10,
                ..LIMITS
            }
        ),
        Err(ScriptError::Timeout)
    );
    assert_eq!(api.calls.get(), 1);
}
#[test]
fn sparse_arrays_are_rejected_as_non_json_data() {
    assert!(matches!(
        runtime().run("return new Array(1000000)", Rc::new(Api::default()), LIMITS),
        Err(ScriptError::Forbidden { .. })
    ));
}

#[test]
fn exponent_syntax_is_refused_without_rejecting_strings_comments_or_template_text() {
    for source in [
        "return 2 ** 3",
        "let x=2;x **=3",
        "return `${2 ** 3}`",
        "return `${`${2 ** 3}`}`",
    ] {
        assert!(
            matches!(
                runtime().run(source, Rc::new(Api::default()), LIMITS),
                Err(ScriptError::Forbidden { .. })
            ),
            "{source}"
        );
    }
    for source in [
        "return '**'",
        "// **\nreturn 1",
        "/* ** */return 1",
        "return `** ${1+2}`",
        "return `a${`** ${1+2}`}b`",
        "return 'escaped \\\' **'",
    ] {
        runtime()
            .run(source, Rc::new(Api::default()), LIMITS)
            .unwrap();
    }
}
#[test]
fn portable_math_obeys_js_edge_cases() {
    assert_eq!(runtime().run("return [Math.pow(2,10),Number.isNaN(Math.pow(1,Infinity)),Number.isNaN(Math.sin()),Math.hypot(3,4),Math.hypot(NaN,Infinity) === Infinity]",Rc::new(Api::default()),LIMITS).unwrap().value,ScriptValue::List(vec![ScriptValue::Number(1024.),ScriptValue::Bool(true),ScriptValue::Bool(true),ScriptValue::Number(5.),ScriptValue::Bool(true)]));
}

#[test]
fn exponent_guard_handles_javascript_line_terminators_and_html_comments() {
    for source in [
        "// quote '\u{2028}return 1.234 ** 5.678",
        "// comment\u{2029}return 2**3",
        "// comment\u{2028}--> '\nreturn 2**3",
        "<!-- '\nreturn 2**3",
        "<!-- `\nreturn 2**3",
        "--> '\nreturn 2**3",
    ] {
        assert!(
            matches!(
                runtime().run(source, Rc::new(Api::default()), LIMITS),
                Err(ScriptError::Forbidden { .. })
            ),
            "{source}"
        );
    }
    assert!(
        runtime()
            .run("return /x/", Rc::new(Api::default()), LIMITS)
            .is_err(),
        "regex compiler must remain unavailable to keep slash unambiguous"
    );
}

#[test]
fn array_boundaries_preserve_indices_and_refuse_disguised_sparse_arrays() {
    let result = runtime()
        .run(
            "const x=[7];Object.defineProperty(x,'0',{enumerable:false});return doc.echo(x)",
            Rc::new(Api::default()),
            LIMITS,
        )
        .unwrap();
    assert_eq!(
        result.value,
        ScriptValue::List(vec![ScriptValue::Number(7.)])
    );
    assert!(
        runtime()
            .run(
                "const x=[7];x.length=2;x.extra=9;return x",
                Rc::new(Api::default()),
                LIMITS
            )
            .is_err()
    );
}
#[test]
fn non_json_and_oversized_host_results_fail_without_a_followup_call() {
    struct Bad {
        value: ScriptValue,
        calls: Cell<u32>,
    }
    impl ScriptApi for Bad {
        fn functions(&self) -> &[&str] {
            &["bad"]
        }
        fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
            Ok(0)
        }
        fn call(&self, _: &str, _: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.value.clone())
        }
    }
    for value in [
        ScriptValue::Number(f64::NAN),
        ScriptValue::Text("x".repeat(2 * 1024 * 1024)),
    ] {
        let api = Rc::new(Bad {
            value,
            calls: Cell::new(0),
        });
        assert!(
            runtime()
                .run(
                    "try{doc.bad()}catch{};try{doc.bad()}catch{}",
                    api.clone(),
                    LIMITS
                )
                .is_err()
        );
        assert_eq!(api.calls.get(), 1);
    }
}

#[test]
fn source_preflight_has_a_bounded_nesting_stack() {
    let source = format!("return {}1{}", "`${".repeat(256), "}`".repeat(256));
    assert!(matches!(
        runtime().run(&source, Rc::new(Api::default()), LIMITS),
        Err(ScriptError::Forbidden { .. })
    ));
}

#[test]
fn deadline_includes_sandbox_setup_and_source_preflight() {
    struct SetupApi {
        now: Rc<Cell<u64>>,
        calls: Cell<u32>,
    }
    impl ScriptApi for SetupApi {
        fn functions(&self) -> &[&str] {
            self.now.set(100);
            &["echo"]
        }
        fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
            Ok(0)
        }
        fn call(&self, _: &str, _: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
            self.calls.set(self.calls.get() + 1);
            Ok(ScriptValue::Null)
        }
    }
    let now = Rc::new(Cell::new(0));
    let api = Rc::new(SetupApi {
        now: now.clone(),
        calls: Cell::new(0),
    });
    let rt = QuickJsRuntime::new(move || now.get(), 42);
    assert_eq!(
        rt.run(
            "doc.echo()",
            api.clone(),
            ScriptLimits {
                time_ms: 20,
                ..LIMITS
            }
        ),
        Err(ScriptError::Timeout)
    );
    assert_eq!(api.calls.get(), 0);
}

#[test]
fn unicode_whitespace_cannot_hide_exponent_syntax_after_an_html_comment() {
    for space in [
        '\u{a0}', '\u{1680}', '\u{2000}', '\u{200a}', '\u{202f}', '\u{205f}', '\u{3000}',
        '\u{feff}',
    ] {
        let source = format!("{space}--> '\nreturn 1.234**5.678");
        assert!(
            matches!(
                runtime().run(&source, Rc::new(Api::default()), LIMITS),
                Err(ScriptError::Forbidden { .. })
            ),
            "{source}"
        );
    }
}
