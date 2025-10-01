use indexmap::IndexSet;
use std::ptr::null_mut;
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::*,
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
            HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
        },
    },
};

static mut HOOK: HHOOK = HHOOK(null_mut());
static mut CALLBACK: Option<Box<dyn FnMut(u32, u32) + Send>> = None;

pub unsafe fn register_hook<F>(cb: F)
where
    F: FnMut(u32, u32) + Send + 'static,
{
    CALLBACK = Some(Box::new(cb));

    let hmod = GetModuleHandleW(None).expect("GetModuleHandleW failed");
    HOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), hmod, 0)
        .expect("SetWindowsHookExW failed");

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        if TranslateMessage(&msg).0 > 0 {
            break;
        }
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code == HC_ACTION as i32 {
        let kb: &KBDLLHOOKSTRUCT = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let msg = w_param.0 as u32;

        #[allow(static_mut_refs)]
        if let Some(cb) = &mut CALLBACK {
            cb(kb.vkCode, msg);
        }
    }
    CallNextHookEx(HOOK, n_code, w_param, l_param)
}

pub unsafe fn vk_to_text(vk: u32) -> String {
    let layout = GetKeyboardLayout(0);
    let keystate = [0u8; 256];
    let mut buf = [0u16; 8];

    match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2C => "PrintScreen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x90 => "NumLock",
        0x91 => "ScrollLock",

        // Modifier keys
        0x10 => "Shift",
        0xA0 => "Shift",
        0xA1 => "Shift",

        0x11 => "Ctrl",
        0xA2 => "Ctrl", // Left Ctrl
        0xA3 => "Ctrl", // Right Ctrl

        0x12 => "Alt",
        0xA4 => "Alt", // Left Alt
        0xA5 => "Alt", // Right Alt

        0x5B => "Win", // Left Windows
        0x5C => "Win", // Right Windows

        0x5D => "Apps",

        0x15 => "Kana",  // VK_KANA
        0x19 => "Kanji", // VK_KANJI

        0x70..=0x7B => {
            let n = vk - 0x6F; // F1..F12
            return format!("F{n}");
        }
        _ => {
            let rc = ToUnicodeEx(vk, 0, &keystate, &mut buf, 0, layout);
            if rc > 0 {
                return String::from_utf16_lossy(&buf[..rc as usize]).to_uppercase();
            } else {
                return format!("VK_{vk:02X}");
            }
        }
    }
    .to_string()
}

pub fn key_combination_to_string(keys: &mut IndexSet<u32>) -> String {
    let modifier_priority = |vk: u32| -> u16 {
        match VIRTUAL_KEY(vk as u16) {
            VK_LCONTROL | VK_RCONTROL => 0, // Ctrl
            VK_LSHIFT | VK_RSHIFT => 1,     // Shift
            VK_LMENU | VK_RMENU => 2,       // Alt
            VK_LWIN | VK_RWIN => 3,         // Meta(Win/Super)
            _ => 10,
        }
    };

    keys.sort_by_key(|&vk| modifier_priority(vk));

    unsafe {
        keys.iter()
            .map(|&vk| vk_to_text(vk))
            .collect::<Vec<_>>()
            .join(" + ")
    }
}
