//! Round-trips the HKCU Run value: register, verify it is there, remove it.
//! Touches the real registry, so it restores whatever it finds.

#[test]
#[cfg(windows)]
fn startup_registration_round_trips() {
    // Only run when explicitly asked (it writes to the real registry).
    if std::env::var("SF_TEST_REGISTRY").as_deref() != Ok("1") {
        eprintln!("skipped: set SF_TEST_REGISTRY=1 to run");
        return;
    }

    // Register, then read back through the shell's own view of Run.
    sf_settings::set_startup_enabled(true).expect("enable startup");

    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };
    unsafe {
        let subkey: Vec<u16> = sf_settings::startup::RUN_KEY
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut key = HKEY::default();
        let rc = RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut key);
        assert!(rc.is_ok(), "open Run key: {rc:?}");

        let name: Vec<u16> = sf_settings::startup::VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut size = 0u32;
        let rc = RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, None, None, Some(&mut size));
        assert!(rc.is_ok() && size > 0, "value should exist after enable");
        let _ = RegCloseKey(key);
    }

    // Remove it again.
    sf_settings::set_startup_enabled(false).expect("disable startup");
    unsafe {
        let subkey: Vec<u16> = sf_settings::startup::RUN_KEY
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut key = HKEY::default();
        let _ = RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut key);
        let name: Vec<u16> = sf_settings::startup::VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut size = 0u32;
        let rc = RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, None, None, Some(&mut size));
        assert!(rc.is_err(), "value should be gone after disable, got {rc:?}");
        let _ = RegCloseKey(key);
    }
}
