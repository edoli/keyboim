use anyhow::{Context, Result};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_ALLOW_NCPAINT, DWMWA_BORDER_COLOR,
        DWMWA_CAPTION_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    },
    UI::Shell::{DefSubclassProc, SetWindowSubclass},
    UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, GWL_EXSTYLE,
        GWL_STYLE, HWND_TOPMOST, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW, WS_BORDER, WS_CAPTION, WS_DLGFRAME,
        WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_LAYERED, WS_EX_STATICEDGE,
        WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WM_NCACTIVATE, WM_NCCALCSIZE,
        WM_NCPAINT, WM_STYLECHANGING, STYLESTRUCT,
    },
};

#[cfg(target_os = "windows")]
const BORDERLESS_SUBCLASS_ID: usize = 1;

pub fn configure_window(window: &Window) -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        install_borderless_subclass(hwnd)?;
        apply_non_client_policy(hwnd).ok();
        let ex_style = borderless_ex_style(GetWindowLongW(hwnd, GWL_EXSTYLE))
            | WS_EX_LAYERED.0 as i32
            | WS_EX_NOACTIVATE.0 as i32;
        SetWindowLongW(hwnd, GWL_STYLE, borderless_style(GetWindowLongW(hwnd, GWL_STYLE)));
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).ok();
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        )
        .ok();
    }

    Ok(())
}

pub fn set_click_through(window: &Window, enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        install_borderless_subclass(hwnd)?;
        apply_non_client_policy(hwnd).ok();
        SetWindowLongW(hwnd, GWL_STYLE, borderless_style(GetWindowLongW(hwnd, GWL_STYLE)));
        let mut ex_style = borderless_ex_style(GetWindowLongW(hwnd, GWL_EXSTYLE));
        ex_style |= WS_EX_LAYERED.0 as i32;
        ex_style |= WS_EX_NOACTIVATE.0 as i32;
        if enabled {
            ex_style |= WS_EX_TRANSPARENT.0 as i32;
        } else {
            ex_style &= !(WS_EX_TRANSPARENT.0 as i32);
        }
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        )
        .ok();
    }

    Ok(())
}

pub fn show_ready_window(window: &Window) -> Result<()> {
    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = hwnd(window)?;
        install_borderless_subclass(hwnd)?;
        apply_non_client_policy(hwnd).ok();
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOACTIVATE
                | SWP_NOOWNERZORDER
                | SWP_FRAMECHANGED
                | SWP_SHOWWINDOW,
        )
        .ok();
    }

    #[cfg(not(target_os = "windows"))]
    window.set_visible(true);

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn install_borderless_subclass(hwnd: HWND) -> windows::core::Result<()> {
    SetWindowSubclass(
        hwnd,
        Some(borderless_subclass_proc),
        BORDERLESS_SUBCLASS_ID,
        0,
    )
    .ok()
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn borderless_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        WM_NCCALCSIZE | WM_NCPAINT => LRESULT(0),
        WM_NCACTIVATE => LRESULT(1),
        WM_STYLECHANGING => {
            if lparam.0 != 0 {
                let style = &mut *(lparam.0 as *mut STYLESTRUCT);
                if wparam.0 as i32 == GWL_STYLE.0 {
                    style.styleNew = borderless_style_bits(style.styleNew);
                } else if wparam.0 as i32 == GWL_EXSTYLE.0 {
                    style.styleNew =
                        borderless_ex_style_bits(style.styleNew) | WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0;
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

#[cfg(target_os = "windows")]
unsafe fn apply_non_client_policy(hwnd: HWND) -> windows::core::Result<()> {
    let nc_rendering_policy = DWMNCRP_DISABLED;
    let allow_nc_paint: i32 = 0;
    let color_none = DWMWA_COLOR_NONE;
    let corner_preference = DWMWCP_DONOTROUND;

    DwmSetWindowAttribute(
        hwnd,
        DWMWA_NCRENDERING_POLICY,
        &nc_rendering_policy as *const _ as *const _,
        std::mem::size_of_val(&nc_rendering_policy) as u32,
    )?;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_ALLOW_NCPAINT,
        &allow_nc_paint as *const _ as *const _,
        std::mem::size_of_val(&allow_nc_paint) as u32,
    )?;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR,
        &color_none as *const _ as *const _,
        std::mem::size_of_val(&color_none) as u32,
    )?;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_CAPTION_COLOR,
        &color_none as *const _ as *const _,
        std::mem::size_of_val(&color_none) as u32,
    )?;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner_preference as *const _ as *const _,
        std::mem::size_of_val(&corner_preference) as u32,
    )?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn borderless_style(style: i32) -> i32 {
    borderless_style_bits(style as u32) as i32
}

#[cfg(target_os = "windows")]
fn borderless_style_bits(style: u32) -> u32 {
    let frame_mask = (WS_CAPTION.0
        | WS_THICKFRAME.0
        | WS_SYSMENU.0
        | WS_MINIMIZEBOX.0
        | WS_MAXIMIZEBOX.0
        | WS_BORDER.0
        | WS_DLGFRAME.0) as i32;
    (style & !(frame_mask as u32)) | WS_POPUP.0
}

#[cfg(target_os = "windows")]
fn borderless_ex_style(ex_style: i32) -> i32 {
    borderless_ex_style_bits(ex_style as u32) as i32
}

#[cfg(target_os = "windows")]
fn borderless_ex_style_bits(ex_style: u32) -> u32 {
    let frame_mask = (WS_EX_DLGMODALFRAME.0
        | WS_EX_CLIENTEDGE.0
        | WS_EX_STATICEDGE.0
        | WS_EX_WINDOWEDGE.0) as i32;
    ex_style & !(frame_mask as u32)
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
