//! Real screen capture via GDI BitBlt. Port of the GDI path in
//! `ScreenForge/Capture/ScreenCapture.cs`.
//!
//! Produces BGRA, the pixel order the whole Rust pipeline (frame store, editor,
//! GIF encoder) already expects. DXGI desktop duplication is a faster path the C#
//! app added later; BitBlt is the portable baseline and the fallback.

/// Capture rectangle as `(x, y, width, height)`.
pub type CaptureRect = (i32, i32, i32, i32);

/// A captured frame: BGRA pixels plus its size.
pub struct Captured {
    pub width: i32,
    pub height: i32,
    pub bgra: Vec<u8>,
}

impl Captured {
    /// Copies a sub-rectangle out into a new BGRA buffer, clamped to bounds.
    pub fn crop(&self, rect: CaptureRect) -> Captured {
        let (x, y) = (rect.0.clamp(0, (self.width - 1).max(0)), rect.1.clamp(0, (self.height - 1).max(0)));
        let w = rect.2.clamp(0, self.width - x);
        let h = rect.3.clamp(0, self.height - y);

        if w <= 0 || h <= 0 {
            return Captured {
                width: 0,
                height: 0,
                bgra: Vec::new(),
            };
        }

        let row_bytes = w as usize * 4;
        let mut out = vec![0u8; row_bytes * h as usize];
        for row in 0..h as usize {
            let src = ((y as usize + row) * self.width as usize + x as usize) * 4;
            let dst = row * row_bytes;
            out[dst..dst + row_bytes].copy_from_slice(&self.bgra[src..src + row_bytes]);
        }

        Captured {
            width: w,
            height: h,
            bgra: out,
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::Captured;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        HBITMAP, HDC, HGDIOBJ, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    /// The virtual screen rectangle across all monitors: (x, y, width, height).
    pub fn virtual_screen_bounds() -> (i32, i32, i32, i32) {
        unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        }
    }

    /// Captures the whole virtual desktop as BGRA.
    pub fn capture_virtual_screen() -> Result<Captured, String> {
        let (vx, vy, vw, vh) = virtual_screen_bounds();
        if vw <= 0 || vh <= 0 {
            return Err("virtual screen has no size".to_string());
        }
        capture_region(vx, vy, vw, vh)
    }

    /// RAII wrapper that releases a screen DC however the function exits.
    struct ScreenDc(HDC);
    impl Drop for ScreenDc {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseDC(None, self.0);
            }
        }
    }

    /// Captures the screen region at `(x, y, w, h)` in virtual-screen coordinates.
    pub fn capture_region(x: i32, y: i32, w: i32, h: i32) -> Result<Captured, String> {
        if w <= 0 || h <= 0 {
            return Err("invalid capture size".to_string());
        }

        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return Err("GetDC failed".to_string());
            }
            let _screen_guard = ScreenDc(screen_dc);

            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            if mem_dc.is_invalid() {
                return Err("CreateCompatibleDC failed".to_string());
            }

            let bitmap: HBITMAP = CreateCompatibleBitmap(screen_dc, w, h);
            if bitmap.is_invalid() {
                let _ = DeleteDC(mem_dc);
                return Err("CreateCompatibleBitmap failed".to_string());
            }

            let old = SelectObject(mem_dc, HGDIOBJ(bitmap.0));

            let mut result: Result<Captured, String> = Err("BitBlt failed".to_string());

            if BitBlt(mem_dc, 0, 0, w, h, Some(screen_dc), x, y, SRCCOPY | CAPTUREBLT).is_ok() {
                let mut info = BITMAPINFO::default();
                info.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    // Negative height requests a top-down (row 0 = top) buffer.
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                };

                let mut buffer = vec![0u8; (w as usize) * (h as usize) * 4];
                let scanned = GetDIBits(
                    mem_dc,
                    bitmap,
                    0,
                    h as u32,
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut info,
                    DIB_RGB_COLORS,
                );

                if scanned != 0 {
                    result = Ok(Captured {
                        width: w,
                        height: h,
                        bgra: buffer,
                    });
                } else {
                    result = Err("GetDIBits failed".to_string());
                }
            }

            // Cleanup in reverse order.
            if !old.is_invalid() {
                SelectObject(mem_dc, old);
            }
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);

            result
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Captured;

    pub fn virtual_screen_bounds() -> (i32, i32, i32, i32) {
        (0, 0, 0, 0)
    }

    pub fn capture_virtual_screen() -> Result<Captured, String> {
        Err("screen capture is only supported on Windows".to_string())
    }

    pub fn capture_region(_x: i32, _y: i32, _w: i32, _h: i32) -> Result<Captured, String> {
        Err("screen capture is only supported on Windows".to_string())
    }
}

pub use imp::{capture_region, capture_virtual_screen, virtual_screen_bounds};
