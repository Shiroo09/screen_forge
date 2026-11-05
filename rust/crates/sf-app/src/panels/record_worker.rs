//! The capture loop behind the Record panel.
//!
//! Runs on a worker thread: the region is captured once and every frame comes
//! from that rectangle only — never the whole desktop. Pause, resume and restart
//! are observed here through the shared [`sf_record::PauseHandle`], which the
//! panel and the status overlay both drive.
//!
//! GIF frames stream to disk (nothing accumulates), while the byte count is
//! reported so the overlay can show memory; MP4 goes through Media Foundation
//! and may carry an AAC track.

use std::time::{Duration, Instant};

use sf_record::PauseHandle;

/// Everything the loop needs, resolved on the UI thread before the thread starts.
#[derive(Clone, Debug)]
pub struct RecordingOptions {
    /// Capture rectangle in virtual-screen coordinates.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub fps: i32,
    /// GIF only: stop once this many frames have been written.
    pub max_frames: Option<u64>,
    /// GIF only: stop once this many frame bytes have been captured
    /// (`settings.gif.max_frame_memory_mb`).
    pub max_bytes: Option<u64>,
    /// Draw the mouse cursor into the frames.
    pub capture_cursor: bool,
    /// MP4 only: open an AAC track fed with the system loopback audio.
    pub system_audio: bool,
    /// MP4 only: add the microphone to the audio mix.
    pub microphone: bool,
}

impl Default for RecordingOptions {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            fps: 10,
            max_frames: None,
            max_bytes: None,
            capture_cursor: false,
            system_audio: false,
            microphone: false,
        }
    }
}

/// Where a capture comes from. The loop only knows this trait, so a test can
/// drive it with a synthetic "screen" and prove the region is honoured.
pub trait CaptureSource {
    fn capture(&self, x: i32, y: i32, w: i32, h: i32) -> Result<sf_capture::Captured, String>;
}

/// The real screen (GDI BitBlt).
pub struct ScreenSource;

impl CaptureSource for ScreenSource {
    fn capture(&self, x: i32, y: i32, w: i32, h: i32) -> Result<sf_capture::Captured, String> {
        sf_capture::capture_region(x, y, w, h)
    }
}

/// Per-recording GIF encoder settings, copied out of `settings.gif`.
pub struct GifEncode {
    pub color_count: i32,
    pub quantizer: String,
    pub sampling_factor: i32,
    pub dithering: bool,
    pub use_global_palette: bool,
}

/// Captures `opts.fps` frames per second from `opts`' region into `path`.
///
/// Returns the number of frames written.
pub fn record_gif(
    path: &str,
    opts: &RecordingOptions,
    encode: &GifEncode,
    handle: &PauseHandle,
    source: &dyn CaptureSource,
) -> Result<u64, String> {
    use sf_gif::gif_file::{GifFile, Rect};

    let fps = opts.fps.clamp(1, 60);
    let delay_ms = (1000.0 / fps as f64).round().max(1.0) as i32;
    let interval = Duration::from_millis((1000 / fps).max(1) as u64);

    let first = source.capture(opts.x, opts.y, opts.w, opts.h)?;
    let (w, h) = (first.width, first.height);
    if w <= 0 || h <= 0 {
        return Err("seçilen bölge yakalanamadı".to_string());
    }
    // The capture clamps to the screen; the encoder rect follows the result.
    let rect = Rect { x: 0, y: 0, width: w, height: h };

    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    // Buffered writer: frames stream to disk instead of accumulating in memory.
    let mut gif = GifFile::new(std::io::BufWriter::new(file));
    gif.set_canvas_size(w, h);
    gif.maximum_number_color = encode.color_count.clamp(2, 256);
    gif.quantizer_type = if encode.quantizer.eq_ignore_ascii_case("Octree") {
        sf_gif::QuantizerType::Octree
    } else {
        sf_gif::QuantizerType::Neural
    };
    gif.sampling_factor = encode.sampling_factor.clamp(1, 20);
    gif.use_dithering = encode.dithering;
    gif.use_global_palette = encode.use_global_palette;
    if encode.use_global_palette {
        // shortcut: the palette comes from the first frame only, because frames
        // stream to disk and are never held for sampling. Turn the setting off
        // (its default) or use the CLI recorder for a full-run palette.
        gif.build_global_palette(std::slice::from_ref(&first.bgra));
    }

    let mut frames = 0u64;
    let mut next = Some(first);
    let (mut started_at, mut paused_total) = (Instant::now(), Duration::ZERO);
    let mut generation = handle.session().restart_generation();

    loop {
        // A restart wipes the collected frames and re-bases the clock. The file
        // is already open, so the new run continues in it from frame zero.
        if handle.session().restart_generation() != generation {
            generation = handle.session().restart_generation();
            handle.session().reset_progress();
            started_at = Instant::now();
            paused_total = Duration::ZERO;
        }

        let frame_started = Instant::now();
        let mut shot = match next.take() {
            Some(f) => f,
            None => source.capture(opts.x, opts.y, opts.w, opts.h)?,
        };
        if opts.capture_cursor {
            sf_capture::overlay_window::draw_cursor(&mut shot, opts.x, opts.y);
        }
        gif.add_frame(&shot.bgra, rect, delay_ms, false);
        frames += 1;
        handle.session().add_frame(shot.bgra.len() as u64);

        if handle.is_cancelled() {
            break;
        }
        if opts.max_frames.is_some_and(|max| frames >= max) {
            break;
        }
        if opts.max_bytes.is_some_and(|max| handle.session().bytes() >= max) {
            break;
        }

        let spent = frame_started.elapsed();
        if spent < interval {
            // A pause is slept off the clock, so it never reaches the output.
            paused_total += handle.session().wait_while_paused(interval - spent);
        }
        handle
            .session()
            .set_elapsed_ms(started_at.elapsed().saturating_sub(paused_total).as_millis() as u64);
    }
    gif.finish();
    Ok(frames)
}

/// Real-time screen recording to an H.264 MP4 from `opts`' region. Returns the
/// number of frames written.
pub fn record_mp4(
    path: &str,
    opts: &RecordingOptions,
    video_quality: sf_settings::VideoQuality,
    handle: &PauseHandle,
    source: &dyn CaptureSource,
) -> Result<u64, String> {
    let fps = opts.fps.clamp(1, 60);
    let interval = Duration::from_millis((1000 / fps).max(1) as u64);

    let first = source.capture(opts.x, opts.y, opts.w, opts.h)?;
    // H.264 wants even dimensions; cap the long edge (2560) so a 4K desktop
    // does not encode at full size and crawl.
    let (w, h) = sf_media::cap_long_edge(first.width, first.height, sf_media::MAX_LONG_EDGE);
    if w == 0 || h == 0 {
        return Err("seçilen bölge yakalanamadı".to_string());
    }

    let bitrate = sf_media::bits_per_second(video_quality, w, h);
    let want_audio = opts.system_audio || opts.microphone;
    let mut writer = sf_record::Mp4Writer::with_audio(
        path,
        w,
        h,
        fps,
        bitrate,
        want_audio.then_some(48000),
    )?;
    let frame_dur = writer.frame_duration();

    let mut audio = AudioFeed::open(opts, &writer)?;

    let mut frames = 0u64;
    let mut next = Some(first);
    let (mut started_at, mut paused_total) = (Instant::now(), Duration::ZERO);
    let mut generation = handle.session().restart_generation();

    loop {
        if handle.session().restart_generation() != generation {
            generation = handle.session().restart_generation();
            handle.session().reset_progress();
            started_at = Instant::now();
            paused_total = Duration::ZERO;
            frames = 0;
        }

        let frame_started = Instant::now();
        let mut shot = match next.take() {
            Some(f) => f,
            None => source.capture(opts.x, opts.y, opts.w, opts.h)?,
        };
        if opts.capture_cursor {
            sf_capture::overlay_window::draw_cursor(&mut shot, opts.x, opts.y);
        }

        let pixels = if shot.width == w && shot.height == h {
            shot.bgra
        } else {
            sf_edit::image_ops::resize_pixels(&shot.bgra, shot.width, shot.height, w, h)
        };
        writer.write_bgra(&pixels, frames as i64 * frame_dur)?;
        frames += 1;
        handle.session().add_frame(pixels.len() as u64);

        if let Some(feed) = audio.as_mut() {
            feed.pump(&mut writer);
        }

        if handle.is_cancelled() {
            break;
        }

        let spent = frame_started.elapsed();
        if spent < interval {
            paused_total += handle.session().wait_while_paused(interval - spent);
        }
        handle
            .session()
            .set_elapsed_ms(started_at.elapsed().saturating_sub(paused_total).as_millis() as u64);
    }

    writer.finish()?;
    Ok(frames)
}

/// Pulls system loopback and/or microphone audio, mixes it and hands PCM16 to
/// the writer. Port of the `AudioMixer` pump (`Record/AudioMixer.cs`).
struct AudioFeed {
    loopback: Option<sf_record::AudioCapture>,
    microphone: Option<sf_record::AudioCapture>,
    pumps: Vec<sf_media::PcmPump>,
    /// Queued PCM16 stereo bytes per source.
    queues: Vec<Vec<u8>>,
    /// Mixed but not yet written; the encoder wants whole 1024-sample frames.
    pending: Vec<u8>,
    next_ts: i64,
    rate: i32,
}

const QUANTUM_FRAMES: usize = 1024;

impl AudioFeed {
    fn open(opts: &RecordingOptions, writer: &sf_record::Mp4Writer) -> Result<Option<Self>, String> {
        if !writer.has_audio() {
            return Ok(None);
        }
        let rate = writer.audio_sample_rate();

        let mut feed = Self {
            loopback: None,
            microphone: None,
            pumps: Vec::new(),
            queues: Vec::new(),
            pending: Vec::new(),
            next_ts: 0,
            rate,
        };
        if opts.system_audio {
            if let Ok(cap) = sf_record::AudioCapture::loopback() {
                feed.pumps.push(pump_for(&cap, rate));
                feed.queues.push(Vec::new());
                feed.loopback = Some(cap);
            }
        }
        if opts.microphone {
            if let Ok(cap) = sf_record::AudioCapture::microphone() {
                feed.pumps.push(pump_for(&cap, rate));
                feed.queues.push(Vec::new());
                feed.microphone = Some(cap);
            }
        }
        if feed.pumps.is_empty() {
            // The setting was on but no endpoint could be opened; record silent
            // video rather than failing the whole recording.
            return Ok(None);
        }
        Ok(Some(feed))
    }

    fn pump(&mut self, writer: &mut sf_record::Mp4Writer) {
        for i in 0..self.queues.len() {
            let mut pending: Vec<f32> = Vec::new();
            loop {
                let packet = if i == 0 {
                    self.loopback.as_ref().and_then(|c| c.read().ok())
                } else {
                    self.microphone.as_ref().and_then(|c| c.read().ok())
                };
                match packet {
                    Some(p) if !p.is_empty() => pending.extend_from_slice(&p),
                    _ => break,
                }
            }
            if !pending.is_empty() {
                // The pump's PcmFormat says float32, so the samples go back to
                // bytes and its (tested) channel/rate handling does the rest.
                let mut bytes = Vec::with_capacity(pending.len() * 4);
                for s in &pending {
                    bytes.extend_from_slice(&s.to_le_bytes());
                }
                let (stereo, _peak) = self.pumps[i].push(&bytes, bytes.len());
                extend_pcm16(&mut self.queues[i], &stereo);
            }
        }

        // System audio is the clock; the microphone only fills what is there.
        let frames = self.queues.iter().map(|q| q.len() / 4).min().unwrap_or(0);
        if frames == 0 {
            return;
        }
        for f in 0..frames {
            let (mut l, mut r) = (0i32, 0i32);
            for queue in &self.queues {
                l += i16::from_le_bytes([queue[f * 4], queue[f * 4 + 1]]) as i32;
                r += i16::from_le_bytes([queue[f * 4 + 2], queue[f * 4 + 3]]) as i32;
            }
            let l = l.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let r = r.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            self.pending.extend_from_slice(&l.to_le_bytes());
            self.pending.extend_from_slice(&r.to_le_bytes());
        }
        let consumed = frames * 4;
        for queue in &mut self.queues {
            queue.drain(..consumed);
        }

        while self.pending.len() >= QUANTUM_FRAMES * 4 {
            let chunk: Vec<u8> = self.pending.drain(..QUANTUM_FRAMES * 4).collect();
            let _ = writer.write_pcm16(&chunk, self.next_ts);
            self.next_ts += QUANTUM_FRAMES as i64 * 10_000_000 / self.rate.max(1) as i64;
        }
    }
}

fn pump_for(capture: &sf_record::AudioCapture, target: i32) -> sf_media::PcmPump {
    // WASAPI mix format is float32, interleaved, 4 bytes per frame.
    sf_media::PcmPump::new(
        sf_media::PcmFormat {
            sample_rate: capture.sample_rate() as i32,
            channels: capture.channels() as i32,
            bits_per_sample: 32,
            block_align: capture.channels() as i32 * 4,
            is_float: true,
        },
        target,
    )
}

fn extend_pcm16(queue: &mut Vec<u8>, samples: &[f32]) {
    for pair in samples.chunks_exact(2) {
        let l = (pair[0].clamp(-1.0, 1.0) * 32767.0) as i16;
        let r = (pair[1].clamp(-1.0, 1.0) * 32767.0) as i16;
        queue.extend_from_slice(&l.to_le_bytes());
        queue.extend_from_slice(&r.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Synthetic screen: each pixel carries its absolute virtual-screen
    /// coordinate, so any buffer the loop writes can be traced back to the
    /// rectangle it came from. If the loop ever falls back to the whole screen,
    /// the recorded call list (and the GIF canvas) gives it away.
    struct FakeScreen {
        calls: Mutex<Vec<(i32, i32, i32, i32)>>,
    }

    impl FakeScreen {
        fn new() -> Self {
            Self { calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<(i32, i32, i32, i32)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CaptureSource for FakeScreen {
        fn capture(&self, x: i32, y: i32, w: i32, h: i32) -> Result<sf_capture::Captured, String> {
            assert!(w > 0 && h > 0, "loop asked for an empty region: {w}x{h}");
            self.calls.lock().unwrap().push((x, y, w, h));

            let mut bgra = Vec::with_capacity((w * h * 4) as usize);
            for row in 0..h {
                for col in 0..w {
                    let (px, py) = (x + col, y + row);
                    bgra.extend_from_slice(&[
                        (px & 0xFF) as u8,
                        (py & 0xFF) as u8,
                        ((px >> 8) & 0xFF) as u8,
                        ((py >> 8) & 0xFF) as u8,
                    ]);
                }
            }
            Ok(sf_capture::Captured { width: w, height: h, bgra })
        }
    }

    fn options(x: i32, y: i32, w: i32, h: i32, frames: u64) -> RecordingOptions {
        RecordingOptions {
            x,
            y,
            w,
            h,
            fps: 30,
            max_frames: Some(frames),
            max_bytes: None,
            capture_cursor: false,
            system_audio: false,
            microphone: false,
        }
    }

    fn encode() -> GifEncode {
        GifEncode {
            color_count: 16,
            quantizer: "Octree".into(),
            sampling_factor: 5,
            dithering: false,
            use_global_palette: false,
        }
    }

    fn temp_gif(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("sf-record-region-{}-{name}.gif", std::process::id()));
        path
    }

    /// Reads the GIF logical screen descriptor (16-bit little-endian size).
    fn canvas_size(bytes: &[u8]) -> (u16, u16) {
        (
            u16::from_le_bytes([bytes[6], bytes[7]]),
            u16::from_le_bytes([bytes[8], bytes[9]]),
        )
    }

    #[test]
    fn gif_records_only_the_chosen_region() {
        let screen = FakeScreen::new();
        let handle = sf_record::PauseHandle::new();
        let path = temp_gif("crop");
        let path_str = path.to_string_lossy().into_owned();

        // A 64x48 region at (100, 60) — away from the origin, so a whole-screen
        // fallback would be obviously wrong.
        let frames = record_gif(
            &path_str,
            &options(100, 60, 64, 48, 3),
            &encode(),
            &handle,
            &screen,
        )
        .expect("record");

        assert_eq!(frames, 3, "one capture per frame, plus none extra");

        // True today: the first frame is taken before the loop starts.
        let calls = screen.calls();
        assert_eq!(calls.len(), 3, "calls: {calls:?}");
        for call in &calls {
            assert_eq!(*call, (100, 60, 64, 48), "captured outside the region");
        }

        // ...and the encoder canvas is the region, not the desktop.
        let bytes = std::fs::read(&path).expect("read gif");
        assert!(bytes.starts_with(b"GIF89a"), "not a GIF");
        assert_eq!(canvas_size(&bytes), (64, 48), "canvas is not the region size");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn negative_origin_region_survives_untouched() {
        let screen = FakeScreen::new();
        let handle = sf_record::PauseHandle::new();
        let path = temp_gif("monitor-left");
        let path_str = path.to_string_lossy().into_owned();

        record_gif(
            &path_str,
            &options(-320, 0, 300, 200, 1),
            &encode(),
            &handle,
            &screen,
        )
        .expect("record");

        // A monitor left of the primary has negative x; the loop must pass it
        // straight through, not clamp it to 0.
        assert_eq!(screen.calls()[0], (-320, 0, 300, 200));
        let bytes = std::fs::read(&path).expect("read gif");
        assert_eq!(canvas_size(&bytes), (300, 200));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stopping_mid_recording_keeps_the_frames_so_far() {
        let screen = FakeScreen::new();
        let handle = sf_record::PauseHandle::new();
        let path = temp_gif("stop");
        let path_str = path.to_string_lossy().into_owned();

        // No max_frames: the loop only ends because the handle says so.
        let stopper = handle.clone();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(80));
                stopper.cancel();
            });
            let mut opts = options(0, 0, 32, 32, 0);
            opts.max_frames = None;
            let frames = record_gif(&path_str, &opts, &encode(), &handle, &screen).expect("record");
            assert!(frames >= 1, "no frames kept: {frames}");
            assert_eq!(handle.session().frames(), frames);
        });

        // A finished file, not a truncated one: the GIF trailer is written.
        let bytes = std::fs::read(&path).expect("read gif");
        assert_eq!(*bytes.last().unwrap(), 0x3B, "missing GIF trailer");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_paused_recording_costs_the_recording_no_time() {
        let screen = FakeScreen::new();
        let handle = sf_record::PauseHandle::new();
        let path = temp_gif("pause");
        let path_str = path.to_string_lossy().into_owned();

        handle.pause();
        let resumer = handle.clone();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(120));
                resumer.resume();
            });
            let started = Instant::now();
            let frames =
                record_gif(&path_str, &options(0, 0, 32, 32, 4), &encode(), &handle, &screen)
                    .expect("record");
            assert_eq!(frames, 4, "every requested frame is still captured");
            assert!(
                started.elapsed() >= Duration::from_millis(100),
                "the pause was not slept through at all"
            );
        });

        // 4 frames at 30 fps are ~130 ms of recording; the 120 ms pause is gone.
        let recorded = handle.session().elapsed_ms();
        assert!(
            recorded < 200,
            "pause leaked into the recording clock: {recorded} ms"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn restart_bumps_the_generation_the_loop_watches() {
        let handle = sf_record::PauseHandle::new();
        assert_eq!(handle.session().restart_generation(), 0);
        handle.restart();
        assert_eq!(handle.session().restart_generation(), 1);
    }

    #[test]
    fn gif_encoder_settings_are_applied() {
        let screen = FakeScreen::new();
        let handle = sf_record::PauseHandle::new();
        let path = temp_gif("settings");
        let path_str = path.to_string_lossy().into_owned();

        let mut enc = encode();
        enc.color_count = 64;
        enc.quantizer = "Neural".into();
        enc.use_global_palette = true;
        enc.dithering = true;
        record_gif(&path_str, &options(0, 0, 24, 24, 2), &enc, &handle, &screen).expect("record");

        let bytes = std::fs::read(&path).expect("read gif");
        assert!(bytes.starts_with(b"GIF89a"));
        // Global palette flag: bit 7 of the packed field after the canvas size.
        assert_ne!(bytes[10] & 0x80, 0, "global palette flag not set");

        let _ = std::fs::remove_file(&path);
    }
}
