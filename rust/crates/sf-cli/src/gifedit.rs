//! GIF editing: decode an existing GIF, apply frame operations, re-encode.
//! This is the headless core of the GIF editor; the timeline UI drives the same
//! `sf_edit::frame_ops` functions this exposes.

use sf_edit::frame::{EditorFrame, FrameInput};
use sf_edit::frame_ops;
use sf_gif::gif_file::{GifFile, Rect};

/// A decoded GIF: frames (BGRA) plus per-frame delays.
pub struct DecodedGif {
    pub width: i32,
    pub height: i32,
    pub frames: Vec<EditorFrameRef>,
}

/// Frame reference the editor works with, holding the decoded BGRA.
pub struct EditorFrameRef {
    pub bgra: Vec<u8>,
    pub delay_ms: i32,
}

/// Reads a GIF into BGRA frames + delays.
pub fn decode(path: &str) -> Result<DecodedGif, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);

    let mut reader = opts
        .read_info(std::io::Cursor::new(&bytes))
        .map_err(|e| format!("gif decode: {e}"))?;

    let width = reader.width() as i32;
    let height = reader.height() as i32;
    let mut frames = Vec::new();

    // The decoder returns a full canvas per frame (RGBA); convert to BGRA.
    let mut canvas = vec![0u8; (width * height * 4) as usize];

    while let Some(frame) = reader.read_next_frame().map_err(|e| format!("frame: {e}"))? {
        // Composite the sub-rectangle at its offset, RGBA→BGRA while copying.
        let fx = frame.left as i32;
        let fy = frame.top as i32;
        let fw = frame.width as i32;
        let fh = frame.height as i32;
        for y in 0..fh {
            for x in 0..fw {
                let src = ((y * fw + x) * 4) as usize;
                let dx = fx + x;
                let dy = fy + y;
                if dx < 0 || dy < 0 || dx >= width || dy >= height {
                    continue;
                }
                let dst = ((dy * width + dx) * 4) as usize;
                canvas[dst] = frame.buffer[src + 2]; // R→B
                canvas[dst + 1] = frame.buffer[src + 1]; // G
                canvas[dst + 2] = frame.buffer[src]; // B→R
                canvas[dst + 3] = frame.buffer[src + 3];
            }
        }

        let delay_ms = frame.delay as i32 * 10; // GIF is in 1/100 s
        frames.push(EditorFrameRef {
            bgra: canvas.clone(),
            delay_ms,
        });
    }

    if frames.is_empty() {
        return Err("gif has no frames".to_string());
    }

    Ok(DecodedGif {
        width,
        height,
        frames,
    })
}

/// Re-encodes frames to a GIF. `ops` is applied through `sf_edit::frame_ops`
/// before encoding, so the editor operations run on real frames.
pub fn encode(
    path: &str,
    width: i32,
    height: i32,
    frames: &[EditorFrameRef],
    keep: usize,
    remove: usize,
    reverse: bool,
) -> Result<usize, String> {
    // Build EditorFrames so the shared frame operations apply.
    let mut edit: Vec<std::sync::Arc<EditorFrame>> = frames
        .iter()
        .map(|f| EditorFrame::new(f.bgra.clone(), f.delay_ms, FrameInput::default()))
        .collect();

    edit = frame_ops::reduce(&edit, keep.max(1), remove.max(1), frame_ops::DelayMergeMode::Distribute);
    if reverse {
        edit = frame_ops::reverse(&edit);
    }

    let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
    let writer = std::io::BufWriter::new(file);
    let mut gif = GifFile::new(writer);
    gif.set_canvas_size(width, height);
    gif.maximum_number_color = 256;
    gif.quantizer_type = sf_gif::QuantizerType::Octree;

    for frame in &edit {
        let bgra = frame.pixels_uncached();
        gif.add_frame(
            &bgra,
            Rect { x: 0, y: 0, width, height },
            frame.delay,
            false,
        );
    }
    gif.finish();

    Ok(edit.len())
}
