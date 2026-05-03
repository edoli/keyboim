use anyhow::{Context, Result};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{COLORREF, HWND},
    Graphics::Dwm::DwmExtendFrameIntoClientArea,
    UI::Controls::MARGINS,
    UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, GWL_EXSTYLE,
        HWND_TOP, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, WS_EX_LAYERED,
        WS_EX_TRANSPARENT,
    },
};

pub fn configure_window(window: &Window) -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as i32);
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).ok();
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        )
        .ok();
    }

    Ok(())
}

pub fn set_click_through(window: &Window, enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        let mut ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        ex_style |= WS_EX_LAYERED.0 as i32;
        if enabled {
            ex_style |= WS_EX_TRANSPARENT.0 as i32;
        } else {
            ex_style &= !(WS_EX_TRANSPARENT.0 as i32);
        }
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);
        SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        )
        .ok();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn hwnd(window: &Window) -> Result<HWND> {
    let handle = window.window_handle().context("getting raw window handle")?;
    let raw = handle.as_raw();
    match raw {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => anyhow::bail!("not running on Windows"),
    }
}
