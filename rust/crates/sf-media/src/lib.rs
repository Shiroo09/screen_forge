//! Pure media math and PCM conversion, ported from `ScreenForge/Record`.
//!
//! These modules carry no Windows dependency, so behaviour is pinned against the
//! C# original (`ScreenForge.Tests/AudioConvertTests.cs`, plus the geometry and
//! bitrate rules) rather than against a captured frame.

pub mod audio;
pub mod bitrate;
pub mod geometry;

pub use audio::{to_stereo, PcmFormat, PcmPump};
pub use bitrate::bits_per_second;
pub use geometry::{cap_long_edge, even_size, MAX_LONG_EDGE};
