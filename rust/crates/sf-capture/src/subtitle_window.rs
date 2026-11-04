//! Live-subtitle overlay window: a borderless, topmost box that shows translated
//! text over the screen. Port of `ScreenForge/Subtitle/SubtitleOverlayWindow.cs`.
//!
//! Positioning and OCR sampling math live in `sf_overlay::subtitle`; this module
//! owns the window and paints the text box with the configured theme colors and
//! opacity. Text rendering uses the same bitmap font as `sf_draw::text`, so the
//! overlay needs no font stack.

/// Rect as `(x, y, width, height)` — inlined to avoid a capture→overlay cycle.
pub type Rect = (i32, i32, i32, i32);

/// Theme colors for the subtitle box, parsed from the settings hex strings.
#[derive(Clone, Debug)]
pub struct SubtitleStyle {
    pub back: [u8; 4],
    pub text: [u8; 4],
    pub border: [u8; 4],
    pub opacity: f32,
    pub border_thickness: f32,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            back: [0x10, 0x12, 0x18, 0xFF],
            text: [0xFF, 0xFF, 0xFF, 0xFF],
            border: [0xFF, 0xFF, 0xFF, 0xFF],
            opacity: 0.82,
            border_thickness: 0.0,
        }
    }
}

impl SubtitleStyle {
    /// Parses `#RRGGBB` or `#AARRGGBB` into RGBA bytes. Bad input falls back.
    pub fn parse_color(value: &str, fallback: [u8; 4]) -> [u8; 4] {
        let hex = value.trim().trim_start_matches('#');
        match hex.len() {
            6 => {
                let n = u32::from_str_radix(hex, 16).unwrap_or(0);
                [
                    ((n >> 16) & 0xFF) as u8,
                    ((n >> 8) & 0xFF) as u8,
                    (n & 0xFF) as u8,
                    0xFF,
                ]
            }
            8 => {
                let n = u32::from_str_radix(hex, 16).unwrap_or(0);
                [
                    ((n >> 16) & 0xFF) as u8,
                    ((n >> 8) & 0xFF) as u8,
                    (n & 0xFF) as u8,
                    ((n >> 24) & 0xFF) as u8,
                ]
            }
            _ => fallback,
        }
    }
}

/// Runs the subtitle overlay loop. `text_source` yields the current subtitle text
/// each frame (empty clears the box). Returns when the user closes it (Esc).
#[cfg(windows)]
pub fn run(box_rect: Rect, style: SubtitleStyle, text: String) {
    imp::run(box_rect, style, text)
}

#[cfg(not(windows))]
pub fn run(_box_rect: Rect, _style: SubtitleStyle, _text: String) {}

/// Paints a subtitle box into a BGRA buffer of `w`×`h`. Exposed for tests and for
/// callers that composite the subtitle into a captured frame instead of a window.
pub fn paint_box(
    canvas: &mut sf_draw::canvas::Canvas,
    box_rect: Rect,
    text: &str,
    style: &SubtitleStyle,
    scale: f32,
) {
    use sf_draw::shapes::Stroke;

    let (x, y, w, h) = box_rect;
    let back = sf_draw::Rgba::new(style.back[0], style.back[1], style.back[2], (style.back[3] as f32 * style.opacity) as u8);

    // Background fill.
    sf_draw::shapes::draw_rect(
        canvas,
        x as f32,
        y as f32,
        w as f32,
        h as f32,
        Stroke::new(back, 0.5).with_fill(back),
    );

    // Border.
    if style.border_thickness > 0.0 {
        let border = sf_draw::Rgba::new(style.border[0], style.border[1], style.border[2], style.border[3]);
        sf_draw::shapes::draw_rect(
            canvas,
            x as f32,
            y as f32,
            w as f32,
            h as f32,
            Stroke::new(border, style.border_thickness),
        );
    }

    // Text, vertically centred in the box.
    if !text.trim().is_empty() {
        let text_color =
            sf_draw::Rgba::new(style.text[0], style.text[1], style.text[2], style.text[3]);
        let (tw, th) = sf_draw::text::measure(text, scale);
        let tx = x as f32 + ((w as f32 - tw) / 2.0).max(6.0);
        let ty = y as f32 + ((h as f32 - th) / 2.0).max(4.0);
        sf_draw::text::draw_text(canvas, tx, ty, text, scale, text_color, None);
    }
}

/// The default box placement for a source region on a screen. Mirrors
/// `sf_overlay::subtitle::default_box` (kept local to avoid a dependency cycle).
pub fn default_box(source: Rect, screen: Rect) -> Rect {
    let margin = 8;
    let max_w = (screen.2 - margin * 2).max(40);
    let max_h = (screen.3 - margin * 2).max(28);
    let w = 420.min(max_w);
    let h = 56.min(max_h);
    let mut x = source.0 + (source.2 - w) / 2;
    let mut y = source.1 + source.3 + 16;
    if y + h > screen.1 + screen.3 - margin {
        y = source.1 - h - 16;
    }
    x = x.clamp(screen.0 + margin, (screen.0 + margin).max(screen.0 + screen.2 - w - margin));
    y = y.clamp(screen.1 + margin, (screen.1 + margin).max(screen.1 + screen.3 - h - margin));
    (x, y, w, h)
}

#[cfg(windows)]
mod imp {
    use super::{paint_box, Rect, SubtitleStyle};
    use sf_draw::canvas::Canvas;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
        HGDIOBJ, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, IsWindow,
        RegisterClassW, SetForegroundWindow, ShowWindow, UpdateLayeredWindow, CS_HREDRAW,
        CS_VREDRAW, MSG, SW_SHOW, ULW_ALPHA, WM_KEYDOWN, WM_PAINT, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WNDCLASSW,
    };

    const VK_ESCAPE: usize = 0x1B;
    const AC_SRC_OVER: u8 = 0;

    struct State {
        rect: Rect,
        style: SubtitleStyle,
        text: String,
    }

    pub fn run(rect: Rect, style: SubtitleStyle, text: String) {
        unsafe {
            let Ok(instance) = GetModuleHandleW(None) else { return };
            let class_name = w!("ScreenForgeSubtitle");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let (vx, vy, _, _) = crate::screen::virtual_screen_bounds();
            let mut state = State { rect, style, text };

            // Position the window at the box rect (virtual-screen coords).
            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
                class_name,
                w!(""),
                WS_POPUP,
                rect.0 + vx,
                rect.1 + vy,
                rect.2.max(1),
                rect.3.max(1),
                None,
                None,
                Some(instance.into()),
                Some(&mut state as *mut _ as *mut _),
            ) else {
                return;
            };

            let sp = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut State;
            if sp.is_null() {
                let _ = DestroyWindow(hwnd);
                return;
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = DispatchMessageW(&msg);
                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }
            let _ = sp;
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        use windows::Win32::UI::WindowsAndMessaging::{GWLP_USERDATA, SetWindowLongPtrW, WM_NCCREATE};

        if msg == WM_NCCREATE {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let sp = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
            as *mut State;
        if sp.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let s = &mut *sp;

        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let _ = BeginPaint(hwnd, &mut ps);
                paint_layered(hwnd, s);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == VK_ESCAPE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn paint_layered(hwnd: HWND, s: &State) {
        let (w, h) = (s.rect.2.max(1), s.rect.3.max(1));
        let screen_dc = windows::Win32::Graphics::Gdi::GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));

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

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(Some(mem_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(mem_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return;
        };

        let old = SelectObject(mem_dc, HGDIOBJ(dib.0));
        let len = (w * h * 4) as usize;
        let buffer = std::slice::from_raw_parts_mut(bits as *mut u8, len);
        buffer.fill(0);

        {
            let mut canvas = Canvas::new(buffer, w, h);
            // Draw the box at the origin (the window is already positioned).
            let local = (0, 0, w, h);
            paint_box(&mut canvas, local, &s.text, &s.style, 2.0);
        }

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1,
        };
        let size = windows::Win32::Foundation::SIZE { cx: w, cy: h };
        let src = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        let _ = UpdateLayeredWindow(
            hwnd,
            Some(screen_dc),
            None,
            Some(&size),
            Some(mem_dc),
            Some(&src),
            windows::Win32::Foundation::COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        if !old.is_invalid() {
            SelectObject(mem_dc, old);
        }
        let _ = DeleteObject(HGDIOBJ(dib.0));
        let _ = DeleteDC(mem_dc);
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
    }
}
