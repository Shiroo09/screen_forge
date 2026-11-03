//! Mirrors `ScreenForge.Tests/{ReverseImageSearchTests,TranslateLanguageRouterTests,GoogleLensClientTests}.cs`
//! and pins the protobuf wire output and upload helpers.

use sf_net::lang_router::{image_route, languages_equal, should_translate_to_pair};
use sf_net::lens::{build_ocr_request, build_request};
use sf_net::proto::ProtoWriter;
use sf_net::reverse_search::{build_lens_upload_html, try_parse_yandex_cbir};
use sf_net::upload::{extract_url, safe_upload_name};

// ─── Language router ──────────────────────────────────────────────────────────

#[test]
fn tr_native_tr_source_routes_to_pair() {
    assert!(should_translate_to_pair("tr", "tr", "Merhaba", "Merhaba"));
}

#[test]
fn tr_native_en_source_keeps_native() {
    assert!(!should_translate_to_pair("tr", "en", "Hello", "Merhaba"));
}

#[test]
fn identity_without_source_routes_to_pair() {
    assert!(should_translate_to_pair("tr", "", "Merhaba", "Merhaba"));
}

#[test]
fn identity_english_with_en_source_does_not_route_to_pair() {
    assert!(!should_translate_to_pair("tr", "en", "Hello", "Hello"));
}

#[test]
fn zh_cn_matches_zh() {
    assert!(languages_equal("zh-CN", "zh"));
}

#[test]
fn image_route_always_auto_source_to_native() {
    let (target, source) = image_route(Some("tr"));
    assert_eq!(target, "tr");
    assert!(source.is_none());

    // Null/blank native falls back to "tr".
    assert_eq!(image_route(None).0, "tr");
    assert_eq!(image_route(Some("  ")).0, "tr");
}

// ─── Reverse image search ─────────────────────────────────────────────────────

#[test]
fn yandex_fixture_reads_id_and_url() {
    let json = r#"{"blocks":[{"params":{"cbirId":"abc123","originalImageUrl":"https://i.yandex/x"}}]}"#;
    let (id, url) = try_parse_yandex_cbir(json).expect("parse");
    assert_eq!(id, "abc123");
    assert_eq!(url.as_deref(), Some("https://i.yandex/x"));
}

#[test]
fn yandex_missing_cbir_id_returns_none() {
    let json = r#"{"blocks":[{"params":{"originalImageUrl":"https://i.yandex/x"}}]}"#;
    assert!(try_parse_yandex_cbir(json).is_none());
}

#[test]
fn yandex_garbage_returns_none() {
    assert!(try_parse_yandex_cbir("not-json").is_none());
}

#[test]
fn lens_html_posts_encoded_image() {
    let png = [0x89u8, 0x50, 0x4E, 0x47];
    let html = build_lens_upload_html(&png, 1_700_000_000_000);
    assert!(html.contains("https://lens.google.com/v3/upload"));
    assert!(html.contains("name=\"encoded_image\""));
    assert!(html.contains("iVBORw==")); // base64 of the 4 magic bytes
    assert!(html.contains("document.getElementById(\"f\").submit()"));
}

// ─── Protobuf / Lens requests ─────────────────────────────────────────────────

#[test]
fn varint_encodes_multi_byte_values() {
    let mut w = ProtoWriter::new();
    w.write_varint(300); // 0xAC 0x02
    assert_eq!(w.into_bytes(), vec![0xAC, 0x02]);
}

#[test]
fn tag_packs_field_and_wire_type() {
    let mut w = ProtoWriter::new();
    w.write_tag(1, 2);
    assert_eq!(w.into_bytes(), vec![0x0A]);
}

#[test]
fn ocr_request_differs_from_translate_request() {
    let png = [0x89u8, 0x50, 0x4E, 0x47, 1, 2, 3, 4];
    let ocr = build_ocr_request(&png, 8, 8, 12345);
    let translate = build_request(&png, 8, 8, "tr", None, 12345);
    assert_ne!(translate, ocr);
}

#[test]
fn ocr_request_is_a_protobuf_payload() {
    let png = [0x89u8, 0x50, 0x4E, 0x47, 1, 2, 3, 4];
    let ocr = build_ocr_request(&png, 8, 8, 12345);
    assert!(ocr.len() > 32);
    // Envelope starts with field 1, wire type 2 (length-delimited).
    assert_eq!(ocr[0], 0x0A);
}

#[test]
fn translate_request_is_deterministic_for_fixed_uuid() {
    let png = [1u8, 2, 3, 4];
    let a = build_request(&png, 4, 4, "tr", Some("en"), 42);
    let b = build_request(&png, 4, 4, "tr", Some("en"), 42);
    assert_eq!(a, b);
}

#[test]
fn translate_request_carries_source_language_only_when_present() {
    let png = [1u8, 2, 3, 4];
    let with_source = build_request(&png, 4, 4, "tr", Some("en"), 1);
    let without_source = build_request(&png, 4, 4, "tr", None, 1);
    assert!(with_source.len() > without_source.len());
}

// ─── Upload helpers ───────────────────────────────────────────────────────────

#[test]
fn extract_url_takes_first_http_token() {
    assert_eq!(
        extract_url("https://litter.catbox.moe/abc.png\n"),
        Some("https://litter.catbox.moe/abc.png".to_string())
    );
    assert_eq!(
        extract_url("uploaded: https://x/y.gif trailing"),
        Some("https://x/y.gif".to_string())
    );
    assert_eq!(extract_url("no url here"), None);
}

#[test]
fn safe_upload_name_strips_header_breakers() {
    assert_eq!(safe_upload_name(Some("shot.png"), "image/png"), "shot.png");
    assert_eq!(
        safe_upload_name(Some("a/b\\\"c\r\n.png"), "image/png"),
        "c.png"
    );
    // No usable name → MIME-derived default.
    assert_eq!(safe_upload_name(None, "image/jpeg"), "screenforge.jpg");
    assert_eq!(safe_upload_name(Some(".."), "image/gif"), "screenforge.gif");
    assert_eq!(safe_upload_name(Some(""), "image/webp"), "screenforge.webp");
}

// ─── Golden protobuf fixtures (byte-identical to the C# ProtoWriter) ───────────

const GOLDEN_UUID: u64 = 1234567890123456789;
const GOLDEN_PNG: [u8; 12] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e} — run SF_WRITE_GOLDEN=1 dotnet test"))
}

#[test]
fn translate_request_matches_csharp_bytes() {
    let actual = build_request(&GOLDEN_PNG, 8, 8, "tr", Some("en"), GOLDEN_UUID);
    assert_eq!(actual, fixture("lens_translate.bin"));
}

#[test]
fn ocr_request_matches_csharp_bytes() {
    let actual = build_ocr_request(&GOLDEN_PNG, 8, 8, GOLDEN_UUID);
    assert_eq!(actual, fixture("lens_ocr.bin"));
}
