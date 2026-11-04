//! Interactive capture + annotation overlay.
//!
//! Phase 1: drag a region. Phase 2: draw on it with the current tool. Tab cycles
//! tools, Esc cancels, Enter/right-click finishes. Returns the selected rectangle
//! and the shapes (both in virtual-screen coordinates) so the caller can crop and
//! render. Port of the flow in `ScreenForge/Windows/CaptureOverlayWindow.xaml.cs`,
//! reusing the pure selection state and the `sf_draw` rasterizer.

use crate::selection::SelectionRect;
use sf_draw::model::Shape;

/// The result of an editing session.
pub struct EditResult {
    pub region: SelectionRect,
    pub shapes: Vec<Shape>,
}

/// Opens the interactive overlay. Returns `None` if the user cancels.
#[cfg(windows)]
pub fn edit_region() -> Option<EditResult> {
    imp::edit_region()
}

#[cfg(not(windows))]
pub fn edit_region() -> Option<EditResult> {
    None
}

#[cfg(windows)]
mod imp {
    use super::{EditResult, SelectionRect};
    use crate::selection::SelectionState;
    use sf_draw::canvas::Canvas;
    use sf_draw::model::{self, Shape, Tool};
    use sf_draw::canvas::Rgba;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FrameRect, InvalidateRect,
        StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HGDIOBJ,
        PAINTSTRUCT, SRCCOPY,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, IsWindow,
        LoadCursorW, RegisterClassW, SetForegroundWindow, ShowWindow, CS_HREDRAW, CS_VREDRAW, MSG,
        SW_SHOW, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN,
        WS_EX_TOPMOST, WS_POPUP, WNDCLASSW,
    };

    const VK_ESCAPE: usize = 0x1B;
    const VK_TAB: usize = 0x09;
    const VK_RETURN: usize = 0x0D;
    const MIN_SIZE: i32 = 4;
    const IDC_CROSS: u16 = 32515;

    /// Phase of the interaction.
    #[derive(PartialEq)]
    enum Phase {
        Selecting,
        Drawing,
    }

    struct State {
        shot: crate::screen::Captured,
        origin: (i32, i32),
        selection: SelectionState,
        region: Option<SelectionRect>,
        phase: Phase,
        tool: Tool,
        color: Rgba,
        shapes: Vec<Shape>,
        drag_start: (f32, f32),
        pen_points: Vec<(f32, f32)>,
        step_count: u32,
        cancelled: bool,
    }

    pub fn edit_region() -> Option<EditResult> {
        let shot = crate::screen::capture_virtual_screen().ok()?;
        let (vx, vy, vw, vh) = crate::screen::virtual_screen_bounds();

        unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class_name = w!("ScreenForgeEditorOverlay");
            let cursor =
                LoadCursorW(None, windows::core::PCWSTR(IDC_CROSS as usize as *const u16)).ok();

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                hCursor: cursor.unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let mut state = State {
                shot,
                origin: (vx, vy),
                selection: SelectionState::new(),
                region: None,
                phase: Phase::Selecting,
                tool: Tool::Rect,
                color: Rgba::rgb(0xEA6F12),
                shapes: Vec::new(),
                drag_start: (0.0, 0.0),
                pen_points: Vec::new(),
                step_count: 0,
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

            let state_ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut State;
            if state_ptr.is_null() {
                let _ = DestroyWindow(hwnd);
                return None;
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

            let state = &mut *state_ptr;
            if state.cancelled {
                return None;
            }
            let region = state.region?;
            Some(EditResult {
                region,
                shapes: state.shapes.clone(),
            })
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
                let hdc = BeginPaint(hwnd, &mut ps);
                paint(hdc, s);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = (lo(lparam), hi(lparam));
                match s.phase {
                    Phase::Selecting => s.selection.update(x, y),
                    Phase::Drawing => {
                        if s.tool == Tool::Pen && !s.pen_points.is_empty() {
                            s.pen_points.push((x as f32, y as f32));
                        }
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (lo(lparam), hi(lparam));
                match s.phase {
                    Phase::Selecting => s.selection.begin(x, y),
                    Phase::Drawing => {
                        s.drag_start = (x as f32, y as f32);
                        if s.tool == Tool::Pen {
                            s.pen_points = vec![(x as f32, y as f32)];
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let (x, y) = (lo(lparam), hi(lparam));
                match s.phase {
                    Phase::Selecting => {
                        if let Some(rect) = s.selection.finish(MIN_SIZE) {
                            s.region = Some((
                                rect.0 + s.origin.0,
                                rect.1 + s.origin.1,
                                rect.2,
                                rect.3,
                            ));
                            // Switch to drawing inside the selection.
                            s.phase = Phase::Drawing;
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        } else {
                            s.cancelled = true;
                            let _ = DestroyWindow(hwnd);
                        }
                    }
                    Phase::Drawing => {
                        commit_shape(s, x as f32, y as f32);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                // Right-click finishes once a region exists, otherwise cancels.
                s.cancelled = s.phase == Phase::Selecting;
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                match wparam.0 {
                    VK_ESCAPE => {
                        s.cancelled = true;
                        let _ = DestroyWindow(hwnd);
                    }
                    VK_TAB => {
                        s.tool = s.tool.next();
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                    // Enter finishes the edit once drawing has begun.
                    VK_RETURN if s.phase == Phase::Drawing => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Turns the in-progress drag into a stored shape.
    fn commit_shape(s: &mut State, x: f32, y: f32) {
        let (sx, sy) = s.drag_start;
        match s.tool {
            Tool::Rect => {
                let (rx, ry, rw, rh) = norm(sx, sy, x, y);
                s.shapes.push(Shape::Rect { x: rx, y: ry, w: rw, h: rh, color: s.color });
            }
            Tool::Ellipse => {
                let (rx, ry, rw, rh) = norm(sx, sy, x, y);
                s.shapes.push(Shape::Ellipse { x: rx, y: ry, w: rw, h: rh, color: s.color });
            }
            Tool::Arrow => s.shapes.push(Shape::Arrow { x0: sx, y0: sy, x1: x, y1: y, color: s.color }),
            Tool::Line => s.shapes.push(Shape::Line { x0: sx, y0: sy, x1: x, y1: y, color: s.color }),
            Tool::Pen => {
                s.shapes.push(Shape::Pen { points: std::mem::take(&mut s.pen_points), color: s.color });
            }
            Tool::Step => {
                s.step_count += 1;
                s.shapes.push(Shape::Step {
                    cx: x,
                    cy: y,
                    radius: 18.0,
                    number: s.step_count,
                    color: s.color,
                });
            }
            Tool::Text => {
                // No text-entry dialog in the overlay yet: place a caption at the
                // click point. The caller can edit the string before export.
                s.shapes.push(Shape::Text {
                    x,
                    y,
                    text: "TEXT".to_string(),
                    scale: 2.0,
                    color: s.color,
                    ribbon: true,
                });
            }
            Tool::Blur => {
                let (rx, ry, rw, rh) = norm(sx, sy, x, y);
                s.shapes.push(Shape::Blur {
                    x: rx,
                    y: ry,
                    w: rw,
                    h: rh,
                    kind: sf_draw::blur::MaskKind::Blur,
                    strength: 8.0,
                });
            }
            Tool::Highlight => {
                s.shapes.push(Shape::Highlight {
                    points: vec![(sx, sy), (x, y)],
                    color: s.color,
                    width: 14.0,
                });
            }
            // Select has no shape of its own (it would move existing items).
            Tool::Select => {}
        }
    }

    fn norm(x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32, f32, f32) {
        (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }

    unsafe fn paint(hdc: HDC, s: &State) {
        let (w, h) = (s.shot.width, s.shot.height);
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

        // Draw the frozen screenshot plus committed shapes: render shapes onto a
        // copy of the capture, then blit that so the drawing appears live.
        let mut buffer = s.shot.bgra.clone();
        {
            let mut canvas = Canvas::new(&mut buffer, w, h);
            model::render_all(&mut canvas, &s.shapes);
        }

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
            Some(buffer.as_ptr() as *const _),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );

        // Selection frame while choosing.
        if s.phase == Phase::Selecting && s.selection.is_dragging() {
            let (x, y, sw, sh) = s.selection.current_rect();
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

    fn lo(l: LPARAM) -> i32 {
        (l.0 as u32 & 0xFFFF) as i16 as i32
    }
    fn hi(l: LPARAM) -> i32 {
        ((l.0 as u32 >> 16) & 0xFFFF) as i16 as i32
    }
}
