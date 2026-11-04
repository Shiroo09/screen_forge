//! Confirms WASAPI endpoints open and deliver a mix format. Capture content is
//! silent on an idle machine, so the test asserts on format and liveness rather
//! than on non-zero samples.

#[test]
#[cfg(windows)]
fn loopback_endpoint_opens_with_a_usable_format() {
    use windows::Win32::System::Com::{
        CoInitializeEx, COINIT_MULTITHREADED,
    };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let capture = match sf_record::AudioCapture::loopback() {
        Ok(c) => c,
        // A headless/VM session may have no render endpoint; skip rather than fail.
        Err(e) => {
            eprintln!("no loopback endpoint available: {e}");
            return;
        }
    };

    assert!(capture.channels() >= 1, "channels {}", capture.channels());
    assert!(capture.sample_rate() >= 8000, "rate {}", capture.sample_rate());

    // Draining must not panic and returns interleaved f32 samples.
    let packet = capture.read().expect("read packet");
    assert_eq!(packet.len() % capture.channels() as usize, 0, "sample count must be whole frames");
}

#[test]
#[cfg(windows)]
fn microphone_endpoint_opens_or_reports_absent() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    match sf_record::AudioCapture::microphone() {
        Ok(capture) => {
            assert!(capture.channels() >= 1);
            assert!(capture.sample_rate() >= 8000);
        }
        Err(e) => eprintln!("no microphone endpoint: {e}"),
    }
}
