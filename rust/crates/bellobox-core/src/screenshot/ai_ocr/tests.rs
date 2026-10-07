use super::*;
use crate::{
    ai::{Config, Provider},
    screenshot::{
        AnnotationKind, AnnotationStyle, MaskPattern, Point, Rect, RgbaColor, ScreenshotEditSession,
    },
};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::Rgba;
use serde_json::{Value, json};

fn document(width: u32, height: u32) -> ScreenshotDocument {
    ScreenshotDocument::from_rgba(RgbaImage::from_pixel(
        width,
        height,
        Rgba([24, 72, 136, 255]),
    ))
    .unwrap()
}
fn config(provider: Provider) -> Config {
    Config {
        provider,
        endpoint: "https://example.invalid/prefix/v1/".into(),
        model: "approved-old-model".into(),
        system_prompt: "WRITING_PROMPT_MUST_NEVER_LEAK".into(),
        max_output_tokens: 4096,
    }
}
fn authority(provider: Provider) -> ProviderAuthority {
    ProviderAuthority::new(config(provider), "synthetic-key".into()).unwrap()
}
fn prepared() -> PreparedImage {
    PreparedImage::prepare(&document(32, 24), &UploadOptions::default()).unwrap()
}
fn body(request: &ImageRequest) -> Value {
    serde_json::from_slice(request.body()).unwrap()
}
fn envelope(provider: Provider, text: &str) -> Vec<u8> {
    let value = match provider {
        Provider::OpenAIChat => {
            json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]})
        }
        Provider::OpenAIResponses => {
            json!({"status":"completed","output":[{"type":"reasoning","content":[{"type":"output_text","text":"HIDDEN"}]},{"type":"message","content":[{"type":"output_text","text":text}]}]})
        }
        Provider::Anthropic => {
            json!({"content":[{"type":"thinking","text":"HIDDEN"},{"type":"text","text":text}],"stop_reason":"end_turn"})
        }
    };
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn crop_mask_eraser_and_decoration_have_one_final_privacy_render() {
    let mut image = RgbaImage::from_pixel(120, 90, Rgba([24, 72, 136, 255]));
    for (x, _, p) in image.enumerate_pixels_mut() {
        if x < 10 {
            *p = Rgba([251, 103, 7, 255]);
        }
    }
    image.put_pixel(45, 35, Rgba([231, 17, 199, 255]));
    let mut session = ScreenshotEditSession::new(ScreenshotDocument::from_rgba(image).unwrap());
    session
        .set_crop(Some(Rect::new(10., 10., 90., 60.)))
        .unwrap();
    session
        .add_annotation(
            AnnotationKind::Blur(Rect::new(20., 20., 40., 30.)),
            AnnotationStyle::mask(RgbaColor::from_rgb8(0, 0, 0), MaskPattern::Solid),
        )
        .unwrap();
    session
        .erase_stroke(vec![Point::new(30., 30.)], 8.)
        .unwrap();
    session
        .add_annotation(
            AnnotationKind::Rectangle(Rect::new(65., 20., 20., 20.)),
            AnnotationStyle {
                fill_color: Some(RgbaColor::from_rgb8(255, 0, 0)),
                ..AnnotationStyle::default()
            },
        )
        .unwrap();
    let prepared = PreparedImage::prepare(session.document(), &UploadOptions::default()).unwrap();
    let preview = prepared.decode_preview().unwrap();
    assert_eq!(preview.dimensions(), (90, 60));
    assert_eq!(preview.get_pixel(35, 25).0, [0, 0, 0, 255]);
    assert_eq!(preview.get_pixel(20, 20).0, [24, 72, 136, 255]);
    assert_eq!(preview.get_pixel(60, 20).0, [24, 72, 136, 255]);
    assert!(!preview.pixels().any(|p| matches!(
        p.0,
        [251, 103, 7, 255] | [231, 17, 199, 255] | [255, 0, 0, 255]
    )));
    let exported = image::load_from_memory(&session.document().render_png().unwrap())
        .unwrap()
        .to_rgba8();
    assert_ne!(preview, exported);
}
#[test]
fn transparent_hidden_rgb_is_erased_before_encoding() {
    let image = RgbaImage::from_fn(4, 1, |x, _| {
        Rgba(match x {
            0 => [231, 17, 199, 0],
            1 => [3, 200, 91, 0],
            2 => [10, 20, 30, 128],
            _ => [40, 50, 60, 255],
        })
    });
    let doc = ScreenshotDocument::from_rgba(image).unwrap();
    let prepared = PreparedImage::prepare(&doc, &UploadOptions::default()).unwrap();
    let decoded = prepared.decode_preview().unwrap();
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 255, 255, 255]);
    assert_eq!(decoded.get_pixel(1, 0).0, [255, 255, 255, 255]);
    assert_eq!(decoded.get_pixel(2, 0).0, [132, 137, 142, 255]);
    assert_eq!(decoded.get_pixel(3, 0).0, [40, 50, 60, 255]);
    assert!(prepared.background_notice().contains("white"));
}
#[test]
fn different_invisible_rgb_produces_identical_downsampled_upload() {
    let options = UploadOptions {
        max_long_edge: 800,
        ..UploadOptions::default()
    };
    let make = |hidden: [u8; 3]| {
        let image = RgbaImage::from_fn(1601, 7, |x, _| {
            if x % 3 == 0 {
                Rgba([hidden[0], hidden[1], hidden[2], 0])
            } else {
                Rgba([12, 83, 172, 255])
            }
        });
        PreparedImage::prepare(&ScreenshotDocument::from_rgba(image).unwrap(), &options).unwrap()
    };
    let a = make([251, 1, 147]);
    let b = make([0, 200, 8]);
    assert!(
        a.png() == b.png(),
        "Hidden RGB must not affect final bytes."
    );
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.dimensions(), (800, 3));
    assert!(a.decode_preview().unwrap().pixels().all(|p| p[3] == 255));
}
#[test]
fn approved_bytes_preview_digest_and_clone_remain_identical_after_edit() {
    let mut session = ScreenshotEditSession::new(document(64, 48));
    let first = PreparedImage::prepare(session.document(), &UploadOptions::default()).unwrap();
    let cloned = first.clone();
    session.set_crop(Some(Rect::new(0., 0., 10., 10.))).unwrap();
    let second = PreparedImage::prepare(session.document(), &UploadOptions::default()).unwrap();
    assert_eq!(first.dimensions(), (64, 48));
    assert_eq!(second.dimensions(), (10, 10));
    assert!(std::ptr::eq(first.png().as_ptr(), cloned.png().as_ptr()));
    let digest: String = Sha256::digest(first.png())
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    assert_eq!(first.digest(), digest);
    let direct = image::load_from_memory_with_format(first.png(), image::ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    assert_eq!(first.decode_preview().unwrap(), direct);
}
#[test]
fn upload_edge_bounds_floor_aspect_and_no_upscale() {
    assert_eq!(UploadOptions::default().max_long_edge, 2200);
    for edge in [0, 1, 799, 5001, u32::MAX] {
        assert!(
            PreparedImage::prepare(
                &document(4, 4),
                &UploadOptions {
                    max_long_edge: edge,
                    ..UploadOptions::default()
                }
            )
            .is_err()
        );
    }
    for (width, height, expected) in [
        (17, 31, (17, 31)),
        (2401, 1201, (800, 400)),
        (1, 2401, (1, 800)),
        (2401, 1, (800, 1)),
    ] {
        let output = PreparedImage::prepare(
            &document(width, height),
            &UploadOptions {
                max_long_edge: 800,
                ..UploadOptions::default()
            },
        )
        .unwrap();
        assert_eq!(output.dimensions(), expected);
        assert!(output.byte_count() <= MAX_UPLOAD_BYTES);
    }
}
#[test]
fn bounded_writer_rejects_overflow_and_never_recovers() {
    let mut writer = BoundedBytes::new(8);
    writer.write_all(&[0; 8]).unwrap();
    assert!(writer.write_all(&[1]).is_err());
    assert!(writer.exceeded);
    assert_eq!(writer.bytes.len(), 8);
    assert!(writer.write(&[]).is_err());
    assert_eq!(MAX_BASE64_BYTES, 27_962_028);
}
#[test]
fn fixture_source_identity_accepts_edits_but_not_equal_pixels_or_retirement() {
    let doc = document(8, 8);
    let binding = DocumentBinding::new(&doc);
    let image = PreparedImage::prepare(&doc, &UploadOptions::default()).unwrap();
    assert!(binding.matches_document(&doc.clone()));
    assert!(binding.matches_prepared(&image));
    assert!(!binding.matches_document(&document(8, 8)));
    drop(doc);
    assert!(!binding.matches_prepared(&image));
}
#[test]
fn all_image_builders_embed_only_one_exact_prepared_png_and_ocr_prompt() {
    let image = prepared();
    for provider in [
        Provider::OpenAIChat,
        Provider::OpenAIResponses,
        Provider::Anthropic,
    ] {
        let request = build_request(
            &image,
            &UploadOptions::default(),
            &authority(provider).lease(),
        )
        .unwrap();
        let body = body(&request);
        assert_eq!(body["stream"], false);
        assert_eq!(body["model"], "approved-old-model");
        assert!(
            !request
                .body()
                .windows(b"WRITING_PROMPT".len())
                .any(|v| v == b"WRITING_PROMPT")
        );
        let (encoded, prompt) = match provider {
            Provider::OpenAIChat => {
                assert_eq!(body["messages"].as_array().unwrap().len(), 1);
                (
                    body["messages"][0]["content"][1]["image_url"]["url"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("data:image/png;base64,")
                        .unwrap(),
                    body["messages"][0]["content"][0]["text"].as_str().unwrap(),
                )
            }
            Provider::OpenAIResponses => {
                assert_eq!(body["input"].as_array().unwrap().len(), 1);
                assert_eq!(body["text"]["format"]["type"], "json_object");
                (
                    body["input"][0]["content"][1]["image_url"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("data:image/png;base64,")
                        .unwrap(),
                    body["input"][0]["content"][0]["text"].as_str().unwrap(),
                )
            }
            Provider::Anthropic => {
                assert_eq!(body["messages"].as_array().unwrap().len(), 1);
                assert_eq!(
                    body["messages"][0]["content"][0]["source"]["media_type"],
                    "image/png"
                );
                (
                    body["messages"][0]["content"][0]["source"]["data"]
                        .as_str()
                        .unwrap(),
                    body["messages"][0]["content"][1]["text"].as_str().unwrap(),
                )
            }
        };
        let bytes = STANDARD.decode(encoded).unwrap();
        assert!(bytes == image.png());
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().to_rgba8(),
            image.decode_preview().unwrap()
        );
        assert!(prompt.contains("Do not infer hidden"));
        assert!(!prompt.contains("Local Mac OCR hint"));
        assert!(request.body().len() <= MAX_REQUEST_BYTES);
    }
}
#[test]
fn endpoints_resolve_exact_routes_and_reject_credentials_controls_and_queries() {
    for provider in [
        Provider::OpenAIChat,
        Provider::OpenAIResponses,
        Provider::Anthropic,
    ] {
        let lease = authority(provider).lease();
        let route = match provider {
            Provider::OpenAIChat => "chat/completions",
            Provider::OpenAIResponses => "responses",
            Provider::Anthropic => "messages",
        };
        assert_eq!(
            lease.destination(),
            format!("https://example.invalid/prefix/v1/{route}")
        );
        let mut exact = config(provider);
        exact.endpoint = lease.destination().into();
        assert_eq!(
            ProviderAuthority::new(exact, "fake".into())
                .unwrap()
                .lease()
                .destination(),
            lease.destination()
        );
    }
    for endpoint in [
        "ftp://example.invalid",
        "https:example.invalid",
        "https://example.invalid\\path",
        "\nhttps://example.invalid",
        "https://u:k@example.invalid",
        "http://@example.invalid",
        "https://example.invalid?q=x",
        "https://example.invalid/#x",
        "https://exam\nple.invalid",
        "https://example.invalid/%0a",
        "https://example.invalid/%00",
        "https://example.invalid/%7F",
    ] {
        let mut c = config(Provider::OpenAIChat);
        c.endpoint = endpoint.into();
        assert!(ProviderAuthority::new(c, "fake".into()).is_err());
    }
    for key in [
        "fake\rkey",
        "fake\nkey",
        "fake\0key",
        "fake\tkey",
        "fake\u{7f}key",
        "é",
    ] {
        assert!(ProviderAuthority::new(config(Provider::OpenAIChat), key.into()).is_err());
    }
}
#[test]
fn openai_allows_empty_key_anthropic_requires_nonempty_bounded_key() {
    for provider in [Provider::OpenAIChat, Provider::OpenAIResponses] {
        let authority = ProviderAuthority::new(config(provider), String::new()).unwrap();
        let req =
            build_request(&prepared(), &UploadOptions::default(), &authority.lease()).unwrap();
        assert!(
            !req.headers()
                .iter()
                .any(|(name, _)| name == "authorization")
        );
    }
    assert!(ProviderAuthority::new(config(Provider::Anthropic), " ".into()).is_err());
    assert!(ProviderAuthority::new(config(Provider::OpenAIChat), "x".repeat(8193)).is_err());
}
#[test]
fn approved_old_config_options_key_survive_new_settings_but_explicit_revoke_blocks() {
    let mut original = config(Provider::OpenAIChat);
    let options = GenerationOptions {
        temperature: Some(0.4),
        reasoning_effort: Some(ReasoningEffort::High),
        thinking: Thinking::ProviderDefault,
    };
    let owner =
        ProviderAuthority::with_options(original.clone(), "old-fake-key".into(), options).unwrap();
    let lease = owner.lease();
    original.model = "new-model".into();
    original.endpoint = "https://new.invalid".into();
    original.provider = Provider::OpenAIResponses;
    original.max_output_tokens = 1234;
    let replacement = ProviderAuthority::new(original, "new-fake-key".into()).unwrap();
    assert!(lease.is_valid());
    assert!(!lease.same_authority(&replacement.lease()));
    let request = build_request(&prepared(), &UploadOptions::default(), &lease).unwrap();
    assert_eq!(body(&request)["model"], "approved-old-model");
    assert_eq!(body(&request)["max_completion_tokens"], 4096);
    assert_eq!(body(&request)["reasoning_effort"], "high");
    assert_eq!(body(&request)["temperature"], 0.4);
    assert!(
        request
            .headers()
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer old-fake-key")
    );
    owner.revoke();
    assert!(!lease.is_valid());
    assert!(!owner.lease().is_valid());
    assert!(build_request(&prepared(), &UploadOptions::default(), &lease).is_err());
}
#[test]
fn provider_generation_options_are_frozen_and_wire_specific() {
    for provider in [
        Provider::OpenAIChat,
        Provider::OpenAIResponses,
        Provider::Anthropic,
    ] {
        let owner = ProviderAuthority::with_options(
            config(provider),
            "fake".into(),
            GenerationOptions {
                temperature: Some(0.6),
                reasoning_effort: Some(ReasoningEffort::High),
                thinking: if provider == Provider::Anthropic {
                    Thinking::Budgeted(8192)
                } else {
                    Thinking::ProviderDefault
                },
            },
        )
        .unwrap();
        let request =
            build_request(&prepared(), &UploadOptions::default(), &owner.lease()).unwrap();
        let b = body(&request);
        match provider {
            Provider::OpenAIChat => assert_eq!(b["reasoning_effort"], "high"),
            Provider::OpenAIResponses => assert_eq!(b["reasoning"]["effort"], "high"),
            Provider::Anthropic => {
                assert_eq!(b["output_config"]["effort"], "high");
                assert_eq!(b["max_tokens"], 10240);
                assert!(b.get("temperature").is_none());
                assert_eq!(b["thinking"]["budget_tokens"], 8192);
            }
        }
    }
}
#[test]
fn changed_upload_options_cannot_rebuild_approved_request() {
    let image = prepared();
    let options = UploadOptions {
        output_format: OutputFormat::TableMarkdown,
        ..UploadOptions::default()
    };
    assert!(build_request(&image, &options, &authority(Provider::OpenAIChat).lease()).is_err());
}
#[test]
fn provider_envelopes_exclude_thinking_and_parse_plain_markdown_warnings() {
    for provider in [
        Provider::OpenAIChat,
        Provider::OpenAIResponses,
        Provider::Anthropic,
    ] {
        let text=json!({"plainText":"visible","markdownText":"**visible**","warnings":["unclear glyph"]}).to_string();
        let parsed = parse_response(provider, &envelope(provider, &text)).unwrap();
        assert_eq!(parsed.plain_text, "visible");
        assert_eq!(parsed.markdown_text.as_deref(), Some("**visible**"));
        assert_eq!(parsed.warnings, ["unclear glyph"]);
    }
}
#[test]
fn fenced_json_alias_markdown_only_and_literal_fallback_match_source() {
    for text in [
        "```json\n{\"text\":\"visible\"}\n```",
        "{\"markdownText\":\"visible\"}",
        "{\"plainText\":\"  \",\"markdownText\":\"visible\"}",
    ] {
        assert_eq!(parse_ocr_text(text).unwrap().plain_text, "visible");
    }
    for text in [
        "[unclear] first line",
        "[1, 2]",
        "{ unfinished code",
        "<script>alert(1)</script>",
        "plain fallback",
    ] {
        let parsed = parse_ocr_text(text).unwrap();
        assert_eq!(parsed.plain_text, text);
        assert_eq!(parsed.warnings, ["Provider returned non-JSON OCR text."]);
    }
    for text in [
        "",
        "  ",
        "{}",
        "{\"plainText\":\"\",\"markdownText\":\"\"}",
        "```json\n```",
    ] {
        assert!(parse_ocr_text(text).is_err());
    }
}
#[test]
fn provider_schema_depth_output_warning_and_response_budgets_fail_generically() {
    use super::response::*;
    for text in [
        json!({"plainText":42}),
        json!({"plainText":"ok","markdownText":[]}),
        json!({"plainText":"ok","warnings":[42]}),
        json!({"plainText":"ok","warnings":vec!["x";33]}),
        json!({"plainText":"ok","warnings":["x".repeat(4097)]}),
        json!({"plainText":"ok","warnings":vec!["x".repeat(4096);9]}),
        json!({"plainText":"x".repeat(MAX_OUTPUT_BYTES+1)}),
    ] {
        assert!(parse_ocr_text(&text.to_string()).is_err());
    }
    assert!(parse_ocr_text(&("[".repeat(33) + &"]".repeat(33))).is_err());
    assert!(parse_ocr_text(&format!("[{}]", "[],".repeat(16384))).is_err());
    assert!(parse_response(Provider::OpenAIChat, &vec![b' '; MAX_RESPONSE_BYTES + 1]).is_err());
    let error = parse_response(Provider::OpenAIChat, b"PRIVATE_SYNTHETIC_RESPONSE_SECRET")
        .err()
        .unwrap();
    assert!(!error.contains("PRIVATE_SYNTHETIC"));
    for (provider, value) in [
        (
            Provider::OpenAIChat,
            json!({"choices":[{"finish_reason":"length","message":{"content":"partial"}}]}),
        ),
        (
            Provider::OpenAIResponses,
            json!({"status":"incomplete","output_text":"partial"}),
        ),
        (
            Provider::Anthropic,
            json!({"stop_reason":"max_tokens","content":[{"type":"text","text":"partial"}]}),
        ),
    ] {
        assert!(parse_response(provider, &serde_json::to_vec(&value).unwrap()).is_err());
    }
}
#[test]
fn bounded_valid_max_text_and_warning_edges_are_accepted() {
    let text = "x".repeat(super::response::MAX_OUTPUT_BYTES);
    assert_eq!(parse_ocr_text(&text).unwrap().plain_text.len(), text.len());
    let result = parse_ocr_text(
        &json!({"plainText":"visible","warnings":vec!["w".repeat(1024);32]}).to_string(),
    )
    .unwrap();
    assert_eq!(result.warnings.len(), 32);
}

#[test]
fn png_encoder_cannot_publish_over_limit_even_when_only_final_chunk_exceeds() {
    let image = RgbaImage::from_pixel(8, 8, Rgba([17, 91, 204, 255]));
    let bytes = encode_upload_png(&image, MAX_UPLOAD_BYTES).unwrap();
    assert!(encode_upload_png(&image, bytes.len()).unwrap() == bytes);
    assert!(encode_upload_png(&image, bytes.len() - 1).is_err());
    assert!(encode_upload_png(&image, 8).is_err());
}
