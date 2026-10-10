use super::*;

#[test]
fn public_admission_and_preview_limits_are_exact_decimal_bytes() {
    assert_eq!(MAX_DRAFT_BYTES, 500_000);
    assert_eq!(MAX_HEADER_BYTES, 64_000);
    assert_eq!(MAX_HEADER_COUNT, 256);
    assert_eq!(MAX_RESPONSE_BYTES, 2_000_000);
    assert_eq!(MAX_RESPONSE_TEXT_BYTES, 2_064_512);
    let input = format!("https://example.test/{}", "a".repeat(500_000));
    assert!(import_request(&input).is_err());
}

fn draft() -> HttpRequestDraft {
    HttpRequestDraft {
        url: "https://example.test/fixture".into(),
        ..Default::default()
    }
}
fn response(body: &[u8]) -> HttpInspectionResult {
    let mut preview = ResponseBodyPreview::new();
    preview.push(body);
    render_response(
        201,
        Duration::from_micros(12_500),
        &[("X-Fixture".into(), "yes".into())],
        &preview,
    )
    .unwrap()
}

#[test]
fn default_and_literal_url_import_are_inert_gets() {
    assert_eq!(HttpRequestDraft::default().method, "GET");
    assert!(HttpRequestDraft::default().prepare().is_err());
    let d = import_request(" \nhttps://example.test/fixture?q=a+b&tag=one&tag=two#part\t").unwrap();
    assert_eq!(
        d.url,
        "https://example.test/fixture?q=a+b&tag=one&tag=two#part"
    );
    let p = d.prepare().unwrap();
    assert_eq!(p.method(), "GET");
    assert_eq!(p.body(), b"");
    assert!(p.headers().is_empty());
}

#[test]
fn methods_are_normalized_but_not_arbitrary_http_tokens() {
    for (input, expected) in [
        (" post \n", "POST"),
        ("m-search", "M-SEARCH"),
        ("CUSTOM", "CUSTOM"),
    ] {
        let p = HttpRequestDraft {
            method: input.into(),
            ..draft()
        }
        .prepare()
        .unwrap();
        assert_eq!(p.method(), expected);
    }
    for bad in [
        "",
        " ",
        "GET1",
        "GET\r\nX",
        "GE T",
        "GET\0",
        "GET_",
        "ßßßßßßßßßßßßßßßßßßßßßßßßßßßßßßßßß",
    ] {
        assert!(
            HttpRequestDraft {
                method: bad.into(),
                ..draft()
            }
            .prepare()
            .is_err(),
            "{bad:?}"
        );
    }
    assert!(
        HttpRequestDraft {
            method: "A".repeat(MAX_METHOD_BYTES),
            ..draft()
        }
        .prepare()
        .is_ok()
    );
    assert!(
        HttpRequestDraft {
            method: "A".repeat(MAX_METHOD_BYTES + 1),
            ..draft()
        }
        .prepare()
        .is_err()
    );
}

#[test]
fn safe_urls_keep_queries_ipv6_and_numeric_loopback() {
    for url in [
        "http://127.0.0.1:1234/a?x=%0D%0A",
        "http://[::1]:9090/fixture?q=a+b",
        "https://EXAMPLE.test:443/a%20b?x=%25#ok",
        "https://example.test/雪?x=é",
    ] {
        assert!(
            HttpRequestDraft {
                url: url.into(),
                ..draft()
            }
            .prepare()
            .is_ok(),
            "{url}"
        );
    }
    let p = HttpRequestDraft {
        url: "HTTP://127.0.0.1:80/a?x=1&x=2".into(),
        ..draft()
    }
    .prepare()
    .unwrap();
    assert_eq!(p.url(), "http://127.0.0.1/a?x=1&x=2");
}

#[test]
fn urls_reject_implicit_rewrites_credentials_and_invalid_escapes() {
    for bad in [
        "",
        "example.test",
        "file:///tmp/file",
        "ftp://example.test/",
        "http:/example.test",
        "http:///example.test",
        "https://",
        "https://[::1",
        "https://example.test:65536",
        "https://example.test/a b",
        "https://example.test/\tfoo",
        "https://example.test/a\n",
        "https://example.test/\\evil",
        "https://example.test/%",
        "https://example.test/%2",
        "https://example.test/%xz",
        "https://@example.test/",
        "https://user:synthetic@example.test/",
        "https://example.test/\u{7f}",
    ] {
        assert!(
            HttpRequestDraft {
                url: bad.into(),
                ..draft()
            }
            .prepare()
            .is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn ordered_duplicate_headers_and_body_are_snapshot_owned() {
    let mut d = HttpRequestDraft {
        method: "PATCH".into(),
        headers: " X-Test : first \nX-Test: second\nX-Tab:\ta\tb\t\nX-Empty:\n\t\n".into(),
        body: "🚀 literal\0data".into(),
        ..draft()
    };
    let p = d.prepare().unwrap();
    assert_eq!(
        p.headers(),
        &[
            ("X-Test".into(), "first".into()),
            ("X-Test".into(), "second".into()),
            ("X-Tab".into(), "a\tb".into()),
            ("X-Empty".into(), "".into())
        ]
    );
    d.body = "changed".into();
    d.headers.clear();
    d.url.clear();
    assert_eq!(p.body(), "🚀 literal\0data".as_bytes());
    assert_eq!(p.method(), "PATCH");
}

#[test]
fn caller_framing_is_always_removed_after_validation() {
    let d = HttpRequestDraft {
        headers: "Content-Length: 900\ntransfer-ENCODING: chunked\nContent-Length: 0\nX-OK: yes"
            .into(),
        body: "edited".into(),
        ..draft()
    };
    assert_eq!(
        d.prepare().unwrap().headers(),
        &[("X-OK".into(), "yes".into())]
    );
    for h in ["Content-Length: 1\r", "Transfer-Encoding: chunked\0"] {
        assert!(
            HttpRequestDraft {
                headers: h.into(),
                ..draft()
            }
            .prepare()
            .is_err()
        );
    }
}

#[test]
fn invalid_header_names_and_controls_fail() {
    for h in [
        "Missing colon",
        ": empty",
        "Bad Name: value",
        "Bad(Thing): value",
        "é: value",
        "X: a\r\nInjected: yes",
        "X: a\0",
        "X: a\u{1f}",
        "X: a\u{7f}",
        "\r",
    ] {
        assert!(
            HttpRequestDraft {
                headers: h.into(),
                ..draft()
            }
            .prepare()
            .is_err(),
            "{h:?}"
        );
    }
    assert!(
        HttpRequestDraft {
            headers: "!#$%&'*+-.^_`|~: unicode café\tallowed".into(),
            ..draft()
        }
        .prepare()
        .is_ok()
    );
}

#[test]
fn raw_draft_and_normalized_admission_are_bounded() {
    let mut d = draft();
    d.body = "a".repeat(MAX_DRAFT_BYTES - d.byte_len());
    assert_eq!(d.byte_len(), MAX_DRAFT_BYTES);
    assert!(d.prepare().is_ok());
    d.body.push('a');
    assert!(d.prepare().is_err());
    assert!(import_request(&" ".repeat(MAX_DRAFT_BYTES + 1)).is_err());
    let d = HttpRequestDraft {
        url: format!("https://example.test/{}", "雪".repeat(100_000)),
        ..draft()
    };
    assert!(d.byte_len() < MAX_DRAFT_BYTES);
    assert!(d.prepare().is_err());
}

#[test]
fn header_byte_and_count_boundaries_include_discarded_framing() {
    let mut d = HttpRequestDraft {
        headers: format!("X: {}", "a".repeat(MAX_HEADER_BYTES - 3)),
        ..draft()
    };
    assert_eq!(d.headers.len(), MAX_HEADER_BYTES);
    assert!(d.prepare().is_ok());
    d.headers.push('a');
    assert!(d.prepare().is_err());
    d.headers = vec!["X: a"; MAX_HEADER_COUNT].join("\n");
    assert!(d.prepare().is_ok());
    d.headers.push_str("\nContent-Length: 0");
    assert!(d.prepare().is_err());
}

#[test]
fn curl_source_json_body_and_defaults() {
    let d = import_request(r#"curl 'https://example.test/api?a=1&b=2' -H 'X-Test: a:b' --json '{"id":123,"name":"A B"}'"#).unwrap();
    assert_eq!(d.method, "POST");
    assert_eq!(d.body, r#"{"id":123,"name":"A B"}"#);
    assert_eq!(
        d.headers,
        "X-Test: a:b\nContent-Type: application/json\nAccept: application/json"
    );
    assert!(d.url.ends_with("?a=1&b=2"));
}

#[test]
fn curl_attached_values_and_consumed_option_looking_bodies_are_literal() {
    let d = import_request(
        r#"curl --url='https://example.test' -XPOST -H'X-Test: yes' --json='{"a":true}'"#,
    )
    .unwrap();
    assert_eq!(d.method, "POST");
    assert_eq!(d.body, r#"{"a":true}"#);
    for value in [
        "-dhello",
        "--json=literal",
        "-HContent-Type: example",
        "--url=https://other.test",
    ] {
        let d =
            import_request(&format!("curl 'https://example.test' --data-raw '{value}'")).unwrap();
        assert_eq!(d.body, value);
    }
    for command in [
        "curl --request=PUT --url=https://example.test --data-raw=@literal",
        "curl https://example.test -XPUT --data-raw '@literal'",
    ] {
        let d = import_request(command).unwrap();
        assert_eq!(d.method, "PUT");
        assert_eq!(d.body, "@literal");
    }
}

#[test]
fn curl_supported_no_value_options_do_not_change_the_snapshot() {
    let d = import_request(
        "curl https://example.test --compressed -s --silent -S --show-error --globoff -g",
    )
    .unwrap();
    assert_eq!(d.method, "GET");
    assert!(d.headers.is_empty());
    assert!(d.body.is_empty());
    assert_eq!(
        import_request("curl https://example.test --head")
            .unwrap()
            .method,
        "HEAD"
    );
    assert_eq!(
        import_request("curl https://example.test -I -XPOST")
            .unwrap()
            .method,
        "POST"
    );
    assert_eq!(
        import_request("curl https://example.test -XPOST -I")
            .unwrap()
            .method,
        "HEAD"
    );
}

#[test]
fn curl_form_json_and_empty_parts_follow_source_joining() {
    let d =
        import_request("curl https://example.test -d a=1 --data-binary b=2 --data-raw ''").unwrap();
    assert_eq!(d.body, "a=1&b=2&");
    assert_eq!(d.headers, "Content-Type: application/x-www-form-urlencoded");
    assert_eq!(
        import_request("curl https://example.test --json '{' --json '}'")
            .unwrap()
            .body,
        "{}"
    );
    assert_eq!(
        import_request("curl https://example.test -d ''")
            .unwrap()
            .method,
        "POST"
    );
    for command in [
        "curl https://example.test --json '{}' -d x",
        "curl https://example.test -d '' --json '{}'",
        "curl https://example.test --json '' --data-raw x",
    ] {
        assert!(import_request(command).is_err());
    }
}

#[test]
fn curl_explicit_headers_suppress_defaults_case_insensitively() {
    let d = import_request(
        "curl https://example.test -H 'content-TYPE: custom/type' -H 'ACCEPT: custom' --json '{}'",
    )
    .unwrap();
    assert_eq!(d.headers, "content-TYPE: custom/type\nACCEPT: custom");
    // The Swift importer tests an untrimmed prefix; editing preparation trims it.
    let d = import_request("curl https://example.test -H ' Content-Type: custom' -d x").unwrap();
    assert_eq!(d.prepare().unwrap().headers().len(), 2);
}

#[test]
fn curl_get_preserves_existing_query_fragment_and_explicit_method() {
    let d = import_request(
        "curl 'https://example.test?a=1#f' -G -d 'q=hello%20world' -d 'n=%E9%9B%AA'",
    )
    .unwrap();
    assert_eq!(
        d.url,
        "https://example.test?a=1&q=hello%20world&n=%E9%9B%AA#f"
    );
    assert!(d.body.is_empty());
    assert!(d.headers.is_empty());
    assert_eq!(d.method, "GET");
    let d = import_request("curl 'https://example.test?' --get -XPUT --json 'x=1'").unwrap();
    assert_eq!(d.url, "https://example.test?x=1");
    assert_eq!(d.method, "PUT");
    assert_eq!(d.headers, "Accept: application/json");
    assert_eq!(
        import_request("curl https://example.test -G -d ''")
            .unwrap()
            .url,
        "https://example.test?"
    );
    for value in [
        "hello world",
        "%",
        "%xz",
        "é",
        "x=#fragment",
        "x=[1]",
        "x=%FF",
        "x=%C3",
        "x=%ED%A0%80",
    ] {
        assert!(
            import_request(&format!("curl https://example.test -G -d '{value}'")).is_err(),
            "{value}"
        );
    }
}

#[test]
fn curl_cookies_and_only_synthetic_basic_auth_are_literal() {
    let d = import_request(
        "curl https://example.test -b 'name=value; other=two' --cookie=a=b -usynthetic:example",
    )
    .unwrap();
    assert_eq!(
        d.headers,
        "Cookie: name=value; other=two\nCookie: a=b\nAuthorization: Basic c3ludGhldGljOmV4YW1wbGU="
    );
    assert!(import_request("curl https://example.test -b cookies.txt").is_err());
    assert!(import_request("curl https://example.test -b 'a=b\ninjected'").is_err());
}

#[test]
fn curl_file_imports_shell_operations_and_unsupported_flags_are_rejected() {
    for command in [
        "curl https://example.test | sh",
        "curl https://example.test; echo x",
        "curl https://example.test && echo x",
        "curl https://example.test > out",
        "curl https://example.test < in",
        "curl https://example.test `pwd`",
        "curl https://example.test/$TOKEN",
        "curl https://example.test --data @secret.txt",
        "curl https://example.test --data-binary=@secret.txt",
        "curl https://example.test --json @secret.txt",
        "curl https://example.test -H @headers.txt",
        "curl https://example.test --config config",
        "curl https://example.test --location",
        "curl https://example.test --insecure",
        "curl https://example.test --unknown",
        "curl https://example.test -sS",
        "curl https://example.test --",
    ] {
        assert!(import_request(command).is_err(), "{command}");
    }
}

#[test]
fn curl_missing_values_multiple_urls_and_incomplete_quotes_are_rejected() {
    for command in [
        "curl",
        "curl -X",
        "curl -H",
        "curl -d",
        "curl --url",
        "curl --user",
        "curl --cookie",
        "curl https://example.test https://other.test",
        "curl --url https://example.test --url https://other.test",
        "curl --url=https://example.test https://other.test",
        "curl 'https://example.test",
        "curl https://example.test \\",
        "curlish https://example.test",
        "CURL https://example.test",
    ] {
        assert!(import_request(command).is_err(), "{command}");
    }
}

#[test]
fn tokenizer_preserves_quotes_empty_tokens_and_double_quote_escape_rules() {
    assert_eq!(
        tokenize_curl(r#"curl "https://example.test" --data-raw "a\nb""#)
            .unwrap()
            .last()
            .unwrap(),
        r#"a\nb"#
    );
    assert_eq!(
        tokenize_curl(r#"curl '' a"b"'c' "\$\`\"\\" '$HOME `literal`'"#).unwrap(),
        vec!["curl", "", "abc", "$`\"\\", "$HOME `literal`"]
    );
    assert_eq!(
        tokenize_curl("curl https://example.test \\\n-d x \\\r\n-H 'X: y'").unwrap(),
        vec!["curl", "https://example.test", "-d", "x", "-H", "X: y"]
    );
    for command in [
        r#"curl "https://example.test/$TOKEN""#,
        r#"curl "`pwd`""#,
        "curl \0",
        "curl \u{1b}",
    ] {
        assert!(tokenize_curl(command).is_err());
    }
    assert_eq!(
        import_request(
            "curl 'https://example.test/$literal?x=a&b=c' --data-raw '$(nothing); | & < >'"
        )
        .unwrap()
        .body,
        "$(nothing); | & < >"
    );
}

#[test]
fn tokenizer_token_budget_is_explicit() {
    assert_eq!(
        tokenize_curl(&vec!["''"; MAX_IMPORT_TOKENS].join(" "))
            .unwrap()
            .len(),
        MAX_IMPORT_TOKENS
    );
    assert!(tokenize_curl(&vec!["''"; MAX_IMPORT_TOKENS + 1].join(" ")).is_err());
}

#[test]
fn response_formats_lossless_json_and_sorts_headers() {
    let mut p = ResponseBodyPreview::new();
    p.push(br#"{"z":1e400,"id":9007199254740993,"negative":-0.00}"#);
    let r = render_response(
        201,
        Duration::from_micros(1_499),
        &[("Z".into(), "last".into()), ("A".into(), "first".into())],
        &p,
    )
    .unwrap();
    assert_eq!(
        r.body(),
        "{\n  \"id\": 9007199254740993,\n  \"negative\": -0.00,\n  \"z\": 1e400\n}"
    );
    assert_eq!(r.headers(), "A: first\nZ: last");
    assert!(r.status().starts_with("HTTP 201 · 1 ms · "));
    assert_eq!(
        r.text(),
        format!("{}\n\n{}\n\n{}", r.status(), r.headers(), r.body())
    );
}

#[test]
fn all_three_digit_statuses_are_inspectable_including_redirects_and_errors() {
    let p = ResponseBodyPreview::new();
    for status in [
        100, 200, 204, 301, 302, 307, 308, 400, 404, 418, 500, 599, 999,
    ] {
        let r = render_response(
            status,
            Duration::ZERO,
            &[("Location".into(), "https://other.test/".into())],
            &p,
        )
        .unwrap();
        assert!(r.status().starts_with(&format!("HTTP {status} · ")));
        assert!(r.headers().contains("Location:"));
    }
    for bad in [0, 99, 1000, u16::MAX] {
        assert!(render_response(bad, Duration::ZERO, &[], &p).is_err());
    }
}

#[test]
fn response_cap_requires_an_observed_extra_byte_and_retains_exact_prefix() {
    let mut p = ResponseBodyPreview::new();
    assert!(!p.push(&vec![b'a'; MAX_RESPONSE_BYTES - 1]));
    assert!(!p.push(b"b"));
    assert_eq!(p.retained_len(), MAX_RESPONSE_BYTES);
    assert!(!p.truncated());
    assert!(p.push(b"c"));
    assert!(p.truncated());
    assert_eq!(p.bytes().last(), Some(&b'b'));
    assert!(p.push(&vec![b'z'; MAX_RESPONSE_BYTES]));
    assert_eq!(p.retained_len(), MAX_RESPONSE_BYTES);
    assert!(p.bytes.capacity() <= MAX_RESPONSE_BYTES);
    let r = render_response(200, Duration::ZERO, &[], &p).unwrap();
    assert_eq!(r.body().len(), MAX_RESPONSE_BYTES);
    assert_eq!(r.retained_len(), MAX_RESPONSE_BYTES);
    assert!(r.truncated());
    assert_eq!(
        r.status(),
        "HTTP 200 · 0 ms · 2,000,000 bytes · preview limited to 2 MB"
    );
    assert!(r.text().len() <= MAX_RESPONSE_TEXT_BYTES);
}

#[test]
fn chunk_boundaries_do_not_change_retained_bytes_or_truncation() {
    let bytes = vec![b'x'; MAX_RESPONSE_BYTES + 17];
    let mut single = ResponseBodyPreview::new();
    single.push(&bytes);
    for chunk_size in [1, 7, 8192, MAX_RESPONSE_BYTES, MAX_RESPONSE_BYTES + 1] {
        let mut chunks = ResponseBodyPreview::new();
        for chunk in bytes.chunks(chunk_size) {
            if chunks.push(chunk) {
                break;
            }
        }
        assert_eq!(chunks, single);
        assert!(chunks.bytes.capacity() <= MAX_RESPONSE_BYTES);
    }
}

#[test]
fn utf8_binary_and_cap_split_multibyte_are_not_lossily_replaced() {
    assert_eq!(response("café 🚀".as_bytes()).body(), "café 🚀");
    assert_eq!(response(&[0xff, 0xfe]).body(), "Binary response (2 bytes).");
    let mut p = ResponseBodyPreview::new();
    p.push(&vec![b'a'; MAX_RESPONSE_BYTES - 1]);
    p.push("🚀".as_bytes());
    let r = render_response(200, Duration::ZERO, &[], &p).unwrap();
    assert_eq!(r.body(), "Binary response (2,000,000 bytes).");
    assert!(r.truncated());
    // Source treats valid UTF-8 NUL as text, rather than guessing MIME/binary.
    assert_eq!(response(b"a\0b").body(), "a\0b");
}

#[test]
fn json_failure_size_depth_node_and_expansion_limits_preserve_original() {
    for raw in ["{bad json}", "  plain text\n", "{\"a\":1,\"a\":2}", "[1,]"] {
        assert_eq!(response(raw.as_bytes()).body(), raw);
    }
    let oversized = format!("\"{}\"", "a".repeat(500_001));
    assert_eq!(response(oversized.as_bytes()).body(), oversized);
    let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert_eq!(response(deep.as_bytes()).body(), deep);
    // Like HTTPRequestTool.swift, any document DeveloperJSON accepts is pretty-printed;
    // there is no value-count limit (removed 2026-10-10).
    let nodes = format!("[{}]", vec!["0"; 20_001].join(","));
    let pretty = super::super::json_formatter::execute(&nodes, "").unwrap();
    assert_ne!(pretty, nodes);
    assert_eq!(response(nodes.as_bytes()).body(), pretty);
    // Under the input/depth caps, indentation can still expand >2 MB.
    let expanded = format!(
        "{}[{}]{}",
        "[".repeat(60),
        vec!["0"; 18_000].join(","),
        "]".repeat(60)
    );
    assert_eq!(response(expanded.as_bytes()).body(), expanded);
}

#[test]
fn truncated_valid_json_prefix_is_not_pretty_printed() {
    let mut bytes = br#"{"a":1}"#.to_vec();
    bytes.resize(MAX_RESPONSE_BYTES + 1, b' ');
    let mut p = ResponseBodyPreview::new();
    p.push(&bytes);
    let r = render_response(200, Duration::ZERO, &[], &p).unwrap();
    assert_eq!(r.body().as_bytes(), &bytes[..MAX_RESPONSE_BYTES]);
}

#[test]
fn response_headers_and_total_rendering_are_bounded() {
    let p = ResponseBodyPreview::new();
    let headers = vec![("X".into(), "a".into()); MAX_HEADER_COUNT];
    assert!(render_response(200, Duration::ZERO, &headers, &p).is_ok());
    let headers = vec![("X".into(), "a".into()); MAX_HEADER_COUNT + 1];
    assert!(render_response(200, Duration::ZERO, &headers, &p).is_err());
    assert!(
        render_response(
            200,
            Duration::ZERO,
            &[("X".into(), "a".repeat(MAX_HEADER_BYTES - 4))],
            &p
        )
        .is_ok()
    );
    assert!(
        render_response(
            200,
            Duration::ZERO,
            &[("X".into(), "a".repeat(MAX_HEADER_BYTES - 3))],
            &p
        )
        .is_err()
    );
    assert!(
        render_response(
            200,
            Duration::ZERO,
            &[("X".into(), "line\ninjection".into())],
            &p
        )
        .is_err()
    );
    let mut p = ResponseBodyPreview::new();
    p.push(&vec![b'a'; MAX_RESPONSE_BYTES]);
    assert!(
        render_response(
            599,
            Duration::MAX,
            &[("X".into(), "b".repeat(MAX_HEADER_BYTES - 4))],
            &p
        )
        .unwrap()
        .text()
        .len()
            <= MAX_RESPONSE_TEXT_BYTES
    );
}

#[test]
fn debug_and_errors_do_not_disclose_request_or_response_content() {
    let d = HttpRequestDraft {
        headers: "Authorization: synthetic-secret".into(),
        body: "synthetic-body".into(),
        ..draft()
    };
    for description in [
        format!("{d:?}"),
        format!("{:?}", d.prepare().unwrap()),
        format!("{:?}", response(b"synthetic-body")),
    ] {
        for secret in ["synthetic-secret", "synthetic-body", "example.test"] {
            assert!(!description.contains(secret));
        }
    }
    assert!(
        !import_request("curl https://example.test --synthetic-secret")
            .unwrap_err()
            .contains("synthetic-secret")
    );
}

#[test]
fn legacy_cli_http_raw_request_contract_is_unchanged() {
    let text = super::super::execute(
        "http",
        "GET https://example.test/path HTTP/1.1\nAccept: application/json",
        "",
    )
    .unwrap();
    assert!(text.contains("GET"));
    assert!(super::super::execute("http", "curl https://example.test", "").is_err());
}
