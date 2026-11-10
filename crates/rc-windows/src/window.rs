//! Win32 window registration, creation and small geometry helpers.

use windows::core::{w, HSTRING, PCWSTR, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const CLASS: PCWSTR = w!("RustChromeWindow");

/// Icon + version resources are embedded by `build.rs` with id 1.
const ICON_ID: usize = 1;

pub fn instance() -> HINSTANCE {
    unsafe {
        match GetModuleHandleW(None) {
            Ok(m) => HINSTANCE(m.0),
            Err(_) => HINSTANCE(std::ptr::null_mut()),
        }
    }
}

pub fn register(hinst: HINSTANCE) -> Result<()> {
    unsafe {
        let icon = LoadIconW(Some(hinst), PCWSTR(ICON_ID as *const u16)).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(crate::app::dispatch),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 as isize + 1) as *mut _),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: CLASS,
            hIconSm: icon,
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        Ok(())
    }
}

/// Never ask for a window larger than the desktop work area: Windows will
/// happily create one that is a few pixels wider than the screen, which puts the
/// client area outside the display on small monitors (and makes the window
/// un-draggable).
fn clamp_to_work_area(width: i32, height: i32) -> (i32, i32) {
    unsafe {
        let mut r = RECT::default();
        let ok = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut r as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok();
        if ok {
            let aw = (r.right - r.left).max(640);
            let ah = (r.bottom - r.top).max(480);
            return (width.min(aw - 24), height.min(ah - 24));
        }
    }
    (width, height)
}

pub fn create(hinst: HINSTANCE, title: &str, width: i32, height: i32) -> Result<HWND> {
    let (width, height) = clamp_to_work_area(width, height);
    let title = HSTRING::from(title);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            &title,
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            None,
            None,
            Some(hinst),
            None,
        )
    }
}

pub fn show(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
}

pub fn client_size(hwnd: HWND) -> (i32, i32) {
    let mut r = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
    }
    ((r.right - r.left).max(0), (r.bottom - r.top).max(0))
}

pub fn dpi_scale(hwnd: HWND) -> f64 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

pub fn set_title(hwnd: HWND, title: &str) {
    unsafe {
        let _ = SetWindowTextW(hwnd, &HSTRING::from(title));
    }
}

pub fn message_box(hwnd: HWND, text: &str, caption: &str) {
    unsafe {
        MessageBoxW(
            Some(hwnd),
            &HSTRING::from(text),
            &HSTRING::from(caption),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Blend the native title bar into the browser chrome where the OS allows it:
/// Win11 honours an exact caption colour, Win10 1809+ at least honours the
/// dark-mode flag. Unsupported attributes simply fail and are ignored.
pub fn apply_frame_theme(hwnd: HWND, dark: bool) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    unsafe {
        let flag = windows::core::BOOL(if dark { 1 } else { 0 });
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &flag as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<windows::core::BOOL>() as u32,
        );
        // COLORREF is 0x00BBGGRR: #dee1e6 (headerbar light) / #202124 (dark).
        let colour: u32 = if dark { 0x0024_2120 } else { 0x00E6_E1DE };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &colour as *const _ as *const core::ffi::c_void,
            4,
        );
    }
}

/// `GetDpiForWindow` lives in the HiDpi module but is used everywhere here.
use windows::Win32::UI::HiDpi::GetDpiForWindow;
