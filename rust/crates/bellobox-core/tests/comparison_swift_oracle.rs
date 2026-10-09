//! Pinned synthetic TextComparison oracle; no app or real data access.
#![cfg(feature = "developer-tools")]
use sha2::{Digest, Sha256};
const INSPECTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/InspectionTools.swift"
));
const UTILITY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/DeveloperJSON.swift"
));
const FIXTURES: &str = include_str!("fixtures/comparison-swift-oracle.json");
const DRIVER: &str = r####"let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
let rows = try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
func payload(_ value: String) -> [String: Any] {
    ["text": value, "utf8_base64": Data(value.utf8).base64EncodedString()]
}
var results: [[String: Any]] = []
for row in rows {
    var result: [String: Any] = ["id": row["id"]!]
    do {
        let mode = ComparisonMode(rawValue: row["mode"] as! String)!
        let value = try TextComparison.compare(row["left"] as! String, row["right"] as! String,
            mode: mode, ignoreWhitespace: row["ignoreWhitespace"] as! Bool)
        result["ok"] = true
        result["added"] = value.added
        result["removed"] = value.removed
        result["copy"] = payload(value.text)
        result["rows"] = value.rows.map { item -> [String: Any] in
            let kind: String
            switch item.kind { case .same: kind = "same"; case .added: kind = "added"; case .removed: kind = "removed" }
            return ["kind": kind, "text": payload(item.text)]
        }
    } catch { result["ok"] = false; result["error"] = error.localizedDescription }
    results.append(result)
}
let output: [String: Any] = ["results": results, "host": ["os": ProcessInfo.processInfo.operatingSystemVersionString, "foundation": NSFoundationVersionNumber, "locale": Locale.current.identifier]]
FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys]))
"####;
fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn swift_source() -> String {
    assert_eq!(
        digest(INSPECTION),
        "4b74934aa3bdf1bfe88a2d25c5bfd6a63c58b536f7d81f4f03fc9efdab5f2fc7"
    );
    assert_eq!(
        digest(UTILITY),
        "3dc58c0f68e76a0db3205bef989040b0828c13d589f616bab661e1929639069b"
    );
    let excerpt = format!(
        "{}{}",
        &INSPECTION[..INSPECTION.find("enum JWTInspector").unwrap()],
        UTILITY.replacen("import Foundation\n", "", 1)
    );
    assert_eq!(
        digest(&excerpt),
        "11c520fe16ab1eae50509b419014c9283c51498ddbb934fed041e10a3514680e"
    );
    format!("{excerpt}\n{DRIVER}")
}
#[test]
fn pinned_comparison_source_and_synthetic_fixture_binding() {
    assert!(!swift_source().is_empty());
    assert_eq!(
        digest(FIXTURES),
        "6af24b2d1d24e018d084e94427405728cebc55f61c61528900b338d9dedfddfb"
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(rows.len(), 115);
}
mod native {
    use super::*;
    use std::{
        fs,
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    fn run(command: &mut Command, dir: &Path, name: &str) -> Vec<u8> {
        let out = dir.join(format!("{name}.stdout"));
        let err = dir.join(format!("{name}.stderr"));
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let timer = Instant::now();
        let mut child = command
            .stdin(Stdio::null())
            .stdout(fs::File::create(&out).unwrap())
            .stderr(fs::File::create(&err).unwrap())
            .spawn()
            .unwrap();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if timer.elapsed() > Duration::from_secs(120)
                || fs::metadata(&out).unwrap().len() > 262144
                || fs::metadata(&err).unwrap().len() > 262144
            {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{name}: timeout or output bound");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert!(
            fs::metadata(&out).unwrap().len() <= 262144
                && fs::metadata(&err).unwrap().len() <= 262144
        );
        let stdout = fs::read(out).unwrap();
        let stderr = fs::read(err).unwrap();
        println!(
            "{name}: start_unix_utc={start} end_unix_utc={end} elapsed_s={} exit={status}\nstdout={}\nstderr={}",
            timer.elapsed().as_secs_f64(),
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        assert!(status.success(), "{name} failed");
        stdout
    }

    fn rust_observation(row: &serde_json::Value) -> serde_json::Value {
        use bellobox_core::developer::comparison::{Mode, Options, RowKind, compare};
        use std::sync::atomic::AtomicBool;
        let mode = match row["mode"].as_str().unwrap() {
            "Lines" => Mode::Lines,
            "Words" => Mode::Words,
            "JSON fields" => Mode::Json,
            _ => unreachable!(),
        };
        match compare(
            row["left"].as_str().unwrap(),
            row["right"].as_str().unwrap(),
            Options {
                mode,
                ignore_whitespace: row["ignoreWhitespace"].as_bool().unwrap(),
            },
            &AtomicBool::new(false),
        ) {
            Ok(value) => {
                serde_json::json!({"ok":true,"added":value.added,"removed":value.removed,"copy":value.copy_text,
                "rows":value.rows.iter().map(|r| serde_json::json!({"kind":match r.kind { RowKind::Same => "same", RowKind::Added => "added", RowKind::Removed => "removed" }, "text":r.text})).collect::<Vec<_>>() })
            }
            Err(error) => serde_json::json!({"ok":false,"error":error}),
        }
    }
    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "requires native Swift/Foundation; Linux validates binding and Rust compilation only"
    )]
    fn native_swift_comparison_bounded_rows_and_copy() {
        let dir = tempfile::tempdir().unwrap();
        let swift = dir.path().join("main.swift");
        let binary = dir.path().join("oracle");
        fs::write(&swift, swift_source()).unwrap();
        run(
            Command::new("/usr/bin/xcrun").args(["swiftc", "--version"]),
            dir.path(),
            "swift-version",
        );
        run(
            &mut Command::new("/usr/bin/sw_vers"),
            dir.path(),
            "macos-version",
        );
        run(
            Command::new("/usr/bin/xcrun").args(["--sdk", "macosx", "--show-sdk-version"]),
            dir.path(),
            "sdk-version",
        );
        run(
            Command::new("/usr/bin/xcrun")
                .args(["swiftc", "-swift-version", "5"])
                .arg(&swift)
                .arg("-o")
                .arg(&binary),
            dir.path(),
            "compile",
        );
        let rows: Vec<serde_json::Value> = serde_json::from_str(FIXTURES).unwrap();
        for row in &rows {
            let id = row["id"].as_str().unwrap();
            assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let fixture = dir.path().join(format!("{id}.json"));
            fs::write(&fixture, serde_json::to_vec(&vec![row]).unwrap()).unwrap();
            let bytes = run(Command::new(&binary).arg(&fixture), dir.path(), id);
            let native: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let n = &native["results"][0];
            let r = rust_observation(row);
            assert_eq!(
                n["ok"], r["ok"],
                "{id}: acceptance mismatch; native={n} rust={r}"
            );
            if n["ok"] == true {
                assert_eq!(n["added"], r["added"], "{id}: added");
                assert_eq!(n["removed"], r["removed"], "{id}: removed");
                let native_rows = n["rows"].as_array().unwrap();
                let rust_rows = r["rows"].as_array().unwrap();
                assert_eq!(native_rows.len(), rust_rows.len(), "{id}: row count");
                for (a, b) in native_rows.iter().zip(rust_rows) {
                    assert_eq!(a["kind"], b["kind"], "{id}: row kind");
                    assert_eq!(
                        a["text"]["text"].as_str().unwrap().as_bytes(),
                        b["text"].as_str().unwrap().as_bytes(),
                        "{id}: exact row bytes"
                    );
                }
                assert_eq!(
                    n["copy"]["text"].as_str().unwrap().as_bytes(),
                    r["copy"].as_str().unwrap().as_bytes(),
                    "{id}: exact signed copy bytes"
                );
            }
        }
        println!(
            "native-comparison: {} exact acceptance/row/copy vectors checked; bounded corpus only",
            rows.len()
        );
    }
}
