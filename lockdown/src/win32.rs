//! Win32 API bindings and safe wrappers.
//!
//! All `unsafe` FFI lives in this module. Everything else in the codebase
//! calls only the safe wrapper functions exported here.

#![allow(non_snake_case, dead_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub type HWND = isize;
pub type HINSTANCE = isize;
pub type HDC = isize;
pub type HBITMAP = isize;
pub type HGDIOBJ = isize;
pub type HBRUSH = isize;
pub type HFONT = isize;
pub type HHOOK = isize;
pub type HKEY = isize;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type ATOM = u16;
pub type DWORD = u32;
pub type LONG = i32;
pub type BYTE = u8;
pub type UINT = u32;
pub type BOOL = i32;

pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;
pub type HOOKPROC = Option<unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT>;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Window styles
pub const WS_POPUP: u32 = 0x80000000;
pub const WS_VISIBLE: u32 = 0x10000000;
pub const WS_EX_TOPMOST: u32 = 0x00000008;
pub const WS_EX_TOOLWINDOW: u32 = 0x00000080;
pub const WS_EX_LAYERED: u32 = 0x00080000;
pub const WS_EX_NOACTIVATE: u32 = 0x08000000;

// Window messages
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_QUIT: u32 = 0x0012;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_USER: u32 = 0x0400;

// Custom message to signal shutdown from another thread.
pub const WM_LOCKDOWN_QUIT: u32 = WM_USER + 1;

// ShowWindow commands
pub const SW_HIDE: i32 = 0;
pub const SW_SHOW: i32 = 5;

// Hooks
pub const WH_KEYBOARD_LL: i32 = 13;

// Virtual key codes
pub const VK_TAB: i32 = 0x09;
pub const VK_ESCAPE: i32 = 0x1B;
pub const VK_F4: i32 = 0x73;
pub const VK_LWIN: i32 = 0x5B;
pub const VK_RWIN: i32 = 0x5C;
pub const VK_DELETE: i32 = 0x2E;

// Keyboard hook flags
pub const LLKHF_ALTDOWN: u32 = 0x20;

// GDI
pub const SRCCOPY: u32 = 0x00CC0020;
pub const BI_RGB: u32 = 0;
pub const DIB_RGB_COLORS: u32 = 0;
pub const TRANSPARENT: i32 = 1;
pub const DT_CENTER: u32 = 0x0001;
pub const DT_VCENTER: u32 = 0x0004;
pub const DT_SINGLELINE: u32 = 0x0020;
pub const DT_WORDBREAK: u32 = 0x0010;
pub const DT_NOPREFIX: u32 = 0x0800;

// GetSystemMetrics
pub const SM_XVIRTUALSCREEN: i32 = 76;
pub const SM_YVIRTUALSCREEN: i32 = 77;
pub const SM_CXVIRTUALSCREEN: i32 = 78;
pub const SM_CYVIRTUALSCREEN: i32 = 79;
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

// SetWindowPos
pub const HWND_TOPMOST: isize = -1;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOSIZE: u32 = 0x0001;
pub const SWP_SHOWWINDOW: u32 = 0x0040;

// Registry
pub const HKEY_CURRENT_USER: HKEY = -2147483647isize; // 0x80000001 as isize
pub const KEY_SET_VALUE: u32 = 0x0002;
pub const REG_DWORD: u32 = 4;

// Font weight
pub const FW_NORMAL: i32 = 400;
pub const FW_BOLD: i32 = 700;

// Charset
pub const DEFAULT_CHARSET: u32 = 1;
pub const OUT_DEFAULT_PRECIS: u32 = 0;
pub const CLIP_DEFAULT_PRECIS: u32 = 0;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const DEFAULT_PITCH: u32 = 0;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: isize,
    pub hCursor: isize,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: isize,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: i32,
    pub scanCode: i32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE; 32],
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

// ---------------------------------------------------------------------------
// FFI declarations
// ---------------------------------------------------------------------------

extern "system" {
    // Window management
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    pub fn CreateWindowExW(
        dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16,
        dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32,
        hWndParent: HWND, hMenu: isize, hInstance: HINSTANCE, lpParam: isize,
    ) -> HWND;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn SetWindowPos(
        hWnd: HWND, hWndInsertAfter: HWND, x: i32, y: i32, cx: i32, cy: i32, uFlags: u32,
    ) -> BOOL;
    pub fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> HWND;
    pub fn PostMessageW(hWnd: HWND, msg: u32, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn PostThreadMessageW(idThread: u32, msg: u32, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn DefWindowProcW(hWnd: HWND, msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn BringWindowToTop(hWnd: HWND) -> BOOL;
    pub fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: u32, lpTimerFunc: usize) -> usize;

    // Display metrics
    pub fn GetSystemMetrics(nIndex: i32) -> i32;

    // Painting
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn DrawTextW(hdc: HDC, lpchText: *const u16, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;

    // GDI objects
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn CreateFontW(
        cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32,
        cWeight: i32, bItalic: u32, bUnderline: u32, bStrikeOut: u32,
        iCharSet: u32, iOutPrecision: u32, iClipPrecision: u32,
        iQuality: u32, iPitchAndFamily: u32, pszFaceName: *const u16,
    ) -> HFONT;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;

    // Screen capture
    pub fn GetDC(hWnd: HWND) -> HDC;
    pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
    pub fn BitBlt(hdc: HDC, x: i32, y: i32, cx: i32, cy: i32, hdcSrc: HDC, x1: i32, y1: i32, rop: u32) -> BOOL;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn GetDIBits(
        hdc: HDC, hbm: HBITMAP, start: u32, cLines: u32,
        lpvBits: *mut u8, lpbmi: *mut BITMAPINFO, usage: u32,
    ) -> i32;

    // Hooks
    pub fn SetWindowsHookExW(idHook: i32, lpfn: HOOKPROC, hmod: HINSTANCE, dwThreadId: u32) -> HHOOK;
    pub fn UnhookWindowsHookEx(hhk: HHOOK) -> BOOL;
    pub fn CallNextHookEx(hhk: HHOOK, nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;

    // Module
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    pub fn GetCurrentThreadId() -> u32;

    // Registry
    pub fn RegOpenKeyExW(hKey: HKEY, lpSubKey: *const u16, ulOptions: u32, samDesired: u32, phkResult: *mut HKEY) -> i32;
    pub fn RegSetValueExW(hKey: HKEY, lpValueName: *const u16, Reserved: u32, dwType: u32, lpData: *const u8, cbData: u32) -> i32;
    pub fn RegDeleteValueW(hKey: HKEY, lpValueName: *const u16) -> i32;
    pub fn RegCloseKey(hKey: HKEY) -> i32;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a Rust string to a null-terminated wide string (UTF-16).
pub fn wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Pack R, G, B into a Win32 COLORREF (0x00BBGGRR).
pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

// ---------------------------------------------------------------------------
// Safe wrappers
// ---------------------------------------------------------------------------

/// Find a window by class name. Returns 0 if not found.
pub fn find_window(class_name: &str) -> HWND {
    let cls = wide_string(class_name);
    unsafe { FindWindowW(cls.as_ptr(), std::ptr::null()) }
}

/// Show or hide a window.
pub fn show_window(hwnd: HWND, show: bool) -> bool {
    unsafe { ShowWindow(hwnd, if show { SW_SHOW } else { SW_HIDE }) != 0 }
}

/// Find and hide/show the Windows taskbar.
pub fn set_taskbar_visible(visible: bool) -> bool {
    let hwnd = find_window("Shell_TrayWnd");
    if hwnd == 0 {
        return false;
    }
    show_window(hwnd, visible);
    // Also handle secondary taskbar on multi-monitor setups.
    let hwnd2 = find_window("Shell_SecondaryTrayWnd");
    if hwnd2 != 0 {
        show_window(hwnd2, visible);
    }
    true
}

/// Disable or re-enable Task Manager via the registry.
pub fn set_task_manager_disabled(disabled: bool) -> bool {
    let subkey = wide_string("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System");
    let value_name = wide_string("DisableTaskMgr");
    let mut hkey: HKEY = 0;

    unsafe {
        let result = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey);
        if result != 0 {
            // Key doesn't exist yet — that's fine for the enable case.
            if !disabled {
                return true;
            }
            // For disable, we'd need to create it. Simplified: just return false.
            return false;
        }

        let ok = if disabled {
            let val: u32 = 1;
            RegSetValueExW(
                hkey,
                value_name.as_ptr(),
                0,
                REG_DWORD,
                &val as *const u32 as *const u8,
                4,
            ) == 0
        } else {
            // Delete the value (re-enables Task Manager).
            let _ = RegDeleteValueW(hkey, value_name.as_ptr());
            true
        };

        RegCloseKey(hkey);
        ok
    }
}

/// Get the virtual screen bounds (all monitors combined).
pub fn virtual_screen_rect() -> RECT {
    unsafe {
        RECT {
            left: GetSystemMetrics(SM_XVIRTUALSCREEN),
            top: GetSystemMetrics(SM_YVIRTUALSCREEN),
            right: GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: GetSystemMetrics(SM_YVIRTUALSCREEN) + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

/// Get primary screen dimensions.
pub fn primary_screen_size() -> (i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
        )
    }
}

/// Capture the primary screen and return raw BGRA pixel data + dimensions.
pub fn capture_screen() -> Result<(Vec<u8>, i32, i32), String> {
    let (w, h) = primary_screen_size();
    if w == 0 || h == 0 {
        return Err("GetSystemMetrics returned 0".into());
    }

    unsafe {
        let hdc_screen = GetDC(0);
        if hdc_screen == 0 {
            return Err("GetDC failed".into());
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem == 0 {
            ReleaseDC(0, hdc_screen);
            return Err("CreateCompatibleDC failed".into());
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
        if hbm == 0 {
            DeleteDC(hdc_mem);
            ReleaseDC(0, hdc_screen);
            return Err("CreateCompatibleBitmap failed".into());
        }

        let old = SelectObject(hdc_mem, hbm);
        let ok = BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, 0, 0, SRCCOPY);
        SelectObject(hdc_mem, old);

        if ok == 0 {
            DeleteObject(hbm);
            DeleteDC(hdc_mem);
            ReleaseDC(0, hdc_screen);
            return Err("BitBlt failed".into());
        }

        // Read pixels via GetDIBits.
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // Negative = top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [0],
        };

        let buf_size = (w * h * 4) as usize;
        let mut pixels = vec![0u8; buf_size];

        let lines = GetDIBits(
            hdc_mem,
            hbm,
            0,
            h as u32,
            pixels.as_mut_ptr(),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(0, hdc_screen);

        if lines == 0 {
            return Err("GetDIBits failed".into());
        }

        Ok((pixels, w, h))
    }
}

/// Post a message to a specific thread.
pub fn post_thread_message(thread_id: u32, msg: u32) -> bool {
    unsafe { PostThreadMessageW(thread_id, msg, 0, 0) != 0 }
}
