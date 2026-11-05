//! Settings window. Port of `ScreenForge/Windows/SettingsWindow.xaml.cs`.
//!
//! The C# window was WPF/XAML. This builds the same controls with the built-in
//! Win32 control classes (`BUTTON`, `COMBOBOX`, `EDIT`, `STATIC`), runs a modal
//! loop, and writes edits back into `AppSettings`. Cancel discards them.

use sf_settings::{AppSettings, ImageFormat};

/// Opens the settings window modally. Returns `true` if the user pressed Save
/// (settings already mutated), `false` on Cancel/close.
#[cfg(windows)]
pub fn show(settings: &mut AppSettings) -> bool {
    imp::show(settings)
}

#[cfg(not(windows))]
pub fn show(_settings: &mut AppSettings) -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use super::{AppSettings, ImageFormat};

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetWindowTextW,
        GetWindowLongPtrW, IsWindow, PostQuitMessage, RegisterClassW, SendMessageW, SetWindowLongPtrW,
        ShowWindow, CREATESTRUCTW, CW_USEDEFAULT, GWLP_USERDATA, HMENU, MSG,
        SW_SHOW, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NCCREATE,
        WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
    };

    // Control ids.
    const ID_SHOW_CURSOR: i32 = 100;
    const ID_LAUNCH: i32 = 101;
    const ID_AUTOCOPY: i32 = 102;
    const ID_FORMAT: i32 = 103;
    const ID_QUALITY: i32 = 104;
    const ID_SAVEDIR: i32 = 105;
    const ID_GIF: i32 = 106;
    const ID_GIF_FPS: i32 = 107;
    const ID_VIDEO: i32 = 108;
    const ID_VIDEO_FPS: i32 = 109;
    const ID_AUTOCLOSE: i32 = 110;
    const ID_OK: i32 = 1;
    const ID_CANCEL: i32 = 2;

    const BM_GETCHECK: u32 = 0x00F0;
    const BM_SETCHECK: u32 = 0x00F1;
    const BST_CHECKED: usize = 1;
    const CB_ADDSTRING: u32 = 0x0143;
    const CB_SETCURSEL: u32 = 0x014E;
    const CB_GETCURSEL: u32 = 0x0147;
    const WS_GROUP: i32 = 0x00020000;

    // Layout constants (logical pixels).
    const LBL_X: i32 = 16;
    const CTL_X: i32 = 200;
    const ROW_H: i32 = 30;
    const CTL_W: i32 = 240;

    struct State {
        settings: AppSettings,
        saved: bool,
        // Handles are kept to read values back on OK.
        show_cursor: HWND,
        launch: HWND,
        autocopy: HWND,
        autoclose: HWND,
        format: HWND,
        quality: HWND,
        savedir: HWND,
        gif: HWND,
        gif_fps: HWND,
        video: HWND,
        video_fps: HWND,
    }

    pub fn show(settings: &mut AppSettings) -> bool {
        unsafe {
            let instance = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(_) => return false,
            };

            let class_name = w!("ScreenForgeSettings");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let mut state = State {
                settings: settings.clone(),
                saved: false,
                show_cursor: HWND(std::ptr::null_mut()),
                launch: HWND(std::ptr::null_mut()),
                autocopy: HWND(std::ptr::null_mut()),
                autoclose: HWND(std::ptr::null_mut()),
                format: HWND(std::ptr::null_mut()),
                quality: HWND(std::ptr::null_mut()),
                savedir: HWND(std::ptr::null_mut()),
                gif: HWND(std::ptr::null_mut()),
                gif_fps: HWND(std::ptr::null_mut()),
                video: HWND(std::ptr::null_mut()),
                video_fps: HWND(std::ptr::null_mut()),
            };

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("ScreenForge — Ayarlar"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                500,
                560,
                None,
                None,
                Some(instance.into()),
                Some(&mut state as *mut _ as *mut _),
            ) {
                Ok(h) => h,
                Err(_) => return false,
            };

            let state_ptr =
                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if state_ptr.is_null() {
                return false;
            }

            let _ = ShowWindow(hwnd, SW_SHOW);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = DispatchMessageW(&msg);
                if !IsWindow(Some(hwnd)).as_bool() {
                    break;
                }
            }

            let state = &*state_ptr;
            if state.saved {
                *settings = state.settings.clone();
                true
            } else {
                false
            }
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_NCCREATE {
            let cs = lparam.0 as *const CREATESTRUCTW;
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
            WM_CREATE => {
                build_controls(hwnd, s);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                if id == ID_OK {
                    read_back(s);
                    s.saved = true;
                    let _ = DestroyWindow(hwnd);
                } else if id == ID_CANCEL {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Creates the child controls and fills them from the current settings.
    unsafe fn build_controls(hwnd: HWND, s: &mut State) {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let mut row = 1i32;
        let mut next_row = || {
            let y = 16 + row * ROW_H;
            row += 1;
            y
        };

        // Checkboxes.
        s.show_cursor = checkbox(hwnd, instance.into(), ID_SHOW_CURSOR, "İmleci göster", LBL_X, next_row(), s.settings.show_cursor);
        s.launch = checkbox(hwnd, instance.into(), ID_LAUNCH, "Başlangıçta çalıştır", LBL_X, next_row(), s.settings.launch_at_startup);
        s.autocopy = checkbox(hwnd, instance.into(), ID_AUTOCOPY, "Yükledikten sonra linki kopyala", LBL_X, next_row(), s.settings.auto_copy_link_after_upload);
        s.autoclose = checkbox(hwnd, instance.into(), ID_AUTOCLOSE, "Yükleme penceresini otomatik kapat", LBL_X, next_row(), s.settings.auto_close_upload_window);

        // Output format combo.
        let y = next_row();
        label(hwnd, instance.into(), "Çıktı formatı", LBL_X, y);
        s.format = combobox(hwnd, instance.into(), ID_FORMAT, CTL_X, y);
        for name in ["Png", "Jpeg", "Webp"] {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(s.format, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(wide.as_ptr() as isize)));
        }
        let fmt_index = match s.settings.output_format {
            ImageFormat::Png => 0,
            ImageFormat::Jpeg => 1,
            ImageFormat::Webp => 2,
        };
        SendMessageW(s.format, CB_SETCURSEL, Some(WPARAM(fmt_index)), Some(LPARAM(0)));

        // Quality edit.
        let y = next_row();
        label(hwnd, instance.into(), "Kalite (1-100)", LBL_X, y);
        s.quality = edit(hwnd, instance.into(), ID_QUALITY, CTL_X, y, &s.settings.quality.to_string());

        // Save directory edit.
        let y = next_row();
        label(hwnd, instance.into(), "Kayıt klasörü", LBL_X, y);
        s.savedir = edit(hwnd, instance.into(), ID_SAVEDIR, CTL_X, y, &s.settings.save_directory);

        // GIF section.
        s.gif = checkbox(hwnd, instance.into(), ID_GIF, "GIF kaydı etkin", LBL_X, next_row(), s.settings.gif.enabled);
        let y = next_row();
        label(hwnd, instance.into(), "GIF fps", LBL_X, y);
        s.gif_fps = edit(hwnd, instance.into(), ID_GIF_FPS, CTL_X, y, &s.settings.gif.fps.to_string());

        // Video section.
        s.video = checkbox(hwnd, instance.into(), ID_VIDEO, "Video kaydı etkin", LBL_X, next_row(), s.settings.video.enabled);
        let y = next_row();
        label(hwnd, instance.into(), "Video fps", LBL_X, y);
        s.video_fps = edit(hwnd, instance.into(), ID_VIDEO_FPS, CTL_X, y, &s.settings.video.fps.to_string());

        // Buttons at the bottom.
        let btn_y = 16 + row * ROW_H + 8;
        button(hwnd, instance.into(), ID_OK, "Kaydet", CTL_X, btn_y);
        button(hwnd, instance.into(), ID_CANCEL, "İptal", CTL_X + 110, btn_y);
    }

    fn to_wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn checkbox(
        parent: HWND,
        instance: windows::Win32::Foundation::HINSTANCE,
        id: i32,
        text: &str,
        x: i32,
        y: i32,
        checked: bool,
    ) -> HWND {
        let class = to_wide("BUTTON");
        let label = to_wide(text);
        // BS_AUTOCHECKBOX = 0x3
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0x3),
            x,
            y,
            CTL_W + 100,
            24,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None,
        )
        .unwrap_or_default();
        if checked {
            SendMessageW(hwnd, BM_SETCHECK, Some(WPARAM(BST_CHECKED)), Some(LPARAM(0)));
        }
        hwnd
    }

    unsafe fn label(
        parent: HWND,
        instance: windows::Win32::Foundation::HINSTANCE,
        text: &str,
        x: i32,
        y: i32,
    ) {
        let class = to_wide("STATIC");
        let label = to_wide(text);
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            x,
            y + 4,
            CTL_X - LBL_X - 8,
            20,
            Some(parent),
            None,
            Some(instance),
            None,
        );
    }

    unsafe fn combobox(
        parent: HWND,
        instance: windows::Win32::Foundation::HINSTANCE,
        id: i32,
        x: i32,
        y: i32,
    ) -> HWND {
        let class = to_wide("COMBOBOX");
        // CBS_DROPDOWNLIST = 0x3
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_GROUP as u32 | 0x3),
            x,
            y,
            CTL_W,
            200,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None,
        )
        .unwrap_or_default()
    }

    unsafe fn edit(
        parent: HWND,
        instance: windows::Win32::Foundation::HINSTANCE,
        id: i32,
        x: i32,
        y: i32,
        value: &str,
    ) -> HWND {
        let class = to_wide("EDIT");
        let text = to_wide(value);
        // ES_AUTOHSCROLL = 0x80
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0x80),
            x,
            y,
            CTL_W,
            24,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None,
        )
        .unwrap_or_default()
    }

    unsafe fn button(
        parent: HWND,
        instance: windows::Win32::Foundation::HINSTANCE,
        id: i32,
        text: &str,
        x: i32,
        y: i32,
    ) {
        let class = to_wide("BUTTON");
        let label = to_wide(text);
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0x0),
            x,
            y,
            100,
            28,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(instance),
            None,
        );
    }

    unsafe fn checked(hwnd: HWND) -> bool {
        SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))).0 == BST_CHECKED as isize
    }

    unsafe fn read_text(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }

    /// Copies control values back into the state's settings.
    unsafe fn read_back(s: &mut State) {
        s.settings.show_cursor = checked(s.show_cursor);
        s.settings.launch_at_startup = checked(s.launch);
        s.settings.auto_copy_link_after_upload = checked(s.autocopy);
        s.settings.auto_close_upload_window = checked(s.autoclose);

        s.settings.output_format = match SendMessageW(s.format, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0 {
            1 => ImageFormat::Jpeg,
            2 => ImageFormat::Webp,
            _ => ImageFormat::Png,
        };

        if let Ok(q) = read_text(s.quality).trim().parse::<i32>() {
            s.settings.quality = q.clamp(1, 100);
        }
        let dir = read_text(s.savedir);
        if !dir.trim().is_empty() {
            s.settings.save_directory = dir;
        }

        s.settings.gif.enabled = checked(s.gif);
        if let Ok(fps) = read_text(s.gif_fps).trim().parse::<i32>() {
            s.settings.gif.fps = fps.clamp(1, 60);
        }

        s.settings.video.enabled = checked(s.video);
        if let Ok(fps) = read_text(s.video_fps).trim().parse::<i32>() {
            s.settings.video.fps = fps.clamp(1, 60);
        }
    }
}
