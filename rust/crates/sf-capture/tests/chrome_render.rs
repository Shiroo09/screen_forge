//! Krom çiziminin piksellerini dökümler: arka plana magenta sentinel boyanır,
//! panel çizilir ve sonuç ASCII haritaya çevrilir. Beyaz sızıntı varsa burada
//! görünür — canlı ekran görüntüsünde monitör yönü tahmin gerektiriyordu.

#![cfg(windows)]

use sf_capture::overlay_chrome::{draw_mode_bar, draw_tool_strip, ModeChoice};
use sf_draw::model::Tool;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};

const W: i32 = 220;
const H: i32 = 90;

/// Renders `draw` onto an offscreen 32-bit DIB with a magenta sentinel
/// background and returns the BGRA pixels.
fn render(draw: impl FnOnce(windows::Win32::Graphics::Gdi::HDC)) -> Vec<u8> {
    unsafe {
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: W,
                biHeight: -H,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        let old = SelectObject(dc, HGDIOBJ(dib.0));

        let px = std::slice::from_raw_parts_mut(bits as *mut u8, (W * H * 4) as usize);
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[255, 0, 255, 255]); // magenta, BGR
        }

        draw(dc);

        let out = px.to_vec();
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(dib.0));
        let _ = DeleteDC(dc);
        out
    }
}

/// One char per 2x2 pixel block: `.` sentinel, `S` panel surface, `W` white-ish,
/// `A` accent, `o` anything else.
fn map(pixels: &[u8]) -> String {
    let mut s = String::new();
    for y in (0..H).step_by(2) {
        for x in (0..W).step_by(2) {
            let i = ((y * W + x) * 4) as usize;
            let (b, g, r) = (pixels[i], pixels[i + 1], pixels[i + 2]);
            let c = if (r, g, b) == (255, 0, 255) {
                '.'
            } else if r > 235 && g > 235 && b > 235 {
                'W'
            } else if r.abs_diff(0x1F) < 14 && g.abs_diff(0x24) < 14 && b.abs_diff(0x30) < 14 {
                'S'
            } else if r > 180 && g > 80 && g < 150 && b < 60 {
                'A'
            } else {
                'o'
            };
            s.push(c);
        }
        s.push('\n');
    }
    s
}

fn count(pixels: &[u8], want: (u8, u8, u8)) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|p| (p[2], p[1], p[0]) == want)
        .count()
}

#[test]
fn mode_bar_paints_no_white_outside_text() {
    let px = render(|dc| {
        draw_mode_bar(dc, W / 2, 1.0, ModeChoice::Region);
    });
    println!("--- mode bar ---\n{}", map(&px));

    let white = count(&px, (255, 255, 255));
    let surface = count(&px, (0x1F, 0x24, 0x30));
    // Antialiased text contributes some near-white pixels, but a filled white
    // box would be thousands; the bar is ~200x40, so cap well below that.
    println!("white={white} surface={surface}");
    assert!(surface > 2000, "panel fill missing (surface={surface})");
    assert!(white < 600, "white box leak: {white} near-white pixels");
}

#[test]
fn tool_strip_paints_panel_and_swatches() {
    let px = render(|dc| {
        draw_tool_strip(dc, W / 2, W, H, (20, 20, 60, 40), 1.0, Tool::Rect, 0xEA6F12);
    });
    println!("--- tool strip ---\n{}", map(&px));
    let surface = count(&px, (0x1F, 0x24, 0x30));
    println!("surface={surface}");
    assert!(surface > 1000, "strip panel fill missing (surface={surface})");
}

#[test]
fn raw_gdi_solid_brush_roundrect() {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::{CreateSolidBrush, RoundRect, SelectObject};
    // Harness sanity: a plain GDI solid brush must reach the DIB, so a failing
    // chrome test can only mean the draw code, not the offscreen surface.
    let px = render(|dc| unsafe {
        let b = CreateSolidBrush(COLORREF(0x30241F));
        let old = SelectObject(dc, b.into());
        let _ = RoundRect(dc, 0, 0, 100, 60, 6, 6);
        SelectObject(dc, old);
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(b.into());
    });
    assert!(count(&px, (0x1F, 0x24, 0x30)) > 1000, "offscreen harness is broken");
}
