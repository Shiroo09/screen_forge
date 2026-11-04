//! Runs the real u2netp model and checks it produces a plausible mask.
#![cfg(feature = "onnx")]

#[test]
fn segments_a_synthetic_image() {
    let candidates = [
        concat!(env!("CARGO_MANIFEST_DIR"), "/u2netp.onnx"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../ScreenForge/Models/u2netp.onnx"),
    ];
    let model_path = candidates.iter().find(|p| std::path::Path::new(p).exists());
    let Some(path) = model_path else {
        eprintln!("u2netp.onnx not found; skipping");
        return;
    };

    let mut model = sf_bg::model::SubjectModel::load(path).expect("load u2netp");
    let (w, h) = (64i32, 64i32);
    let mut bgra = vec![20u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let inside = (16..48).contains(&x) && (16..48).contains(&y);
            let v = if inside { 230 } else { 20 };
            bgra[i] = v; bgra[i + 1] = v; bgra[i + 2] = v; bgra[i + 3] = 255;
        }
    }

    let (mask, side) = model.segment(&bgra, w, h).expect("segment");
    assert_eq!(side, 320);
    assert_eq!(mask.len(), (w * h) as usize);
    let min = *mask.iter().min().unwrap();
    let max = *mask.iter().max().unwrap();
    assert!(max > min, "mask is flat; inference did not run");
    let centre = mask[((h / 2) * w + w / 2) as usize];
    let corner = mask[0];
    println!("mask min={min} max={max} centre={centre} corner={corner}");
    assert!(centre >= corner);
}
