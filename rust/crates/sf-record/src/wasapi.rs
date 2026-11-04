//! WASAPI capture: system loopback (what plays) and microphone.
//! Port of `ScreenForge/Record/SystemLoopback.cs` and the device capture in
//! `AudioDevices.cs` / `AudioMixer.cs`.
//!
//! The C# code did raw COM interop with hand-declared `IAudioClient` vtable
//! entries. `windows-rs` projects the same interfaces, so this calls them
//! directly. Both paths deliver float32 stereo at the device mix rate, matching
//! what `sf_media::audio::PcmPump` expects downstream.

use windows::Win32::Media::Audio::{
    eConsole, eRender, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;

/// An open WASAPI capture stream (loopback or microphone).
pub struct AudioCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    channels: u16,
    sample_rate: u32,
}

impl AudioCapture {
    /// Opens the default render device in loopback mode (captures system audio).
    pub fn loopback() -> Result<Self, String> {
        Self::open(eRender, AUDCLNT_STREAMFLAGS_LOOPBACK, true)
    }

    /// Opens the default capture device (microphone).
    pub fn microphone() -> Result<Self, String> {
        use windows::Win32::Media::Audio::eCapture;
        Self::open(eCapture, 0, false)
    }

    fn open(
        flow: windows::Win32::Media::Audio::EDataFlow,
        extra_flags: u32,
        _loopback: bool,
    ) -> Result<Self, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("MMDeviceEnumerator: {e}"))?;
            let device = enumerator
                .GetDefaultAudioEndpoint(flow, eConsole)
                .map_err(|e| format!("GetDefaultAudioEndpoint: {e}"))?;

            let client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Activate(IAudioClient): {e}"))?;

            // Shared mode with mix-format float; the mix format is what the loopback
            // tap and the mic both deliver, so no conversion is needed here.
            let mix = client.GetMixFormat().map_err(|e| format!("GetMixFormat: {e}"))?;
            let wfx = *mix;
            let channels = wfx.nChannels;
            let sample_rate = wfx.nSamplesPerSec;

            let flags = AUDCLNT_STREAMFLAGS_LOOPBACK_FLAG | extra_flags;
            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    flags,
                    10_000_000, // 1 s buffer, in 100-ns units
                    0,
                    mix,
                    None,
                )
                .map_err(|e| format!("IAudioClient::Initialize: {e}"))?;

            let capture: IAudioCaptureClient = client
                .GetService()
                .map_err(|e| format!("GetService(IAudioCaptureClient): {e}"))?;

            client.Start().map_err(|e| format!("Start: {e}"))?;

            Ok(Self {
                client,
                capture,
                channels,
                sample_rate,
            })
        }
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Pulls the next packet as interleaved f32 samples. Empty when nothing is ready.
    pub fn read(&self) -> Result<Vec<f32>, String> {
        unsafe {
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;

            self.capture
                .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                .map_err(|e| format!("GetBuffer: {e}"))?;

            let result = if flags & AUDCLNT_BUFFERFLAGS_SILENT != 0 || data.is_null() || frames == 0 {
                Vec::new()
            } else {
                let count = frames as usize * self.channels as usize;
                let slice = std::slice::from_raw_parts(data as *const f32, count);
                slice.to_vec()
            };

            self.capture.ReleaseBuffer(frames).map_err(|e| format!("ReleaseBuffer: {e}"))?;
            Ok(result)
        }
    }

    pub fn stop(&self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

// Distinct from the shared-mode flag value; loopback is its own bit.
const AUDCLNT_STREAMFLAGS_LOOPBACK_FLAG: u32 = 0x0002_0000;

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Whether a WASAPI capture endpoint could be opened on this machine.
#[allow(dead_code)]
pub fn is_available() -> bool {
    AudioCapture::loopback().is_ok()
}
