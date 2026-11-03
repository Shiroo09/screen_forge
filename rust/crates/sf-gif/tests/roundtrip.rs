//! Decodes the GIF this crate writes with an independent decoder (`gif` crate).
//! Header/trailer assertions only prove the file looks right; this proves the
//! LZW stream and palette are actually readable.

use sf_gif::gif_file::{GifFile, Rect};

#[test]
fn written_gif_decodes_with_independent_decoder() {
    let (w, h) = (32usize, 24usize);
    let frame = gradient(w, h);

    let mut out = Vec::new();
    {
        let mut gif = GifFile::new(&mut out);
        gif.set_canvas_size(w as i32, h as i32);
        gif.add_frame(&frame, Rect { x: 0, y: 0, width: w as i32, height: h as i32 }, 100, false);
        gif.finish();
    }

    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(std::io::Cursor::new(&out)).expect("decode");
    assert_eq!(reader.width() as usize, w);
    assert_eq!(reader.height() as usize, h);

    let decoded = reader.read_next_frame().expect("first frame").expect("frame present");
    assert_eq!(decoded.buffer.len(), w * h * 4);

    // BGRA in, RGBA out: top-left is near black, bottom-right is near (127,255,255)
    // (G and B channels both ramp up), minus quantization error.
    let tl = &decoded.buffer[0..3];
    let br = &decoded.buffer[(w * h - 1) * 4..(w * h - 1) * 4 + 3];
    assert!(tl.iter().all(|&c| c < 32), "top-left expected dark, got {tl:?}");
    assert!(br[1] > 180 && br[2] > 180, "bottom-right expected bright G/B, got {br:?}");
}

#[test]
fn multi_frame_gif_decodes_all_frames() {
    let (w, h) = (16usize, 16usize);
    let frames: Vec<Vec<u8>> = (0..3)
        .map(|f| {
            let mut px = gradient(w, h);
            for i in (0..px.len()).step_by(4) {
                px[i] = (f * 80) as u8;
            }
            px
        })
        .collect();

    let mut out = Vec::new();
    {
        let mut gif = GifFile::new(&mut out);
        gif.set_canvas_size(w as i32, h as i32);
        for frame in &frames {
            gif.add_frame(frame, Rect { x: 0, y: 0, width: w as i32, height: h as i32 }, 100, false);
        }
        gif.finish();
    }

    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(std::io::Cursor::new(&out)).expect("decode");

    let mut count = 0;
    while reader.read_next_frame().expect("frame read").is_some() {
        count += 1;
    }
    assert_eq!(count, 3, "expected 3 decodable frames");
}

fn gradient(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            pixels[i] = (x * 255 / (width.saturating_sub(1)).max(1)) as u8;
            pixels[i + 1] = (y * 255 / (height.saturating_sub(1)).max(1)) as u8;
            pixels[i + 2] = ((x + y) * 127 / (width + height).saturating_sub(2).max(1)) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}
