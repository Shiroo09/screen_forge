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

/// Composites a segmented subject onto a new background.
///
/// `mask` is the 8-bit saliency map (255 = subject) at the image's size. Pixels
/// below `threshold` take the background color; above, the subject is kept. This
/// is the headless equivalent of `BackgroundCompositor`'s solid-color fill.
pub fn composite_on_solid(
    bgra: &[u8],
    mask: &[u8],
    background: [u8; 3],
) -> Vec<u8> {
    let mut out = bgra.to_vec();
    let pixels = (bgra.len() / 4).min(mask.len());
    for (p, &m) in mask.iter().enumerate().take(pixels) {
        let i = p * 4;
        // Feathered blend using the mask as alpha keeps soft edges.
        let a = m as u32;
        let ia = 255 - a;
        out[i] = ((bgra[i] as u32 * a + background[2] as u32 * ia) / 255) as u8; // B
        out[i + 1] = ((bgra[i + 1] as u32 * a + background[1] as u32 * ia) / 255) as u8; // G
        out[i + 2] = ((bgra[i + 2] as u32 * a + background[0] as u32 * ia) / 255) as u8; // R
        out[i + 3] = 255;
    }
    out
}

#[cfg(test)]
mod composite_tests {
    use super::composite_on_solid;

    #[test]
    fn subject_kept_background_replaced() {
        // Two pixels: first fully subject, second fully background.
        let bgra = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let mask = vec![255u8, 0];
        let out = composite_on_solid(&bgra, &mask, [255, 255, 255]);

        // Subject pixel unchanged.
        assert_eq!(&out[0..3], &[10, 20, 30]);
        // Background pixel becomes white.
        assert_eq!(&out[4..7], &[255, 255, 255]);
    }
}
