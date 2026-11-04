//! ONNX inference for u2netp via the `ort` crate.
//!
//! Loads the bundled model, runs one 1×3×320×320 input, and returns the first
//! output map (the saliency mask). u2netp emits several side outputs; the C#
//! code took the primary one, which is what this reads.

use crate::preprocess::{self, INPUT_SIZE};
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

/// ONNX Runtime is initialised once per process.
fn environment() -> &'static Environment {
    static ENV: OnceLock<Environment> = OnceLock::new();
    ENV.get_or_init(|| {
        ort::init()
            .with_name("screenforge")
            .build()
            .expect("ONNX Runtime ortamı kurulamadı")
    })
}

/// A loaded u2netp model.
pub struct SubjectModel {
    session: Session,
}

impl SubjectModel {
    /// Loads the model from a file path.
    pub fn load(path: &str) -> Result<Self, String> {
        let session = Session::builder(environment())
            .map_err(|e| format!("ort builder: {e}"))?
            .commit_from_file(path)
            .map_err(|e| format!("model yüklenemedi ({path}): {e}"))?;
        Ok(Self { session })
    }

    /// Runs segmentation and returns (mask, mask_side): an 8-bit mask and the
    /// square side length the model outputs (320).
    pub fn segment(&mut self, bgra: &[u8], width: i32, height: i32) -> Result<(Vec<u8>, i32), String> {
        let input = preprocess::pack_input(bgra, width, height);

        let tensor = Tensor::from_array((
            [1usize, 3, INPUT_SIZE, INPUT_SIZE],
            input.into_boxed_slice(),
        ))
        .map_err(|e| format!("girdi tensörü: {e}"))?;

        // u2netp's input tensor is named "input.1"; fall back to the first input
        // name the session reports if that changes.
        let input_name = self
            .session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "input.1".to_string());

        let outputs = self
            .session
            .run(ort::inputs![input_name.as_str() => tensor])
            .map_err(|e| format!("çıkarım: {e}"))?;

        // u2netp's first output is the fused saliency map, shape [1,1,320,320].
        let (_name, value) = outputs
            .iter()
            .next()
            .ok_or("model çıktı vermedi")?;
        let (shape, data) = value
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("çıktı okunamadı: {e}"))?;

        let dims: Vec<i64> = shape.iter().copied().collect();
        let side = dims.get(2).copied().unwrap_or(INPUT_SIZE as i64) as i32;
        let other = dims.get(3).copied().unwrap_or(INPUT_SIZE as i64) as i32;

        let mask = preprocess::resize_mask(data, other, side, width.max(1), height.max(1));
        Ok((mask, other))
    }
}
