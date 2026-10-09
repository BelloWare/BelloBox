#![cfg(feature = "developer-tools")]

// Standalone prototype lane: does not require modifying shared engine routing.
#[path = "../src/developer/json_formatter.rs"]
mod json_formatter;

#[test]
fn shared_json_pointer_retains_its_existing_distinct_key_contract() {
    let input = "{\"é\":1,\"e\u{301}\":2}";
    assert_eq!(
        bellobox_core::developer::execute("jsonPointer", input, "/é").unwrap(),
        "1"
    );
    assert_eq!(
        bellobox_core::developer::execute("jsonPointer", input, "/e\u{301}").unwrap(),
        "2"
    );
    assert!(json_formatter::execute(input, "validate").is_err());
}

#[test]
fn synthetic_oracle_vectors_emit_reviewable_results() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("json_formatter_prototype_vectors.json")).unwrap();
    let results: Vec<_> = vectors
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let input = v["input"].as_str().unwrap();
            let modes: Vec<_> = ["pretty", "minify", "validate"]
                .into_iter()
                .map(|mode| match json_formatter::execute(input, mode) {
                    Ok(output) => {
                        serde_json::json!({"mode": mode, "accepted": true, "output": output})
                    }
                    Err(error) => {
                        serde_json::json!({"mode": mode, "accepted": false, "error": error})
                    }
                })
                .collect();
            serde_json::json!({"id": v["id"], "input": input, "results": modes})
        })
        .collect();
    assert_eq!(results.len(), 17);
    println!(
        "FORMATTER_ORACLE_RESULTS={}",
        serde_json::to_string(&results).unwrap()
    );
}

#[test]
fn integrated_json_route_matches_formatter_for_all_oracle_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("json_formatter_prototype_vectors.json")).unwrap();
    for vector in vectors.as_array().unwrap() {
        let input = vector["input"].as_str().unwrap();
        for mode in ["pretty", "minify", "validate"] {
            assert_eq!(
                bellobox_core::developer::execute("json", input, mode),
                json_formatter::execute(input, mode),
                "{} {mode}",
                vector["id"]
            );
        }
    }
}
