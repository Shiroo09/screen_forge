//! Verifies the Media Foundation H.264 writer actually produces a playable MP4:
//! create it, feed a few solid-colour frames, finalise, and check the file.

#[test]
#[cfg(windows)]
fn writes_a_valid_mp4() {
    let path = std::env::temp_dir().join("sf-record-test.mp4");
    let _ = std::fs::remove_file(&path);

    let (w, h, fps) = (320, 240, 15);
    let mut writer = sf_record::Mp4Writer::new(path.to_str().unwrap(), w, h, fps, 2_000_000)
        .expect("create MP4 writer (H.264 encoder present?)");

    let frame_dur = writer.frame_duration();
    let stride = (w * h * 4) as usize;

    // 15 frames, 1 s: colour cycles so the encoder emits real keyframes.
    for i in 0..15i64 {
        let mut bgra = vec![0u8; stride];
        let v = (i * 16) as u8;
        for px in bgra.chunks_exact_mut(4) {
            px[0] = v; // B
            px[1] = 128; // G
            px[2] = 255 - v; // R
            px[3] = 255; // A
        }
        writer.write_bgra(&bgra, i * frame_dur).expect("write frame");
    }
    writer.finish().expect("finalize");

    let meta = std::fs::metadata(&path).expect("mp4 exists");
    assert!(meta.len() > 1024, "mp4 too small: {} bytes", meta.len());

    // An MP4 has an 'ftyp' box in the first bytes and a 'moov' box after finalize.
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[4..8], b"ftyp", "missing ftyp box");
    let has_moov = bytes.windows(4).any(|w| w == b"moov");
    assert!(has_moov, "missing moov box (finalize did not run)");

    let _ = std::fs::remove_file(&path);
}
