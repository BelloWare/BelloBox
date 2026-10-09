//! Bounded synthetic URL oracle. No app, networking, credentials or clipboard.
use sha2::{Digest, Sha256};
const INSPECTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/InspectionTools.swift"
));
const UTILITY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/DeveloperJSON.swift"
));
const FIXTURES: &str = include_str!("fixtures/url-swift-oracle.json");
const DRIVER: &str = r####"let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
let rows = try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
func payload(_ s: String) -> [String: Any] {
    return ["text": s, "utf8_base64": Data(s.utf8).base64EncodedString()]
}
func fields(_ draft: URLInspection) -> [String: Any] {
    return ["scheme": draft.scheme, "host": draft.host, "port": draft.port,
            "path": draft.path, "fragment": draft.fragment,
            "parameters": draft.parameters.map { ["name": $0.name, "value": $0.value, "hasValue": $0.hasValue] as [String: Any] }]
}
var results: [[String: Any]] = []
for row in rows {
    var result: [String: Any] = ["id": row["id"]!, "input": payload(row["input"] as! String)]
    do {
        var draft = try URLInspection(row["input"] as! String)
        result["parsed"] = fields(draft)
        if let edits = row["edits"] as? [String: Any] {
            if let value = edits["scheme"] as? String { draft.scheme = value }
            if let value = edits["host"] as? String { draft.host = value }
            if let value = edits["port"] as? String { draft.port = value }
            if let value = edits["path"] as? String { draft.path = value }
            if let value = edits["fragment"] as? String { draft.fragment = value }
            if let values = edits["parameters"] as? [[String: Any]] {
                draft.parameters = values.map { URLParameter(name: $0["name"] as! String, value: $0["value"] as! String, hasValue: $0["hasValue"] as! Bool) }
            }
        }
        result["edited"] = fields(draft)
        do { result["rebuilt"] = payload(try draft.rebuilt()); result["build_ok"] = true }
        catch { result["build_ok"] = false; result["build_error"] = error.localizedDescription }
        result["parse_ok"] = true
    } catch { result["parse_ok"] = false; result["parse_error"] = error.localizedDescription }
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
    let start = INSPECTION.find("struct URLParameter:").unwrap();
    let ustart = UTILITY.find("struct UtilityError:").unwrap();
    let uend = UTILITY.find("/// Keeps number lexemes").unwrap();
    let excerpt = format!("{}{}", &UTILITY[ustart..uend], &INSPECTION[start..]);
    assert_eq!(
        digest(&excerpt),
        "920312a3c4cc8f6b91cae50b614eb177b62c7fb1e8a92ce25e0709163a433ac5"
    );
    format!("import Foundation\n{excerpt}\n{DRIVER}")
}
#[test]
fn url_oracle_source_and_fixture_binding() {
    assert!(swift_source().contains("struct URLInspection"));
    assert_eq!(
        digest(FIXTURES),
        "ce39d52b0f0b268a4631d4e034e07ac0a9284f6925d70759cff5a56e2085bb5c"
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(rows.len(), 29);
    assert!(FIXTURES.len() < 16384);
}
#[cfg(unix)]
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
        use bellobox_core::url_editor::Draft;
        let mut draft = match Draft::inspect(row["input"].as_str().unwrap()) {
            Ok(d) => d,
            Err(error) => return serde_json::json!({"parse_ok":false,"parse_error":error}),
        };
        if let Some(edits) = row.get("edits") {
            for (key, target) in [
                ("scheme", &mut draft.scheme),
                ("host", &mut draft.host),
                ("port", &mut draft.port),
                ("path", &mut draft.path),
                ("fragment", &mut draft.fragment),
            ] {
                if let Some(s) = edits.get(key).and_then(serde_json::Value::as_str) {
                    *target = s.into();
                }
            }
            if let Some(params) = edits
                .get("parameters")
                .and_then(serde_json::Value::as_array)
            {
                draft.parameters.clear();
                for p in params {
                    let id = draft.add().unwrap();
                    let target = draft.parameters.iter_mut().find(|p| p.id == id).unwrap();
                    target.name = p["name"].as_str().unwrap().into();
                    target.value = p["value"].as_str().unwrap().into();
                    target.has_value = p["hasValue"].as_bool().unwrap();
                }
            }
        }
        match draft.build() {
            Ok(text) => serde_json::json!({"parse_ok":true,"build_ok":true,"rebuilt":text}),
            Err(error) => serde_json::json!({"parse_ok":true,"build_ok":false,"build_error":error}),
        }
    }
    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "requires native Swift/Foundation; Linux validates only binding and Rust compilation"
    )]
    fn native_swift_url_bounded_vectors_and_diagnostics() {
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
        // Exact assertions are intentionally bounded. Other rows expose differences,
        // including deliberately stricter Rust unsupported-input rejection.
        let exact = [
            "duplicate_flag_plus",
            "empty_path",
            "default_port",
            "empty_fragment",
            "remove_all",
            "port_empty",
            "port_one",
            "port_max",
            "port_invalid_large",
            "port_invalid_text",
            "blank_host",
            "blank_path_fragment",
        ];
        let mut checked = 0;
        for row in rows {
            let id = row["id"].as_str().unwrap();
            assert!(id.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
            let fixture = dir.path().join(format!("{id}.json"));
            fs::write(&fixture, serde_json::to_vec(&vec![row.clone()]).unwrap()).unwrap();
            // Separate bounded processes isolate Foundation failures to named vectors.
            let result = run(Command::new(&binary).arg(&fixture), dir.path(), id);
            let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
            let native = &result["results"][0];
            assert_eq!(native["id"], row["id"]);
            let rust = rust_observation(&row);
            println!(
                "url_comparison id={id} exact={} rust={rust} native={native}",
                exact.contains(&id)
            );
            if exact.contains(&id) {
                checked += 1;
                assert_eq!(rust["parse_ok"], native["parse_ok"], "{id} parse");
                if rust["parse_ok"] == true {
                    assert_eq!(rust["build_ok"], native["build_ok"], "{id} build");
                    if rust["build_ok"] == true {
                        assert_eq!(
                            rust["rebuilt"], native["rebuilt"]["text"],
                            "{id} exact UTF-8"
                        );
                    }
                }
            }
        }
        assert_eq!(checked, exact.len());
    }
}
