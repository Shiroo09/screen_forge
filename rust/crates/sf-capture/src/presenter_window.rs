//! Live presenter overlay: a transparent, click-through full-screen window on
//! which the user draws while presenting. Port of the window in
//! `ScreenForge/Presenter/PresenterOverlayWindow.cs`.
//!
//! The window is layered (`WS_EX_LAYERED | WS_EX_TRANSPARENT`), so the desktop
//! shows through and mouse events fall to whatever is underneath except while a
//! drawing tool is active. The stroke geometry comes from `sf_draw`/`sf_overlay`;
//! this module owns the window and input loop.

// Presenter model values, inlined here to avoid a capture→overlay→edit→capture
// dependency cycle. Mirrors sf_overlay::presenter.
mod model {
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub enum PresenterTool {
        #[default]
        None,
        Pen,
        Laser,
        Rectangle,
        Ellipse,
        Line,
        Arrow,
    }
    impl PresenterTool {
        pub fn is_drawing(self) -> bool {
            matches!(self, Self::Pen | Self::Rectangle | Self::Ellipse | Self::Line | Self::Arrow)
        }
    }

    #[derive(Clone, Copy, Default)]
    pub struct PresenterState {
        pub tool: PresenterTool,
        pub spotlight: bool,
    }
    impl PresenterState {
        pub fn toggle_tool(&mut self, tool: PresenterTool) {
            self.tool = if self.tool == tool { PresenterTool::None } else { tool };
        }
        pub fn toggle_spotlight(&mut self) {
            self.spotlight = !self.spotlight;
        }
        pub fn cancel(&mut self) {
            self.tool = PresenterTool::None;
            self.spotlight = false;
        }
    }

    pub struct LaserTrail {
        points: Vec<(f32, f32, i64)>,
        fade_ms: i64,
    }
    impl LaserTrail {
        pub fn new(fade_ms: i64) -> Self {
            Self { points: Vec::new(), fade_ms: fade_ms.max(1) }
        }
        pub fn push(&mut self, x: f32, y: f32, now_ms: i64) {
            self.points.push((x, y, now_ms));
        }
        pub fn expire(&mut self, now_ms: i64) {
            let cutoff = now_ms - self.fade_ms;
            self.points.retain(|p| p.2 >= cutoff);
        }
        pub fn visible(&self, now_ms: i64) -> Vec<(f32, f32, f32)> {
            self.points
                .iter()
                .map(|p| {
                    let age = (now_ms - p.2).max(0) as f32;
                    let alpha = (1.0 - age / self.fade_ms as f32).clamp(0.0, 1.0);
                    (p.0, p.1, alpha)
                })
                .collect()
        }
    }
}
use model::{LaserTrail, PresenterState, PresenterTool};

/// Runs the presenter overlay until the user exits (Esc). Returns nothing.
#[cfg(windows)]
pub fn run() {
    imp::run()
}

#[cfg(not(windows))]
pub fn run() {}

/// The shared model the overlay draws from — used by the window loop and testable
/// on its own.
pub struct PresenterModel {
    pub state: PresenterState,
    pub trail: LaserTrail,
    /// Committed freehand strokes, each a list of points.
    pub strokes: Vec<Vec<(f32, f32)>>,
    /// The stroke currently being drawn.
    pub live: Vec<(f32, f32)>,
    pub color: sf_draw::Rgba,
    pub laser_color: sf_draw::Rgba,
}

impl PresenterModel {
    pub fn new() -> Self {
        Self {
            state: PresenterState::default(),
            trail: LaserTrail::new(700),
            strokes: Vec::new(),
            live: Vec::new(),
            color: sf_draw::Rgba::rgb(0xEA6F12),
            laser_color: sf_draw::Rgba::rgb(0xFF3D6E),
        }
    }

    pub fn on_move(&mut self, x: f32, y: f32, now_ms: i64) {
        if self.state.tool == PresenterTool::Laser {
            self.trail.push(x, y, now_ms);
            self.trail.expire(now_ms);
        } else if self.state.tool.is_drawing() && !self.live.is_empty() {
            self.live.push((x, y));
        }
    }

    pub fn on_press(&mut self, x: f32, y: f32) {
        if self.state.tool.is_drawing() {
            self.live = vec![(x, y)];
        }
    }

    pub fn on_release(&mut self) {
        if !self.live.is_empty() {
            self.strokes.push(std::mem::take(&mut self.live));
        }
    }

    /// Paints the current strokes/laser onto a BGRA buffer.
    pub fn paint(&self, canvas: &mut sf_draw::canvas::Canvas, now_ms: i64) {
        if self.state.tool == PresenterTool::Laser {
            let pts = self.trail.visible(now_ms);
            sf_draw::presenter::draw_laser(canvas, &pts, self.laser_color, 6.0);
        }

        let width = self.strokes.iter().map(Vec::len).sum::<usize>() + self.live.len();
        if width > 0 {
            for stroke in self.strokes.iter().chain(std::iter::once(&self.live)) {
                if stroke.len() >= 2 {
                    sf_draw::shapes::draw_stroke_path(
                        canvas,
                        stroke,
                        sf_draw::shapes::Stroke::new(self.color, 4.0),
                    );
                }
            }
        }
    }
}

impl Default for PresenterModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod imp {
    use super::PresenterModel;
    use super::model::PresenterTool;
    use sf_draw::canvas::Canvas;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP,
        HGDIOBJ, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
        GetMessageW, IsWindow, RegisterClassW, SetForegroundWindow, ShowWindow, UpdateLayeredWindow,
        CS_HREDRAW, CS_VREDRAW, MSG, SW_SHOW, ULW_ALPHA, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
        WM_MOUSEMOVE, WM_PAINT, WS_EX_LAYERED, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        WNDCLASSW,
    };

    const VK_ESCAPE: usize = 0x1B;
    const AC_SRC_OVER: u8 = 0;

    struct State {
        model: PresenterModel,
        width: i32,
        height: i32,
    }

    pub fn run() {
        let (vx, vy, vw, vh) = crate::screen::virtual_screen_bounds();
        if vw <= 0 || vh <= 0 {
            return;
        }

        unsafe {
            let Ok(instance) = GetModuleHandleW(None) else { return };
            let class_name = w!("ScreenForgePresenter");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let mut state = State {
                model: PresenterModel::new(),
                width: vw,
                height: vh,
            };

            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT,
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
                let hdc = BeginPaint(hwnd, &mut ps);
                paint_layered(hwnd, s);
                let _ = EndPaint(hwnd, &ps);
                let _ = hdc;
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (lo(lparam), hi(lparam));
                s.model.on_press(x as f32, y as f32);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = (lo(lparam), hi(lparam));
                let now = now_ms();
                s.model.on_move(x as f32, y as f32, now);
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                s.model.on_release();
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                match wparam.0 {
                    VK_ESCAPE => {
                        s.model.state.cancel();
                        let _ = DestroyWindow(hwnd);
                    }
                    // 1 = pen, 2 = laser, 3 = spotlight (ZoomIt-style).
                    0x31 => s.model.state.toggle_tool(PresenterTool::Pen),
                    0x32 => s.model.state.toggle_tool(PresenterTool::Laser),
                    0x33 => s.model.state.toggle_spotlight(),
                    _ => {}
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Paints the model into an ARGB DIB and blits it with per-pixel alpha.
    unsafe fn paint_layered(hwnd: HWND, s: &State) {
        let mut rect = windows::Win32::Foundation::RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);

        let screen_dc = windows::Win32::Graphics::Gdi::GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: s.width,
                biHeight: -s.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib: HBITMAP = match CreateDIBSection(
            Some(mem_dc),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {
            Ok(b) => b,
            Err(_) => {
                let _ = DeleteDC(mem_dc);
                let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
                return;
            }
        };

        let old = SelectObject(mem_dc, HGDIOBJ(dib.0));

        // ARGB buffer, fully transparent to start.
        let len = (s.width * s.height * 4) as usize;
        let buffer = std::slice::from_raw_parts_mut(bits as *mut u8, len);
        buffer.fill(0);

        {
            let mut canvas = Canvas::new(buffer, s.width, s.height);
            // The overlay draws over a transparent surface; draw the strokes with
            // full alpha and let the blend function composite them.
            s.model.paint(&mut canvas, now_ms());
        }

        // Layered blit: AC_SRC_ALPHA reads the per-pixel alpha we just wrote.
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1, // AC_SRC_ALPHA
        };
        let size = windows::Win32::Foundation::SIZE {
            cx: s.width,
            cy: s.height,
        };
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

    fn now_ms() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn lo(l: LPARAM) -> i32 {
        (l.0 as u32 & 0xFFFF) as i16 as i32
    }
    fn hi(l: LPARAM) -> i32 {
        ((l.0 as u32 >> 16) & 0xFFFF) as i16 as i32
    }
}
