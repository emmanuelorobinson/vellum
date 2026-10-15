//! Original sandbox regression corpus, shared by native and WASM. TEST DATA.
use std::{cell::RefCell, rc::Rc};
use vellum_core::ports::{ScriptApi, ScriptError, ScriptLimits, ScriptRuntime, ScriptValue};

pub const LIMITS: ScriptLimits = ScriptLimits {
    time_ms: 100,
    max_ops: 32,
    max_nodes: 8,
    memory_mb: 8,
};

/// Every source must fail with a typed error. These sources never need real host capabilities.
pub const ESCAPES: &[(&str, &str)] = &[
    ("exponent", "return 1.234 ** 5.678"),
    ("exponent template", "return `${1.234 ** 5.678}`"),
    (
        "exponent after unicode whitespace",
        "\u{a0}--> '\nreturn 1.234**5.678",
    ),
    (
        "exponent after unicode newline",
        "// comment\u{2028}--> '\nreturn 2**3",
    ),
    ("sparse array", "const x=[7];x.length=2;x.extra=9;return x"),
    ("network", "fetch('https://invalid.example')"),
    ("xhr", "new XMLHttpRequest()"),
    ("filesystem", "require('fs').readFileSync('/etc/passwd')"),
    ("process", "process.exit(1)"),
    ("quickjs std", "std.open('file','r')"),
    ("quickjs os", "os.exec(['true'])"),
    ("timeout", "setTimeout(()=>doc.echo(1),0)"),
    ("interval", "setInterval(()=>{},0)"),
    ("microtask", "queueMicrotask(()=>doc.echo(1))"),
    ("worker", "new Worker('file')"),
    ("wasm", "WebAssembly.compile(new Uint8Array())"),
    ("static import", "import x from 'file'"),
    ("dynamic import", "import('file')"),
    ("caught dynamic import", "try{import('file')}catch{}"),
    ("promise", "new Promise(()=>doc.echo(1))"),
    ("async", "(async()=>{doc.echo(1)})()"),
    ("caught async", "try{(async()=>{doc.echo(1)})()}catch{}"),
    ("date", "Date.now()"),
    ("performance", "performance.now()"),
    ("crypto", "crypto.getRandomValues([])"),
    ("eval", "eval('process')"),
    ("caught eval", "try{eval('1')}catch{};doc.echo(1)"),
    (
        "constructor",
        "({}).constructor.constructor('return process')()",
    ),
    (
        "api constructor",
        "doc.echo.constructor('return process')()",
    ),
    (
        "async constructor",
        "(async()=>{}).constructor('return process')()",
    ),
    (
        "generator constructor",
        "(function*(){}).constructor('yield process')().next()",
    ),
    ("prototype assignment", "Object.prototype.polluted=1"),
    (
        "prototype descriptor",
        "Object.defineProperty(Object.prototype,'polluted',{value:1})",
    ),
    ("api tamper", "doc.echo=()=>1"),
    (
        "prototype change",
        "Object.setPrototypeOf(doc,{echo(){return 1}})",
    ),
    (
        "prototype data",
        "doc.echo(JSON.parse('{\"__proto__\":{\"polluted\":true}}'))",
    ),
    ("getter", "doc.echo({get x(){while(true){}}})"),
    ("proxy", "doc.echo(new Proxy({},{get(){while(true){}}}))"),
    ("cycle", "const x={};x.x=x;return x"),
    ("nonfinite", "doc.echo(Infinity)"),
    ("function value", "doc.echo(()=>1)"),
    ("symbol value", "doc.echo(Symbol('x'))"),
    (
        "deep boundary",
        "let x={};for(let i=0;i<100;i++)x={x};doc.echo(x)",
    ),
    ("infinite loop", "while(true){}"),
    ("caught interrupt", "for(;;){try{while(true){}}catch{}}"),
    (
        "memory bomb",
        "const xs=[];for(;;)xs.push(new Array(100000).fill(1))",
    ),
    ("recursion", "function f(){return f()+1};f()"),
];

#[derive(Default)]
pub struct RecordingApi(pub RefCell<Vec<(String, Vec<ScriptValue>)>>);
impl ScriptApi for RecordingApi {
    fn functions(&self) -> &[&str] {
        &["echo", "insert"]
    }
    fn node_cost(&self, function: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
        Ok(u32::from(function == "insert"))
    }
    fn call(&self, function: &str, args: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
        self.0.borrow_mut().push((function.into(), args.into()));
        Ok(args.first().cloned().unwrap_or_default())
    }
}

pub fn check_escapes(runtime: &dyn ScriptRuntime) {
    for (name, source) in ESCAPES {
        let result = runtime.run(source, Rc::new(RecordingApi::default()), LIMITS);
        assert!(
            result.is_err(),
            "escape {name} unexpectedly succeeded: {result:?}"
        );
    }
    assert_eq!(
        runtime.run(
            ESCAPES.iter().find(|x| x.0 == "memory bomb").unwrap().1,
            Rc::new(RecordingApi::default()),
            LIMITS
        ),
        Err(ScriptError::MemoryLimit)
    );
    assert_eq!(
        runtime.run("while(true){}", Rc::new(RecordingApi::default()), LIMITS),
        Err(ScriptError::Timeout)
    );
    assert_eq!(
        runtime.run(
            "for(let i=0;i<33;i++)doc.echo(i)",
            Rc::new(RecordingApi::default()),
            LIMITS
        ),
        Err(ScriptError::OpLimit)
    );
    assert_eq!(
        runtime.run(
            "for(let i=0;i<9;i++)doc.insert({})",
            Rc::new(RecordingApi::default()),
            LIMITS
        ),
        Err(ScriptError::NodeLimit)
    );
    assert_eq!(
        runtime
            .run("return 7", Rc::new(RecordingApi::default()), LIMITS)
            .unwrap()
            .value,
        ScriptValue::Number(7.)
    );
}

pub const DETERMINISTIC: &[&str] = &[
    "const x=[7];Object.defineProperty(x,'0',{enumerable:false});return doc.echo(x)",
    "return [Math.sqrt(2),Math.asin(0.1),Math.acos(0.1),Math.atan(0.1),Math.atan2(0.1,0.2),Math.cosh(0.1),Math.sinh(0.1),Math.tanh(0.1),Math.acosh(2),Math.asinh(0.1),Math.atanh(0.1),Math.expm1(0.1),Math.log1p(0.1),Math.log2(3),Math.log10(3),Math.cbrt(2),Math.hypot(3,4,5)]",
    "return [Math.sin(1e20),Math.cos(1e20),Math.tan(1e20),Math.exp(0.1),Math.log(123.456),Math.pow(1.234,5.678)]",
    "for(let i=0;i<8;i++)doc.insert({x:Math.floor(Math.random()*1000),y:Math.random(),name:'layer-'+i});return 8",
    "const x={z:1,a:[true,false,null,'text']};return doc.echo(x)",
    "return [Math.random(),Math.random(),Math.random()]",
    "let sum=0;for(let i=0;i<20;i++){sum+=i;doc.echo(sum)}return sum",
    "return JSON.parse(JSON.stringify({text:'αβ',n:0.1+0.2}))",
];

/// Stable text of returned values and recorded calls; the WASM lane compares it byte for byte.
pub fn transcript(runtime: &dyn ScriptRuntime) -> String {
    let mut transcript = String::new();
    for source in DETERMINISTIC {
        let a = Rc::new(RecordingApi::default());
        let b = Rc::new(RecordingApi::default());
        let first = runtime.run(source, a.clone(), LIMITS).unwrap();
        assert_eq!(first, runtime.run(source, b.clone(), LIMITS).unwrap());
        assert_eq!(*a.0.borrow(), *b.0.borrow());
        transcript.push_str(&format!("{:?}|{:?}\n", first, a.0.borrow()));
    }
    transcript
}
