//! Fullscreen lock overlay using native Win32 APIs.
//!
//! The overlay runs on a dedicated Rust thread with its own message loop.
//! To unlock, we post WM_QUIT to that thread — the window is destroyed
//! instantly because it lives in our process.

use std::sync::Mutex;
use tracing::info;

#[cfg(windows)]
use crate::win32;

/// State of the lock overlay thread.
#[allow(dead_code)]
struct LockThreadState {
    /// Win32 thread ID of the overlay's message loop.
    thread_id: u32,
    /// Join handle so we can wait for cleanup.
    join_handle: std::thread::JoinHandle<()>,
}

static LOCK_STATE: Mutex<Option<LockThreadState>> = Mutex::new(None);

/// Available visual templates for the lock screen.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockTemplate {
    Minimal,
    Warning,
    Elegant,
    Terminal,
}

impl Default for LockTemplate {
    fn default() -> Self {
        Self::Minimal
    }
}

/// Colors for a template: (background RGB, text RGB, font_name, font_size, bold, italic).
#[cfg(windows)]
struct TemplateStyle {
    bg: (u8, u8, u8),
    fg: (u8, u8, u8),
    font_name: &'static str,
    font_size: i32,
    bold: bool,
    italic: bool,
}

#[cfg(windows)]
fn get_style(template: &LockTemplate) -> TemplateStyle {
    match template {
        LockTemplate::Minimal => TemplateStyle {
            bg: (10, 10, 14),
            fg: (232, 232, 237),
            font_name: "Segoe UI",
            font_size: 48,
            bold: false,
            italic: false,
        },
        LockTemplate::Warning => TemplateStyle {
            bg: (60, 0, 0),
            fg: (255, 59, 92),
            font_name: "Segoe UI",
            font_size: 54,
            bold: true,
            italic: false,
        },
        LockTemplate::Elegant => TemplateStyle {
            bg: (18, 18, 28),
            fg: (200, 180, 140),
            font_name: "Georgia",
            font_size: 46,
            bold: false,
            italic: true,
        },
        LockTemplate::Terminal => TemplateStyle {
            bg: (0, 0, 0),
            fg: (52, 211, 153),
            font_name: "Consolas",
            font_size: 42,
            bold: false,
            italic: false,
        },
    }
}

/// Data passed to the overlay thread.
#[cfg(windows)]
struct OverlayParams {
    text: String,
    style: TemplateStyle,
}

// Thread-local storage for overlay data (needed by the window proc callback).
#[cfg(windows)]
thread_local! {
    static OVERLAY_TEXT: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
    static OVERLAY_BG: std::cell::Cell<u32> = std::cell::Cell::new(0);
    static OVERLAY_FG: std::cell::Cell<u32> = std::cell::Cell::new(0);
    static OVERLAY_FONT: std::cell::Cell<win32::HFONT> = std::cell::Cell::new(0);
    static OVERLAY_BRUSH: std::cell::Cell<win32::HBRUSH> = std::cell::Cell::new(0);
    static KB_HOOK: std::cell::Cell<win32::HHOOK> = std::cell::Cell::new(0);
}

/// The keyboard hook callback. Blocks dangerous shortcuts.
/// Must be minimal — no allocations, no panics.
#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    if code >= 0 {
        let kbd = &*(lparam as *const win32::KBDLLHOOKSTRUCT);
        let alt = (kbd.flags & win32::LLKHF_ALTDOWN) != 0;

        // Block Win keys
        if kbd.vkCode == win32::VK_LWIN || kbd.vkCode == win32::VK_RWIN {
            return 1;
        }
        // Block Alt+Tab
        if alt && kbd.vkCode == win32::VK_TAB {
            return 1;
        }
        // Block Alt+F4
        if alt && kbd.vkCode == win32::VK_F4 {
            return 1;
        }
        // Block Alt+Esc
        if alt && kbd.vkCode == win32::VK_ESCAPE {
            return 1;
        }
        // Block Ctrl+Shift+Esc and Ctrl+Esc are handled by Task Manager being disabled.
        // Ctrl+Alt+Del cannot be hooked (handled by the kernel), but with Task Manager
        // disabled, the user can't do much from that screen anyway.
    }

    let hook = KB_HOOK.with(|h| h.get());
    win32::CallNextHookEx(hook, code, wparam, lparam)
}

/// The window procedure for the overlay.
#[cfg(windows)]
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: win32::HWND,
    msg: win32::UINT,
    wparam: win32::WPARAM,
    lparam: win32::LPARAM,
) -> win32::LRESULT {
    match msg {
        win32::WM_PAINT => {
            let mut ps = std::mem::zeroed::<win32::PAINTSTRUCT>();
            let hdc = win32::BeginPaint(hwnd, &mut ps);

            // Fill background
            let brush = OVERLAY_BRUSH.with(|b| b.get());
            let mut rc = std::mem::zeroed::<win32::RECT>();
            win32::GetClientRect(hwnd, &mut rc);
            win32::FillRect(hdc, &rc, brush);

            // Draw text
            let fg = OVERLAY_FG.with(|f| f.get());
            let font = OVERLAY_FONT.with(|f| f.get());
            win32::SetTextColor(hdc, fg);
            win32::SetBkMode(hdc, win32::TRANSPARENT);
            let old_font = win32::SelectObject(hdc, font);

            OVERLAY_TEXT.with(|t| {
                let text = t.borrow();
                let wide: Vec<u16> = text.encode_utf16().collect();
                win32::DrawTextW(
                    hdc,
                    wide.as_ptr(),
                    wide.len() as i32,
                    &mut rc,
                    win32::DT_CENTER | win32::DT_VCENTER | win32::DT_WORDBREAK | win32::DT_NOPREFIX,
                );
            });

            win32::SelectObject(hdc, old_font);
            win32::EndPaint(hwnd, &ps);
            0
        }

        win32::WM_TIMER => {
            // Re-assert topmost every tick.
            win32::SetWindowPos(
                hwnd,
                win32::HWND_TOPMOST,
                0, 0, 0, 0,
                win32::SWP_NOMOVE | win32::SWP_NOSIZE | win32::SWP_SHOWWINDOW,
            );
            win32::SetForegroundWindow(hwnd);
            win32::BringWindowToTop(hwnd);
            0
        }

        win32::WM_CLOSE => {
            // Ignore close attempts (Alt+F4 is blocked anyway, but just in case).
            0
        }

        win32::WM_DESTROY => {
            win32::PostQuitMessage(0);
            0
        }

        _ => win32::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Run the overlay on the current thread (blocks until WM_QUIT is received).
#[cfg(windows)]
fn run_overlay(params: OverlayParams) {
    unsafe {
        let hinstance = win32::GetModuleHandleW(std::ptr::null());
        let class_name = win32::wide_string("LockdownOverlayClass");

        // Register window class.
        let wc = win32::WNDCLASSEXW {
            cbSize: std::mem::size_of::<win32::WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: 0,
        };
        win32::RegisterClassExW(&wc);

        // Set up thread-local state for the window proc.
        let style = &params.style;
        let bg_color = win32::rgb(style.bg.0, style.bg.1, style.bg.2);
        let fg_color = win32::rgb(style.fg.0, style.fg.1, style.fg.2);
        let brush = win32::CreateSolidBrush(bg_color);
        let font_name = win32::wide_string(style.font_name);
        let font = win32::CreateFontW(
            -style.font_size, 0, 0, 0,
            if style.bold { win32::FW_BOLD } else { win32::FW_NORMAL },
            if style.italic { 1 } else { 0 },
            0, 0,
            win32::DEFAULT_CHARSET,
            win32::OUT_DEFAULT_PRECIS,
            win32::CLIP_DEFAULT_PRECIS,
            win32::CLEARTYPE_QUALITY,
            win32::DEFAULT_PITCH,
            font_name.as_ptr(),
        );

        OVERLAY_TEXT.with(|t| *t.borrow_mut() = params.text);
        OVERLAY_BG.with(|b| b.set(bg_color));
        OVERLAY_FG.with(|f| f.set(fg_color));
        OVERLAY_FONT.with(|f| f.set(font));
        OVERLAY_BRUSH.with(|b| b.set(brush));

        // Install keyboard hook.
        let hook = win32::SetWindowsHookExW(
            win32::WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            hinstance,
            0,
        );
        KB_HOOK.with(|h| h.set(hook));

        // Get virtual screen bounds.
        let vr = win32::virtual_screen_rect();
        let w = vr.right - vr.left;
        let h = vr.bottom - vr.top;

        // Create the overlay window.
        let title = win32::wide_string("LockdownOverlay");
        let hwnd = win32::CreateWindowExW(
            win32::WS_EX_TOPMOST | win32::WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            win32::WS_POPUP | win32::WS_VISIBLE,
            vr.left, vr.top, w, h,
            0, 0, hinstance, 0,
        );

        // Set a timer for periodic topmost re-assertion (300ms).
        win32::SetTimer(hwnd, 1, 300, 0);

        // Force to front.
        win32::SetForegroundWindow(hwnd);
        win32::BringWindowToTop(hwnd);

        // Message loop — runs until WM_QUIT.
        let mut msg = std::mem::zeroed::<win32::MSG>();
        while win32::GetMessageW(&mut msg, 0, 0, 0) > 0 {
            win32::TranslateMessage(&msg);
            win32::DispatchMessageW(&msg);
        }

        // Cleanup.
        if hook != 0 {
            win32::UnhookWindowsHookEx(hook);
        }
        win32::DeleteObject(font);
        win32::DeleteObject(brush);
        win32::DestroyWindow(hwnd);
    }
}

/// Launch the fullscreen lock overlay.
///
/// Sequence:
/// 1. Disable Task Manager (instant registry write)
/// 2. Hide taskbar (instant FindWindow + ShowWindow)
/// 3. Spawn overlay thread (instant window creation, no compilation)
pub fn engage_lock(text: &str, template: &LockTemplate) -> Result<(), String> {
    {
        let state = LOCK_STATE.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;
        if state.is_some() {
            return Err("Screen is already locked".into());
        }
    }

    #[cfg(windows)]
    {
        // Step 1 & 2: Instant system changes.
        win32::set_task_manager_disabled(true);
        win32::set_taskbar_visible(false);

        // Step 3: Spawn overlay thread.
        let style = get_style(template);
        let overlay_text = text.to_string();
        let (tx, rx) = std::sync::mpsc::channel();

        let join_handle = std::thread::spawn(move || {
            // Report our Win32 thread ID back to the main thread.
            let tid = unsafe { win32::GetCurrentThreadId() };
            let _ = tx.send(tid);

            let params = OverlayParams {
                text: overlay_text,
                style,
            };
            run_overlay(params);
        });

        // Wait for the thread to report its ID.
        let thread_id = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Overlay thread failed to start".to_string())?;

        let mut state = LOCK_STATE.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;
        *state = Some(LockThreadState {
            thread_id,
            join_handle,
        });

        info!("Lock engaged: overlay thread {thread_id}, taskbar hidden, Task Manager disabled");
        Ok(())
    }

    #[cfg(not(windows))]
    {
        info!("[stub] engage_lock: text={text:?}, template={template:?}");
        let mut state = LOCK_STATE.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;
        *state = Some(LockThreadState {
            thread_id: 0,
            join_handle: std::thread::spawn(|| {}),
        });
        Ok(())
    }
}

/// Kill the lock overlay and restore system access.
///
/// 1. Clear state immediately (so UI updates instantly)
/// 2. Post WM_QUIT to the overlay thread (instant)
/// 3. Restore taskbar and Task Manager
pub fn disengage_lock() -> Result<(), String> {
    let thread_state = {
        let mut state = LOCK_STATE.lock().map_err(|e| format!("Mutex poisoned: {e}"))?;
        state.take()
    };

    #[cfg(windows)]
    {
        if let Some(ts) = thread_state {
            // Post WM_QUIT to the overlay thread — this makes GetMessage return 0,
            // exiting the message loop cleanly and destroying the window.
            win32::post_thread_message(ts.thread_id, win32::WM_QUIT);

            // Wait briefly for cleanup, but don't block forever.
            let _ = ts.join_handle.join();
        }

        // Always restore system state, even if thread was already gone.
        win32::set_taskbar_visible(true);
        win32::set_task_manager_disabled(false);

        info!("Lock disengaged: overlay destroyed, taskbar restored, Task Manager re-enabled");
        Ok(())
    }

    #[cfg(not(windows))]
    {
        if let Some(ts) = thread_state {
            let _ = ts.join_handle.join();
        }
        info!("[stub] disengage_lock");
        Ok(())
    }
}

/// Restore system to a clean state. Called on startup to recover from crashes.
pub fn startup_cleanup() {
    #[cfg(windows)]
    {
        win32::set_taskbar_visible(true);
        win32::set_task_manager_disabled(false);
        info!("Startup cleanup: taskbar shown, Task Manager enabled");
    }
}

/// Check if the screen is currently locked.
#[allow(dead_code)]
pub fn is_locked() -> bool {
    LOCK_STATE
        .lock()
        .map(|s| s.is_some())
        .unwrap_or(false)
}
