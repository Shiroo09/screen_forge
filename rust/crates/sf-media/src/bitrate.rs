//! H.264 target bitrate. Port of `ScreenForge/Record/VideoBitrate.cs`.

use sf_settings::VideoQuality;

/// Baseline bits/s scaled by pixel count relative to 1080p, clamped to
/// 1–40 Mbit/s. C# `Math.Round` is banker's rounding (round-half-to-even).
pub fn bits_per_second(quality: VideoQuality, width: i32, height: i32) -> i32 {
    let baseline = match quality {
        VideoQuality::Low => 2_500_000.0,
        VideoQuality::High => 16_000_000.0,
        VideoQuality::Medium => 8_000_000.0,
    };

    let scale = width as f64 * height as f64 / (1920.0 * 1080.0);
    let bits = round_half_even(baseline * scale);
    bits.clamp(1_000_000, 40_000_000)
}

/// `Math.Round(x)` for a positive value: round half to even.
fn round_half_even(value: f64) -> i32 {
    let floor = value.floor();
    let frac = value - floor;
    let rounded = if (frac - 0.5).abs() < f64::EPSILON {
        // Exactly .5 → nearest even.
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        value.round()
    };
    rounded as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_half_to_even_like_dotnet() {
        assert_eq!(round_half_even(2.5), 2);
        assert_eq!(round_half_even(3.5), 4);
        assert_eq!(round_half_even(8_000_000.0), 8_000_000);
    }
}
