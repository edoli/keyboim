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
        GWL_STYLE, HWND_TOP, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, WS_BORDER,
        WS_CAPTION, WS_DLGFRAME, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_LAYERED,
        WS_EX_STATICEDGE, WS_EX_TRANSPARENT, WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    },
};

pub fn configure_window(window: &Window) -> Result<()> {
    window.set_decorations(false);

    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        let ex_style = borderless_ex_style(GetWindowLongW(hwnd, GWL_EXSTYLE)) | WS_EX_LAYERED.0 as i32;
        SetWindowLongW(hwnd, GWL_STYLE, borderless_style(GetWindowLongW(hwnd, GWL_STYLE)));
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);
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
    window.set_decorations(false);

    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        SetWindowLongW(hwnd, GWL_STYLE, borderless_style(GetWindowLongW(hwnd, GWL_STYLE)));
        let mut ex_style = borderless_ex_style(GetWindowLongW(hwnd, GWL_EXSTYLE));
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
fn borderless_style(style: i32) -> i32 {
    let frame_mask = (WS_CAPTION.0
        | WS_THICKFRAME.0
        | WS_SYSMENU.0
        | WS_MINIMIZEBOX.0
        | WS_MAXIMIZEBOX.0
        | WS_BORDER.0
        | WS_DLGFRAME.0) as i32;
    (style & !frame_mask) | WS_POPUP.0 as i32
}

#[cfg(target_os = "windows")]
fn borderless_ex_style(ex_style: i32) -> i32 {
    let frame_mask = (WS_EX_DLGMODALFRAME.0
        | WS_EX_CLIENTEDGE.0
        | WS_EX_STATICEDGE.0
        | WS_EX_WINDOWEDGE.0) as i32;
    ex_style & !frame_mask
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
