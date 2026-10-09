//! Hash-bound, synthetic HTTP/cURL oracle: no request is sent, no app is started,
//! and no imported file or credential is read. Native execution requires macOS.
//! Shared assertions are bounded vectors, not universal Foundation/wire parity.
#![cfg(feature = "developer-tools")]
use bellobox_core::developer::http_request::{
    HttpRequestDraft, ResponseBodyPreview, import_request, render_response, tokenize_curl,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;

const HTTP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/HTTPRequestTool.swift"
));
const UTILITY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/DeveloperTools/DeveloperJSON.swift"
));
const FIXTURES: &str = include_str!("fixtures/http-swift-oracle.json");
const CHARACTERIZATION: &str = include_str!("fixtures/http-swift-characterization.json");

const DRIVER: &str = r####"
func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
func hashText(_ text: String) -> String { hash(Data(text.utf8)) }
func unhex(_ text: String) -> Data {
    var data = Data(), index = text.startIndex
    while index < text.endIndex { let end = text.index(index, offsetBy: 2); data.append(UInt8(text[index..<end], radix: 16)!); index = end }
    return data
}
let rows = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))) as! [[String: Any]]
var results: [[String: Any]] = []
for row in rows {
    var out: [String: Any] = ["id": row["id"]!]
    do {
        switch row["kind"] as! String {
        case "tokenize": out["tokens"] = try CurlImporter.tokenize(row["input"] as! String)
        case "response":
            var bytes = Data((row["prefix"] as? String ?? "").utf8)
            bytes.append(Data((row["body"] as? String ?? "").utf8))
            if let hex = row["body_hex"] as? String { bytes.append(unhex(hex)) }
            if let count = row["repeat_count"] as? Int { bytes.append(Data(repeating: UInt8(row["repeat_byte"] as! Int), count: count)) }
            bytes.append(Data((row["suffix"] as? String ?? "").utf8))
            if let hex = row["suffix_hex"] as? String { bytes.append(unhex(hex)) }
            let truncated = bytes.count > 2_000_000
            let data = Data(bytes.prefix(2_000_000))
            let result = renderPinnedResponse(row["status"] as! Int, data, truncated)
            out["body_sha256"] = hashText(result.body); out["body_utf8_bytes"] = result.body.utf8.count
            out["retained"] = data.count; out["truncated"] = truncated; out["status"] = result.status
        default:
            var draft = HTTPRequestDraft()
            if row["kind"] as! String == "import" {
                let input = (row["input"] as! String).trimmingCharacters(in: .whitespacesAndNewlines)
                if input.hasPrefix("curl") { draft = try CurlImporter.parse(input) }
                else { draft.url = input; _ = try draft.request() }
            } else {
                let fields = row["fields"] as? [String: String] ?? [:]
                draft = HTTPRequestDraft(method: fields["method"] ?? "GET", url: fields["url"] ?? "https://example.test/fixture", headers: fields["headers"] ?? "", body: fields["body"] ?? "")
                if let count = row["repeat_body"] as? Int { draft.body = String(repeating: "a", count: count) }
            }
            let request = try draft.request()
            out["draft"] = ["method": draft.method, "url": draft.url, "headers": draft.headers, "body_sha256": hashText(draft.body)]
            out["prepared_method"] = request.httpMethod!
            out["prepared_url"] = request.url!.absoluteString
            out["prepared_body_sha256"] = hash(request.httpBody ?? Data())
            out["prepared_headers"] = (request.allHTTPHeaderFields ?? [:]).map { [$0.key.lowercased(), $0.value] }.sorted { $0[0] < $1[0] }
        }
        out["ok"] = true
    } catch { out["ok"] = false; out["error"] = error.localizedDescription }
    results.append(out)
}
FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: ["results": results, "host": ["os": ProcessInfo.processInfo.operatingSystemVersionString, "foundation": NSFoundationVersionNumber, "locale": Locale.current.identifier]], options: [.sortedKeys]))
"####;

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
fn swift_source() -> String {
    assert_eq!(
        digest(HTTP),
        "795a4c498292aa3e60cb019d98f62cc5a688c47dbe6a5cdcffaaedf5e3f72d1e"
    );
    assert_eq!(
        digest(UTILITY),
        "3dc58c0f68e76a0db3205bef989040b0828c13d589f616bab661e1929639069b"
    );
    let pure = &HTTP[..HTTP.find("final class RequestRedirectPolicy").unwrap()];
    let tail = &HTTP[HTTP
        .find("        let headers = http.allHeaderFields")
        .unwrap()..HTTP.rfind("\n    }\n}").unwrap()];
    assert_eq!(
        digest(pure),
        "1f1ebd1a03ada04ba91df4c8ef42a8dfed60b62dcf3a5f14bc87ed2fc7eb42df"
    );
    assert_eq!(
        digest(tail),
        "a7652a3ac6d57e5783fb6dd18fc1d339b06c4f39b2046db7d571f34dbfd6a359"
    );
    // Only the pure portions and exact response-rendering statements are used.
    // Neither URLSession nor HTTPRequestTool.send is present in this program.
    format!(
        "import Foundation\nimport CryptoKit\n{UTILITY}\n{pure}\nfunc renderPinnedResponse(_ code: Int, _ data: Data, _ truncated: Bool) -> HTTPInspectionResult {{\nlet http = HTTPURLResponse(url: URL(string: \"https://example.test/fixture\")!, statusCode: code, httpVersion: \"HTTP/1.1\", headerFields: [:])!\nlet duration = 0.0\n{tail}\n}}\n{DRIVER}"
    )
}

fn fixture_rows(text: &str) -> Vec<Value> {
    serde_json::from_str(text).unwrap()
}

#[test]
fn pinned_http_source_and_synthetic_fixture_binding() {
    let source = swift_source();
    assert!(!source.contains("URLSession("));
    assert!(!source.contains("HTTPRequestTool.send"));
    assert_eq!(
        digest(FIXTURES),
        "b55f277666591fe90cf4d2b9c6abd2e338bce3d7060a14e2a108a5a70cb160a4"
    );
    assert_eq!(
        digest(CHARACTERIZATION),
        "38d4a1f7fc530b8223a7fea541ab7c80fc2b9cd54ca7ffb3610996f6af4ac2e4"
    );
    assert_eq!(fixture_rows(FIXTURES).len(), 95);
    assert_eq!(fixture_rows(CHARACTERIZATION).len(), 15);
    let mut ids = std::collections::BTreeSet::new();
    for row in fixture_rows(FIXTURES)
        .into_iter()
        .chain(fixture_rows(CHARACTERIZATION))
    {
        let id = row["id"].as_str().unwrap();
        assert!(
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        );
        assert!(ids.insert(id.to_owned()));
        assert!(row["repeat_count"].as_u64().unwrap_or(0) <= 2_000_001);
        assert!(row["repeat_body"].as_u64().unwrap_or(0) <= 512_001);
    }
}

fn string<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap_or("")
}
fn unhex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn observe(row: &Value) -> Result<Value, String> {
    match row["kind"].as_str().unwrap() {
        "tokenize" => Ok(json!({"tokens": tokenize_curl(string(row, "input"))?})),
        "response" => {
            let mut bytes = string(row, "prefix").as_bytes().to_vec();
            bytes.extend_from_slice(string(row, "body").as_bytes());
            bytes.extend(unhex(string(row, "body_hex")));
            bytes.extend(std::iter::repeat_n(
                row["repeat_byte"].as_u64().unwrap_or(0) as u8,
                row["repeat_count"].as_u64().unwrap_or(0) as usize,
            ));
            bytes.extend_from_slice(string(row, "suffix").as_bytes());
            bytes.extend(unhex(string(row, "suffix_hex")));
            let mut preview = ResponseBodyPreview::new();
            preview.push(&bytes);
            let result = render_response(
                row["status"].as_u64().unwrap() as u16,
                Duration::ZERO,
                &[],
                &preview,
            )?;
            Ok(
                json!({"body_sha256": digest(result.body()), "body_utf8_bytes": result.body().len(), "retained": result.retained_len(), "truncated": result.truncated(), "status":result.status()}),
            )
        }
        kind => {
            let draft = if kind == "import" {
                import_request(string(row, "input"))?
            } else {
                let fields = &row["fields"];
                HttpRequestDraft {
                    method: fields["method"].as_str().unwrap_or("GET").into(),
                    url: fields["url"]
                        .as_str()
                        .unwrap_or("https://example.test/fixture")
                        .into(),
                    headers: string(fields, "headers").into(),
                    body: row["repeat_body"].as_u64().map_or_else(
                        || string(fields, "body").to_owned(),
                        |n| "a".repeat(n as usize),
                    ),
                }
            };
            let prepared = draft.prepare()?;
            let mut headers = prepared
                .headers()
                .iter()
                .map(|(k, v)| vec![k.to_ascii_lowercase(), v.clone()])
                .collect::<Vec<_>>();
            headers.sort();
            Ok(
                json!({"draft":{"method":draft.method, "url":draft.url, "headers":draft.headers,"body_sha256":digest(&draft.body)},"prepared_method":prepared.method(),"prepared_url":prepared.url(),"prepared_headers":headers,"prepared_body_sha256":digest(prepared.body())}),
            )
        }
    }
}

#[test]
fn portable_http_oracle_vectors_are_bounded_and_exercised() {
    let mut accepted = 0;
    let rows = fixture_rows(FIXTURES);
    for row in &rows {
        let result = observe(row);
        assert_eq!(
            result.is_ok(),
            row["expected_ok"].as_bool().unwrap(),
            "{}: {result:?}",
            row["id"]
        );
        if result.is_ok() {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 47);
    // The characterization corpus reports intentional URL/admission differences,
    // without converting a native observation into a universal parity assertion.
    for row in fixture_rows(CHARACTERIZATION) {
        let _ = observe(&row);
    }
}

mod native {
    use super::*;
    use std::{
        fs,
        path::Path,
        process::{Command, Stdio},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    fn run(command: &mut Command, dir: &Path, name: &str) -> Vec<u8> {
        let out = dir.join(format!("{name}.stdout"));
        let err = dir.join(format!("{name}.stderr"));
        let started = SystemTime::now()
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
                || fs::metadata(&out).unwrap().len() > 1_048_576
                || fs::metadata(&err).unwrap().len() > 1_048_576
            {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{name}: exceeded 120-second/1-MiB output bound");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(
            fs::metadata(&out).unwrap().len() <= 1_048_576
                && fs::metadata(&err).unwrap().len() <= 1_048_576
        );
        let stdout = fs::read(out).unwrap();
        let stderr = fs::read(err).unwrap();
        println!(
            "{name}: start_unix_utc={started} end_unix_utc={} elapsed_s={} exit={status} stdout_sha256={} stderr_sha256={}\nstdout={}\nstderr={}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            timer.elapsed().as_secs_f64(),
            digest(&stdout),
            digest(&stderr),
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        assert!(status.success(), "{name} failed");
        stdout
    }

    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "requires native Swift/Foundation; Linux verifies source binding and Rust vectors only"
    )]
    fn native_swift_http_import_request_and_response_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let swift = dir.path().join("main.swift");
        let binary = dir.path().join("oracle");
        let fixture = dir.path().join("cases.json");
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
        for (name, text) in [("shared", FIXTURES), ("characterization", CHARACTERIZATION)] {
            fs::write(&fixture, text).unwrap();
            let bytes = run(Command::new(&binary).arg(&fixture), dir.path(), name);
            let native: Value = serde_json::from_slice(&bytes).unwrap();
            let rows = fixture_rows(text);
            let results = native["results"].as_array().unwrap();
            assert_eq!(rows.len(), results.len());
            for (row, native) in rows.iter().zip(results) {
                let id = row["id"].as_str().unwrap();
                assert_eq!(row["id"], native["id"]);
                let rust = observe(row);
                if name == "characterization" {
                    println!("http-characterization {id}: native={native} rust={rust:?}");
                    assert!(native["ok"].is_boolean());
                    continue;
                }
                assert_eq!(
                    native["ok"],
                    rust.is_ok(),
                    "{id}: acceptance; native={native} rust={rust:?}"
                );
                if let Ok(rust) = rust {
                    let keys: &[&str] = match row["kind"].as_str().unwrap() {
                        "tokenize" => &["tokens"],
                        "response" => &["body_sha256", "body_utf8_bytes", "retained", "truncated"],
                        _ => &["draft", "prepared_method", "prepared_body_sha256"],
                    };
                    for key in keys {
                        assert_eq!(
                            native[key], rust[key],
                            "{id}: {key}; native={native} rust={rust}"
                        );
                    }
                    if matches!(row["kind"].as_str(), Some("draft" | "import"))
                        && row["compare_headers"] != false
                    {
                        assert_eq!(
                            native["prepared_headers"], rust["prepared_headers"],
                            "{id}: prepared headers"
                        );
                    }
                    if row["kind"] == "response" {
                        assert!(
                            native["status"]
                                .as_str()
                                .unwrap()
                                .starts_with(&format!("HTTP {} · 0 ms · ", row["status"]))
                        );
                        assert_eq!(
                            native["status"]
                                .as_str()
                                .unwrap()
                                .contains("preview limited to 2 MB"),
                            rust["truncated"] == true
                        );
                    }
                }
            }
            println!(
                "http-oracle: {} {name} vectors; no network, no universal URL normalization/header-wire/locale parity claim",
                rows.len()
            );
        }
    }
}
