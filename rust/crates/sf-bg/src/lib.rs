//! Subject segmentation. Port of `ScreenForge/Background/LocalSubjectSegmenter.cs`
//! (the u2netp ONNX path).
//!
//! The C# app preferred the Windows App SDK AI object extractor but fell back to a
//! bundled `u2netp.onnx` through Windows ML. There is no Rust binding for the WinAI
//! API, so the ONNX path becomes the only path — run through the `ort` crate.
//!
//! The preprocessing (resize to 320×320, max-normalize, ImageNet mean/std, RGB
//! planar NCHW) is pure and unit-tested; only `segment` touches the model.

pub mod preprocess;
#[cfg(feature = "onnx")]
pub mod model;
