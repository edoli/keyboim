use crate::mouse;

pub const BASE_WINDOW_WIDTH: f32 = 640.0;
pub const BASE_WINDOW_HEIGHT: f32 = 160.0;
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
pub const TITLE_SIDE_PADDING: f32 = 10.0;
const BODY_CORNER_RADIUS: f32 = 4.0;
const TITLE_CORNER_RADIUS: f32 = 4.0;
const CONTROL_SPACING: f32 = 18.0;
const CHECKBOX_SIZE: f32 = 18.0;
const BUTTON_HEIGHT: f32 = 22.0;
const BUTTON_WIDTH: f32 = 76.0;
const CLOSE_BUTTON_SIZE: f32 = 22.0;
const CONTENT_PADDING: f32 = 10.0;
const MOUSE_ICON_SIZE: f32 = 64.0;
const TEXT_PADDING_LEFT: f32 = 14.0;
const CONTROL_Y_OFFSET: f32 = 5.0;
const CONTENT_TEXT_SIZE: f32 = 56.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub fn from_min_size(min: Point, size: Size) -> Self {
        Self {
            min,
            max: Point {
                x: min.x + size.width,
                y: min.y + size.height,
            },
        }
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn center(self) -> Point {
        Point {
            x: (self.min.x + self.max.x) * 0.5,
            y: (self.min.y + self.max.y) * 0.5,
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadii {
    pub const ZERO: Self = Self {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetId {
    TitleBar,
    OutlineCheckbox,
    ShowMouseCheckbox,
    OverlayButton,
    CloseButton,
}

#[derive(Clone, Debug)]
pub struct HitRegion {
    pub id: WidgetId,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub enum DrawCommand {
    RoundedRectFill {
        rect: Rect,
        radii: CornerRadii,
        color: Color,
    },
    RoundedRectStroke {
        rect: Rect,
        radii: CornerRadii,
        color: Color,
        width: f32,
    },
    Polygon {
        points: Vec<Point>,
        color: Color,
    },
    Polyline {
        points: Vec<Point>,
        color: Color,
        width: f32,
        closed: bool,
    },
    Text {
        position: Point,
        text: String,
        size: f32,
        color: Color,
    },
}

#[derive(Clone, Debug)]
pub struct UiScene {
    pub physical_size: (u32, u32),
    pub clear_color: Color,
    pub commands: Vec<DrawCommand>,
    pub hit_regions: Vec<HitRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub background_fill: Color,
    pub title_fill: Color,
    pub frame_stroke: Color,
    pub text: Color,
    pub weak_text: Color,
    pub control_fill: Color,
    pub control_fill_hover: Color,
    pub control_fill_active: Color,
    pub accent: Color,
    pub shadow: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_fill: Color::rgb(27, 27, 27),
            title_fill: Color::rgb(25, 26, 27),
            frame_stroke: Color::rgb(60, 60, 60),
            text: Color::rgb(230, 230, 230),
            weak_text: Color::rgb(140, 140, 140),
            control_fill: Color::rgb(36, 36, 36),
            control_fill_hover: Color::rgb(46, 46, 46),
            control_fill_active: Color::rgb(60, 60, 60),
            accent: Color::rgb(200, 200, 200),
            shadow: Color::rgb(0, 0, 0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextVisual {
    pub content: String,
    pub alpha: u8,
    pub outlined: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewModel {
    pub physical_size: (u32, u32),
    pub scale_factor: f32,
    pub overlay: bool,
    pub show_mouse: bool,
    pub outline_text: bool,
    pub mouse_buttons: [bool; 5],
    pub text: Option<TextVisual>,
    pub hovered: Option<WidgetId>,
    pub pressed: Option<WidgetId>,
}

pub fn hit_test(scene: &UiScene, point: Point) -> Option<WidgetId> {
    scene.hit_regions
        .iter()
        .rev()
        .find(|region| region.rect.contains(point))
        .map(|region| region.id)
}

pub fn build_scene(view: &ViewModel) -> UiScene {
    let mut builder = SceneBuilder::new(view.physical_size, view.scale_factor);
    let theme = Theme::default();
    let logical_window = builder.logical_window_rect();

    if !view.overlay {
        let title_rect = Rect::from_min_size(
            logical_window.min,
            Size {
                width: logical_window.width(),
                height: TITLE_BAR_HEIGHT,
            },
        );
        let body_rect = Rect::from_min_size(
            Point {
                x: logical_window.min.x,
                y: logical_window.min.y + TITLE_BAR_HEIGHT - 1.0,
            },
            Size {
                width: logical_window.width(),
                height: logical_window.height() - TITLE_BAR_HEIGHT,
            },
        );

        builder.push_hit_region(WidgetId::TitleBar, title_rect);
        builder.rounded_rect_fill(
            title_rect,
            CornerRadii {
                top_left: TITLE_CORNER_RADIUS,
                top_right: TITLE_CORNER_RADIUS,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
            theme.title_fill,
        );
        builder.rounded_rect_stroke(
            title_rect,
            CornerRadii {
                top_left: TITLE_CORNER_RADIUS,
                top_right: TITLE_CORNER_RADIUS,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
            theme.frame_stroke,
            1.0,
        );
        builder.rounded_rect_fill(
            body_rect,
            CornerRadii {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: BODY_CORNER_RADIUS,
                bottom_left: BODY_CORNER_RADIUS,
            },
            theme.background_fill,
        );
        builder.rounded_rect_stroke(
            body_rect,
            CornerRadii {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: BODY_CORNER_RADIUS,
                bottom_left: BODY_CORNER_RADIUS,
            },
            theme.frame_stroke,
            1.0,
        );
        builder.text(
            Point {
                x: TITLE_SIDE_PADDING,
                y: 7.0,
            },
            "Keyboim",
            16.0,
            theme.weak_text,
        );

        let control_top = title_rect.min.y + CONTROL_Y_OFFSET;
        let outline_rect = checkbox_layout_rect(100.0, control_top, 102.0, BUTTON_HEIGHT);
        let mouse_rect = checkbox_layout_rect(
            outline_rect.max.x + CONTROL_SPACING,
            control_top,
            96.0,
            BUTTON_HEIGHT,
        );
        let overlay_rect = Rect::from_min_size(
            Point {
                x: mouse_rect.max.x + CONTROL_SPACING,
                y: control_top,
            },
            Size {
                width: BUTTON_WIDTH,
                height: BUTTON_HEIGHT,
            },
        );
        let close_rect = Rect::from_min_size(
            Point {
                x: logical_window.max.x - 28.0,
                y: control_top,
            },
            Size {
                width: CLOSE_BUTTON_SIZE,
                height: CLOSE_BUTTON_SIZE,
            },
        );

        builder.checkbox(
            WidgetId::OutlineCheckbox,
            outline_rect,
            "Outline Text",
            view.outline_text,
            widget_state(view, WidgetId::OutlineCheckbox),
            theme,
        );
        builder.checkbox(
            WidgetId::ShowMouseCheckbox,
            mouse_rect,
            "Show Mouse",
            view.show_mouse,
            widget_state(view, WidgetId::ShowMouseCheckbox),
            theme,
        );
        builder.button(
            WidgetId::OverlayButton,
            overlay_rect,
            "Overlay",
            widget_state(view, WidgetId::OverlayButton),
            theme,
        );
        builder.close_button(
            WidgetId::CloseButton,
            close_rect,
            widget_state(view, WidgetId::CloseButton),
            theme,
        );
    }

    let content_origin = Point {
        x: CONTENT_PADDING,
        y: TITLE_BAR_HEIGHT + CONTENT_PADDING,
    };

    if view.show_mouse {
        mouse::append_mouse_icon(
            &mut builder,
            content_origin,
            MOUSE_ICON_SIZE,
            view.mouse_buttons,
            theme.weak_text,
            theme.text,
        );
    }

    if let Some(text) = &view.text {
        let text_color = theme.text.with_alpha(text.alpha);
        let shadow_color = theme.shadow.with_alpha(text.alpha / 4);
        let start = Point {
            x: content_origin.x + if view.show_mouse { MOUSE_ICON_SIZE + TEXT_PADDING_LEFT } else { 0.0 },
            y: content_origin.y - 2.0,
        };
        if text.outlined {
            for offset in [
                (-2.0, 0.0),
                (2.0, 0.0),
                (0.0, -2.0),
                (0.0, 2.0),
                (-1.4, -1.4),
                (1.4, -1.4),
                (-1.4, 1.4),
                (1.4, 1.4),
            ] {
                builder.text(
                    Point {
                        x: start.x + offset.0,
                        y: start.y + offset.1,
                    },
                    &text.content,
                    CONTENT_TEXT_SIZE,
                    shadow_color,
                );
            }
        }
        builder.text(start, &text.content, CONTENT_TEXT_SIZE, text_color);
    }

    builder.finish()
}

fn checkbox_layout_rect(x: f32, y: f32, label_width: f32, height: f32) -> Rect {
    Rect::from_min_size(
        Point { x, y },
        Size {
            width: CHECKBOX_SIZE + 8.0 + label_width,
            height,
        },
    )
}

fn widget_state(view: &ViewModel, id: WidgetId) -> WidgetVisualState {
    WidgetVisualState {
        hovered: view.hovered == Some(id),
        pressed: view.pressed == Some(id),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetVisualState {
    pub hovered: bool,
    pub pressed: bool,
}

pub struct SceneBuilder {
    scale_factor: f32,
    physical_size: (u32, u32),
    commands: Vec<DrawCommand>,
    hit_regions: Vec<HitRegion>,
}

impl SceneBuilder {
    pub fn new(physical_size: (u32, u32), scale_factor: f32) -> Self {
        Self {
            scale_factor,
            physical_size,
            commands: Vec::new(),
            hit_regions: Vec::new(),
        }
    }

    pub fn logical_window_rect(&self) -> Rect {
        Rect::from_min_size(
            Point { x: 0.0, y: 0.0 },
            Size {
                width: self.physical_size.0 as f32 / self.scale_factor,
                height: self.physical_size.1 as f32 / self.scale_factor,
            },
        )
    }

    pub fn finish(self) -> UiScene {
        UiScene {
            physical_size: self.physical_size,
            clear_color: Color::TRANSPARENT,
            commands: self.commands,
            hit_regions: self.hit_regions,
        }
    }

    pub fn push_hit_region(&mut self, id: WidgetId, rect: Rect) {
        self.hit_regions.push(HitRegion {
            id,
            rect: self.to_physical_rect(rect),
        });
    }

    pub fn rounded_rect_fill(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        self.commands.push(DrawCommand::RoundedRectFill {
            rect: self.to_physical_rect(rect),
            radii: self.to_physical_radii(radii),
            color,
        });
    }

    pub fn rounded_rect_stroke(
        &mut self,
        rect: Rect,
        radii: CornerRadii,
        color: Color,
        width: f32,
    ) {
        self.commands.push(DrawCommand::RoundedRectStroke {
            rect: self.to_physical_rect(rect),
            radii: self.to_physical_radii(radii),
            color,
            width: self.scale(width),
        });
    }

    pub fn polygon(&mut self, points: Vec<Point>, color: Color) {
        self.commands.push(DrawCommand::Polygon {
            points: points.into_iter().map(|point| self.to_physical_point(point)).collect(),
            color,
        });
    }

    pub fn polyline(&mut self, points: Vec<Point>, color: Color, width: f32, closed: bool) {
        self.commands.push(DrawCommand::Polyline {
            points: points.into_iter().map(|point| self.to_physical_point(point)).collect(),
            color,
            width: self.scale(width),
            closed,
        });
    }

    pub fn text(&mut self, position: Point, text: &str, size: f32, color: Color) {
        self.commands.push(DrawCommand::Text {
            position: self.to_physical_point(position),
            text: text.to_owned(),
            size: self.scale(size),
            color,
        });
    }

    pub fn checkbox(
        &mut self,
        id: WidgetId,
        rect: Rect,
        label: &str,
        checked: bool,
        state: WidgetVisualState,
        theme: Theme,
    ) {
        self.push_hit_region(id, rect);
        let square_rect = Rect::from_min_size(
            Point {
                x: rect.min.x,
                y: rect.min.y + (rect.height() - CHECKBOX_SIZE) * 0.5,
            },
            Size {
                width: CHECKBOX_SIZE,
                height: CHECKBOX_SIZE,
            },
        );
        let fill = if state.pressed {
            theme.control_fill_active
        } else if state.hovered {
            theme.control_fill_hover
        } else {
            theme.control_fill
        };
        self.rounded_rect_fill(square_rect, CornerRadii::ZERO, fill);
        self.rounded_rect_stroke(square_rect, CornerRadii::ZERO, theme.frame_stroke, 1.0);
        if checked {
            let mark = vec![
                Point {
                    x: square_rect.min.x + 3.5,
                    y: square_rect.min.y + 9.0,
                },
                Point {
                    x: square_rect.min.x + 7.5,
                    y: square_rect.min.y + 13.0,
                },
                Point {
                    x: square_rect.min.x + 14.0,
                    y: square_rect.min.y + 5.0,
                },
            ];
            self.polyline(mark, theme.accent, 2.0, false);
        }
        self.text(
            Point {
                x: rect.min.x + CHECKBOX_SIZE + 8.0,
                y: rect.min.y + 2.0,
            },
            label,
            14.0,
            theme.weak_text,
        );
    }

    pub fn button(
        &mut self,
        id: WidgetId,
        rect: Rect,
        label: &str,
        state: WidgetVisualState,
        theme: Theme,
    ) {
        self.push_hit_region(id, rect);
        let fill = if state.pressed {
            theme.control_fill_active
        } else if state.hovered {
            theme.control_fill_hover
        } else {
            theme.control_fill
        };
        self.rounded_rect_fill(
            rect,
            CornerRadii {
                top_left: 3.0,
                top_right: 3.0,
                bottom_right: 3.0,
                bottom_left: 3.0,
            },
            fill,
        );
        self.rounded_rect_stroke(
            rect,
            CornerRadii {
                top_left: 3.0,
                top_right: 3.0,
                bottom_right: 3.0,
                bottom_left: 3.0,
            },
            theme.frame_stroke,
            1.0,
        );
        self.text(
            Point {
                x: rect.min.x + 14.0,
                y: rect.min.y + 2.0,
            },
            label,
            14.0,
            theme.weak_text,
        );
    }

    pub fn close_button(
        &mut self,
        id: WidgetId,
        rect: Rect,
        state: WidgetVisualState,
        theme: Theme,
    ) {
        self.push_hit_region(id, rect);
        let fill = if state.pressed {
            theme.control_fill_active
        } else if state.hovered {
            theme.control_fill_hover
        } else {
            Color::TRANSPARENT
        };
        if fill.a > 0 {
            self.rounded_rect_fill(
                rect,
                CornerRadii {
                    top_left: 3.0,
                    top_right: 3.0,
                    bottom_right: 3.0,
                    bottom_left: 3.0,
                },
                fill,
            );
        }
        self.text(
            Point {
                x: rect.min.x + 7.0,
                y: rect.min.y + 0.0,
            },
            "\u{00D7}",
            16.0,
            theme.weak_text,
        );
    }

    fn scale(&self, value: f32) -> f32 {
        value * self.scale_factor
    }

    fn to_physical_point(&self, point: Point) -> Point {
        Point {
            x: self.scale(point.x),
            y: self.scale(point.y),
        }
    }

    fn to_physical_rect(&self, rect: Rect) -> Rect {
        Rect {
            min: self.to_physical_point(rect.min),
            max: self.to_physical_point(rect.max),
        }
    }

    fn to_physical_radii(&self, radii: CornerRadii) -> CornerRadii {
        CornerRadii {
            top_left: self.scale(radii.top_left),
            top_right: self.scale(radii.top_right),
            bottom_right: self.scale(radii.bottom_right),
            bottom_left: self.scale(radii.bottom_left),
        }
    }
}
