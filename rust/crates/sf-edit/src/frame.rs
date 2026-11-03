//! Frame model. Ports `EditorFrame`/`FrameInput` from
//! `ScreenForge/Gif/Editing/EditorDocument.cs` and `Gif/Input/FrameInput.cs`.
//!
//! Frames are immutable and shared (the undo stack holds references, not copies),
//! so pixels are `Arc`-wrapped: a delay change clones two pointers, never the
//! 8 MB buffer. Pixels stay Deflate-compressed until asked for, and the decoded
//! buffer is cached until `release_decoded` drops it. The C# export path's
//! multi-GB peak came from never calling that release.

use std::sync::{Arc, Mutex};

use sf_gif::frame_store;

/// Mouse buttons held on a frame, shared with the input layer.
pub use sf_capture::MouseButtons;

/// Input activity carried by a frame.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FrameInput {
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub cursor_visible: bool,
    pub buttons: MouseButtons,
    pub click_started: bool,
    pub keys: Vec<String>,
}

impl FrameInput {
    /// Whether the frame carries an event that needs an overlay drawn.
    pub fn has_any_input(&self) -> bool {
        !self.buttons.is_none() || !self.keys.is_empty()
    }
}

/// One frame: compressed pixels, a delay, and its input activity.
pub struct EditorFrame {
    packed: Arc<Vec<u8>>,
    raw_length: usize,
    pub delay: i32,
    pub input: FrameInput,
    decoded: Mutex<Option<Arc<Vec<u8>>>>,
}

impl EditorFrame {
    /// Creates a frame from raw BGRA pixels (compressing them).
    pub fn new(pixels: Vec<u8>, delay: i32, input: FrameInput) -> Arc<Self> {
        let raw_length = pixels.len();
        let packed = frame_store::compress(&pixels);
        Arc::new(Self {
            packed: Arc::new(packed),
            raw_length,
            delay,
            input,
            decoded: Mutex::new(Some(Arc::new(pixels))),
        })
    }

    /// Creates a frame from already-compressed pixels; raw pixels are not decoded.
    pub fn from_packed(
        packed: Vec<u8>,
        raw_length: usize,
        delay: i32,
        input: FrameInput,
    ) -> Arc<Self> {
        Arc::new(Self {
            packed: Arc::new(packed),
            raw_length,
            delay,
            input,
            decoded: Mutex::new(None),
        })
    }

    /// BGRA pixel data. Decoded on first access and cached. Cheap to clone.
    pub fn pixels(&self) -> Arc<Vec<u8>> {
        if let Some(d) = self.decoded.lock().unwrap().as_ref() {
            return Arc::clone(d);
        }

        let mut out = vec![0u8; self.raw_length];
        frame_store::decompress_into(&self.packed, &mut out);
        let arc = Arc::new(out);
        *self.decoded.lock().unwrap() = Some(Arc::clone(&arc));
        arc
    }

    /// Decodes without caching — for the streaming export path that must not
    /// pin every frame it touches.
    pub fn pixels_uncached(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.raw_length];
        frame_store::decompress_into(&self.packed, &mut out);
        out
    }

    pub fn raw_length(&self) -> usize {
        self.raw_length
    }

    pub fn packed(&self) -> &[u8] {
        &self.packed
    }

    pub fn is_decoded(&self) -> bool {
        self.decoded.lock().unwrap().is_some()
    }

    /// Same pixels, new delay. Only pointer clones — the buffer is shared.
    pub fn with_delay(self: &Arc<Self>, delay: i32) -> Arc<EditorFrame> {
        if delay == self.delay {
            return Arc::clone(self);
        }
        Arc::new(Self {
            packed: Arc::clone(&self.packed),
            raw_length: self.raw_length,
            delay,
            input: self.input.clone(),
            decoded: Mutex::new(self.decoded.lock().unwrap().clone()),
        })
    }

    /// Same frame, new pixels (recompresses).
    pub fn with_pixels(self: &Arc<Self>, pixels: &[u8]) -> Arc<EditorFrame> {
        Self::new(pixels.to_vec(), self.delay, self.input.clone())
    }

    pub fn release_decoded(&self) {
        *self.decoded.lock().unwrap() = None;
    }
}
