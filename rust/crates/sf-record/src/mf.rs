//! Media Foundation sink writer, matching `MfH264Writer.cs`.

use windows::core::{HSTRING, GUID};
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSample, IMFSinkWriter, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFCreateSinkWriterFromURL, MFShutdown, MFStartup, MFSTARTUP_NOSOCKET,
    MFMediaType_Audio, MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, MF_MT_AAC_PAYLOAD_TYPE,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_SUBTYPE, MF_VERSION,
};
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::COINIT_MULTITHREADED;

// GUIDs the MF headers don't export under friendly names here.
#[allow(non_upper_case_globals)]
const MFMediaType_Video: GUID = GUID::from_u128(0x73646976_0000_0010_8000_00AA00389B71);
#[allow(non_upper_case_globals)]
const MFVideoFormat_H264: GUID = GUID::from_u128(0x34363248_0000_0010_8000_00AA00389B71);
#[allow(non_upper_case_globals)]
const MFVideoFormat_RGB32: GUID = GUID::from_u128(0x00000016_0000_0010_8000_00AA00389B71);
// NOTE: these are the *media* format GUIDs (taken from `MFAudioFormat_*`);
// `MFVideoFormat_AAC` (…1611) is a different, rejected type.
#[allow(non_upper_case_globals)]
const MFAudioFormat_AAC: GUID = GUID::from_u128(0x00001610_0000_0010_8000_00AA00389B71);
#[allow(non_upper_case_globals)]
const MFAudioFormat_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00AA00389B71);

/// Stereo 16-bit: the input format `AudioMixer.WritePcm16` produces.
const AUDIO_BYTES_PER_FRAME: i32 = 4;
/// LC-AAC profile level. Windows builds the AAC MFTs only with this set.
const AAC_PROFILE_LEVEL_LC2: u32 = 0x29;

// MFVideoInterlace_Progressive = 2.
const INTERLACE_PROGRESSIVE: u32 = 2;

/// A "packed" MF attribute value: two u32s in one u64, high word first.
fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

/// MP4 H.264 writer. Frames are BGRA (top-down); they are flipped to bottom-up
/// inside the sample buffer, matching the C# row copy.
pub struct Mp4Writer {
    writer: IMFSinkWriter,
    stream_index: u32,
    width: i32,
    height: i32,
    frame_duration: i64, // in 100-ns units
    started: bool,
    /// AAC stream index, or -1 when audio was not requested or could not be added.
    audio_stream: i32,
    audio_rate: i32,
}

impl Mp4Writer {
    /// Opens `path` for H.264 output at `width`×`height`, `fps`, `bitrate` bits/s.
    pub fn new(path: &str, width: i32, height: i32, fps: i32, bitrate: i32) -> Result<Self, String> {
        Self::with_audio(path, width, height, fps, bitrate, None)
    }

    /// Same, with an optional AAC stereo track at `audio_sample_rate` (48000
    /// unless a sensible rate is given). A failed audio stream is not fatal: the
    /// video is still written, exactly as the C# `TryAddAudio` falls back.
    pub fn with_audio(
        path: &str,
        width: i32,
        height: i32,
        fps: i32,
        bitrate: i32,
        audio_sample_rate: Option<i32>,
    ) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("invalid frame size".to_string());
        }
        let fps = fps.max(1);
        let frame_duration = 10_000_000i64 / fps as i64;
        let audio_rate = match audio_sample_rate {
            Some(r) if r == 44100 || r == 48000 => r,
            _ => 48000,
        };
        let want_audio = audio_sample_rate.is_some();

        unsafe {
            // COM may already be initialised on this thread; a duplicate call is fine.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|e| format!("MFStartup: {e}"))?;

            let path_w = HSTRING::from(path);
            let writer: IMFSinkWriter = MFCreateSinkWriterFromURL(&path_w, None, None)
                .map_err(|e| format!("MFCreateSinkWriterFromURL ({path}): {e}"))?;

            // Output type: H.264 video.
            let out_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
            out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| e.to_string())?;
            out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|e| e.to_string())?;
            out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate as u32).map_err(|e| e.to_string())?;
            out_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, INTERLACE_PROGRESSIVE)
                .map_err(|e| e.to_string())?;
            out_type
                .SetUINT64(&MF_MT_FRAME_SIZE, pack(width as u32, height as u32))
                .map_err(|e| e.to_string())?;
            out_type
                .SetUINT64(&MF_MT_FRAME_RATE, pack(fps as u32, 1))
                .map_err(|e| e.to_string())?;
            out_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))
                .map_err(|e| e.to_string())?;

            let stream_index = writer.AddStream(&out_type).map_err(|e| format!("AddStream: {e}"))?;

            // Input type: RGB32 in the same size/framerate.
            let in_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
            in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| e.to_string())?;
            in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|e| e.to_string())?;
            in_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, INTERLACE_PROGRESSIVE)
                .map_err(|e| e.to_string())?;
            in_type
                .SetUINT64(&MF_MT_FRAME_SIZE, pack(width as u32, height as u32))
                .map_err(|e| e.to_string())?;
            in_type
                .SetUINT64(&MF_MT_FRAME_RATE, pack(fps as u32, 1))
                .map_err(|e| e.to_string())?;
            in_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))
                .map_err(|e| e.to_string())?;

            writer
                .SetInputMediaType(stream_index, &in_type, None)
                .map_err(|e| format!("SetInputMediaType: {e}"))?;

            let audio_stream = if want_audio {
                match add_audio_stream(&writer, audio_rate) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("sf-record: audio stream rejected: {e}");
                        -1
                    }
                }
            } else {
                -1
            };

            writer.BeginWriting().map_err(|e| format!("BeginWriting: {e}"))?;

            Ok(Self {
                writer,
                stream_index,
                width,
                height,
                frame_duration,
                started: true,
                audio_stream,
                audio_rate,
            })
        }
    }

    /// True when an AAC audio track was accepted by the container.
    pub fn has_audio(&self) -> bool {
        self.audio_stream >= 0
    }

    /// Sample rate of the audio track the input PCM must already be at.
    pub fn audio_sample_rate(&self) -> i32 {
        self.audio_rate
    }

    /// Writes one PCM16 stereo packet (interleaved, 4 bytes per frame) at
    /// `timestamp` (100-ns units). No-op without an audio stream.
    pub fn write_pcm16(&mut self, pcm: &[u8], timestamp: i64) -> Result<(), String> {
        if self.audio_stream < 0 || pcm.len() < AUDIO_BYTES_PER_FRAME as usize {
            return Ok(());
        }
        if !self.started {
            return Ok(());
        }

        let duration = (pcm.len() as i64 * 10_000_000) / (self.audio_rate as i64 * AUDIO_BYTES_PER_FRAME as i64);

        unsafe {
            let buffer = MFCreateMemoryBuffer(pcm.len() as u32).map_err(|e| e.to_string())?;
            let mut data: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(|e| e.to_string())?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), data, pcm.len());
            buffer.Unlock().map_err(|e| e.to_string())?;
            buffer.SetCurrentLength(pcm.len() as u32).map_err(|e| e.to_string())?;

            let sample: IMFSample = MFCreateSample().map_err(|e| e.to_string())?;
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample.SetSampleTime(timestamp).map_err(|e| e.to_string())?;
            sample.SetSampleDuration(duration.max(1)).map_err(|e| e.to_string())?;

            self.writer
                .WriteSample(self.audio_stream as u32, &sample)
                .map_err(|e| format!("WriteSample(audio): {e}"))?;
        }
        Ok(())
    }

    /// Writes one BGRA frame at `timestamp` (100-ns units). Returns false if the
    /// writer was already shut down — the caller should not treat that as an error.
    pub fn write_bgra(&mut self, bgra: &[u8], timestamp: i64) -> Result<bool, String> {
        if !self.started {
            return Ok(false);
        }
        let stride = self.width as usize * 4;
        let byte_count = stride * self.height as usize;
        if bgra.len() < byte_count {
            return Err("frame buffer smaller than frame size".to_string());
        }

        unsafe {
            let buffer = MFCreateMemoryBuffer(byte_count as u32).map_err(|e| e.to_string())?;
            let mut data: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(|e| e.to_string())?;

            // Flip rows top→bottom as we copy (RGB32 expects bottom-up).
            for y in 0..self.height as usize {
                let src = y * stride;
                let dst = (self.height as usize - 1 - y) * stride;
                std::ptr::copy_nonoverlapping(
                    bgra.as_ptr().add(src),
                    data.add(dst),
                    stride,
                );
            }
            buffer.Unlock().map_err(|e| e.to_string())?;
            buffer.SetCurrentLength(byte_count as u32).map_err(|e| e.to_string())?;

            let sample: IMFSample = MFCreateSample().map_err(|e| e.to_string())?;
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample.SetSampleTime(timestamp).map_err(|e| e.to_string())?;
            sample.SetSampleDuration(self.frame_duration).map_err(|e| e.to_string())?;

            self.writer
                .WriteSample(self.stream_index, &sample)
                .map_err(|e| format!("WriteSample: {e}"))?;
        }

        Ok(true)
    }

    /// Finalises the file (writes the moov atom) and releases the writer.
    pub fn finish(&mut self) -> Result<(), String> {
        if !self.started {
            return Ok(());
        }
        self.started = false;
        unsafe {
            // Finalize writes the container index; without it the MP4 is unplayable.
            self.writer.Finalize().map_err(|e| format!("Finalize: {e}"))?;
            let _ = MFShutdown();
        }
        Ok(())
    }

    /// Convenience: the frame duration in 100-ns units.
    pub fn frame_duration(&self) -> i64 {
        self.frame_duration
    }
}

impl Drop for Mp4Writer {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Adds an AAC stereo output stream fed with PCM16 stereo. Mirrors
/// `MfH264Writer.TryAddAudio` (`Record/MfH264Writer.cs:153`); `Err` there means
/// "record without audio", never a hard failure.
unsafe fn add_audio_stream(writer: &IMFSinkWriter, rate: i32) -> Result<i32, String> {
    let aac: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
    aac.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|e| e.to_string())?;
    aac.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(|e| e.to_string())?;
    aac.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(|e| e.to_string())?;
    aac.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate as u32).map_err(|e| e.to_string())?;
    aac.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(|e| e.to_string())?;
    aac.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 16000).map_err(|e| e.to_string())?;
    aac.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0).map_err(|e| e.to_string())?;
    // Without the profile level the AAC MFT is built on some Windows builds and
    // then shuts the sink down on the first sample.
    aac.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, AAC_PROFILE_LEVEL_LC2)
        .map_err(|e| e.to_string())?;

    let stream = writer.AddStream(&aac).map_err(|e| format!("AddStream(audio): {e}"))?;

    let pcm: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
    pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|e| e.to_string())?;
    pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(|e| e.to_string())?;
    pcm.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(|e| e.to_string())?;
    pcm.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate as u32).map_err(|e| e.to_string())?;
    pcm.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(|e| e.to_string())?;
    pcm.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, AUDIO_BYTES_PER_FRAME as u32)
        .map_err(|e| e.to_string())?;
    pcm.SetUINT32(
        &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
        rate as u32 * AUDIO_BYTES_PER_FRAME as u32,
    )
    .map_err(|e| e.to_string())?;

    writer
        .SetInputMediaType(stream, &pcm, None)
        .map_err(|e| format!("SetInputMediaType(audio): {e}"))?;

    Ok(stream as i32)
}
