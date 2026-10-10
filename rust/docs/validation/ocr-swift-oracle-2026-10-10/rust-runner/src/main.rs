// Prints bello-platform's local OCR text for one image (stdout) and its time (stderr).
fn main() {
    let path = std::env::args().nth(1).expect("usage: rust-ocr-runner IMAGE");
    let start = std::time::Instant::now();
    match bello_platform::Platform::new().recognize_text(std::path::Path::new(&path), None) {
        Ok(text) => {
            eprintln!("rust-ms {:.1}", start.elapsed().as_secs_f64() * 1000.0);
            print!("{text}");
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
