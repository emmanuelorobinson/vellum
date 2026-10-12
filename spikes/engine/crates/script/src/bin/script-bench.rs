//! Native timing for the 500-op script (measure-script.mjs runs it). Prints JSON.
//!
//!   cargo run --release -p spike-script --bin script-bench -- [runs]

use std::time::Instant;

use spike_script::{Engine, Limits, SCRIPT_500, run};

fn stats(mut xs: Vec<f64>) -> String {
    xs.sort_by(f64::total_cmp);
    let p95 = xs[(xs.len() as f64 * 0.95).ceil() as usize - 1];
    format!(
        "{{\"medianMs\": {:.3}, \"p95Ms\": {p95:.3}, \"runs\": {}}}",
        xs[xs.len() / 2],
        xs.len()
    )
}

fn main() {
    let runs: usize = std::env::args()
        .nth(1)
        .map_or(200, |a| a.parse().expect("runs"));
    let limits = Limits::default();
    let time = |f: &dyn Fn() -> usize| {
        (0..10).for_each(|_| {
            f();
        });
        (0..runs)
            .map(|_| {
                let t = Instant::now();
                assert_eq!(f(), 500);
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect::<Vec<_>>()
    };
    let cold = time(&|| run(SCRIPT_500, &limits).expect("run").len());
    let engine = Engine::new(&limits).expect("engine");
    let warm = time(&|| engine.run(SCRIPT_500).expect("run").len());
    println!("{{\"cold\": {}, \"warm\": {}}}", stats(cold), stats(warm));
}
