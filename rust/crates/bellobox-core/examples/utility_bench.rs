//! Small deterministic-workload core benchmark. It does not measure UI frame time.
#[cfg(feature = "developer-tools")]
fn main() {
    use bellobox_core::developer;
    use std::{hint::black_box, time::Instant};
    let mut results = Vec::new();
    for tool in developer::catalog() {
        let options = developer::example_options(tool.id);
        // Reject error-only paths as a benchmark of functionality.
        if let Err(error) = developer::execute(tool.id, tool.example, options) {
            results
                .push(serde_json::json!({"tool": tool.id, "status": "skipped", "reason": error}));
            continue;
        }
        for _ in 0..10 {
            black_box(developer::execute(tool.id, tool.example, options).unwrap());
        }
        let mut micros = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let output = developer::execute(
                black_box(tool.id),
                black_box(tool.example),
                black_box(options),
            )
            .unwrap();
            black_box(output);
            micros.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        }
        micros.sort_by(f64::total_cmp);
        results.push(serde_json::json!({"tool": tool.id, "status":"measured", "samples":micros.len(), "input_bytes":tool.example.len()+options.len(), "p50_us":micros[49],"p95_us":micros[94],"p99_us":micros[98]}));
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"method":"warm example-input CPU execution; synthetic public fixtures; no UI measurement", "debug_assertions":cfg!(debug_assertions), "results":results})).unwrap());
}
#[cfg(not(feature = "developer-tools"))]
fn main() {
    eprintln!("Enable developer-tools to run this benchmark");
    std::process::exit(2);
}
