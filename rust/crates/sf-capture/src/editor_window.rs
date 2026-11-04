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
    /// The user's chosen action bar button, or `None` if they finished with
    /// Enter / right-click. `Cancel` is never returned — it yields `None` from
    /// `edit_region`.
    pub action: Option<crate::overlay_chrome::Action>,
    /// Stroke width the user picked in the option bar, in logical pixels. The
    /// `Shape` model has no width field, so the caller applies this at render.
    pub stroke_width: f32,
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
    use crate::overlay_chrome::{self, Action, Clickable, Hit, ModeChoice};
    use crate::selection::SelectionState;
    use sf_draw::canvas::Canvas;
    use sf_draw::model::{self, Shape, Tool};
    use sf_draw::canvas::Rgba;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FrameRect, GetDC, GetDeviceCaps,
        InvalidateRect, ReleaseDC, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HDC, HGDIOBJ, LOGPIXELSY, PAINTSTRUCT, SRCCOPY,
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
    #[derive(Clone, Copy, PartialEq)]
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
        /// Chrome (mod çubuğu / araç şeridi) için ölçek; 96 DPI'da 1.0.
        scale: f32,
        /// Yakalanan monitörün genişliği (istemci genişliğiyle aynı).
        mon_w: i32,
        /// Son paint'te çizilen tıklanabilir alanlar; isabet testi buradan yapılır.
        clickables: Vec<Clickable>,
        /// Seçili çizgi kalınlığı (mantıksal px).
        stroke_width: u32,
        /// Kullanıcının seçtiği son eylem; `None` ise Enter/iptal ile bitmiştir.
        action: Option<Action>,
    }

    pub fn edit_region() -> Option<EditResult> {
        let cursor = cursor_monitor();
        let shot = match cursor {
            Some((x, y, w, h)) => crate::screen::capture_region(x, y, w, h).ok()?,
            None => crate::screen::capture_virtual_screen().ok()?,
        };
        let (vx, vy, vw, vh) = cursor.unwrap_or_else(crate::screen::virtual_screen_bounds);

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
                scale: 1.0,
                mon_w: vw,
                clickables: Vec::new(),
                stroke_width: sf_draw::LINE_WIDTH.round() as u32,
                action: None,
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

            // Ölçek: DPI'a göre krom boyutu. Pencere DPI-farkında olmadığı için
            // sistem DPI'ı yeterli.
            {
                let hdc = GetDC(Some(hwnd));
                if !hdc.is_invalid() {
                    let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSY);
                    let _ = ReleaseDC(Some(hwnd), hdc);
                    if dpi > 0 {
                        state.scale = (dpi as f32 / 96.0).clamp(1.0, 2.0);
                    }
                }
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
            let action = match state.action {
                Some(crate::overlay_chrome::Action::Cancel) => None,
                other => other,
            };
            Some(EditResult {
                region,
                shapes: state.shapes.clone(),
                action,
                stroke_width: state.stroke_width as f32,
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
                            // Şekiller sanal ekran koordinatında saklanır (kırpma
                            // sırasında bölge başlangıcı çıkarılır); istemci
                            // koordinatına pencere başlangıcını ekle.
                            s.pen_points.push(virt(s, x, y));
                        }
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (lo(lparam), hi(lparam));
                // Önce krom: araç çubuğu tıklaması çizim/seçim başlatmaz.
                if let Some(hit) = hit_test(s, x, y) {
                    apply_hit(hwnd, s, hit);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    return LRESULT(0);
                }
                match s.phase {
                    Phase::Selecting => s.selection.begin(x, y),
                    Phase::Drawing => {
                        s.drag_start = virt(s, x, y);
                        if s.tool == Tool::Pen {
                            s.pen_points = vec![virt(s, x, y)];
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let (x, y) = (lo(lparam), hi(lparam));
                // Krom üzerinde bırakılan tıklama seçim/şekil üretmez.
                if hit_test(s, x, y).is_some() {
                    return LRESULT(0);
                }
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
                        let (vx, vy) = virt(s, x, y);
                        commit_shape(s, vx, vy);
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

    /// İmlecin bulunduğu monitörün (x, y, w, h) sınırları. Overlay yalnız o
    /// monitörü kaplar; çoklu monitörde krom sanal masaüstünün ortasına
    /// düşmesin (C# `GetUiMonitorDip` davranışı).
    fn cursor_monitor() -> Option<(i32, i32, i32, i32)> {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        unsafe {
            let mut pt = POINT::default();
            windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt).ok()?;
            let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(mon, &mut info).as_bool() {
                return None;
            }
            let r = info.rcMonitor;
            Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
        }
    }

    /// İstemci (pencere) koordinatını sanal ekran koordinatına çevirir. Seçim
    /// bölgesiyle aynı uzay: şekiller sanal koordinatta saklanır, çağıran kırpma
    /// sırasında bölge başlangıcını çıkarır.
    fn virt(s: &State, x: i32, y: i32) -> (f32, f32) {
        (x as f32 + s.origin.0 as f32, y as f32 + s.origin.1 as f32)
    }

    /// İstemci koordinatı son paint'te çizilen krom alanlarından birine düşüyor mu.
    fn hit_test(s: &State, x: i32, y: i32) -> Option<Hit> {
        s.clickables.iter().find_map(|c| {
            let r = c.rect;
            (x >= r.left && x < r.right && y >= r.top && y < r.bottom).then_some(c.hit)
        })
    }

    fn apply_hit(hwnd: HWND, s: &mut State, hit: Hit) {
        match hit {
            // Tam ekran modu overlay içinde seçim gerektirmediği için bölgeyi
            // doğrudan tüm ekran yapar ve bitirir.
            Hit::Mode(ModeChoice::FullScreen) => {
                s.region = Some((s.origin.0, s.origin.1, s.shot.width, s.shot.height));
                s.cancelled = false;
                let _ = unsafe { DestroyWindow(hwnd) };
            }
            Hit::Mode(ModeChoice::Region) => {}
            Hit::Tool(t) => s.tool = t,
            Hit::Color(rgb) => s.color = Rgba::rgb(rgb),
            Hit::Width(w) => s.stroke_width = w,
            // Dolgu bu aşamada çizim modeline işlenmiyor (Shape dolgu alanı
            // taşımıyor); çip durum gösterir, çizim davranışı değişmez.
            Hit::Fill(_) => {}
            Hit::Action(a) => {
                s.action = Some(a);
                if a == Action::Cancel {
                    s.cancelled = true;
                } else {
                    if s.region.is_none() {
                        s.region = Some((s.origin.0, s.origin.1, s.shot.width, s.shot.height));
                    }
                    s.cancelled = false;
                }
                let _ = unsafe { DestroyWindow(hwnd) };
            }
        }
    }

    /// Turns the in-progress drag into a stored shape. A click that moved less
    /// than `MIN_DRAG` px is discarded, so a stray click cannot leave an
    /// invisible zero-size shape in the output.
    fn commit_shape(s: &mut State, x: f32, y: f32) {
        let (sx, sy) = s.drag_start;
        const MIN_DRAG: f32 = 3.0;
        if (x - sx).abs() < MIN_DRAG && (y - sy).abs() < MIN_DRAG {
            return;
        }
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

    /// Seçim dışını karartır (C# XAML'deki dört `#7A000000` dikdörtgenin
    /// karşılığı). GDI'da alfa karıştırmak yerine piksel tamponunda çarpma
    /// yapılıyor: daha ucuz ve %48 alfa tam uygulanıyor.
    pub(super) fn dim_outside(buffer: &mut [u8], w: i32, h: i32, keep: Option<(i32, i32, i32, i32)>) {
        const NUM: u32 = 256 - 0x7A; // 0x7A/255 opaklık karartma
        const DEN: u32 = 256;
        let (kx0, ky0, kx1, ky1) = match keep {
            Some((x, y, rw, rh)) => (x, y, x + rw, y + rh),
            None => (0, 0, 0, 0),
        };
        let mut row = 0i32;
        while row < h {
            let inside_y = row >= ky0 && row < ky1;
            let base = (row * w) as usize * 4;
            let mut col = 0i32;
            // Karartma gereken yatay aralıkları yürüyerek bul; satır içi
            // segmentasyon seçim dışındaki tek koşuyu verir.
            while col < w {
                let inside_x = inside_y && col >= kx0 && col < kx1;
                if inside_x {
                    col = kx1.min(w);
                    continue;
                }
                let end = if inside_y && kx0 > col { kx0.min(w) } else { w };
                let span = (base + col as usize * 4)..(base + end as usize * 4);
                for (k, v) in buffer[span].iter_mut().enumerate() {
                    // Dördüncü bayt alfa; BGRA'da korunur.
                    if k % 4 != 3 {
                        *v = ((*v as u32 * NUM) / DEN) as u8;
                    }
                }
                col = end.max(col + 1);
            }
            row += 1;
        }
    }

    unsafe fn paint(hdc: HDC, s: &mut State) {
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
        // Şekiller sanal koordinatta; tampon istemci koordinatında, pencere
        // başlangıcını çıkararak hizala.
        let mut buffer = s.shot.bgra.clone();
        // Seçim dışı karartılır; seçim içi ve henüz çizilmemiş şekiller net kalır.
        // Seçili bölge istemci koordinatında: bölge sanal uzayda, origin çıkar.
        let keep = match (s.phase, s.region) {
            (Phase::Drawing, Some((rx, ry, rw, rh))) => {
                Some((rx - s.origin.0, ry - s.origin.1, rw, rh))
            }
            (Phase::Selecting, _) if s.selection.is_dragging() => {
                let r = s.selection.current_rect();
                Some(r)
            }
            _ => None,
        };
        dim_outside(&mut buffer, w, h, keep);
        {
            let mut canvas = Canvas::new(&mut buffer, w, h);
            for shape in &s.shapes {
                model::render_with(
                    &mut canvas,
                    &shape.translate(-(s.origin.0 as f32), -(s.origin.1 as f32)),
                    s.stroke_width as f32,
                );
            }
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

        // Krom: seçim öncesi mod çubuğu + ipucu, seçim sonrası araç şeridi.
        s.clickables.clear();
        match s.phase {
            Phase::Selecting => {
                s.clickables =
                    overlay_chrome::draw_mode_bar(hdc, s.mon_w / 2, s.scale, ModeChoice::Region);
                let _ = overlay_chrome::draw_hint(
                    hdc,
                    s.mon_w / 2,
                    s.scale,
                    if s.selection.is_dragging() {
                        "Bırakınca düzenlemeye geçilir"
                    } else {
                        "Alan seçmek için sürükleyin · Esc: iptal"
                    },
                );
                // Ölçü rozeti imlecin yanında, seçim sürüklenirken.
                if s.selection.is_dragging() {
                    let (x, y, sw, sh) = s.selection.current_rect();
                    let _ = overlay_chrome::draw_size_badge(
                        hdc,
                        x + sw + 10,
                        y + sh + 10,
                        w,
                        h,
                        s.scale,
                        sw,
                        sh,
                    );
                }
            }
            Phase::Drawing => {
                let region = s.region.unwrap_or((0, 0, w, h));
                let client_region =
                    (region.0 - s.origin.0, region.1 - s.origin.1, region.2, region.3);
                s.clickables = overlay_chrome::draw_tool_strip(
                    hdc,
                    s.mon_w / 2,
                    w,
                    h,
                    client_region,
                    s.scale,
                    s.tool,
                    ((s.color.r as u32) << 16) | ((s.color.g as u32) << 8) | s.color.b as u32,
                );
                // Seçenek çubuğu araç şeridinin altına, eylem çubuğu ekranın
                // altına; ikisi de aynı isabet listesine eklenir.
                let strip =
                    overlay_chrome::tool_strip_rect(s.mon_w / 2, w, h, client_region, s.scale);
                let strip_center = strip.left + (strip.right - strip.left) / 2;
                s.clickables.extend(overlay_chrome::draw_option_bar(
                    hdc,
                    strip_center,
                    strip.bottom,
                    w,
                    h,
                    s.scale,
                    s.stroke_width,
                    false,
                ));
                s.clickables
                    .extend(overlay_chrome::draw_action_bar(hdc, w, h, s.scale));
            }
        }
    }

    fn lo(l: LPARAM) -> i32 {
        (l.0 as u32 & 0xFFFF) as i16 as i32
    }
    fn hi(l: LPARAM) -> i32 {
        ((l.0 as u32 >> 16) & 0xFFFF) as i16 as i32
    }
}

#[cfg(all(test, windows))]
mod dim_tests {
    /// Dims only the pixels outside `keep`, leaving alpha untouched.
    fn dim(buffer: &mut [u8], w: i32, h: i32, keep: Option<(i32, i32, i32, i32)>) {
        super::imp::dim_outside(buffer, w, h, keep);
    }

    fn px(buffer: &[u8], w: i32, x: i32, y: i32) -> (u8, u8, u8, u8) {
        let i = ((y * w + x) * 4) as usize;
        (buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3])
    }

    #[test]
    fn dims_outside_and_spares_inside() {
        let (w, h) = (10i32, 10i32);
        let mut buf = vec![200u8; (w * h * 4) as usize];
        for i in (3..buf.len()).step_by(4) {
            buf[i] = 0xFF; // opaque alpha
        }
        dim(&mut buf, w, h, Some((2, 2, 4, 4)));

        // Inside the kept rect: untouched.
        assert_eq!(px(&buf, w, 3, 3), (200, 200, 200, 0xFF));
        // Outside: scaled by (256 - 0x7A) / 256 = 0.5234...
        let (b, ..) = px(&buf, w, 0, 0);
        assert_eq!(b, (200 * 0x86 / 256) as u8);
        // A pixel just past the rect's far edge is dimmed again.
        assert_eq!(px(&buf, w, 6, 3).0, (200 * 0x86 / 256) as u8);
    }

    #[test]
    fn no_keep_rect_dims_everything() {
        let (w, h) = (4i32, 4i32);
        let mut buf = vec![255u8; (w * h * 4) as usize];
        dim(&mut buf, w, h, None);
        assert_eq!(px(&buf, w, 1, 1).0, (255 * 0x86 / 256) as u8);
    }
}
