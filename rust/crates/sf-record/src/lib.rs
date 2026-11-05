//! MP4 H.264 recording via Media Foundation.
//! Port of `ScreenForge/Record/MfH264Writer.cs`.
//!
//! The C# writer used Vortice's `IMFSinkWriter`. This drives the same COM object
//! through `windows-rs`: create a sink writer for the output file, add an H.264
//! output stream, feed it RGB32 frames. Frames are flipped to bottom-up, matching
//! the source's `Marshal.Copy` row order.

pub mod session;

#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod wasapi;
#[cfg(windows)]
pub use mf::Mp4Writer;
#[cfg(windows)]
pub use wasapi::AudioCapture;

pub use session::{PauseHandle, RecordPhase, Session, PAUSE_SLICE};

/// Whether MP4 recording is available on this platform/build.
pub const fn is_supported() -> bool {
    cfg!(windows)
}

#[cfg(not(windows))]
pub struct Mp4Writer;

#[cfg(not(windows))]
impl Mp4Writer {
    pub fn new(
        _path: &str,
        _width: i32,
        _height: i32,
        _fps: i32,
        _bitrate: i32,
    ) -> Result<Self, String> {
        Err("MP4 recording is only supported on Windows".to_string())
    }

    pub fn with_audio(
        _path: &str,
        _width: i32,
        _height: i32,
        _fps: i32,
        _bitrate: i32,
        _audio_sample_rate: Option<i32>,
    ) -> Result<Self, String> {
        Err("MP4 recording is only supported on Windows".to_string())
    }

    pub fn has_audio(&self) -> bool {
        false
    }

    pub fn audio_sample_rate(&self) -> i32 {
        48000
    }

    pub fn write_pcm16(&mut self, _pcm: &[u8], _timestamp: i64) -> Result<(), String> {
        Ok(())
    }
}
