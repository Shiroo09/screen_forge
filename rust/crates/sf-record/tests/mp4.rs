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

/// The AAC track is what `settings.video.record_system_audio` needs; without it
/// the setting would be a label. Checks the stream is accepted and written.
#[test]
#[cfg(windows)]
fn writes_an_mp4_with_an_aac_audio_track() {
    let path = std::env::temp_dir().join("sf-record-audio-test.mp4");
    let _ = std::fs::remove_file(&path);

    let (w, h, fps, rate) = (320, 240, 15, 48000);
    let mut writer =
        sf_record::Mp4Writer::with_audio(path.to_str().unwrap(), w, h, fps, 2_000_000, Some(rate))
            .expect("create MP4 writer");
    assert!(writer.has_audio(), "AAC stream not accepted");
    assert_eq!(writer.audio_sample_rate(), 48000);

    let frame_dur = writer.frame_duration();
    let stride = (w * h * 4) as usize;

    // 1 s of video, and one 1024-sample AAC frame per video frame at 15 fps
    // (1024 * 15 ≈ 15 kHz, deliberately sparse — just enough to be non-empty).
    let quantum = 1024usize;
    for i in 0..15i64 {
        let bgra = vec![0x80u8; stride];
        writer.write_bgra(&bgra, i * frame_dur).expect("write frame");

        let mut pcm = Vec::with_capacity(quantum * 4);
        for s in 0..quantum {
            let v = ((i * 1000 + s as i64) as f32 * 0.01).sin();
            let sample = (v * 8000.0) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes()); // L
            pcm.extend_from_slice(&sample.to_le_bytes()); // R
        }
        writer
            .write_pcm16(&pcm, i * 1024 * 10_000_000 / rate as i64)
            .expect("write audio");
    }
    writer.finish().expect("finalize");

    let bytes = std::fs::read(&path).expect("mp4 exists");
    assert!(bytes.len() > 1024, "mp4 too small: {} bytes", bytes.len());
    assert_eq!(&bytes[4..8], b"ftyp", "missing ftyp box");
    assert!(bytes.windows(4).any(|w| w == b"moov"), "missing moov box");
    // An audio track shows up as a 'soun' handler; a video-only file has none.
    assert!(
        bytes.windows(4).any(|w| w == b"soun"),
        "no audio track in the output"
    );

    let _ = std::fs::remove_file(&path);
}
