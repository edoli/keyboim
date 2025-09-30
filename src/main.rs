use std::ptr::null_mut;
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{GetKeyboardLayout, ToUnicodeEx},
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
            HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, HC_ACTION, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};

static mut HOOK: HHOOK = HHOOK(null_mut()); // ✅ 포인터로 초기화

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code == HC_ACTION as i32 {
        let kb: &KBDLLHOOKSTRUCT = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let msg = w_param.0 as u32;

        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let vk = kb.vkCode;

            if let Some(ch) = vk_to_text(vk) {
                println!("VK={vk:#04X} {ch}");
            } else {
                println!("VK={vk:#04X}");
            }
        }
    }
    CallNextHookEx(HOOK, n_code, w_param, l_param)
}

unsafe fn vk_to_text(vk: u32) -> Option<String> {
    let layout = GetKeyboardLayout(0);   // HKL
    let keystate = [0u8; 256];
    let mut buf = [0u16; 8];

    // ✅ 마지막 파라미터: HKL (Option 아님)
    let rc = ToUnicodeEx(vk, 0, &keystate, &mut buf, 0, layout);

    if rc > 0 {
        Some(String::from_utf16_lossy(&buf[..rc as usize]))
    } else {
        None
    }
}

fn main() {
    unsafe {
        let hmod = GetModuleHandleW(None).expect("GetModuleHandleW failed");

        // ✅ .into() 없이 그대로 전달
        HOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), hmod, 0)
            .expect("SetWindowsHookExW failed");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
