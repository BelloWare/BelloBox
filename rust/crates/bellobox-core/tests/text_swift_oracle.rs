//! Synthetic host oracle for pinned Swift e43b1c4595c42383e087fe70f38c28d07c31dda0.
//! No application launch, permissions, credentials or locale-sensitive sort comparison.
use sha2::{Digest, Sha256};
const ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/Tools/TextTransforms.swift"
));
const FIXTURES: &str = r###"{
  "stats": [
    [
      "empty",
      ""
    ],
    [
      "combining_cjk",
      "e\u0301 \u754c"
    ],
    [
      "family",
      "\ud83d\udc68\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66"
    ],
    [
      "flag",
      "\ud83c\uddfa\ud83c\uddf3"
    ],
    [
      "crlf",
      "\r\n"
    ],
    [
      "mixed_lines",
      "a\rb\r\nc\n"
    ],
    [
      "trailing_cr",
      "a\r"
    ],
    [
      "trailing_lf",
      "a\n"
    ],
    [
      "repeated_crlf",
      "\r\n\r\n"
    ],
    [
      "cafe_nfc",
      "caf\u00e9"
    ],
    [
      "cafe_nfd",
      "cafe\u0301"
    ]
  ],
  "dedupe": [
    [
      "nfc_first",
      "\u00e9\ne\u0301\n\u00e9"
    ],
    [
      "nfd_first",
      "e\u0301\n\u00e9\ne\u0301"
    ],
    [
      "hangul",
      "\uac00\n\u1100\u1161"
    ],
    [
      "case",
      "A\na"
    ],
    [
      "space",
      " x\nx"
    ],
    [
      "empty_lines",
      "\n\n"
    ],
    [
      "mixed_lines",
      "a\r\nb\ra\n"
    ]
  ],
  "whitespace_scalars": [
    "0009",
    "000B",
    "000C",
    "0085",
    "00A0",
    "1680",
    "2003",
    "200B",
    "2028",
    "2029",
    "202F",
    "205F",
    "3000",
    "FEFF"
  ]
}
"###;
const DRIVER: &str = r###"
import Foundation
/* DECLARATIONS */
func bytes(_ s: String) -> [String: Any] {
    let data = Data(s.utf8)
    return ["utf8_hex": data.map { String(format: "%02x", $0) }.joined(), "base64": data.base64EncodedString()]
}
let input = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
let fixtures = try JSONSerialization.jsonObject(with: input) as! [String: Any]
let stats = (fixtures["stats"] as! [[String]]).map { row -> [String: Any] in
    let s = row[1]
    return ["id": row[0], "input": bytes(s), "characters": TextStats.characters(s), "without_whitespace": TextStats.charactersNoSpaces(s), "words": TextStats.words(s), "lines": TextStats.lines(s)]
}
let unique = (fixtures["dedupe"] as! [[String]]).map { row -> [String: Any] in
    return ["id": row[0], "input": bytes(row[1]), "output": bytes(LineTool.apply(row[1], .dedupe))]
}
let diagnostics = (fixtures["whitespace_scalars"] as! [String]).map { hex -> [String: Any] in
    let c = Unicode.Scalar(UInt32(hex, radix: 16)!)!
    let s = String(c)
    let cases = [s + "x" + s, s].map { value -> [String: Any] in
        return ["input": bytes(value), "trim": bytes(LineTool.apply(value, .trim)), "removeEmpty": bytes(LineTool.apply(value, .removeEmpty))]
    }
    return ["scalar": hex, "whitespaces_member": CharacterSet.whitespaces.contains(c), "cases": cases]
}
let result: [String: Any] = ["stats": stats, "unique": unique, "whitespace_diagnostics_only": diagnostics,
    "host": ["os": ProcessInfo.processInfo.operatingSystemVersionString, "locale": Locale.current.identifier,
             "preferred_languages": Locale.preferredLanguages, "foundation": NSFoundationVersionNumber]]
let output = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(output)
"###;
fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn source(original: &str) -> Result<String, &'static str> {
    if digest(original) != "3816ee35e6e135a2040357cd8774450b0bdfa69057a7c037d1e23c3cd7a38b14" {
        return Err("Pinned Swift source changed");
    }
    let begin = "enum LineTool {";
    let end = "\nprivate enum HexCodec {";
    if original.matches(begin).count() != 1 || original.matches(end).count() != 1 {
        return Err("Nonunique extraction markers");
    }
    let excerpt = original
        .get(original.find(begin).unwrap()..original.find(end).unwrap())
        .ok_or("Extraction order")?;
    if digest(excerpt) != "e1b4d762be1eaa22908718d8e71a3e2bc014ab3e524439f50891ca4d295fcd58" {
        return Err("Swift declarations changed");
    }
    if DRIVER.matches("/* DECLARATIONS */").count() != 1 {
        return Err("Driver marker changed");
    }
    Ok(DRIVER.replace("/* DECLARATIONS */", excerpt))
}
#[test]
fn pinned_source_and_fixed_fixtures_are_bound() {
    let generated = source(ORIGINAL).unwrap();
    assert!(generated.contains("enum TextStats {"));
    assert!(source(&ORIGINAL.replacen("enum LineTool", "enum Changed", 1)).is_err());
    assert_eq!(
        digest(FIXTURES),
        "06643c9322958e2b9402cb3263f7102a50444e01a433b000f9073ea30a14156e"
    );
    let v: serde_json::Value = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(v["stats"].as_array().unwrap().len(), 11);
    assert_eq!(v["dedupe"].as_array().unwrap().len(), 7);
    assert_eq!(v["whitespace_scalars"].as_array().unwrap().len(), 14);
    assert!(FIXTURES.len() < 8192);
    // All approved fixtures are bounded synthetic UTF-8 strings.
    for key in ["stats", "dedupe"] {
        for row in v[key].as_array().unwrap() {
            assert!(row[1].as_str().unwrap().len() < 128);
        }
    }
}
#[cfg(unix)]
mod native {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
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
    fn check_bytes(value: &serde_json::Value, expected: &[u8]) {
        assert_eq!(
            STANDARD.decode(value["base64"].as_str().unwrap()).unwrap(),
            expected
        );
        assert_eq!(
            value["utf8_hex"].as_str().unwrap(),
            expected
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
    }
    #[test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "requires native macOS Swift/Foundation; source binding still runs"
    )]
    fn native_swift_count_and_unique_match_pinned_source() {
        let dir = tempfile::tempdir().unwrap();
        let swift = dir.path().join("main.swift");
        let binary = dir.path().join("oracle");
        let fixtures = dir.path().join("vectors.json");
        fs::write(&swift, source(ORIGINAL).unwrap()).unwrap();
        fs::write(&fixtures, FIXTURES).unwrap();
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
        let output = run(Command::new(&binary).arg(&fixtures), dir.path(), "oracle");
        let actual: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let inputs: serde_json::Value = serde_json::from_str(FIXTURES).unwrap();
        assert_eq!(actual["stats"].as_array().unwrap().len(), 11);
        assert_eq!(actual["unique"].as_array().unwrap().len(), 7);
        assert_eq!(
            actual["whitespace_diagnostics_only"]
                .as_array()
                .unwrap()
                .len(),
            14
        );
        for (row, result) in inputs["stats"]
            .as_array()
            .unwrap()
            .iter()
            .zip(actual["stats"].as_array().unwrap())
        {
            assert_eq!(row[0], result["id"]);
            let input = row[1].as_str().unwrap();
            check_bytes(&result["input"], input.as_bytes());
            let expected = bellobox_core::text::counts(input, "gpt-4o");
            for (key, value) in [
                ("characters", expected.characters),
                ("without_whitespace", expected.without_whitespace),
                ("words", expected.words),
                ("lines", expected.lines),
            ] {
                assert_eq!(
                    result[key].as_u64().unwrap(),
                    value as u64,
                    "{} {key}",
                    row[0]
                );
            }
        }
        for (row, result) in inputs["dedupe"]
            .as_array()
            .unwrap()
            .iter()
            .zip(actual["unique"].as_array().unwrap())
        {
            assert_eq!(row[0], result["id"]);
            let input = row[1].as_str().unwrap();
            check_bytes(&result["input"], input.as_bytes());
            check_bytes(
                &result["output"],
                bellobox_core::text::lines(input, bellobox_core::text::LineOperation::Unique)
                    .as_bytes(),
            );
        }
        // Trim and Remove empty lines now follow Swift's CharacterSet.whitespaces
        // exactly. (The pinned driver keeps its original field name.)
        for scalar in actual["whitespace_diagnostics_only"].as_array().unwrap() {
            for case in scalar["cases"].as_array().unwrap() {
                let input = String::from_utf8(
                    STANDARD
                        .decode(case["input"]["base64"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap();
                check_bytes(
                    &case["trim"],
                    bellobox_core::text::lines(&input, bellobox_core::text::LineOperation::Trim)
                        .as_bytes(),
                );
                check_bytes(
                    &case["removeEmpty"],
                    bellobox_core::text::lines(
                        &input,
                        bellobox_core::text::LineOperation::Nonempty,
                    )
                    .as_bytes(),
                );
            }
        }
    }
}
