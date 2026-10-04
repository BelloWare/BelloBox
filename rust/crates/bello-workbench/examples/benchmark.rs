//! Deterministic, dependency-free microbenchmarks. Run with `cargo run --release
//! -p bello-workbench --example benchmark`. Reports latency, not GUI frame time.
use bello_workbench::{
    diff::ParsedDiff,
    editor::{Editor, Key, TextBuffer},
};
use std::{hint::black_box, time::Instant};
fn report(name: &str, mut micros: Vec<f64>) {
    micros.sort_by(f64::total_cmp);
    let n = micros.len();
    println!(
        "{{\"benchmark\":\"{name}\",\"iterations\":{n},\"p50_us\":{:.3},\"p95_us\":{:.3},\"max_us\":{:.3}}}",
        micros[n / 2],
        micros[(n * 95 / 100).min(n - 1)],
        micros[n - 1]
    );
}
fn main() {
    let line = "fn sample(value: usize) -> usize { value + 1 } // 😀\n";
    let source = line.repeat(1_048_576 / line.len());
    let mut times = Vec::new();
    for _ in 0..40 {
        let t = Instant::now();
        black_box(TextBuffer::new(source.clone()));
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    report("index_1mib_utf8", times);
    for (position, name) in [
        (0, "typing_1mib_start"),
        (source.len() / 2, "typing_1mib_middle"),
        (source.len(), "typing_1mib_end"),
    ] {
        let mut ed = Editor::new(source.clone());
        let mut times = Vec::new();
        for _ in 0..2_000 {
            ed.set_cursor(position);
            let t = Instant::now();
            ed.insert_text("x");
            ed.key(Key::Backspace);
            black_box(ed.buffer.utf16_offset(ed.cursor));
            times.push(t.elapsed().as_secs_f64() * 1e6);
        }
        report(name, times);
    }
    let ed = Editor::new(source);
    let mut times = Vec::new();
    for first in (0..ed.buffer.line_count() - 40).step_by(19) {
        let t = Instant::now();
        for row in first..first + 40 {
            black_box(ed.buffer.line(row));
        }
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    report("viewport_40_rows_1mib", times);
    let patch = format!(
        "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1,20000 +1,20000 @@\n{}",
        "-removed source line\n+added source line\n".repeat(10_000)
    );
    let mut times = Vec::new();
    for _ in 0..40 {
        let t = Instant::now();
        black_box(ParsedDiff::parse(&patch, 20_000));
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    report("parse_20k_diff_rows", times);
}
