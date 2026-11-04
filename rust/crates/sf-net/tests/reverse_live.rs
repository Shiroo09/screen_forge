//! Lens path is local (no network): it must produce a runnable HTML file.
//! Yandex needs the network and is left to manual runs.

#[test]
fn lens_html_file_is_written_and_valid() {
    // Minimal valid PNG bytes.
    let png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

    let path = sf_net::reverse_http::search_url(&png, sf_net::reverse_http::Engine::GoogleLens)
        .expect("write lens html");

    let html = std::fs::read_to_string(&path).expect("read back");
    assert!(html.contains("lens.google.com/v3/upload"));
    assert!(html.contains("encoded_image"));
    assert!(html.contains("document.getElementById(\"f\").submit()"));

    let _ = std::fs::remove_file(&path);
}
