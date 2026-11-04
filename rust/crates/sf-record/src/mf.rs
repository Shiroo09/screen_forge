//! Media Foundation sink writer, matching `MfH264Writer.cs`.

use windows::core::{HSTRING, GUID};
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSample, IMFSinkWriter, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFCreateSinkWriterFromURL, MFShutdown, MFStartup, MFSTARTUP_NOSOCKET,
    MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE,
    MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_VERSION,
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
}

impl Mp4Writer {
    /// Opens `path` for H.264 output at `width`×`height`, `fps`, `bitrate` bits/s.
    pub fn new(path: &str, width: i32, height: i32, fps: i32, bitrate: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("invalid frame size".to_string());
        }
        let fps = fps.max(1);
        let frame_duration = 10_000_000i64 / fps as i64;

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

            writer.BeginWriting().map_err(|e| format!("BeginWriting: {e}"))?;

            Ok(Self {
                writer,
                stream_index,
                width,
                height,
                frame_duration,
                started: true,
            })
        }
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
