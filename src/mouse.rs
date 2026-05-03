use crate::ui::{Color, Point, SceneBuilder};

pub fn append_mouse_icon(
    builder: &mut SceneBuilder,
    origin: Point,
    size: f32,
    mouse_buttons: [bool; 5],
    fill_color: Color,
    stroke_color: Color,
) {
    let orig_w = 320.0;
    let orig_h = 416.0;
    let pad = 4.0;
    let avail_w = size - 2.0 * pad;
    let avail_h = size - 2.0 * pad;
    let scale = (avail_w / orig_w).min(avail_h / orig_h);
    let offset_x = origin.x + (size - orig_w * scale) * 0.5 - 96.0 * scale;
    let offset_y = origin.y + (size - orig_h * scale) * 0.5 - 48.0 * scale;

    let transform = |x: f32, y: f32| Point {
        x: offset_x + x * scale,
        y: offset_y + y * scale,
    };

    if mouse_buttons[0] {
        builder.polygon(
            [
                (256.0, 48.0),
                (96.0, 128.0),
                (96.0, 256.0),
                (256.0, 300.0),
                (256.0, 238.0),
                (222.0, 238.0),
                (222.0, 110.0),
                (256.0, 110.0),
            ]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
            fill_color,
        );
    }

    if mouse_buttons[1] {
        builder.polygon(
            [
                (256.0, 48.0),
                (416.0, 128.0),
                (416.0, 256.0),
                (256.0, 300.0),
                (256.0, 238.0),
                (288.0, 238.0),
                (288.0, 110.0),
                (256.0, 110.0),
            ]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
            fill_color,
        );
    }

    if mouse_buttons[2] {
        builder.polygon(
            [
                (222.0, 110.0),
                (288.0, 110.0),
                (288.0, 238.0),
                (222.0, 238.0),
            ]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
            fill_color,
        );
    }

    let stroke_width = 2.0;

    builder.polyline(
        [
            (256.0, 48.0),
            (96.0, 128.0),
            (96.0, 360.0),
            (256.0, 464.0),
            (416.0, 360.0),
            (416.0, 128.0),
            (256.0, 48.0),
        ]
        .into_iter()
        .map(|(x, y)| transform(x, y))
        .collect(),
        stroke_color,
        stroke_width,
        false,
    );
    builder.polyline(
        [(96.0, 256.0), (256.0, 300.0), (416.0, 256.0)]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
        stroke_color,
        stroke_width,
        false,
    );
    builder.polyline(
        [(256.0, 48.0), (256.0, 110.0)]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
        stroke_color,
        stroke_width,
        false,
    );
    builder.polyline(
        [(256.0, 300.0), (256.0, 238.0)]
            .into_iter()
            .map(|(x, y)| transform(x, y))
            .collect(),
        stroke_color,
        stroke_width,
        false,
    );
    builder.polyline(
        [
            (222.0, 110.0),
            (288.0, 110.0),
            (288.0, 238.0),
            (222.0, 238.0),
            (222.0, 110.0),
        ]
        .into_iter()
        .map(|(x, y)| transform(x, y))
        .collect(),
        stroke_color,
        stroke_width,
        false,
    );
}
