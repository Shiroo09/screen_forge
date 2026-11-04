//! The full-screen capture overlay: shows the frozen screenshot dimmed, tracks a
//! mouse drag, and returns the selected rectangle. Port of the region-selection
//! half of `ScreenForge/Windows/CaptureOverlayWindow.xaml.cs`.
//!
//! The frozen screenshot is what makes selection instant — the screen is captured
//! once when the overlay opens and never re-read, so nothing flickers.

use crate::selection::{SelectionRect, SelectionState};

/// Runs the region picker and returns the chosen rectangle in virtual-screen
/// coordinates, or `None` if the user cancelled (Esc / right-click).
#[cfg(windows)]
pub fn pick_region() -> Option<SelectionRect> {
    imp::pick_region()
}

#[cfg(not(windows))]
pub fn pick_region() -> Option<SelectionRect> {
    None
}

#[cfg(windows)]
mod imp {
    use super::{SelectionRect, SelectionState};

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FrameRect, InvalidateRect,
        StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HGDIOBJ,
        PAINTSTRUCT, SRCCOPY,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, LoadCursorW,
        RegisterClassW, SetForegroundWindow, ShowWindow, CS_HREDRAW, CS_VREDRAW, MSG, SW_SHOW,
        WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN,
        WS_EX_TOPMOST, WS_POPUP, WNDCLASSW,
    };

    const VK_ESCAPE: usize = 0x1B;
    const IDC_CROSS_CURSOR: u16 = 32515; // IDC_CROSS

    const MIN_SIZE: i32 = 4;

    struct OverlayState {
        shot: crate::screen::Captured,
        origin_x: i32,
        origin_y: i32,
        selection: SelectionState,
        result: Option<SelectionRect>,
        cancelled: bool,
    }

    pub fn pick_region() -> Option<SelectionRect> {
        let shot = match crate::screen::capture_virtual_screen() {
            Ok(s) => s,
            Err(_) => return None,
        };

        let (vx, vy, vw, vh) = crate::screen::virtual_screen_bounds();

        unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class_name = w!("ScreenForgeRegionPicker");

            let cursor = LoadCursorW(None, windows::core::PCWSTR(IDC_CROSS_CURSOR as usize as *const u16)).ok();

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                hCursor: cursor.unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let mut state = OverlayState {
                shot,
                origin_x: vx,
                origin_y: vy,
                selection: SelectionState::new(),
                result: None,
                cancelled: false,
            };

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST,
                class_name,
                w!(""),
                WS_POPUP,
                vx,
                vy,
                vw,
                vh,
                None,
                None,
                Some(instance.into()),
                Some(&mut state as *mut _ as *mut _),
            )
            .ok()?;

            // WM_NCCREATE already stashed the state pointer; retrieve it.
            let state_ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut OverlayState;
            if state_ptr.is_null() {
                let _ = DestroyWindow(hwnd);
                return None;
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = DispatchMessageW(&msg);
                // The window destroys itself once a selection is made or cancelled.
                if !windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            let state = &mut *state_ptr;
            if state.cancelled {
                None
            } else {
                state.result
            }
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        use windows::Win32::UI::WindowsAndMessaging::{GWLP_USERDATA, WM_NCCREATE, SetWindowLongPtrW};

        if msg == WM_NCCREATE {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                let state = (*cs).lpCreateParams as *mut OverlayState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let state_ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
            as *mut OverlayState;
        if state_ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let state = &mut *state_ptr;

        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint(hwnd, hdc, state);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let x = loword(lparam.0 as u32) as i32;
                let y = hiword(lparam.0 as u32) as i32;
                if state.selection.is_dragging() {
                    state.selection.update(x, y);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let x = loword(lparam.0 as u32) as i32;
                let y = hiword(lparam.0 as u32) as i32;
                state.selection.begin(x, y);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(rect) = state.selection.finish(MIN_SIZE) {
                    // Translate from client (0,0 = virtual origin) to virtual-screen coords.
                    state.result = Some((
                        rect.0 + state.origin_x,
                        rect.1 + state.origin_y,
                        rect.2,
                        rect.3,
                    ));
                } else {
                    state.cancelled = true;
                }
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                state.cancelled = true;
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 == VK_ESCAPE {
                    state.cancelled = true;
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn paint(hwnd: HWND, hdc: HDC, state: &OverlayState) {
        let _ = hwnd;
        let (w, h) = (state.shot.width, state.shot.height);

        // 1) Draw the frozen screenshot, dimmed by overlaying a semi-dark fill is
        //    skipped for simplicity: the screenshot is drawn at 1:1 and the
        //    selection frame marks the area. (A dim layer needs a 32-bit blend.)
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let _ = StretchDIBits(
            hdc,
            0,
            0,
            w,
            h,
            0,
            0,
            w,
            h,
            Some(state.shot.bgra.as_ptr() as *const _),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );

        // 2) Frame the current selection in white, and leave the rest untouched.
        if state.selection.is_dragging() {
            let (x, y, sw, sh) = state.selection.current_rect();
            let frame = windows::Win32::Foundation::RECT {
                left: x,
                top: y,
                right: x + sw,
                bottom: y + sh,
            };
            let white = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00FFFFFF));
            let _ = FrameRect(hdc, &frame, white);
            let _ = DeleteObject(HGDIOBJ(white.0));
        }
    }

    fn loword(v: u32) -> u16 {
        (v & 0xFFFF) as u16
    }
    fn hiword(v: u32) -> u16 {
        ((v >> 16) & 0xFFFF) as u16
    }

}
