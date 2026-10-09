//! Pinned synthetic RegexTester oracle. Native Foundation is the independent
//! runtime reference for this shared portable-pattern corpus, not universal ICU.
#![cfg(feature = "developer-tools")]
use sha2::{Digest, Sha256};
const INSPECTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/InspectionTools.swift"
));
const FIXTURES: &str = include_str!("fixtures/regex-swift-oracle.json");
const CHARACTERIZATION: &str = include_str!("fixtures/regex-swift-characterization.json");
const DRIVER: &str = r####"
let rows = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))) as! [[String: Any]]
var results: [[String: Any]] = []
for row in rows {
    var out: [String: Any] = ["id": row["id"]!]
    do {
        let value = try RegexTester.inspect(text: row["text"] as! String, pattern: row["pattern"] as! String, replacement: row["replacement"] as! String, caseInsensitive: row["ignoreCase"] as! Bool, multiline: row["multiline"] as! Bool)
        out["ok"] = true
        out["ranges"] = value.ranges.map { [$0.location, NSMaxRange($0)] }
        out["details"] = value.details
        out["extracted"] = value.extracted
        out["replaced"] = value.replaced
    } catch { out["ok"] = false; out["error"] = error.localizedDescription }
    results.append(out)
}
FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: ["results": results], options: [.sortedKeys]))
"####;
fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn swift_source() -> String {
    assert_eq!(
        digest(INSPECTION),
        "4b74934aa3bdf1bfe88a2d25c5bfd6a63c58b536f7d81f4f03fc9efdab5f2fc7"
    );
    let excerpt = &INSPECTION[INSPECTION.find("struct RegexInspection").unwrap()
        ..INSPECTION.find("struct URLParameter").unwrap()];
    assert_eq!(
        digest(excerpt),
        "cc1420898946283208a721b646c49dabb11bf3d00abbdd8c53c3a2bc096a1a52"
    );
    format!(
        "import Foundation\nstruct UtilityError: LocalizedError {{ let message: String; init(_ message: String) {{ self.message = message }}; var errorDescription: String? {{ message }} }}\n{excerpt}\n{DRIVER}"
    )
}
#[test]
fn pinned_regex_source_and_synthetic_fixture_binding() {
    assert!(!swift_source().is_empty());
    assert_eq!(
        digest(FIXTURES),
        "d028e2760f03f888e5b884eb8d9820330e69a0a1425abe85f516e5355481c08b"
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(rows.len(), 46);
    assert_eq!(
        digest(CHARACTERIZATION),
        "99610c9f3b24ba5e85dc8451573704b833f1b5e0a156e2c1bf4bd9f37e433a76"
    );
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(CHARACTERIZATION)
            .unwrap()
            .len(),
        22
    );
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
        use bellobox_core::developer::regex_inspection::{Options, inspect};
        match inspect(
            row["text"].as_str().unwrap(),
            row["pattern"].as_str().unwrap(),
            row["replacement"].as_str().unwrap(),
            Options {
                ignore_case: row["ignoreCase"].as_bool().unwrap(),
                multiline: row["multiline"].as_bool().unwrap(),
            },
            &std::sync::atomic::AtomicBool::new(false),
        ) {
            Ok(r) => {
                serde_json::json!({"ok": true, "ranges": r.matches.iter().map(|m| [m.utf16.start, m.utf16.end]).collect::<Vec<_>>(), "details": r.details, "extracted": r.extracted, "replaced": r.replaced})
            }
            Err(error) => serde_json::json!({"ok":false, "error":error}),
        }
    }
    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "requires native Swift/Foundation; Linux checks source binding only"
    )]
    fn native_swift_regex_ranges_and_templates() {
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
        let fixture = dir.path().join("cases.json");
        fs::write(&fixture, FIXTURES).unwrap();
        let bytes = run(Command::new(&binary).arg(&fixture), dir.path(), "cases");
        let native: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(FIXTURES).unwrap();
        assert_eq!(native["results"].as_array().unwrap().len(), rows.len());
        for (row, n) in rows.iter().zip(native["results"].as_array().unwrap()) {
            let id = row["id"].as_str().unwrap();
            assert_eq!(n["id"], row["id"]);
            let r = rust_observation(row);
            assert_eq!(n["ok"], r["ok"], "{id}: acceptance; native={n} rust={r}");
            if n["ok"] == true {
                for key in ["ranges", "details", "extracted", "replaced"] {
                    assert_eq!(n[key], r[key], "{id}: {key}; native={n} rust={r}");
                }
            }
        }
        println!(
            "native-regex: {} exact acceptance/UTF-16/details/extract/replacement vectors; shared portable corpus only",
            rows.len()
        );
        // Deliberately separate from exact shared-corpus assertions: native
        // progress callbacks can affect the 1000-match boundary, and ICU's line
        // separators/anchors differ from the disclosed Rust pattern dialect.
        fs::write(&fixture, CHARACTERIZATION).unwrap();
        let bytes = run(
            Command::new(&binary).arg(&fixture),
            dir.path(),
            "characterization",
        );
        let native: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(CHARACTERIZATION).unwrap();
        assert_eq!(native["results"].as_array().unwrap().len(), rows.len());
        for (row, n) in rows.iter().zip(native["results"].as_array().unwrap()) {
            let id = row["id"].as_str().unwrap();
            assert_eq!(n["id"], row["id"]);
            assert!(n["ok"].is_boolean());
            let r = rust_observation(row);
            if id.ends_with("1001") {
                assert_eq!(n["ok"], false);
                assert_eq!(r["ok"], false);
            }
            println!(
                "native-regex-characterization: id={id} native_ok={} rust_ok={} native_ranges={} rust_ranges={} native_error={} rust_error={}",
                n["ok"], r["ok"], n["ranges"], r["ranges"], n["error"], r["error"]
            );
        }
    }
}
