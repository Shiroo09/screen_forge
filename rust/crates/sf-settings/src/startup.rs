//! Launch-at-startup via the HKCU Run key.
//! Port of `ScreenForge/Settings/StartupManager.cs`.
//!
//! Windows-only side effect; the pure decision (what value to write) is separated
//! so it can be reasoned about without touching the registry.

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "ScreenForge";

/// The command string the Run value should hold for `exe_path`: the quoted path.
pub fn startup_command(exe_path: &str) -> String {
    format!("\"{exe_path}\"")
}

/// Whether `path` lives under `directory` (case-insensitive, path-aware).
pub fn is_under_directory(path: &str, directory: &str) -> bool {
    let dir = directory.trim_end_matches(['\\', '/']).to_lowercase();
    if dir.is_empty() {
        return false;
    }
    let lower = path.to_lowercase();
    lower.starts_with(&format!("{dir}\\")) || lower == dir
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    imp::set_enabled(enabled)
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Err("launch-at-startup is only supported on Windows".to_string())
}

#[cfg(windows)]
mod imp {
    use super::{VALUE_NAME, RUN_KEY};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        unsafe {
            let subkey = wide(RUN_KEY);
            let mut key = HKEY::default();
            let rc = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                KEY_SET_VALUE,
                &mut key,
            );
            if rc != ERROR_SUCCESS {
                return Err(format!("RegOpenKeyExW failed: {rc:?}"));
            }

            let name = wide(VALUE_NAME);
            let result = if enabled {
                let exe = std::env::current_exe()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if exe.is_empty() {
                    Err("cannot resolve current executable".to_string())
                } else {
                    // REG_SZ data is the UTF-16 command including the terminator.
                    let data = wide(&super::startup_command(&exe));
                    let bytes: Vec<u8> = data.iter().flat_map(|u| u.to_le_bytes()).collect();
                    let rc = RegSetValueExW(
                        key,
                        PCWSTR(name.as_ptr()),
                        None,
                        REG_SZ,
                        Some(&bytes),
                    );
                    if rc == ERROR_SUCCESS {
                        Ok(())
                    } else {
                        Err(format!("RegSetValueExW failed: {rc:?}"))
                    }
                }
            } else {
                let rc = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
                // Deleting a missing value is not an error.
                if rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(format!("RegDeleteValueW failed: {rc:?}"))
                }
            };

            let _ = RegCloseKey(key);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_quoted() {
        assert_eq!(startup_command(r"C:\App\app.exe"), r#""C:\App\app.exe""#);
    }

    #[test]
    fn directory_check_is_path_aware() {
        assert!(is_under_directory(r"C:\Users\x\app\app.exe", r"C:\Users\x\app"));
        assert!(!is_under_directory(r"C:\Users\x\app2\app.exe", r"C:\Users\x\app"));
        // Case-insensitive.
        assert!(is_under_directory(r"C:\USERS\X\APP\a.exe", r"c:\users\x\app"));
    }
}
