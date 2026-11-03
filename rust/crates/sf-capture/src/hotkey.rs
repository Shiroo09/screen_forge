//! Global hotkey registration and the message loop that delivers it.
//! Port of the `RegisterHotKey` path in `ScreenForge/Hotkeys/HotkeyService.cs`.
//!
//! A hidden message-only window receives `WM_HOTKEY` and calls the supplied
//! callback. The callback runs on the message-loop thread, matching the C# app
//! which dispatched hotkeys on the UI thread.

/// A registered global hotkey; unregisters and stops its loop when dropped.
pub struct HotkeyRegistration {
    id: i32,
    window: isize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HotkeyRegistration {
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Stops the message loop and unregisters the hotkey.
    pub fn stop(mut self) {
        self.signal_quit();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    fn signal_quit(&self) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
            use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
            let _ = PostMessageW(Some(HWND(self.window as *mut _)), WM_APP_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for HotkeyRegistration {
    fn drop(&mut self) {
        self.signal_quit();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Message posted to the hidden window to end the message loop.
#[cfg(windows)]
pub const WM_APP_QUIT: u32 = 0x8000 + 1;

#[cfg(windows)]
pub fn register(
    modifiers: u32,
    vk: u32,
    callback: impl FnMut() + Send + 'static,
) -> Result<HotkeyRegistration, String> {
    imp::register(modifiers, vk, callback)
}

#[cfg(windows)]
mod imp {
    use super::HotkeyRegistration;
    use std::sync::mpsc;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, MSG,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_HOTKEY, WNDCLASSW,
    };

    const HOTKEY_ID: i32 = 1;

    // Message sent to the hidden window to end the loop.
    const WM_APP_QUIT: u32 = 0x8000 + 1;

    pub fn register(
        modifiers: u32,
        vk: u32,
        callback: impl FnMut() + Send + 'static,
    ) -> Result<HotkeyRegistration, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();

        let thread = std::thread::spawn(move || unsafe {
            let instance = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("GetModuleHandleW: {e}")));
                    return;
                }
            };

            let class_name = windows::core::w!("ScreenForgeHotkeySink");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Message-only window (HWND_MESSAGE parent).
            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                windows::core::w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND(-3isize as *mut _)), // HWND_MESSAGE
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

            if let Err(e) = RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk) {
                let _ = ready_tx.send(Err(format!("RegisterHotKey: {e}")));
                return;
            }

            // Publish the callback and the target window so wndproc can call it.
            CALLBACK.with(|c| *c.borrow_mut() = Some(Box::new(callback)));

            let _ = ready_tx.send(Ok(hwnd.0 as isize));

            // Message loop; exits on WM_APP_QUIT or WM_QUIT.
            let mut msg = MSG::default();
            loop {
                let r = GetMessageW(&mut msg, None, 0, 0);
                if r.0 == 0 || r.0 == -1 {
                    break;
                }
                if msg.message == WM_APP_QUIT {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }

            let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
        });

        // Wait for setup to finish before returning.
        match ready_rx.recv() {
            Ok(Ok(window)) => Ok(HotkeyRegistration {
                id: HOTKEY_ID,
                window,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err("hotkey thread died during setup".to_string()),
        }
    }

    thread_local! {
        static CALLBACK: std::cell::RefCell<Option<Box<dyn FnMut()>>> =
            const { std::cell::RefCell::new(None) };
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_HOTKEY {
            CALLBACK.with(|c| {
                if let Some(cb) = c.borrow_mut().as_mut() {
                    cb();
                }
            });
            return LRESULT(0);
        }
        if msg == WM_APP_QUIT {
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

#[cfg(not(windows))]
pub fn register(
    _modifiers: u32,
    _vk: u32,
    _callback: impl FnMut() + Send + 'static,
) -> Result<HotkeyRegistration, String> {
    Err("global hotkeys are only supported on Windows".to_string())
}
