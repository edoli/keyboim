mod key_hook;
mod platform;

use std::{
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use eframe::{egui, egui::Rgba};
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

const TITLE_BAR_HEIGHT: f32 = 32.0;
const TITLE_SIDE_PADDING: f32 = 10.0;

fn title_bar_ui(ui: &mut egui::Ui, title_bar_rect: egui::Rect, title: &str, is_focused: bool) {
    use egui::{Id, PointerButton, Sense};

    // 타이틀 바 영역을 인터랙션 대상으로 등록
    let bar_resp = ui.interact(
        title_bar_rect,
        Id::new("title_bar"),
        Sense::click_and_drag(),
    );

    // 2) 배경 그리기
    let p = ui.painter_at(title_bar_rect);
    let visuals = ui.visuals();

    let (fill, stroke) = if bar_resp.is_pointer_button_down_on() {
        // pressed 상태일 때 조금 진하게
        (
            visuals.widgets.active.bg_fill,
            visuals.widgets.active.bg_stroke,
        )
    } else if bar_resp.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.hovered.bg_stroke,
        )
    } else {
        // 포커스 여부로 기본색 약간 다르게
        let base = visuals.window_fill();
        let fill = if is_focused {
            base
        } else {
            base.gamma_multiply(0.95)
        };
        (fill, visuals.window_stroke())
    };

    let corner_radius = egui::CornerRadius {
        nw: 4,
        ne: 4,
        sw: 0,
        se: 0,
    };
    p.rect_filled(title_bar_rect, corner_radius, fill);
    p.rect_stroke(
        title_bar_rect,
        corner_radius,
        stroke,
        egui::StrokeKind::Inside,
    );

    // 3) 제목 텍스트
    let title_pos = egui::pos2(
        title_bar_rect.left() + TITLE_SIDE_PADDING,
        title_bar_rect.center().y,
    );
    p.text(
        title_pos,
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(16.0),
        visuals.text_color(),
    );

    if bar_resp.is_pointer_button_down_on() {
        // ← "pressed" 상태
    }
    if bar_resp.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    // 텍스트/라인/버튼 등 그리기 …
}

fn background_ui(ui: &mut egui::Ui, rect: egui::Rect) {
    let p = ui.painter_at(rect);
    let visuals = ui.visuals();

    let corner_radius = egui::CornerRadius {
        nw: 0,
        ne: 0,
        sw: 4,
        se: 4,
    };
    p.rect_filled(rect, corner_radius, visuals.window_fill());
    p.rect_stroke(
        rect,
        corner_radius,
        visuals.window_stroke(),
        egui::StrokeKind::Inside,
    );
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
                // platform::make_click_through_windows(&handle);
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

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let remain_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        ui.min_rect().min.x,
                        ui.min_rect().min.y + TITLE_BAR_HEIGHT - 1.0,
                    ),
                    egui::vec2(
                        ui.min_rect().width(),
                        ui.min_rect().height() - TITLE_BAR_HEIGHT,
                    ),
                );
                background_ui(ui, remain_rect);

                let title_rect = egui::Rect::from_min_size(
                    ui.min_rect().min,
                    egui::vec2(ui.min_rect().width(), TITLE_BAR_HEIGHT),
                );
                title_bar_ui(ui, title_rect, "Key Display", true);

                let area_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        remain_rect.min.x + TITLE_SIDE_PADDING,
                        remain_rect.min.y + TITLE_SIDE_PADDING,
                    ),
                    egui::vec2(
                        remain_rect.width() - TITLE_SIDE_PADDING * 2.0,
                        remain_rect.height() - TITLE_SIDE_PADDING * 2.0,
                    ),
                );
                egui::Area::new(egui::Id::new("root_area"))
                    .fixed_pos(area_rect.min)
                    .default_size(area_rect.size())
                    .show(ui.ctx(), |ui| {
                        if let Ok(lock) = self.current_key.lock() {
                            if let Some(text) = &*lock {
                                ui.label(format!("Last Key: {}", text));
                            } else {
                                ui.label("Press any key...");
                            }
                        }
                    });
            });

        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_always_on_top()
        .with_decorations(false)
        .with_transparent(true);

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
