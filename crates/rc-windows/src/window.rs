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

pub fn create(hinst: HINSTANCE, title: &str, width: i32, height: i32) -> Result<HWND> {
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

/// `GetDpiForWindow` lives in the HiDpi module but is used everywhere here.
use windows::Win32::UI::HiDpi::GetDpiForWindow;
