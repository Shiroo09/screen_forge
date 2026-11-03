//! Mirrors `ScreenForge.Tests/AudioConvertTests.cs` and adds the geometry and
//! bitrate contracts from the C# helpers.

use sf_media::audio::{to_stereo, PcmFormat, PcmPump};
use sf_media::{bits_per_second, cap_long_edge, even_size, MAX_LONG_EDGE};
use sf_settings::VideoQuality;

// ─── Geometry ─────────────────────────────────────────────────────────────────

#[test]
fn even_size_rounds_down_and_rejects_degenerate() {
    assert_eq!(even_size(1921, 1081), (1920, 1080));
    assert_eq!(even_size(1920, 1080), (1920, 1080));
    assert_eq!(even_size(1, 100), (0, 0));
    assert_eq!(even_size(100, 1), (0, 0));
}

#[test]
fn cap_long_edge_preserves_aspect_ratio() {
    // Under the cap: unchanged (still made even).
    assert_eq!(cap_long_edge(1920, 1080, MAX_LONG_EDGE), (1920, 1080));

    // Above the cap: longest edge becomes 2560, ratio preserved, both even.
    let (w, h) = cap_long_edge(3840, 2160, MAX_LONG_EDGE);
    assert_eq!(w, 2560);
    assert_eq!(h, 1440);
    assert_eq!(w % 2, 0);
    assert_eq!(h % 2, 0);

    // Degenerate input stays (0, 0).
    assert_eq!(cap_long_edge(1, 1, MAX_LONG_EDGE), (0, 0));
}

// ─── Bitrate ──────────────────────────────────────────────────────────────────

#[test]
fn bitrate_scales_with_pixels_and_clamps() {
    // 1080p is the reference scale, so it equals the baseline exactly.
    assert_eq!(bits_per_second(VideoQuality::Medium, 1920, 1080), 8_000_000);
    assert_eq!(bits_per_second(VideoQuality::Low, 1920, 1080), 2_500_000);
    assert_eq!(bits_per_second(VideoQuality::High, 1920, 1080), 16_000_000);

    // Tiny frame clamps to the 1 Mbit/s floor.
    assert_eq!(bits_per_second(VideoQuality::Low, 320, 240), 1_000_000);

    // Huge frame clamps to the 40 Mbit/s ceiling.
    assert_eq!(bits_per_second(VideoQuality::High, 7680, 4320), 40_000_000);
}

// ─── Audio ────────────────────────────────────────────────────────────────────

fn format_16bit(rate: i32, channels: i32) -> PcmFormat {
    PcmFormat {
        sample_rate: rate,
        channels,
        bits_per_sample: 16,
        block_align: channels * 2,
        is_float: false,
    }
}

fn format_float(rate: i32, channels: i32) -> PcmFormat {
    PcmFormat {
        sample_rate: rate,
        channels,
        bits_per_sample: 32,
        block_align: channels * 4,
        is_float: true,
    }
}

#[test]
fn same_rate_keeps_duration() {
    let format = format_16bit(48000, 2);
    let frames = 480; // 10 ms
    let pcm = vec![0u8; (frames * format.block_align) as usize];

    let (samples, _) = to_stereo(&pcm, pcm.len(), format);

    assert_eq!(samples.len(), frames as usize * 2);
}

#[test]
fn native_44100_is_not_resampled() {
    let format = format_float(44100, 2);
    let frames = 441;
    let pcm = vec![0u8; (frames * format.block_align) as usize];

    let mut pump = PcmPump::new(format, 44100);
    let (out, _) = pump.push(&pcm, pcm.len());
    let dst = out.len() / 2;

    assert_eq!(dst, frames as usize);
}

#[test]
fn resample_carries_phase_across_packets() {
    let format = format_float(44100, 2);
    let mut pump = PcmPump::new(format, 48000);
    let frames = 100;
    let pcm = vec![0u8; (frames * format.block_align) as usize];

    let mut total = 0;
    for _ in 0..20 {
        let (out, _) = pump.push(&pcm, pcm.len());
        total += out.len() / 2;
    }

    // 2000 * 48000 / 44100 = 2176.8. A per-packet resetting resampler stalls at 2160.
    assert!((2168..=2180).contains(&total), "got {total} frames");
}

#[test]
fn mono_is_duplicated_to_both_channels() {
    let format = format_16bit(48000, 1);
    // One frame, 16-bit sample = 16384 → +0.5
    let pcm = 16384i16.to_le_bytes();
    let (samples, peak) = to_stereo(&pcm, pcm.len(), format);

    assert_eq!(samples.len(), 2);
    assert!((samples[0] - 0.5).abs() < 1e-4);
    assert_eq!(samples[0], samples[1]);
    assert!((peak - 0.5).abs() < 1e-4);
}

#[test]
fn empty_input_returns_nothing() {
    let format = format_16bit(48000, 2);
    let (samples, peak) = to_stereo(&[], 0, format);
    assert!(samples.is_empty());
    assert_eq!(peak, 0.0);
}
