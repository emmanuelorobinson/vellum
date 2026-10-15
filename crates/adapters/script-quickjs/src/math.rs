//! Identical software math in native/WASM. QuickJS otherwise calls the platform's C libm.
use rquickjs::{
    Coerced, Ctx, Function, Object,
    function::{Opt, Rest},
};
type Unary = (&'static str, fn(f64) -> f64);

pub fn install(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    let math: Object = ctx.globals().get("Math")?;
    let unary: &[Unary] = &[
        ("sqrt", libm::sqrt),
        ("sin", libm::sin),
        ("cos", libm::cos),
        ("tan", libm::tan),
        ("asin", libm::asin),
        ("acos", libm::acos),
        ("atan", libm::atan),
        ("sinh", libm::sinh),
        ("cosh", libm::cosh),
        ("tanh", libm::tanh),
        ("asinh", libm::asinh),
        ("acosh", libm::acosh),
        ("atanh", libm::atanh),
        ("exp", libm::exp),
        ("expm1", libm::expm1),
        ("log", libm::log),
        ("log1p", libm::log1p),
        ("log2", libm::log2),
        ("log10", libm::log10),
        ("cbrt", libm::cbrt),
    ];
    for &(name, operation) in unary {
        math.set(
            name,
            Function::new(ctx.clone(), move |x: Opt<Coerced<f64>>| {
                operation(number(x))
            })?,
        )?;
    }
    math.set(
        "atan2",
        Function::new(ctx.clone(), |x: Opt<Coerced<f64>>, y: Opt<Coerced<f64>>| {
            libm::atan2(number(x), number(y))
        })?,
    )?;
    math.set(
        "pow",
        Function::new(ctx.clone(), |x: Opt<Coerced<f64>>, y: Opt<Coerced<f64>>| {
            let (x, y) = (number(x), number(y));
            if x.abs() == 1. && !y.is_finite() {
                f64::NAN
            } else {
                libm::pow(x, y)
            }
        })?,
    )?;
    math.set(
        "hypot",
        Function::new(ctx.clone(), |args: Rest<Coerced<f64>>| {
            if args.iter().any(|x| x.0.is_infinite()) {
                return f64::INFINITY;
            }
            args.iter().fold(0., |total, x| libm::hypot(total, x.0))
        })?,
    )?;
    Ok(())
}
fn number(x: Opt<Coerced<f64>>) -> f64 {
    x.0.map_or(f64::NAN, |x| x.0)
}
