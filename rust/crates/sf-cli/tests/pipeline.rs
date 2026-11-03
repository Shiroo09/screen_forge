//! End-to-end: capture the real screen and encode it, then decode the result back.
//! This is the regression guard for the whole Rust pipeline working together.

#[test]
#[cfg(windows)]
fn capture_to_gif_roundtrip() {
    let first = sf_capture::capture_virtual_screen().expect("capture screen");
    assert!(first.width > 0 && first.height > 0);

    let mut out = Vec::new();
    {
        let mut gif = sf_gif::gif_file::GifFile::new(&mut out);
        gif.set_canvas_size(first.width, first.height);
        gif.add_frame(
            &first.bgra,
            sf_gif::gif_file::Rect {
                x: 0,
                y: 0,
                width: first.width,
                height: first.height,
            },
            100,
            false,
        );
        gif.finish();
    }

    // Decode with an independent decoder to prove the stream is real.
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = opts
        .read_info(std::io::Cursor::new(&out))
        .expect("independent decode");
    assert_eq!(reader.width() as i32, first.width);
    assert_eq!(reader.height() as i32, first.height);
    let frame = reader.read_next_frame().expect("read").expect("frame present");
    assert_eq!(frame.buffer.len(), (first.width * first.height * 4) as usize);
}
