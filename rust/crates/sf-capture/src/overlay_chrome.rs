//! On-screen toolbar chrome for the capture overlay, drawn with raw Win32 GDI.
//!
//! Deliberately GUI-less: the overlay already owns a window and a message loop,
//! so the panels are painted straight onto its HDC and the clickable rectangles
//! come back for the caller's own hit-testing. No widgets, no new crates.

use sf_draw::model::Tool;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, GetStockObject,
    GetTextExtentPoint32W, HGDIOBJ, LineTo, MoveToEx, Polygon, Rectangle, RestoreDC, RoundRect,
    SaveDC, SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_SWISS,
    FW_BOLD, FW_NORMAL, FW_SEMIBOLD, HDC, HOLLOW_BRUSH, OUT_DEFAULT_PRECIS, PS_NULL, PS_SOLID,
    TRANSPARENT,
};

/// The overlay's mode bar chips.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModeChoice {
    Region,
    FullScreen,
}

/// Something the user can click in the overlay chrome.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Mode(ModeChoice),
    Tool(Tool),
    Color(u32),
}

/// One clickable region: where it is (client coordinates) and what it does.
pub struct Clickable {
    pub rect: RECT,
    pub hit: Hit,
}

const SURFACE: u32 = 0x1F2430;
const BORDER: u32 = 0x3A4254;
const TEXT: u32 = 0xF2F4F8;
const TEXT_DIM: u32 = 0x9AA4B8;
const ACCENT: u32 = 0xEA6F12;
const WHITE: u32 = 0xF2F4F8;

/// The eight-colour palette offered by the swatch row, in strip order.
const PALETTE: [u32; 8] = [
    0xEA6F12, 0xE5484D, 0xF5A524, 0x46A758, 0x3B82F6, 0xA855F7, 0xF2F4F8, 0x12151E,
];

/// The tool buttons of the strip, in display order.
const TOOLS: [Tool; 10] = [
    Tool::Select,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Line,
    Tool::Pen,
    Tool::Highlight,
    Tool::Text,
    Tool::Step,
    Tool::Blur,
];

/// Design tokens are authored as `0xRRGGBB`; GDI wants `0x00BBGGRR`.
fn cref(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF))
}

/// One logical pixel in device pixels, never below 1 so hairlines stay visible.
fn px(scale: f32, v: f32) -> i32 {
    ((v * scale).round() as i32).max(1)
}

/// Table-driven values must not round away either; same floor of 1.
fn scaled(scale: f32, v: i32) -> i32 {
    ((v as f32 * scale).round() as i32).max(1)
}

fn rect_at(l: i32, t: i32, w: i32, h: i32) -> RECT {
    RECT { left: l, top: t, right: l + w, bottom: t + h }
}

/// A GDI object that deletes itself, so no draw path can leak one.
struct GdiObj(HGDIOBJ);

impl Drop for GdiObj {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

/// Selects `obj` into `hdc` and restores the previous selection on drop.
struct Sel<'a> {
    hdc: HDC,
    old: HGDIOBJ,
    _obj: &'a GdiObj,
}

impl<'a> Sel<'a> {
    unsafe fn new(hdc: HDC, obj: &'a GdiObj) -> Self {
        let old = SelectObject(hdc, obj.0);
        Self { hdc, old, _obj: obj }
    }
}

impl Drop for Sel<'_> {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc, self.old);
        }
    }
}

/// Segoe UI at a logical size. Negative height means character height, not cell.
unsafe fn segoe(size: f32, weight: i32) -> GdiObj {
    GdiObj(
        CreateFontW(
            -(size.round() as i32),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 as u32) | (FF_SWISS.0 as u32),
            w!("Segoe UI"),
        )
        .into(),
    )
}

/// Width of `text` in the font currently selected into `hdc`.
unsafe fn text_w(hdc: HDC, text: &str) -> i32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    if !GetTextExtentPoint32W(hdc, &wide, &mut size).as_bool() {
        // Measurement failed (font not selected?): a rough advance beats a zero.
        return text.chars().count() as i32 * 7;
    }
    size.cx
}

/// Text automatically centred in `rect`, which spans the full panel height.
unsafe fn text_center(hdc: HDC, rect: RECT, text: &str) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut r = rect;
    DrawTextW(hdc, &mut wide, &mut r, DT_SINGLELINE | DT_CENTER | DT_VCENTER | DT_NOPREFIX);
}

/// A chip or panel: filled rounded rect plus its 1px border.
unsafe fn chrome(hdc: HDC, r: RECT, radius: i32, fill: Option<COLORREF>, border: Option<COLORREF>) {
    let fill_brush = match fill {
        Some(c) => Some(GdiObj(CreateSolidBrush(c).into())),
        // No fill means the screenshot underneath stays visible.
        None => Some(GdiObj(GetStockObject(HOLLOW_BRUSH))),
    };
    let brush = fill_brush.as_ref().map(|b| Sel::new(hdc, b));
    let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    drop(brush);

    if let Some(c) = border {
        // Kenarlık geçişi de fırça seçmek zorunda: seçilmezse varsayılan
        // WHITE_BRUSH ile RoundRect tüm dikdörtgeni beyaza boyar ve yalnızca
        // 1px kenarlık çizilmiş gibi görünür.
        let p = GdiObj(CreatePen(PS_SOLID, 1, c).into());
        let hollow = GdiObj(GetStockObject(HOLLOW_BRUSH));
        let _pen = Sel::new(hdc, &p);
        let _brush = Sel::new(hdc, &hollow);
        let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    }
}

/// Selection phase: draws the top-centre mode bar and returns its clickables.
pub fn draw_mode_bar(hdc: HDC, anchor_x: i32, scale: f32, active: ModeChoice) -> Vec<Clickable> {
    let pad = px(scale, 3.0);
    let chip_h = scaled(scale, 34);
    let gap = scaled(scale, 1);
    let mark = scaled(scale, 26);
    let sep_gap = scaled(scale, 8);
    let after_sep = scaled(scale, 6);
    let glyph = scaled(scale, 18);
    let inner_gap = px(scale, 7.0);
    let font = unsafe { segoe(13.0 * scale, FW_NORMAL.0 as i32) };

    unsafe {
        let saved = SaveDC(hdc);
        SetBkMode(hdc, TRANSPARENT);

        // Chip widths depend on the wordmark, so measure before placing anything.
        let f_word = segoe(14.0 * scale, FW_SEMIBOLD.0 as i32);
        let s = Sel::new(hdc, &f_word);
        let word_w = text_w(hdc, "ScreenForge");
        drop(s);

        let s = Sel::new(hdc, &font);
        let chip_w: Vec<i32> = [ModeChoice::Region, ModeChoice::FullScreen]
            .iter()
            .map(|m| glyph + inner_gap + text_w(hdc, mode_label(*m)) + scaled(scale, 24))
            .collect();
        drop(s);

        let content =
            pad + mark + inner_gap + word_w + sep_gap + chip_w[0] + gap + chip_w[1] + after_sep + pad;
        let panel_rect = rect_at(
            (anchor_x - content / 2).max(0),
            scaled(scale, 18),
            content,
            chip_h + pad * 2,
        );

        // Own SaveDC/RestoreDC per function, independent of the caller's state.
        chrome(hdc, panel_rect, px(scale, 6.0), Some(cref(SURFACE)), Some(cref(BORDER)));

        let row_top = panel_rect.top + pad;

        // Brand mark: ACCENT rounded square, square rather than chip-height.
        let mark_top = row_top + (chip_h - mark) / 2;
        let mark_rect = rect_at(panel_rect.left + pad, mark_top, mark, mark);
        chrome(hdc, mark_rect, px(scale, 6.0), Some(cref(ACCENT)), None);
        {
            let f_mark = segoe(12.0 * scale, FW_BOLD.0 as i32);
            let s = Sel::new(hdc, &f_mark);
            SetTextColor(hdc, cref(WHITE));
            text_center(hdc, mark_rect, "SF");
            drop(s);
        }

        let word_rect = rect_at(mark_rect.right + inner_gap, row_top, word_w, chip_h);
        let s = Sel::new(hdc, &f_word);
        SetTextColor(hdc, cref(TEXT));
        text_center(hdc, word_rect, "ScreenForge");
        drop(s);

        // Separator between the wordmark and the chips.
        let sep_h = scaled(scale, 24);
        let sep_x = word_rect.right + sep_gap;
        let mid = panel_rect.top + panel_rect.bottom;
        let p = GdiObj(CreatePen(PS_SOLID, 1, cref(BORDER)).into());
        let _pen = Sel::new(hdc, &p);
        let _ = MoveToEx(hdc, sep_x, (mid - sep_h) / 2, None);
        let _ = LineTo(hdc, sep_x, (mid + sep_h) / 2);
        drop(_pen);

        let mut out = Vec::with_capacity(2);
        let mut x = sep_x + sep_gap;
        for (mode, w) in [ModeChoice::Region, ModeChoice::FullScreen].iter().zip(&chip_w) {
            let r = rect_at(x, row_top, *w, chip_h);
            let is_active = *mode == active;
            // An inactive chip is just an outline so the screenshot shows through.
            let fill = if is_active { Some(cref(ACCENT)) } else { None };
            let border = if is_active { None } else { Some(cref(BORDER)) };
            chrome(hdc, r, px(scale, 5.0), fill, border);

            let ink = if is_active { cref(WHITE) } else { cref(TEXT) };
            let g = rect_at(r.left + scaled(scale, 12), r.top + (chip_h - glyph) / 2, glyph, glyph);
            mode_glyph(hdc, *mode, g, ink, scale);

            let label_rect = rect_at(g.right + inner_gap, row_top, r.right - g.right - inner_gap, chip_h);
            let s = Sel::new(hdc, &font);
            SetTextColor(hdc, ink);
            text_center(hdc, label_rect, mode_label(*mode));
            drop(s);

            out.push(Clickable { rect: r, hit: Hit::Mode(*mode) });
            x = r.right + gap;
        }

        let _ = RestoreDC(hdc, saved);
        out
    }
}

fn mode_label(mode: ModeChoice) -> &'static str {
    match mode {
        ModeChoice::Region => "Bölgesel",
        ModeChoice::FullScreen => "Tam ekran",
    }
}

/// Selection phase hint box, drawn under the mode bar. Not clickable.
pub fn draw_hint(hdc: HDC, anchor_x: i32, scale: f32, text: &str) -> RECT {
    let pad = px(scale, 3.0);
    let h = scaled(scale, 26);

    let font = unsafe { segoe(13.0 * scale, FW_NORMAL.0 as i32) };
    let saved = unsafe { SaveDC(hdc) };
    let measured = unsafe {
        SetBkMode(hdc, TRANSPARENT);
        let s = Sel::new(hdc, &font);
        let w = text_w(hdc, text);
        drop(s);
        w
    };
    let _ = unsafe { RestoreDC(hdc, saved) };

    // Sits under the mode bar: its inset, its height (chip + padding) and a gap.
    let bar_bottom = scaled(scale, 18) + scaled(scale, 34) + pad * 2;
    let w = (measured + pad * 2 + scaled(scale, 16)).max(scaled(scale, 160));
    let r = rect_at((anchor_x - w / 2).max(0), bar_bottom + px(scale, 8.0), w, h);

    if r.right <= 0 || r.bottom <= 0 {
        return r;
    }

    let saved = unsafe { SaveDC(hdc) };
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        chrome(hdc, r, px(scale, 6.0), Some(cref(SURFACE)), Some(cref(BORDER)));
        let s = Sel::new(hdc, &font);
        SetTextColor(hdc, cref(TEXT_DIM));
        text_center(hdc, r, text);
        drop(s);
        let _ = RestoreDC(hdc, saved);
    }
    r
}

/// Drawing phase: the vertical tool strip plus the colour swatch row.
#[allow(clippy::too_many_arguments)]
pub fn draw_tool_strip(
    hdc: HDC,
    anchor_x: i32,
    client_w: i32,
    client_h: i32,
    region: (i32, i32, i32, i32),
    scale: f32,
    active_tool: Tool,
    active_color: u32,
) -> Vec<Clickable> {
    let (rx, rw) = (region.0, region.2);

    let pad = px(scale, 2.0);
    let grip_w = scaled(scale, 10);
    let btn = scaled(scale, 36);
    let gap = scaled(scale, 1);
    let swatch = scaled(scale, 18);
    let sw_gap = scaled(scale, 4);
    let col_w = swatch * 4 + sw_gap * 3;
    // The 2x4 swatch row is wider than the button column, and both must fit.
    let content_w = btn.max(col_w);
    let panel_w = pad + grip_w + content_w + pad;

    let btn_col_h = btn * 10 + gap * 9;
    let color_block = scaled(scale, 6) + swatch * 2 + sw_gap;
    let panel_h = pad + btn_col_h + color_block + pad;
    let panel_rect = rect_at(
        panel_x(rx, rw, anchor_x, client_w, panel_w, scale),
        (client_h - panel_h) / 2,
        panel_w,
        panel_h,
    );
    let col_x = panel_rect.left + pad + grip_w + (content_w - btn) / 2;

    unsafe {
        let saved = SaveDC(hdc);
        SetBkMode(hdc, TRANSPARENT);
        chrome(hdc, panel_rect, px(scale, 6.0), Some(cref(SURFACE)), Some(cref(BORDER)));

        // Grip column: three dots the user can drag the strip by.
        let dot = scaled(scale, 3);
        let dot_gap = px(scale, 1.5);
        let gx = panel_rect.left + pad + (grip_w - dot) / 2;
        let gy = panel_rect.top + pad + (btn_col_h - (dot * 3 + dot_gap * 2)) / 2;
        let brush = GdiObj(CreateSolidBrush(cref(TEXT_DIM)).into());
        let pen = GdiObj(CreatePen(PS_NULL, 1, cref(TEXT_DIM)).into());
        let _b = Sel::new(hdc, &brush);
        let _p = Sel::new(hdc, &pen);
        for i in 0..3 {
            let t = gy + i * (dot + dot_gap);
            let _ = Ellipse(hdc, gx, t, gx + dot, t + dot);
        }
        drop(_p);
        drop(_b);

        let mut out = Vec::with_capacity(TOOLS.len() + PALETTE.len());
        for (i, tool) in TOOLS.iter().enumerate() {
            let r = rect_at(col_x, panel_rect.top + pad + i as i32 * (btn + gap), btn, btn);
            let is_active = *tool == active_tool;
            let fill = if is_active { Some(cref(ACCENT)) } else { None };
            let border = if is_active { None } else { Some(cref(BORDER)) };
            chrome(hdc, r, px(scale, 5.0), fill, border);
            let ink = if is_active { cref(WHITE) } else { cref(TEXT) };
            glyph(hdc, *tool, inset(r, scaled(scale, 9)), ink, scale);
            out.push(Clickable { rect: r, hit: Hit::Tool(*tool) });
        }

        // Swatch row: 2 x 4 so the strip stays narrow.
        let row_top = panel_rect.top + pad + btn_col_h + scaled(scale, 6);
        let sw_x = panel_rect.left + pad + grip_w + (content_w - col_w) / 2;
        for (i, rgb) in PALETTE.iter().enumerate() {
            let col = (i % 4) as i32;
            let line = (i / 4) as i32;
            let l = sw_x + col * (swatch + sw_gap);
            let t = row_top + line * (swatch + sw_gap);
            let r = rect_at(l, t, swatch, swatch);

            let brush = GdiObj(CreateSolidBrush(cref(*rgb)).into());
            let pen = GdiObj(CreatePen(PS_NULL, 1, cref(*rgb)).into());
            let _b = Sel::new(hdc, &brush);
            let _p = Sel::new(hdc, &pen);
            let _ = Ellipse(hdc, r.left, r.top, r.right, r.bottom);
            drop(_p);
            drop(_b);

            // A ring around the current colour: 2px white, 1px BORDER otherwise.
            let ring = if *rgb == active_color {
                (px(scale, 2.0), cref(WHITE), px(scale, 2.0))
            } else {
                (1, cref(BORDER), 0)
            };
            let inset_by = ring.0 + ring.2;
            let ring_rect = inset(r, inset_by);
            let pen = GdiObj(CreatePen(PS_SOLID, ring.0, ring.1).into());
            let hollow = GdiObj(GetStockObject(HOLLOW_BRUSH));
            let _p = Sel::new(hdc, &pen);
            let _b = Sel::new(hdc, &hollow);
            let _ = Ellipse(hdc, ring_rect.left, ring_rect.top, ring_rect.right, ring_rect.bottom);
            drop(_b);
            drop(_p);

            out.push(Clickable { rect: r, hit: Hit::Color(*rgb) });
        }

        let _ = RestoreDC(hdc, saved);
        out
    }
}

/// Places the strip beside the selection, flipping to the left when the right
/// side has no room, and clamping so it never leaves the client area.
fn panel_x(rx: i32, rw: i32, anchor_x: i32, client_w: i32, panel_w: i32, scale: f32) -> i32 {
    let margin = px(scale, 8.0);
    let right = rx + rw + margin;
    let left = rx - margin - panel_w;
    if right + panel_w <= client_w {
        right
    } else if left >= 0 {
        left
    } else {
        (anchor_x - panel_w / 2).clamp(0, (client_w - panel_w).max(0))
    }
}

/// Shrinks a rect on every side — glyph boxes and rings both need it.
fn inset(r: RECT, by: i32) -> RECT {
    RECT { left: r.left + by, top: r.top + by, right: r.right - by, bottom: r.bottom - by }
}

/// Line-art glyph for a tool, drawn inside `rect`.
pub fn glyph(hdc: HDC, tool: Tool, rect: RECT, color: COLORREF, scale: f32) {
    let w = (1.8 * scale).round().max(1.0) as i32;
    let mut pts: [POINT; 3] = [POINT::default(); 3];

    unsafe {
        let pen = GdiObj(CreatePen(PS_SOLID, w, color).into());
        let hollow = GdiObj(GetStockObject(HOLLOW_BRUSH));
        let _p = Sel::new(hdc, &pen);
        let _b = Sel::new(hdc, &hollow);

        let (l, t, r, b) = (rect.left, rect.top, rect.right, rect.bottom);
        let (cx, cy) = ((l + r) / 2, (t + b) / 2);
        match tool {
            Tool::Select => {
                // Classic pointer outline: tip top-left, notch base bottom-right.
                pts[0] = POINT { x: l, y: t };
                pts[1] = POINT { x: l, y: b };
                pts[2] = POINT { x: cx, y: cy + scaled(scale, 2) };
                let _ = Polygon(hdc, &pts);
                let _ = MoveToEx(hdc, cx, cy + scaled(scale, 2), None);
                let _ = LineTo(hdc, r, b);
            }
            Tool::Rect => {
                let _ = Rectangle(hdc, l, t, r, b);
            }
            Tool::Ellipse => {
                let _ = Ellipse(hdc, l, t, r, b);
            }
            Tool::Arrow => {
                // Shaft plus the two head strokes, all from the top-right tip.
                let head = scaled(scale, 6);
                let _ = MoveToEx(hdc, l, b, None);
                let _ = LineTo(hdc, r, t);
                let _ = MoveToEx(hdc, r, t, None);
                let _ = LineTo(hdc, r - head, t);
                let _ = LineTo(hdc, r, t + head);
            }
            Tool::Line => {
                let _ = MoveToEx(hdc, l, b, None);
                let _ = LineTo(hdc, r, t);
            }
            Tool::Pen => {
                // Slanted nib: body triangle plus the cross-line near the tip.
                pts[0] = POINT { x: l, y: b };
                pts[1] = POINT { x: l, y: b - scaled(scale, 7) };
                pts[2] = POINT { x: r, y: t };
                let _ = Polygon(hdc, &pts);
                let _ = MoveToEx(hdc, l + scaled(scale, 2), b - scaled(scale, 5), None);
                let _ = LineTo(hdc, l + scaled(scale, 5), b - scaled(scale, 2));
            }
            Tool::Highlight => {
                // The bar is thicker than the pen, so a second wide pen is needed.
                let thick = GdiObj(CreatePen(PS_SOLID, scaled(scale, 5), color).into());
                let _t = Sel::new(hdc, &thick);
                let _ = MoveToEx(hdc, l, cy - scaled(scale, 3), None);
                let _ = LineTo(hdc, r, cy - scaled(scale, 3));
                drop(_t);
                let _ = MoveToEx(hdc, l + scaled(scale, 3), b - scaled(scale, 2), None);
                let _ = LineTo(hdc, r - scaled(scale, 3), b - scaled(scale, 2));
            }
            Tool::Text => {
                let _ = MoveToEx(hdc, l, t, None);
                let _ = LineTo(hdc, r, t);
                let _ = MoveToEx(hdc, cx, t, None);
                let _ = LineTo(hdc, cx, b);
            }
            Tool::Step => {
                let _ = Ellipse(hdc, l, t, r, b);
                let f = segoe(12.0 * scale, FW_BOLD.0 as i32);
                let s = Sel::new(hdc, &f);
                SetTextColor(hdc, color);
                text_center(hdc, rect, "1");
                drop(s);
            }
            Tool::Blur => {
                // Three staggered dashes of differing length.
                let d = scaled(scale, 4);
                let rows = [
                    (cy - scaled(scale, 5), l, r - scaled(scale, 2)),
                    (cy, l + scaled(scale, 2), r),
                    (cy + scaled(scale, 5), l, r - d),
                ];
                for (y, x0, x1) in rows {
                    let _ = MoveToEx(hdc, x0, y, None);
                    let _ = LineTo(hdc, x1, y);
                }
            }
        }
    }
}

/// Line-art glyph for a mode chip.
pub fn mode_glyph(hdc: HDC, mode: ModeChoice, rect: RECT, color: COLORREF, scale: f32) {
    let w = (1.8 * scale).round().max(1.0) as i32;
    let (l, t, r, b) = (rect.left, rect.top, rect.right, rect.bottom);
    let (cx, cy) = ((l + r) / 2, (t + b) / 2);

    unsafe {
        let pen = GdiObj(CreatePen(PS_SOLID, w, color).into());
        let _p = Sel::new(hdc, &pen);
        match mode {
            ModeChoice::Region => {
                // Dashed square: eight segments, gaps at the corners.
                let g = scaled(scale, 4);
                for (a, z) in [
                    ((l, t), (cx - g, t)),
                    ((cx + g, t), (r, t)),
                    ((r, t), (r, cy - g)),
                    ((r, cy + g), (r, b)),
                    ((r, b), (cx + g, b)),
                    ((cx - g, b), (l, b)),
                    ((l, b), (l, cy + g)),
                    ((l, cy - g), (l, t)),
                ] {
                    let _ = MoveToEx(hdc, a.0, a.1, None);
                    let _ = LineTo(hdc, z.0, z.1);
                }
            }
            ModeChoice::FullScreen => {
                // Four corner brackets with an empty centre.
                let d = scaled(scale, 5);
                for (c, a, z) in [
                    ((l, t), (l + d, t), (l, t + d)),
                    ((r, t), (r - d, t), (r, t + d)),
                    ((l, b), (l + d, b), (l, b - d)),
                    ((r, b), (r - d, b), (r, b - d)),
                ] {
                    let _ = MoveToEx(hdc, a.0, a.1, None);
                    let _ = LineTo(hdc, c.0, c.1);
                    let _ = LineTo(hdc, z.0, z.1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rounding is the only thing that can put the strip off-screen, so pin it.
    /// `panel_x(rx, rw, anchor_x, client_w, panel_w, scale)`.
    #[test]
    fn panel_placement_prefers_right_then_left_then_clamps() {
        // Room on the right of the selection.
        assert_eq!(panel_x(100, 200, 960, 1920, 70, 1.0), 308);
        // No room on the right, room on the left.
        assert_eq!(panel_x(600, 200, 350, 700, 70, 1.0), 522);
        // Neither side fits: centre on the anchor, clamped inside the client.
        assert_eq!(panel_x(0, 1000, 500, 1000, 70, 1.0), 465);
    }

    /// 2x4 swatches must fit inside the panel, which the button column alone
    /// does not guarantee at fractional DPI scales.
    #[test]
    fn swatch_row_fits_the_panel() {
        for scale in [1.0f32, 1.25, 1.5, 1.75, 2.0] {
            let pad = px(scale, 2.0);
            let swatch = scaled(scale, 18);
            let sw_gap = scaled(scale, 4);
            let col_w = swatch * 4 + sw_gap * 3;
            let content_w = scaled(scale, 36).max(col_w);
            let panel_w = pad + scaled(scale, 10) + content_w + pad;
            let sw_x = pad + scaled(scale, 10) + (content_w - col_w) / 2;
            assert!(sw_x + col_w + pad <= panel_w, "swatches overflow at scale {scale}");
        }
    }
}
