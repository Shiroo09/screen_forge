//! Clipboard copy. Port of the clipboard path in `ScreenForge/Editor/ImageExporter.cs`.
//!
//! The C# app placed a `DIB` on the clipboard so other apps paste the screenshot
//! directly. `CF_DIB` is a bottom-up BGR buffer with a `BITMAPINFOHEADER`; the
//! capture is top-down BGRA, so the rows are flipped and the alpha dropped.

#[cfg(windows)]
pub fn copy_bgra_to_clipboard(width: i32, height: i32, bgra: &[u8]) -> Result<(), String> {
    imp::copy_bgra(width, height, bgra)
}

#[cfg(not(windows))]
pub fn copy_bgra_to_clipboard(_width: i32, _height: i32, _bgra: &[u8]) -> Result<(), String> {
    Err("clipboard copy is only supported on Windows".to_string())
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
    use windows::Win32::System::Ole::CF_DIB;

    pub fn copy_bgra(width: i32, height: i32, bgra: &[u8]) -> Result<(), String> {
        if width <= 0 || height <= 0 || bgra.len() < (width * height * 4) as usize {
            return Err("invalid image for clipboard".to_string());
        }

        let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
        let pixel_bytes = (width * height * 4) as usize;
        let total = header_size + pixel_bytes;

        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE, total).map_err(|e| format!("GlobalAlloc: {e}"))?;

            let ptr = GlobalLock(handle);
            if ptr.is_null() {
                let _ = windows::Win32::Foundation::GlobalFree(Some(handle));
                return Err("GlobalLock failed".to_string());
            }

            // Header: bottom-up 32-bit BGR (negative height would be top-down; CF_DIB
            // consumers expect bottom-up, matching what the C# app wrote).
            let header = ptr as *mut BITMAPINFOHEADER;
            (*header) = BITMAPINFOHEADER {
                biSize: header_size as u32,
                biWidth: width,
                biHeight: height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };

            // Pixels: flip rows top→bottom and drop alpha to 0xFF.
            let dst = (ptr as *mut u8).add(header_size);
            for row in 0..height as usize {
                let src_row = (height as usize - 1 - row) * width as usize * 4;
                let dst_row = row * width as usize * 4;
                for x in 0..width as usize {
                    let s = src_row + x * 4;
                    let d = dst_row + x * 4;
                    *dst.add(d) = *bgra.get_unchecked(s); // B
                    *dst.add(d + 1) = *bgra.get_unchecked(s + 1); // G
                    *dst.add(d + 2) = *bgra.get_unchecked(s + 2); // R
                    *dst.add(d + 3) = 0xFF; // opaque
                }
            }

            let _ = GlobalUnlock(handle);

            if OpenClipboard(None).is_err() {
                let _ = windows::Win32::Foundation::GlobalFree(Some(handle));
                return Err("OpenClipboard failed".to_string());
            }

            // RAII: always close the clipboard.
            struct ClipboardGuard;
            impl Drop for ClipboardGuard {
                fn drop(&mut self) {
                    unsafe {
                        let _ = CloseClipboard();
                    }
                }
            }
            let _guard = ClipboardGuard;

            EmptyClipboard().map_err(|e| format!("EmptyClipboard: {e}"))?;
            SetClipboardData(CF_DIB.0 as u32, Some(HANDLE(handle.0)))
                .map_err(|e| format!("SetClipboardData: {e}"))?;

            // Ownership transferred to the clipboard; do not free it here.
            Ok(())
        }
    }

    // Keep the HGLOBAL import referenced on non-debug builds.
    #[allow(dead_code)]
    fn _assert_types(_: HGLOBAL) {}
}
