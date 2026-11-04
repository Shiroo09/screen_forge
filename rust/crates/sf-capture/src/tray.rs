//! System tray icon. Port of the tray in `ScreenForge/Tray/TrayIconService.cs`.
//!
//! Registers a `NOTIFYICONDATA` on a hidden window, handles the left-click and
//! context-menu callbacks, and removes the icon on drop. The C# app used
//! H.NotifyIcon (a WPF wrapper); this calls `Shell_NotifyIconW` directly so the
//! tray works in a plain Win32 message loop.

/// A live tray icon; removes itself when dropped.
pub struct TrayIcon {
    window: isize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TrayIcon {
    pub fn window(&self) -> isize {
        self.window
    }

    /// Stops the tray's message loop and removes the icon.
    pub fn close(mut self) {
        self.signal_quit();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    fn signal_quit(&self) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
            let _ = PostMessageW(Some(HWND(self.window as *mut _)), WM_TRAY_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.signal_quit();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(windows)]
pub const WM_TRAY_QUIT: u32 = 0x8000 + 2;
/// Callback message: the tray icon was clicked.
#[cfg(windows)]
pub const WM_TRAY_CALLBACK: u32 = 0x8000 + 3;

#[cfg(windows)]
pub fn install(tooltip: &str, on_click: impl FnMut() + Send + 'static) -> Result<TrayIcon, String> {
    imp::install(tooltip.to_string(), on_click)
}

#[cfg(not(windows))]
pub fn install(_tooltip: &str, _on_click: impl FnMut() + Send + 'static) -> Result<TrayIcon, String> {
    Err("system tray is only supported on Windows".to_string())
}

#[cfg(windows)]
mod imp {
    use super::{TrayIcon, WM_TRAY_CALLBACK, WM_TRAY_QUIT};
    use std::sync::mpsc;

    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadIconW,
        RegisterClassW, IDI_APPLICATION, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_LBUTTONUP,
        WM_NCCREATE, WNDCLASSW,
    };

    const TRAY_UID: u32 = 1;

    thread_local! {
        static ON_CLICK: std::cell::RefCell<Option<Box<dyn FnMut()>>> =
            const { std::cell::RefCell::new(None) };
    }

    pub fn install(tooltip: String, on_click: impl FnMut() + Send + 'static) -> Result<TrayIcon, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();

        let thread = std::thread::spawn(move || unsafe {
            let instance = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("GetModuleHandleW: {e}")));
                    return;
                }
            };

            let class_name = w!("ScreenForgeTray");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!(""),
                WINDOW_STYLE(0),
                0, 0, 0, 0,
                Some(HWND(-3isize as *mut _)),
                None,
                Some(instance.into()),
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("CreateWindowExW: {e}")));
                    return;
                }
            };

            ON_CLICK.with(|c| *c.borrow_mut() = Some(Box::new(on_click) as Box<dyn FnMut()>));

            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_UID,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAY_CALLBACK,
                ..Default::default()
            };

            if let Ok(icon) = LoadIconW(None, IDI_APPLICATION) {
                data.hIcon = icon;
            }

            // Tooltip is a fixed 128-wide UTF-16 buffer; copy what fits.
            let tip: Vec<u16> = tooltip.encode_utf16().take(data.szTip.len() - 1).collect();
            data.szTip[..tip.len()].copy_from_slice(&tip);

            if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                let _ = ready_tx.send(Err("Shell_NotifyIconW(NIM_ADD) failed".to_string()));
                return;
            }

            let _ = ready_tx.send(Ok(hwnd.0 as isize));

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                if msg.message == WM_TRAY_QUIT {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }

            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        });

        match ready_rx.recv() {
            Ok(Ok(window)) => Ok(TrayIcon {
                window,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err("tray thread died during setup".to_string()),
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_TRAY_CALLBACK && lparam.0 as u32 == WM_LBUTTONUP {
            ON_CLICK.with(|c| {
                if let Some(cb) = c.borrow_mut().as_mut() {
                    cb();
                }
            });
            return LRESULT(0);
        }
        if msg == WM_NCCREATE {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    // Keep PCWSTR referenced (used by LoadIconW's zero-arg form on some builds).
    #[allow(dead_code)]
    fn _keep(_: PCWSTR) {}
}
