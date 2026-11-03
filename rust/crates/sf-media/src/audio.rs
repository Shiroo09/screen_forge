//! Captured audio → stereo float. Port of `ScreenForge/Record/AudioConvert.cs`.
//!
//! No resampling when the target rate already matches the source. Pulling 44.1 kHz
//! loopback up to 48 kHz per packet used to break phase and make audio stutter, so
//! the resampler carries its fractional position across packets instead.

/// The subset of NAudio's `WaveFormat` the converter reads.
#[derive(Clone, Copy, Debug)]
pub struct PcmFormat {
    pub sample_rate: i32,
    pub channels: i32,
    pub bits_per_sample: i32,
    pub block_align: i32,
    /// True for IEEE float PCM, including the WAVE_FORMAT_EXTENSIBLE subformat.
    pub is_float: bool,
}

/// Converts interleaved PCM to stereo float, returning the samples and peak
/// absolute value seen in this buffer.
pub fn to_stereo(buffer: &[u8], bytes: usize, format: PcmFormat) -> (Vec<f32>, f32) {
    let mut peak = 0.0f32;
    if bytes == 0 || format.sample_rate <= 0 || format.channels <= 0 {
        return (Vec::new(), peak);
    }

    let block = if format.block_align > 0 {
        format.block_align
    } else {
        format.channels * format.bits_per_sample.max(8) / 8
    };
    if block <= 0 {
        return (Vec::new(), peak);
    }

    let src_frames = bytes as i32 / block;
    if src_frames <= 0 {
        return (Vec::new(), peak);
    }

    let channels = format.channels;
    let sample_stride = (block / channels).max(1);
    let mut src = vec![0.0f32; src_frames as usize * 2];

    for i in 0..src_frames as usize {
        let base = i * block as usize;
        let l = sample_at(buffer, base, format);
        let r = if channels > 1 {
            sample_at(buffer, base + sample_stride as usize, format)
        } else {
            l
        };
        src[i * 2] = l;
        src[i * 2 + 1] = r;
        peak = peak.max(l.abs()).max(r.abs());
    }

    (src, peak)
}

fn sample_at(buffer: &[u8], offset: usize, format: PcmFormat) -> f32 {
    if offset >= buffer.len() {
        return 0.0;
    }

    if format.is_float {
        if offset + 4 > buffer.len() {
            return 0.0;
        }
        return f32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());
    }

    if format.bits_per_sample == 16 {
        if offset + 2 > buffer.len() {
            return 0.0;
        }
        return i16::from_le_bytes(buffer[offset..offset + 2].try_into().unwrap()) as f32 / 32768.0;
    }

    if format.bits_per_sample == 32 && offset + 4 <= buffer.len() {
        return i32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as f32
            / 2147483648.0;
    }

    0.0
}

/// Streaming converter: turns arbitrary PCM packets into stereo float at
/// `target_rate`, keeping resampler phase between calls.
pub struct PcmPump {
    format: PcmFormat,
    target_rate: i32,
    passthrough: bool,
    step: f64,
    pending: Vec<f32>,
    phase: f64,
}

impl PcmPump {
    pub fn new(format: PcmFormat, target_rate: i32) -> Self {
        let target = if target_rate > 0 { target_rate } else { 48000 };
        Self {
            format,
            target_rate: target,
            passthrough: format.sample_rate == target,
            step: format.sample_rate as f64 / target.max(1) as f64,
            pending: Vec::new(),
            phase: 0.0,
        }
    }

    pub fn target(&self) -> i32 {
        self.target_rate
    }

    /// Converts one packet. Returns (stereo samples, running peak).
    pub fn push(&mut self, buffer: &[u8], count: usize) -> (Vec<f32>, f32) {
        let (src, mut peak) = to_stereo(buffer, count, self.format);
        if self.passthrough || src.is_empty() {
            return (src, peak);
        }
        let out = self.resample(&src, &mut peak);
        (out, peak)
    }

    fn resample(&mut self, src: &[f32], peak: &mut f32) -> Vec<f32> {
        if self.pending.is_empty() {
            self.pending = src.to_vec();
        } else {
            let mut merged = Vec::with_capacity(self.pending.len() + src.len());
            merged.extend_from_slice(&self.pending);
            merged.extend_from_slice(src);
            self.pending = merged;
        }

        let frames = self.pending.len() / 2;
        let mut acc = Vec::with_capacity(src.len() + 8);
        while self.phase + 1.0 < frames as f64 {
            let i = self.phase as usize;
            let t = (self.phase - i as f64) as f32;
            let a = i * 2;
            let b = a + 2;
            let l = self.pending[a] + (self.pending[b] - self.pending[a]) * t;
            let r = self.pending[a + 1] + (self.pending[b + 1] - self.pending[a + 1]) * t;
            acc.push(l);
            acc.push(r);
            *peak = peak.max(l.abs()).max(r.abs());
            self.phase += self.step;
        }

        let drop = self.phase as usize;
        if drop > 0 {
            let remain = frames as i64 - drop as i64;
            if remain <= 0 {
                self.pending.clear();
                self.phase = 0.0;
            } else {
                self.pending = self.pending[drop * 2..].to_vec();
                self.phase -= drop as f64;
            }
        }

        acc
    }
}
