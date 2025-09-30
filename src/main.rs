mod key_hook;
mod platform;

use std::{
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use eframe::{
    egui,
    egui::{Color32, Rgba},
};
use raw_window_handle::HasWindowHandle;

struct App {
    patched_hwnd: OnceLock<isize>,
    current_key: Arc<Mutex<Option<String>>>,
}

impl App {
    fn new() -> Self {
        let key_state = Arc::new(Mutex::new(None));
        let key_state_clone = key_state.clone();

        thread::spawn(move || unsafe {
            key_hook::register_hook(move |vk, msg| {
                if msg == 256 || msg == 260 {
                    let text = key_hook::vk_to_text(vk);

                    let mut lock = key_state_clone.lock().unwrap();
                    *lock = Some(text);
                }
            });
        });
        Self {
            patched_hwnd: OnceLock::new(),
            current_key: key_state,
        }
    }
}

impl eframe::App for App {
    // 창 배경을 완전 투명으로
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(handle) = frame.window_handle() {
                platform::make_click_through_windows(&handle);
                // eframe이 raw_window_handle을 노출 (PR/이슈에서 합의된 경로)
                // HWND 뽑기
                // if let raw_window_handle::RawWindowHandle::Win32(h) = raw {
                //     let hwnd = h.hwnd.get() as isize;
                //     // 아직 적용 안 했거나, 핸들이 바뀐 경우 재적용
                //     let need = self
                //         .patched_hwnd
                //         .get()
                //         .map(|saved| *saved != hwnd)
                //         .unwrap_or(true);
                //     if need {
                //         unsafe { win::make_click_through(hwnd) };
                //         let _ = self.patched_hwnd.set(hwnd);
                //     }
                // }
            }
        }

        // CentralPanel의 프레임/배경을 끕니다
        egui::CentralPanel::default()
            .frame(egui::Frame::none()) // <- 배경 채우지 않기
            .show(ctx, |ui| {
                ui.label("Frameless Transparent Window");
                ui.label(self.current_key.lock().unwrap().clone().unwrap_or_default());
            });

        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    // 장식 제거 + 투명 창
    let viewport = egui::ViewportBuilder::default()
        .with_always_on_top()
        .with_decorations(false)
        .with_transparent(true); // OS가 지원하면 진짜 투명

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Transparent Window",
        options,
        Box::new(|cc| Ok(Box::new(App::new()))),
    )
}
