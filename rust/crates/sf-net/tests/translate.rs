//! Response parsing (pure) and a live translation check.

use sf_net::translate::parse_response;

#[test]
fn parses_nested_array_response() {
    let body = r#"[["Merhaba dünya","Hello world",null,null,10]]"#;
    let t = parse_response(body).expect("parse");
    assert_eq!(t.text, "Merhaba dünya");
    assert_eq!(t.detected_language.as_deref(), Some("Hello world"));
}

#[test]
fn parses_single_word_nested_response() {
    // The endpoint wraps even a single word in the array form.
    let t = parse_response(r#"[["Merhaba",null,null,null,1]]"#).expect("parse");
    assert_eq!(t.text, "Merhaba");
    assert!(t.detected_language.is_none());
}

#[test]
fn rejects_garbage() {
    assert!(parse_response("not json").is_none());
    assert!(parse_response("{}").is_none());
}

#[test]
#[ignore = "hits the live Google translate endpoint"]
fn live_translates_hello_to_turkish() {
    let t = sf_net::translate::translate("Hello world", "tr").expect("translate");
    assert!(!t.text.trim().is_empty());
    println!("translated: {}", t.text);
    assert!(t.text.to_lowercase().contains("merhaba") || t.text.to_lowercase().contains("dünya"),
        "unexpected: {}", t.text);
}
