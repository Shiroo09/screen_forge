//! Tam ekran damlalık (renk seçici). `ScreenForge/Windows/EyedropperOverlay.cs`
//! portu; tepsi menüsündeki "Renk Seçici" öğesini besler.
//!
//! Ekran overlay açılırken **bir kez** yakalanır: büyüteç kare başına yeniden
//! yakalamaz. C# sürümü her fare hareketinde ekrandan tekrar BitBlt ederdi, ama
//! penceresi `WDA_EXCLUDEFROMCAPTURE` ile yakalamadan gizlendiği için kendi
//! çizimini örneklemezdi; donmuş kare aynı sonucu verir ve fare altında ekranı
//! saniyede onlarca kez okumaz.
//!
//! C# ile aynı: 13×13 piksel örnekleme, 8× zoom (104px büyüteç), merkez piksel
//! nişangâhı, canlı hex etiketi, sol tık = seç / Esc = iptal.
//! C#'tan farklı: Win32 GDI'da alfa kanalı yok (C# gölge ve yarı saydam zemin
//! kullanıyordu) ve büyüteç, C#'ın yedek yolundaki gibi imlecin sağ üstüne
//! yerleşir — böylece artı imleç görünür kalır.

/// Büyüteçte örneklenen piksel karesi (tek sayı olmalı).
const GRID: i32 = 13;
/// Büyüteç kutusunun kenarı; `GRID`e tam bölünür (hücre = 8px zoom).
const LOUPE: i32 = 104;
/// Karenin ortası: imlecin altındaki piksel.
const HALF: i32 = GRID / 2;
/// Bir kaynak pikselin büyüteçte kapladığı alan (C#'taki `cell`).
const CELL: i32 = LOUPE / GRID;

/// Opens the eyedropper. Returns the picked colour as 0xRRGGBB, or None if cancelled.
#[cfg(windows)]
pub fn pick_color() -> Option<u32> {
    imp::pick_color()
}

/// Opens the eyedropper. Returns the picked colour as 0xRRGGBB, or None if cancelled.
#[cfg(not(windows))]
pub fn pick_color() -> Option<u32> {
    None
}

/// Seçilen rengin hex metnini (`#RRGGBB`) panoya kopyalar.
pub fn copy_hex_to_clipboard(hex: &str) -> Result<(), String> {
    crate::clipboard::copy_text_to_clipboard(hex)
}

/// `GRID`×`GRID` BGRA tamponunda merkez pikselin bayt ofseti.
fn centre_offset() -> usize {
    ((HALF * GRID + HALF) * 4) as usize
}

/// BGRA baytlarından `0xRRGGBB`.
fn rgb_from_bgra(buf: &[u8], off: usize) -> u32 {
    ((buf[off + 2] as u32) << 16) | ((buf[off + 1] as u32) << 8) | buf[off] as u32
}

/// `GRID`×`GRID` örneği `(cx, cy)` çevresinden `bgra`ya kopyalar. Kenarda kırpma
/// yerine kenar pikseli tekrarlanır: C# BitBlt negatif koordinatı tanımsız
/// bırakırdı, burada görüntü kenarına yapışır.
fn sample_grid(bgra: &[u8], w: i32, h: i32, cx: i32, cy: i32, out: &mut [u8]) {
    if w <= 0 || h <= 0 || out.len() < (GRID * GRID * 4) as usize {
        return;
    }
    for row in 0..GRID {
        for col in 0..GRID {
            let sx = (cx + col - HALF).clamp(0, w - 1);
            let sy = (cy + row - HALF).clamp(0, h - 1);
            let src = ((sy * w + sx) * 4) as usize;
            let dst = ((row * GRID + col) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&bgra[src..src + 4]);
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::{CELL, GRID, HALF, LOUPE};
    use crate::overlay_chrome::{chrome, cref, GdiObj, Sel};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateEllipticRgn, CreateFontW, CreatePen, DeleteObject, DrawTextW, Ellipse,
        EndPaint, GetStockObject, InvalidateRect, Rectangle, RestoreDC, SaveDC, SelectClipRgn,
        SetBkMode, SetStretchBltMode, SetTextColor, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLORONCOLOR, DEFAULT_CHARSET,
        DEFAULT_PITCH, DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE,
        DT_VCENTER, FF_MODERN, FF_SWISS, FONT_FAMILY, FW_NORMAL, FW_SEMIBOLD, HDC, HGDIOBJ,
        HOLLOW_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
        GetWindowLongPtrW, IsWindow, LoadCursorW, RegisterClassW, SetForegroundWindow,
        SetWindowLongPtrW, ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_CROSS, MSG,
        SW_SHOW, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT,
        WNDCLASSW, WS_EX_TOPMOST, WS_POPUP,
    };

    /// Bilgi kutusu (renk örneği + hex + rgb üçlüsü), cihaz pikseli.
    const INFO_W: i32 = 140;
    const INFO_H: i32 = 40;

    struct State {
        /// Overlay açılırken bir kez alınan tam ekran görüntüsü.
        shot: crate::screen::Captured,
        /// Sanal ekranın sol üst köşesi: istemci = sanal − origin.
        origin: (i32, i32),
        /// Büyütecin örneklediği imleç konumu, istemci koordinatı.
        cursor: (i32, i32),
        /// `GRID`×`GRID` BGRA — imlecin çevresindeki pikseller.
        grid: Vec<u8>,
        /// Seçilen renk `0xRRGGBB`; `None` ise iptal edilmiştir.
        picked: Option<u32>,
        cancelled: bool,
    }

    pub fn pick_color() -> Option<u32> {
        let shot = crate::screen::capture_virtual_screen().ok()?;
        let (vx, vy, vw, vh) = crate::screen::virtual_screen_bounds();

        unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class_name = w!("ScreenForgeColorPickerOverlay");
            // C# yedek yolu gibi artı imleç; sistem imleci gizlenmez.
            let cursor = LoadCursorW(None, IDC_CROSS).ok();

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
                cursor: (vw / 2, vh / 2),
                grid: vec![0u8; (GRID * GRID * 4) as usize],
                picked: None,
                cancelled: false,
            };

            // İlk kareyi doğru yere yerleştir: köşede boş büyüteç görünmesin.
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt).is_ok() {
                state.cursor = (pt.x - vx, pt.y - vy);
            }
            sample(&mut state);

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

            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if state_ptr.is_null() {
                let _ = DestroyWindow(hwnd);
                return None;
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            // Esc için klavye odağı şart; odak başka pencereye kaçarsa iptal edilemez.
            let _ = SetFocus(Some(hwnd));

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = DispatchMessageW(&msg);
                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            let state = &mut *state_ptr;
            if state.cancelled {
                None
            } else {
                state.picked
            }
        }
    }

    /// İmlecin çevresindeki kareyi donmuş ekran görüntüsünden yeniden örnekler.
    /// İmleç istemci koordinatında; görüntü sanal ekran koordinatında, origin eklenir.
    fn sample(s: &mut State) {
        let (cx, cy) = s.cursor;
        let (ox, oy) = s.origin;
        super::sample_grid(
            &s.shot.bgra,
            s.shot.width,
            s.shot.height,
            cx + ox,
            cy + oy,
            &mut s.grid,
        );
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_NCCREATE {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let sp = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
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
            // Kendi çizimimiz tüm istemciyi kaplar; arka planı silmek titretir.
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                s.cursor = (lo(lparam), hi(lparam));
                sample(s);
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                // Tıklanan noktadan örnekle: C# da tıklama anında ekranı okur.
                s.cursor = (lo(lparam), hi(lparam));
                sample(s);
                s.picked = Some(super::rgb_from_bgra(&s.grid, super::centre_offset()));
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
                s.cancelled = true;
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Büyütecin sol üst köşesi: imlecin sağ üstünde, ekran kenarında ters tarafa
    /// geçer (C# yedek yolu).
    fn loupe_pos(cursor: (i32, i32), w: i32) -> (i32, i32) {
        const OFF_X: i32 = 20;
        const OFF_Y: i32 = 20;
        let mut x = cursor.0 + OFF_X;
        let mut y = cursor.1 - OFF_Y - LOUPE;
        if x + LOUPE > w - 10 {
            x = cursor.0 - OFF_X - LOUPE;
        }
        if y < 10 {
            y = cursor.1 + OFF_Y;
        }
        (x, y)
    }

    unsafe fn paint(hdc: HDC, s: &State) {
        let saved = SaveDC(hdc);
        let (w, h) = (s.shot.width, s.shot.height);

        // Donmuş ekran 1:1; C# overlay'i de canlı masaüstünü karartmadan gösterirdi.
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
            Some(s.shot.bgra.as_ptr() as *const _),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );

        let rgb = super::rgb_from_bgra(&s.grid, super::centre_offset());
        let (lx, ly) = loupe_pos(s.cursor, w);
        draw_loupe(hdc, s, lx, ly);

        // Bilgi kutusu büyütecin altında; ekranın dibindeyse üstüne alınır.
        let bx = lx.clamp(0, (w - INFO_W).max(0));
        let mut by = ly + LOUPE + 4;
        if by + INFO_H > h {
            by = ly - INFO_H - 2;
        }
        draw_info_box(hdc, bx, by.max(0), rgb);

        let _ = RestoreDC(hdc, saved);
    }

    /// Dairesel büyüteç: 13×13 örnek 8× büyütülür, ortası nişangâhla işaretlenir.
    unsafe fn draw_loupe(hdc: HDC, s: &State, lx: i32, ly: i32) {
        // Dairesel kırpma (C# `EllipseGeometry`); SaveDC/RestoreDC kırpmayı da geri alır.
        let region = CreateEllipticRgn(lx, ly, lx + LOUPE, ly + LOUPE);
        SelectClipRgn(hdc, Some(region));

        // En yakın komşu: pikseller bulanıklaşmasın (C# NearestNeighbor).
        SetStretchBltMode(hdc, COLORONCOLOR);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: GRID,
                biHeight: -GRID,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = StretchDIBits(
            hdc,
            lx,
            ly,
            LOUPE,
            LOUPE,
            0,
            0,
            GRID,
            GRID,
            Some(s.grid.as_ptr() as *const _),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        SelectClipRgn(hdc, None);
        let _ = DeleteObject(HGDIOBJ(region.0));

        // Orta hücre nişangâhı: koyu dış hat + beyaz iç hat (C# iki kare çizer).
        let hollow = GdiObj(GetStockObject(HOLLOW_BRUSH));
        let cx = lx + HALF * CELL;
        let cy = ly + HALF * CELL;
        let dark = GdiObj(CreatePen(PS_SOLID, 1, cref(0x000000)).into());
        {
            let _p = Sel::new(hdc, &dark);
            let _b = Sel::new(hdc, &hollow);
            let _ = Rectangle(hdc, cx - 2, cy - 2, cx + CELL + 2, cy + CELL + 2);
        }
        let white = GdiObj(CreatePen(PS_SOLID, 1, cref(0xFFFFFF)).into());
        {
            let _p = Sel::new(hdc, &white);
            let _b = Sel::new(hdc, &hollow);
            let _ = Rectangle(hdc, cx - 1, cy - 1, cx + CELL + 1, cy + CELL + 1);
        }

        // 2px beyaz halka (C# Border, BorderThickness 2).
        let ring = GdiObj(CreatePen(PS_SOLID, 2, cref(0xF2F4F8)).into());
        {
            let _p = Sel::new(hdc, &ring);
            let _b = Sel::new(hdc, &hollow);
            let _ = Ellipse(hdc, lx, ly, lx + LOUPE, ly + LOUPE);
        }
    }

    /// Renk örneği + `#RRGGBB` + `rgb(r, g, b)` üçlüsü, fareyle canlı güncellenir.
    unsafe fn draw_info_box(hdc: HDC, x: i32, y: i32, rgb: u32) {
        const PAD: i32 = 6;
        const SWATCH: i32 = 20;
        const GAP: i32 = 6;
        let panel = RECT { left: x, top: y, right: x + INFO_W, bottom: y + INFO_H };
        chrome(hdc, panel, 6, Some(cref(0x1E2432)), Some(cref(0x384450)));

        let sw = RECT {
            left: x + PAD,
            top: y + (INFO_H - SWATCH) / 2,
            right: x + PAD + SWATCH,
            bottom: y + (INFO_H + SWATCH) / 2,
        };
        chrome(hdc, sw, 4, Some(cref(rgb)), Some(cref(0x505050)));

        SetBkMode(hdc, TRANSPARENT);
        let tx = sw.right + GAP;
        let hex = format!("#{rgb:06X}");
        let f_hex = font(12, FW_SEMIBOLD.0 as i32, w!("Consolas"), FF_MODERN);
        let s_hex = Sel::new(hdc, &f_hex);
        SetTextColor(hdc, cref(0xFFFFFF));
        text_in(
            hdc,
            RECT { left: tx, top: y + PAD - 1, right: panel.right - PAD, bottom: y + 19 },
            &hex,
            DT_LEFT,
        );
        drop(s_hex);

        let triple = format!("rgb({}, {}, {})", (rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
        let f_rgb = font(9, FW_NORMAL.0 as i32, w!("Segoe UI"), FF_SWISS);
        let s_rgb = Sel::new(hdc, &f_rgb);
        SetTextColor(hdc, cref(0xC8D4E4));
        text_in(
            hdc,
            RECT { left: tx, top: y + 19, right: panel.right - PAD, bottom: y + INFO_H - PAD },
            &triple,
            DT_LEFT,
        );
        drop(s_rgb);
    }

    /// Segoe UI / Consolas. Negatif yükseklik karakter yüksekliği demek, hücre değil.
    unsafe fn font(height: i32, weight: i32, face: PCWSTR, family: FONT_FAMILY) -> GdiObj {
        GdiObj(
            CreateFontW(
                -height,
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
                (DEFAULT_PITCH.0 as u32) | (family.0 as u32),
                face,
            )
            .into(),
        )
    }

    /// Tek satır metin; `rect` içinde dikey ortalı.
    unsafe fn text_in(hdc: HDC, rect: RECT, text: &str, align: DRAW_TEXT_FORMAT) {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut r = rect;
        let _ = DrawTextW(hdc, &mut wide, &mut r, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | align);
    }

    fn lo(l: LPARAM) -> i32 {
        (l.0 as u32 & 0xFFFF) as i16 as i32
    }

    fn hi(l: LPARAM) -> i32 {
        ((l.0 as u32 >> 16) & 0xFFFF) as i16 as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Merkez örnek tam karenin ortasındaki piksel olmalı; hex/RGB ondan okunur.
    #[test]
    fn centre_pixel_is_the_middle_of_the_grid() {
        assert_eq!(GRID, 13);
        assert_eq!(LOUPE % GRID, 0);
        assert_eq!(centre_offset(), ((6 * GRID + 6) * 4) as usize);

        let mut buf = vec![0u8; (GRID * GRID * 4) as usize];
        let off = centre_offset();
        // BGRA: #EA6F12 → B=0x12, G=0x6F, R=0xEA
        buf[off] = 0x12;
        buf[off + 1] = 0x6F;
        buf[off + 2] = 0xEA;
        assert_eq!(rgb_from_bgra(&buf, off), 0xEA6F12);
    }

    /// Ekran kenarında örnekler taşmaz, kenar pikseline yapışır (siyah değil).
    #[test]
    fn grid_clamps_at_the_screen_edge() {
        let (w, h) = (4i32, 3i32);
        let mut src = vec![0u8; (w * h * 4) as usize];
        // Her pikselin B kanalına düz indeksini yaz: hangi pikselden örneklendiği
        // kırpma testinde tek bakışta görünür.
        for i in 0..(w * h) as usize {
            src[i * 4] = i as u8;
        }
        let mut out = vec![0u8; (GRID * GRID * 4) as usize];
        // İmleç sol üst köşede: karenin çoğu ekranın dışında.
        sample_grid(&src, w, h, 0, 0, &mut out);

        assert_eq!(out[0], 0, "sol üst örnek (0,0) pikseli");
        assert_eq!(out[centre_offset()], 0, "merkez örnek imlecin kendisi");
        // Sağ üst örnek x=3'e kırpılır: piksel (3, 0) → indeks 3.
        let right_top = ((GRID - 1) * 4) as usize;
        assert_eq!(out[right_top], 3, "sağdaki örnek kenara yapışır");
        // Sağ alt örnek piksel (3, 2) → indeks 11.
        let right_bottom = (((GRID - 1) * GRID + (GRID - 1)) * 4) as usize;
        assert_eq!(out[right_bottom], 11, "alttaki örnek kenara yapışır");
    }
}
