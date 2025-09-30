#[cfg(target_os = "windows")]
pub fn make_click_through_windows(window_handle: &raw_window_handle::WindowHandle) {
    use raw_window_handle::HasWindowHandle;
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use winit::platform::windows::WindowExtWindows;
    use winit::platform::windows::*;

    unsafe {
        use raw_window_handle::RawWindowHandle;

        let hwnd_v = match window_handle.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => panic!("not running on Windows"),
        };
        let hwnd = HWND(hwnd_v as *mut _);
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        // 레이어드 + 클릭 스루
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            ex | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32,
        );
    }
}

// use raw_window_handle::{HasWindowHandle, RawWindowHandle};
// use windows::Win32::UI::WindowsAndMessaging::*;
// use windows::Win32::Foundation::HWND;

// // Obtain HWND via raw-window-handle (hwnd() helper not available in this winit version)
// let hwnd = match window.window_handle().ok().map(|h| h.as_raw()) {
//     Some(RawWindowHandle::Win32(h)) => HWND(h.hwnd as _),
//     _ => return, // Not a Win32 window handle; nothing to do.
// };

#[cfg(target_os = "macos")]
pub fn make_click_through_macos(window: &winit::window::Window) {
    use winit::platform::macos::WindowExtMacOS;
    // NSWindow* 얻어서 마우스 무시
    unsafe {
        let ns_window = window.ns_window();
        // Objective-C: [ns_window setIgnoresMouseEvents:YES];
        // cocoa 또는 objc2 계열 크레이트로 호출 가능
        use cocoa::appkit::NSWindow as _;
        use cocoa::base::id;
        let ns_window: id = ns_window as _;
        ns_window.setIgnoresMouseEvents_(true);
    }
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
pub fn make_click_through_x11_or_wayland(window: &winit::window::Window) {
    #[cfg(feature = "x11")]
    {
        use winit::platform::x11::WindowExtX11;
        // xcb 윈도우 ID
        if let Some(xcb_window) = window.xcb_window() {
            // x11rb 크레이트 사용 예 (XFixes로 입력 영역 제거)
            use x11rb::connection::Connection;
            use x11rb::protocol::xfixes::{ConnectionExt as _, Region};
            use x11rb::rust_connection::RustConnection;

            if let Ok((conn, screen_num)) = RustConnection::connect(None) {
                let screen = &conn.setup().roots[screen_num];
                let region = conn.generate_id().unwrap();
                // 빈 영역 생성 후 입력 영역을 빈 영역으로 설정
                let _ = conn.xfixes_create_region(region, &[] as &[(i16, i16, u16, u16)]);
                let _ = conn.xfixes_set_window_shape_region(
                    xcb_window,
                    x11rb::protocol::xfixes::ShapeKind::Input,
                    0,
                    0,
                    Region::from(region),
                );
                let _ = conn.flush();
            }
        }
    }

    #[cfg(feature = "wayland")]
    {
        // Wayland: wl_surface.set_input_region(None)
        use winit::platform::wayland::WindowExtWayland;
        if let Some(wl_surface) = window.wayland_surface() {
            // wayland-client 크레이트로 region을 아예 설정하지 않거나 빈 region 설정
            // (구체 코드는 사용하는 Wayland 바인딩 버전에 따라 달라짐)
            // 개념: wl_surface.set_input_region(None);
        }
    }
}
