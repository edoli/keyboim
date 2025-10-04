#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod key_hook;
mod mouse;
mod platform;

use std::{
    sync::{Arc, Mutex},
    thread,
};

use eframe::{egui, egui::Rgba};
use indexmap::IndexSet;
use raw_window_handle::HasWindowHandle;
use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP};

use crate::{key_hook::is_disable_overlay_key_pressed, mouse::draw_mouse};

struct App {
    pressed_keys: Arc<Mutex<IndexSet<u32>>>,
    mouse_buttons: Arc<Mutex<[bool; 5]>>,
    last_combination: IndexSet<u32>,
    is_key_cleared: bool,
    is_overlay: bool,
    last_update: std::time::Instant,
    is_show_mouse: bool,
    is_outline: bool,
}

impl App {
    fn new() -> Self {
        let pressed_keys = Arc::new(Mutex::new(IndexSet::new()));
        let pressed_keys_clone = pressed_keys.clone();
        let mouse_buttons: Arc<Mutex<[bool; 5]>> = Arc::new(Mutex::new([false; 5]));
        let mouse_buttons_clone = mouse_buttons.clone();

        thread::spawn(move || unsafe {
            key_hook::register_hook(move |vk, msg| {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    let mut lock = pressed_keys_clone.lock().unwrap();
                    lock.insert(vk);
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    let mut lock = pressed_keys_clone.lock().unwrap();
                    lock.swap_remove(&vk);
                }
            });
        });
        // Mouse hook thread
        thread::spawn(move || unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            key_hook::register_mouse_hook(move |msg, _x, _y, data| {
                let mut state = mouse_buttons_clone.lock().unwrap();
                match msg {
                    WM_LBUTTONDOWN => state[0] = true,
                    WM_LBUTTONUP => state[0] = false,
                    WM_RBUTTONDOWN => state[1] = true,
                    WM_RBUTTONUP => state[1] = false,
                    WM_MBUTTONDOWN => state[2] = true,
                    WM_MBUTTONUP => state[2] = false,
                    WM_XBUTTONDOWN => {
                        let which = (data >> 16) & 0xFFFF;
                        if which == 1 {
                            state[3] = true;
                        } else if which == 2 {
                            state[4] = true;
                        }
                    }
                    WM_XBUTTONUP => {
                        let which = (data >> 16) & 0xFFFF;
                        if which == 1 {
                            state[3] = false;
                        } else if which == 2 {
                            state[4] = false;
                        }
                    }
                    _ => {}
                }
            });
        });
        Self {
            pressed_keys,
            last_combination: IndexSet::new(),
            is_key_cleared: false,
            is_overlay: false,
            last_update: std::time::Instant::now(),
            is_show_mouse: true,
            is_outline: true,
            mouse_buttons,
        }
    }
}

const TITLE_BAR_HEIGHT: f32 = 32.0;
const TITLE_SIDE_PADDING: f32 = 10.0;

fn title_bar_ui(ui: &mut egui::Ui, title_bar_rect: egui::Rect, title: &str) {
    use egui::{Id, PointerButton, Sense};

    let bar_resp = ui.interact(
        title_bar_rect,
        Id::new("title_bar"),
        Sense::click_and_drag(),
    );

    let p = ui.painter_at(title_bar_rect);
    let visuals = ui.visuals();

    let fill = visuals.window_fill();
    let stroke = visuals.window_stroke();

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

    if bar_resp.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
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

fn outlined_text(
    ui: &mut egui::Ui,
    text: &str,
    pos: egui::Pos2,
    font_size: f32,
    text_color: egui::Color32,
    outline_color: egui::Color32,
    outline_thickness: f32,
) {
    let font = egui::FontId::proportional(font_size);
    let diagonal = outline_thickness as f32 * 0.7071;
    let offsets = [
        egui::Vec2::new(-outline_thickness, 0.0),
        egui::Vec2::new(outline_thickness, 0.0),
        egui::Vec2::new(0.0, -outline_thickness),
        egui::Vec2::new(0.0, outline_thickness),
        egui::Vec2::new(-diagonal, -diagonal),
        egui::Vec2::new(diagonal, -diagonal),
        egui::Vec2::new(-diagonal, diagonal),
        egui::Vec2::new(diagonal, diagonal),
    ];

    for offset in offsets {
        ui.painter().text(
            pos + offset,
            egui::Align2::LEFT_TOP,
            text,
            font.clone(),
            outline_color,
        );
    }

    ui.painter()
        .text(pos, egui::Align2::LEFT_TOP, text, font, text_color);
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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

                let title_rect = egui::Rect::from_min_size(
                    ui.min_rect().min,
                    egui::vec2(ui.min_rect().width(), TITLE_BAR_HEIGHT),
                );

                if !self.is_overlay {
                    background_ui(ui, remain_rect);
                    title_bar_ui(ui, title_rect, "Keyboim");
                }

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
                        ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
                        ui.horizontal(|ui| {
                            if self.is_show_mouse {
                                if let Ok(mouse_buttons) = self.mouse_buttons.lock() {
                                    draw_mouse(ui, &*mouse_buttons);
                                }
                            }
                            if let Ok(pressed_keys) = self.pressed_keys.lock() {
                                if pressed_keys.is_empty() {
                                    self.is_key_cleared = true;
                                } else {
                                    if pressed_keys.len() > self.last_combination.len()
                                        || self.is_key_cleared
                                    {
                                        self.last_combination = pressed_keys.clone();
                                        self.last_update = std::time::Instant::now();

                                        if is_disable_overlay_key_pressed(&pressed_keys) {
                                            self.is_overlay = false;

                                            #[cfg(target_os = "windows")]
                                            if let Ok(handle) = frame.window_handle() {
                                                platform::disable_click_through_windows(&handle);
                                            }
                                        }
                                    }

                                    self.is_key_cleared = false;
                                }
                            }
                            if !self.last_combination.is_empty() {
                                let pressed_str =
                                    key_hook::key_combination_to_string(&mut self.last_combination);
                                let elapsed = self.last_update.elapsed();
                                let alpha = (255.0
                                    * (3.0 - elapsed.as_millis() as f32 / 1000.0).clamp(0.0, 1.0))
                                    as u8;

                                if self.is_outline {
                                    outlined_text(
                                        ui,
                                        &pressed_str,
                                        ui.cursor().min,
                                        56.0,
                                        egui::Color32::from_white_alpha(alpha)
                                            * ui.visuals().text_color(),
                                        egui::Color32::from_black_alpha(alpha / 4),
                                        2.0,
                                    );
                                } else {
                                    ui.label(egui::RichText::new(pressed_str).size(56.0).color(
                                        egui::Color32::from_white_alpha(alpha)
                                            * ui.visuals().text_color(),
                                    ));
                                }
                            } else {
                                ui.label("");
                            }
                        });
                    });

                if !self.is_overlay {
                    let control_height = ui.style().spacing.interact_size.y;
                    let control_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            title_rect.left() + 100.0,
                            title_rect.top() + (TITLE_BAR_HEIGHT - control_height) / 2.0,
                        ),
                        egui::vec2(title_rect.width() - 200.0, control_height),
                    );

                    egui::Area::new(egui::Id::new("control"))
                        .fixed_pos(control_rect.min)
                        .default_size(control_rect.size())
                        .show(ui.ctx(), |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.is_outline, "Outline Text");
                                ui.checkbox(&mut self.is_show_mouse, "Show Mouse");

                                if ui.button("Overlay").clicked() {
                                    self.is_overlay = true;

                                    #[cfg(target_os = "windows")]
                                    if let Ok(handle) = frame.window_handle() {
                                        platform::enable_click_through_windows(&handle);
                                    }
                                }
                            });
                        });

                    let title_action_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            title_rect.right() - 28.0,
                            title_rect.top() + (TITLE_BAR_HEIGHT - control_height) / 2.0,
                        ),
                        egui::vec2(28.0, control_height),
                    );

                    egui::Area::new(egui::Id::new("title_action"))
                        .fixed_pos(title_action_rect.min)
                        .default_size(title_action_rect.size())
                        .show(ui.ctx(), |ui| {
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(control_height, control_height),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        if ui.button("×").clicked() {
                                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                        }
                                    },
                                );
                            });
                        });
                }
            });

        ctx.request_repaint();
    }
}

const ICON_DATA: &[u8] = include_bytes!("icon.bin");

fn main() -> eframe::Result<()> {
    let icon = egui::IconData {
        rgba: ICON_DATA.to_vec(),
        width: 512,
        height: 512,
    };

    let viewport = egui::ViewportBuilder::default()
        .with_icon(icon)
        .with_always_on_top()
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size(egui::vec2(640.0, 160.0));

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native("Keyboim", options, Box::new(|_cc| Ok(Box::new(App::new()))))
}
