use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use indexmap::IndexSet;
use windows::Win32::UI::WindowsAndMessaging::{
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::Window,
};

use crate::{
    automation::{
        compare_images, save_rgba_png, transparency_report, AutomationAction, AutomationConfig,
        AutomationMode, AutomationRunner,
    },
    key_hook::{self, is_disable_overlay_key_pressed},
    platform,
    renderer::{create_gl_window, GlWindow, PreparedScene, Renderer},
    ui::{self, Point, TextVisual, UiScene, ViewModel, WidgetId},
};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub automation: Option<AutomationConfig>,
    pub enable_hooks: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let mut automation = None;
        let mut dump_dir = PathBuf::from("target\\automation");
        let mut args = std::env::args().skip(1);

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--automation" => {
                    let mode = args.next().unwrap_or_else(|| "smoke".to_string());
                    let mode = match mode.as_str() {
                        "smoke" => AutomationMode::Smoke,
                        _ => AutomationMode::Smoke,
                    };
                    automation = Some(AutomationConfig {
                        mode,
                        dump_dir: dump_dir.clone(),
                    });
                }
                "--dump-dir" => {
                    if let Some(path) = args.next() {
                        dump_dir = PathBuf::from(path);
                        if let Some(config) = &mut automation {
                            config.dump_dir = dump_dir.clone();
                        }
                    }
                }
                _ => {}
            }
        }

        Self {
            enable_hooks: automation.is_none(),
            automation,
        }
    }
}

#[derive(Clone, Debug)]
enum UserEvent {
    Input(InputEvent),
}

#[derive(Clone, Debug)]
enum InputEvent {
    Key { vk: u32, message: u32 },
    MouseButton { index: usize, pressed: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SceneSignature {
    size: (u32, u32),
    scale_milli: u32,
    overlay: bool,
    show_mouse: bool,
    outline_text: bool,
    mouse_buttons: [bool; 5],
    text: Option<TextVisual>,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
}

struct CachedScene {
    signature: SceneSignature,
    scene: UiScene,
    prepared: PreparedScene,
}

struct AppRuntime {
    state: AppState,
    pointer: PointerState,
    scale_factor: f32,
    cached_scene: Option<CachedScene>,
    automation: Option<AutomationRunner>,
    suppress_platform_click_through: bool,
    pending_capture: Option<String>,
    pending_click_through: Option<bool>,
    last_tick: Instant,
}

#[derive(Default)]
struct PointerState {
    cursor: Option<Point>,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
}

struct AppState {
    pressed_keys: IndexSet<u32>,
    mouse_buttons: [bool; 5],
    last_combination: IndexSet<u32>,
    last_combination_text: String,
    key_cleared: bool,
    overlay: bool,
    show_mouse: bool,
    outline_text: bool,
    last_update: Instant,
    automation_preview_text: Option<String>,
    automation_preview_mouse: Option<[bool; 5]>,
    overlay_hint_until: Option<Instant>,
}

impl AppState {
    fn new(now: Instant) -> Self {
        Self {
            pressed_keys: IndexSet::new(),
            mouse_buttons: [false; 5],
            last_combination: IndexSet::new(),
            last_combination_text: String::new(),
            key_cleared: false,
            overlay: false,
            show_mouse: true,
            outline_text: true,
            last_update: now,
            automation_preview_text: None,
            automation_preview_mouse: None,
            overlay_hint_until: None,
        }
    }

    fn visible_text(&self, now: Instant) -> Option<TextVisual> {
        if let Some(preview) = &self.automation_preview_text {
            return Some(TextVisual {
                content: preview.clone(),
                alpha: 255,
                outlined: self.outline_text,
            });
        }

        if self.overlay
            && self.last_combination_text.is_empty()
            && self
                .overlay_hint_until
                .map(|until| until > now)
                .unwrap_or(false)
        {
            return Some(TextVisual {
                content: "Overlay enabled".to_string(),
                alpha: 255,
                outlined: self.outline_text,
            });
        }

        if self.last_combination_text.is_empty() {
            return None;
        }
        let elapsed = now.saturating_duration_since(self.last_update).as_secs_f32();
        let alpha = ((3.0 - elapsed).clamp(0.0, 1.0) * 255.0) as u8;
        if alpha == 0 {
            return None;
        }
        Some(TextVisual {
            content: self.last_combination_text.clone(),
            alpha,
            outlined: self.outline_text,
        })
    }

    fn rendered_mouse_buttons(&self) -> [bool; 5] {
        self.automation_preview_mouse.unwrap_or(self.mouse_buttons)
    }

    fn update_key_state(&mut self, vk: u32, message: u32, now: Instant) {
        match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                self.pressed_keys.insert(vk);
            }
            WM_KEYUP | WM_SYSKEYUP => {
                self.pressed_keys.swap_remove(&vk);
            }
            _ => {}
        }

        if self.pressed_keys.is_empty() {
            self.key_cleared = true;
            return;
        }

        if self.pressed_keys.len() > self.last_combination.len() || self.key_cleared {
            self.last_combination = self.pressed_keys.clone();
            let mut text_keys = self.last_combination.clone();
            self.last_combination_text = key_hook::key_combination_to_string(&mut text_keys);
            self.last_update = now;
            self.key_cleared = false;
        }
    }

    fn update_mouse_button(&mut self, index: usize, pressed: bool) {
        if let Some(button) = self.mouse_buttons.get_mut(index) {
            *button = pressed;
        }
    }
}

impl AppRuntime {
    fn new(config: AppConfig, scale_factor: f32) -> Result<Self> {
        let now = Instant::now();
        let suppress_platform_click_through = config.automation.is_some();
        Ok(Self {
            state: AppState::new(now),
            pointer: PointerState::default(),
            scale_factor,
            cached_scene: None,
            automation: match config.automation {
                Some(config) => Some(AutomationRunner::new(config)?),
                None => None,
            },
            suppress_platform_click_through,
            pending_capture: None,
            pending_click_through: None,
            last_tick: now,
        })
    }

    fn invalidate_scene(&mut self) {
        self.cached_scene = None;
    }

    fn handle_user_event(&mut self, event: UserEvent, _window: &Window) -> Result<bool> {
        match event {
            UserEvent::Input(event) => {
                let now = Instant::now();
                match event {
                    InputEvent::Key { vk, message } => {
                        self.state.update_key_state(vk, message, now);
                        if self.state.overlay
                            && is_disable_overlay_key_pressed(&self.state.pressed_keys)
                        {
                            self.state.overlay = false;
                            self.state.automation_preview_text = None;
                            self.state.automation_preview_mouse = None;
                            self.state.overlay_hint_until = None;
                            self.pending_click_through = Some(false);
                        }
                    }
                    InputEvent::MouseButton { index, pressed } => {
                        self.state.update_mouse_button(index, pressed);
                    }
                }
                self.invalidate_scene();
                Ok(true)
            }
        }
    }

    fn handle_cursor_moved(&mut self, position: Point) -> bool {
        self.pointer.cursor = Some(position);
        let hovered = self
            .cached_scene
            .as_ref()
            .and_then(|cached| ui::hit_test(&cached.scene, position));
        if hovered != self.pointer.hovered {
            self.pointer.hovered = hovered;
            self.invalidate_scene();
            return true;
        }
        false
    }

    fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        window: &Window,
    ) -> Result<MouseInputResult> {
        if button != MouseButton::Left {
            return Ok(MouseInputResult::Noop);
        }

        match state {
            ElementState::Pressed => {
                if let Some(hovered) = self.pointer.hovered {
                    if hovered == WidgetId::TitleBar {
                        let _ = window.drag_window();
                        return Ok(MouseInputResult::NeedsRedraw(false));
                    }
                    self.pointer.pressed = Some(hovered);
                    self.invalidate_scene();
                    return Ok(MouseInputResult::NeedsRedraw(false));
                }
            }
            ElementState::Released => {
                let pressed = self.pointer.pressed.take();
                let activated = if pressed.is_some() && pressed == self.pointer.hovered {
                    pressed
                } else {
                    None
                };
                if let Some(widget) = activated {
                    let close_requested = self.activate_widget(widget, window)?;
                    self.invalidate_scene();
                    return Ok(MouseInputResult::NeedsRedraw(close_requested));
                }
                self.invalidate_scene();
                return Ok(MouseInputResult::NeedsRedraw(false));
            }
        }

        Ok(MouseInputResult::Noop)
    }

    fn activate_widget(&mut self, widget: WidgetId, _window: &Window) -> Result<bool> {
        match widget {
            WidgetId::OutlineCheckbox => {
                self.state.outline_text = !self.state.outline_text;
                Ok(false)
            }
            WidgetId::ShowMouseCheckbox => {
                self.state.show_mouse = !self.state.show_mouse;
                Ok(false)
            }
            WidgetId::OverlayButton => {
                self.state.overlay = true;
                self.state.overlay_hint_until = Some(Instant::now() + Duration::from_millis(1400));
                self.pending_click_through = Some(true);
                Ok(false)
            }
            WidgetId::CloseButton => Ok(true),
            WidgetId::TitleBar => Ok(false),
        }
    }

    fn current_signature(&self, window: &Window) -> SceneSignature {
        SceneSignature {
            size: (window.inner_size().width, window.inner_size().height),
            scale_milli: (self.scale_factor * 1000.0) as u32,
            overlay: self.state.overlay,
            show_mouse: self.state.show_mouse,
            outline_text: self.state.outline_text,
            mouse_buttons: self.state.rendered_mouse_buttons(),
            text: self.state.visible_text(Instant::now()),
            hovered: self.pointer.hovered,
            pressed: self.pointer.pressed,
        }
    }

    fn ensure_scene<'a>(&'a mut self, renderer: &mut Renderer, window: &Window) -> Result<&'a CachedScene> {
        let signature = self.current_signature(window);
        let needs_rebuild = self
            .cached_scene
            .as_ref()
            .map(|cached| cached.signature != signature)
            .unwrap_or(true);
        if needs_rebuild {
            let scene = ui::build_scene(&ViewModel {
                physical_size: (window.inner_size().width, window.inner_size().height),
                scale_factor: self.scale_factor,
                overlay: self.state.overlay,
                show_mouse: self.state.show_mouse,
                outline_text: self.state.outline_text,
                mouse_buttons: self.state.rendered_mouse_buttons(),
                text: self.state.visible_text(Instant::now()),
                hovered: self.pointer.hovered,
                pressed: self.pointer.pressed,
            });
            let prepared = renderer.prepare_scene(&scene)?;
            self.cached_scene = Some(CachedScene {
                signature,
                scene,
                prepared,
            });
        }
        Ok(self.cached_scene.as_ref().expect("cached scene exists"))
    }

    fn render(&mut self, renderer: &mut Renderer, gl_window: &GlWindow) -> Result<()> {
        let prepared_size = {
            let cached = self.ensure_scene(renderer, &gl_window.window)?;
            renderer.render(&cached.prepared)?;
            cached.prepared.size
        };
        gl_window.swap_buffers()?;

        if let Some(name) = self.pending_capture.take() {
            let bytes = renderer.read_pixels(prepared_size);
            let path = self
                .automation
                .as_ref()
                .map(|automation| automation.capture_path(&name))
                .context("missing automation runner for capture")?;
            save_rgba_png(&path, prepared_size.0, prepared_size.1, bytes)?;
            if let Some(automation) = &mut self.automation {
                automation
                    .report_mut()
                    .push(format!("captured {} -> {}", name, path.display()));
                automation.flush_report()?;
            }
        }

        if let Some(enabled) = self.pending_click_through.take() {
            if self.suppress_platform_click_through {
                return Ok(());
            }
            platform::set_click_through(&gl_window.window, enabled)?;
        }

        Ok(())
    }

    fn click_widget(
        &mut self,
        renderer: &mut Renderer,
        window: &Window,
        widget: WidgetId,
    ) -> Result<bool> {
        let center = {
            let cached = self.ensure_scene(renderer, window)?;
            cached
                .scene
                .hit_regions
                .iter()
                .find(|region| region.id == widget)
                .map(|region| region.rect.center())
        };

        let Some(center) = center else {
            return Ok(false);
        };

        self.handle_cursor_moved(center);
        let pressed = self.handle_mouse_input(ElementState::Pressed, MouseButton::Left, window)?;
        let released =
            self.handle_mouse_input(ElementState::Released, MouseButton::Left, window)?;

        let pressed_close = matches!(pressed, MouseInputResult::NeedsRedraw(true));
        let released_close = matches!(released, MouseInputResult::NeedsRedraw(true));
        Ok(pressed_close || released_close)
    }

    fn update_automation(
        &mut self,
        renderer: &mut Renderer,
        window: &Window,
    ) -> Result<AutomationStepResult> {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        let Some(automation) = &mut self.automation else {
            return Ok(AutomationStepResult::default());
        };
        let actions = automation.tick(delta);
        let _ = automation;

        let mut result = AutomationStepResult::default();
        for action in actions {
            if let Some(automation) = self.automation.as_mut() {
                automation.report_mut().push(match &action {
                    AutomationAction::SetPreview { .. } => "action: set-preview".to_string(),
                    AutomationAction::Capture(name) => format!("action: capture-{name}"),
                    AutomationAction::CompareToReference { capture_name, .. } => {
                        format!("action: compare-{capture_name}")
                    }
                    AutomationAction::ClickWidget(widget) => format!("action: click-{widget:?}"),
                    AutomationAction::VerifyTransparency(name) => {
                        format!("action: verify-transparency-{name}")
                    }
                    AutomationAction::Exit => "action: exit".to_string(),
                });
                automation.flush_report()?;
            }

            match action {
                AutomationAction::SetPreview { text, mouse_buttons } => {
                    self.state.automation_preview_text = Some(text);
                    self.state.automation_preview_mouse = Some(mouse_buttons);
                    self.invalidate_scene();
                    result.request_redraw = true;
                }
                AutomationAction::Capture(name) => {
                    self.pending_capture = Some(name.to_string());
                    result.request_redraw = true;
                }
                AutomationAction::CompareToReference {
                    capture_name,
                    reference,
                } => {
                    let automation = self.automation.as_mut().expect("automation runner exists");
                    let message = compare_images(
                        &automation.capture_path(capture_name),
                        &reference,
                        &automation.diff_path(capture_name),
                    )?;
                    automation.report_mut().push(message);
                    automation.flush_report()?;
                }
                AutomationAction::ClickWidget(widget) => {
                    let should_close = self.click_widget(renderer, window, widget)?;
                    self.invalidate_scene();
                    result.request_redraw = true;
                    if should_close {
                        result.exit = true;
                    }
                }
                AutomationAction::VerifyTransparency(name) => {
                    let automation = self.automation.as_mut().expect("automation runner exists");
                    let message = transparency_report(&automation.capture_path(name))?;
                    automation.report_mut().push(message);
                    automation.flush_report()?;
                }
                AutomationAction::Exit => {
                    let automation = self.automation.as_mut().expect("automation runner exists");
                    automation.flush_report()?;
                    result.exit = true;
                }
            }
        }
        Ok(result)
    }

    fn needs_animation(&self) -> bool {
        self.state.visible_text(Instant::now()).is_some() && self.state.automation_preview_text.is_none()
    }

    fn has_pending_automation(&self) -> bool {
        self.automation.is_some()
    }
}

#[derive(Default)]
struct AutomationStepResult {
    request_redraw: bool,
    exit: bool,
}

enum MouseInputResult {
    Noop,
    NeedsRedraw(bool),
}

pub fn run(config: AppConfig) -> Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .context("creating event loop")?;
    let proxy = event_loop.create_proxy();
    let (gl_window, mut renderer) = create_gl_window(&event_loop)?;
    platform::configure_window(&gl_window.window)?;

    if config.enable_hooks {
        spawn_input_threads(proxy);
    }

    let mut runtime = AppRuntime::new(config, gl_window.window.scale_factor() as f32)?;
    let gl_window = gl_window;
    gl_window.window.request_redraw();

    #[allow(deprecated)]
    let run_result = event_loop.run(move |event, target| {
            target.set_control_flow(ControlFlow::Wait);

            match event {
                Event::UserEvent(event) => {
                    match runtime.handle_user_event(event, &gl_window.window) {
                        Ok(_) => gl_window.window.request_redraw(),
                        Err(error) => {
                            eprintln!("{error:#}");
                            target.exit();
                        }
                    }
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Resized(size) => {
                        gl_window.resize(size);
                        runtime.invalidate_scene();
                        gl_window.window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        runtime.scale_factor = scale_factor as f32;
                        runtime.invalidate_scene();
                        gl_window.window.request_redraw();
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        if runtime.handle_cursor_moved(Point {
                            x: position.x as f32,
                            y: position.y as f32,
                        }) {
                            gl_window.window.request_redraw();
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        match runtime.handle_mouse_input(state, button, &gl_window.window) {
                            Ok(MouseInputResult::NeedsRedraw(close)) => {
                                if close {
                                    target.exit();
                                } else {
                                    gl_window.window.request_redraw();
                                }
                            }
                            Ok(MouseInputResult::Noop) => {}
                            Err(error) => {
                                eprintln!("{error:#}");
                                target.exit();
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        if let Err(error) = runtime.render(&mut renderer, &gl_window) {
                            eprintln!("{error:#}");
                            target.exit();
                        }
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    match runtime.update_automation(&mut renderer, &gl_window.window) {
                        Ok(step) => {
                            if step.request_redraw {
                                gl_window.window.request_redraw();
                            }
                            if step.exit {
                                target.exit();
                            }
                        }
                        Err(error) => {
                            eprintln!("{error:#}");
                            target.exit();
                        }
                    }

                    if runtime.has_pending_automation() || runtime.needs_animation() {
                        target.set_control_flow(ControlFlow::WaitUntil(
                            Instant::now() + Duration::from_millis(16),
                        ));
                        if runtime.has_pending_automation() || runtime.needs_animation() {
                            gl_window.window.request_redraw();
                        }
                    }
                }
                _ => {}
            }
        });

    run_result.context("running event loop")
}

fn spawn_input_threads(proxy: EventLoopProxy<UserEvent>) {
    let keyboard_proxy = proxy.clone();
    thread::spawn(move || unsafe {
        key_hook::register_hook(move |vk, message| {
            let _ = keyboard_proxy.send_event(UserEvent::Input(InputEvent::Key { vk, message }));
        });
    });

    thread::spawn(move || unsafe {
        key_hook::register_mouse_hook(move |message, _x, _y, data| {
            let event = match message {
                WM_LBUTTONDOWN => Some(InputEvent::MouseButton {
                    index: 0,
                    pressed: true,
                }),
                WM_LBUTTONUP => Some(InputEvent::MouseButton {
                    index: 0,
                    pressed: false,
                }),
                WM_RBUTTONDOWN => Some(InputEvent::MouseButton {
                    index: 1,
                    pressed: true,
                }),
                WM_RBUTTONUP => Some(InputEvent::MouseButton {
                    index: 1,
                    pressed: false,
                }),
                WM_MBUTTONDOWN => Some(InputEvent::MouseButton {
                    index: 2,
                    pressed: true,
                }),
                WM_MBUTTONUP => Some(InputEvent::MouseButton {
                    index: 2,
                    pressed: false,
                }),
                WM_XBUTTONDOWN => Some(InputEvent::MouseButton {
                    index: decode_xbutton(data),
                    pressed: true,
                }),
                WM_XBUTTONUP => Some(InputEvent::MouseButton {
                    index: decode_xbutton(data),
                    pressed: false,
                }),
                _ => None,
            };

            if let Some(event) = event {
                let _ = proxy.send_event(UserEvent::Input(event));
            }
        });
    });
}

fn decode_xbutton(data: u32) -> usize {
    match (data >> 16) & 0xFFFF {
        1 => 3,
        2 => 4,
        _ => 3,
    }
}
