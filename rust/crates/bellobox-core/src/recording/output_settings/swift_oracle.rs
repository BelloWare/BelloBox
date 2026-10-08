//! Execute the original Swift policy, never a second transcription of its math.
use super::{RecordingOutputSettings, RecordingQualityPreset, tests};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const EXCERPT_SHA256: &str = "402b3a7820e1feb59f741fa3290c1b2a44e9f9427b8700fed98f2b91b534ca5a";

#[test]
fn extracted_swift_policy_matches_all_six_fields_for_every_source_case() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let source = fs::read_to_string(root.join("BelloBox/Recording/RecordingModels.swift")).unwrap();
    let excerpt = format!(
        "{}{}",
        between(
            &source,
            "enum RecordingQualityPreset:",
            "/// What a recording delivers"
        ),
        between(
            &source,
            "struct RecordingOutputSettings:",
            "enum RecordingState:"
        )
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(excerpt.as_bytes())),
        EXCERPT_SHA256,
        "Swift specification changed; review the policy before rebinding the oracle"
    );
    let temp = tempfile::tempdir().unwrap();
    let swift = temp.path().join("Policy.swift");
    fs::write(&swift, format!("import Foundation\n{excerpt}\n{DRIVER}")).unwrap();
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for quality in tests::PRESETS {
        let quality_name = match quality {
            RecordingQualityPreset::Compact => "compact",
            RecordingQualityPreset::Balanced => "balanced",
            RecordingQualityPreset::High => "high",
        };
        for (width, height) in tests::source_cases() {
            inputs.push(json!({"width": width, "height": height, "quality": quality_name}));
            let result = RecordingOutputSettings::make(width, height, quality).unwrap();
            expected.push(json!({
                "width": result.width, "height": result.height,
                "framesPerSecond": result.frames_per_second,
                "videoBitrate": result.video_bitrate,
                "audioSampleRate": result.audio_sample_rate,
                "audioChannelCount": result.audio_channel_count,
            }));
        }
    }
    let input_path = temp.path().join("inputs.json");
    fs::write(&input_path, serde_json::to_vec(&inputs).unwrap()).unwrap();
    let binary = temp.path().join("policy-oracle");
    bounded_output(
        Command::new("/usr/bin/xcrun")
            .arg("swiftc")
            .arg(&swift)
            .arg("-o")
            .arg(&binary)
            .arg("-module-cache-path")
            .arg(temp.path().join("module-cache")),
        temp.path(),
        "compile",
    );
    let output = bounded_output(Command::new(&binary).arg(&input_path), temp.path(), "run");
    let actual: Vec<Value> = serde_json::from_slice(&output).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "Swift mismatch for {}", inputs[index]);
    }
    println!(
        "Swift source SHA256: {:x}",
        Sha256::digest(source.as_bytes())
    );
    println!(
        "Swift excerpt SHA256: {EXCERPT_SHA256}; matched {} cases × 6 fields",
        actual.len()
    );
}

fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    assert_eq!(source.matches(start).count(), 1);
    assert_eq!(source.matches(end).count(), 1);
    &source[source.find(start).unwrap()..source.find(end).unwrap()]
}

const DRIVER: &str = r#"
struct Input: Decodable {
    let width: Double
    let height: Double
    let quality: RecordingQualityPreset
}
let inputs = try JSONDecoder().decode([Input].self,
    from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))
let results = inputs.map {
    RecordingOutputSettings.make(for: CGSize(width: $0.width, height: $0.height), quality: $0.quality)
}
FileHandle.standardOutput.write(try JSONEncoder().encode(results))
"#;

// Compiler and reference process use owned groups, null stdin, bounded output
// and a deadline. They only read generated inputs and the extracted pure policy.
fn bounded_output(command: &mut Command, directory: &Path, label: &str) -> Vec<u8> {
    const LIMIT: u64 = 2 * 1024 * 1024;
    let stdout = directory.join(format!("{label}.stdout"));
    let stderr = directory.join(format!("{label}.stderr"));
    let mut owned = OwnedChild {
        child: command
            .current_dir(directory)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start Swift oracle"),
        reaped: false,
    };
    let started = Instant::now();
    let status = loop {
        for path in [&stdout, &stderr] {
            assert!(
                fs::metadata(path).unwrap().len() <= LIMIT,
                "{label} exceeded output limit"
            );
        }
        if let Some(status) = owned.child.try_wait().unwrap() {
            owned.reaped = true;
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{label} exceeded 60-second deadline"
        );
        thread::sleep(Duration::from_millis(10));
    };
    for path in [&stdout, &stderr] {
        assert!(
            fs::metadata(path).unwrap().len() <= LIMIT,
            "{label} exceeded output limit"
        );
    }
    assert!(
        status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&fs::read(stderr).unwrap())
    );
    fs::read(stdout).unwrap()
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            // SAFETY: this still-owned, unreaped child created its own group.
            // A negative PID targets only that group, including compiler children.
            unsafe {
                kill(-(self.child.id() as i32), 9);
            }
            let _ = self.child.wait();
        }
    }
}
