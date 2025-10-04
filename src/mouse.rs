// Simple ear-clipping triangulation for a simple (non self-intersecting) polygon.
// Returns indices into the points slice (triplets). Ensures counter-clockwise processing.
fn triangulate_polygon(points: &[egui::Pos2]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }

    // Compute signed area to determine winding (shoelace)
    let area: f32 = points.iter().enumerate().fold(0.0, |acc, (i, p)| {
        let q = points[(i + 1) % n];
        acc + (p.x * q.y - q.x * p.y)
    });
    let ccw = area > 0.0;

    // Work list of vertex indices in current polygon
    let mut v: Vec<usize> = if ccw {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };
    let mut triangles = Vec::with_capacity(n - 2);

    // Helpers
    let is_point_in_triangle =
        |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2, p: egui::Pos2| -> bool {
            // Barycentric technique
            let v0 = egui::vec2(c.x - a.x, c.y - a.y);
            let v1 = egui::vec2(b.x - a.x, b.y - a.y);
            let v2 = egui::vec2(p.x - a.x, p.y - a.y);
            let den = v0.x * v1.y - v1.x * v0.y;
            if den.abs() < 1e-6 {
                return false;
            }
            let u = (v2.x * v1.y - v1.x * v2.y) / den;
            let v_ = (v0.x * v2.y - v2.x * v0.y) / den;
            u >= 0.0 && v_ >= 0.0 && (u + v_) <= 1.0
        };

    let is_convex = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| -> bool {
        let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        cross > 0.0 // because we enforced CCW order
    };

    let mut guard = 0; // prevent infinite loop on malformed input
    while v.len() > 3 && guard < 10_000 {
        guard += 1;
        let m = v.len();
        let mut ear_found = false;
        for i in 0..m {
            let prev = v[(i + m - 1) % m];
            let curr = v[i];
            let next = v[(i + 1) % m];
            let a = points[prev];
            let b = points[curr];
            let c = points[next];
            if !is_convex(a, b, c) {
                continue;
            }
            // Check if any other point lies inside triangle abc
            let mut any_inside = false;
            for &other in &v {
                if other == prev || other == curr || other == next {
                    continue;
                }
                if is_point_in_triangle(a, b, c, points[other]) {
                    any_inside = true;
                    break;
                }
            }
            if any_inside {
                continue;
            }
            // It's an ear
            triangles.push([prev, curr, next]);
            v.remove(i);
            ear_found = true;
            break;
        }
        if !ear_found {
            break;
        } // fallback abort
    }
    if v.len() == 3 {
        triangles.push([v[0], v[1], v[2]]);
    }
    triangles
}

fn fill_nonconvex(painter: &egui::Painter, pts: Vec<egui::Pos2>, color: egui::Color32) {
    if pts.len() < 3 {
        return;
    }
    let tris = triangulate_polygon(&pts);
    let mut mesh = egui::epaint::Mesh::default();
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    for [a, b, c] in tris {
        mesh.indices
            .extend_from_slice(&[a as u32, b as u32, c as u32]);
    }
    painter.add(egui::Shape::Mesh(mesh.into()));
}

pub fn draw_mouse(ui: &mut egui::Ui, mouse_buttons: &[bool; 5]) {
    use egui::{Pos2, Stroke};

    // Allocate a square-ish area (can adjust if needed)
    let desired_size = egui::vec2(64.0, 64.0);

    let (_id, rect) = ui.allocate_space(desired_size);
    let painter = ui.painter_at(rect);

    // Original SVG coordinate bounds:
    // x: 96 .. 416 (width 320)
    // y: 48 .. 464 (height 416)
    let orig_w = 320.0f32;
    let orig_h = 416.0f32;

    // Compute uniform scale to fit inside rect with a little padding
    let pad = 4.0;
    let avail_w = rect.width() - 2.0 * pad;
    let avail_h = rect.height() - 2.0 * pad;
    // Choose uniform scale preserving aspect ratio
    let scale_x = avail_w / orig_w;
    let scale_y = avail_h / orig_h;
    let scale = scale_x.min(scale_y);

    // Top-left of original coordinate system mapped into rect
    let offset_x = rect.left() + (rect.width() - orig_w * scale) * 0.5 - 96.0 * scale; // subtract min x * scale
    let offset_y = rect.top() + (rect.height() - orig_h * scale) * 0.5 - 48.0 * scale; // subtract min y * scale

    let transform = |x: f32, y: f32| -> Pos2 {
        Pos2 {
            x: offset_x + x * scale,
            y: offset_y + y * scale,
        }
    };

    // Colors
    let fill_color = ui.style().visuals.weak_text_color();
    let stroke_color = ui.style().visuals.text_color();
    let stroke_width = (16.0 * scale).clamp(1.5, 4.0); // keep stroke readable at small sizes
    let stroke = Stroke::new(stroke_width, stroke_color);

    // --- FILL SHAPES (draw only if pressed) ---
    // Mapping: mouse_buttons[0]=left, [1]=right, [2]=middle
    // Left button polygon (possibly concave) -> triangulate
    if mouse_buttons[0] {
        let pts_raw = [
            (256.0, 48.0),
            (96.0, 128.0),
            (96.0, 256.0),
            (256.0, 300.0),
            (256.0, 238.0),
            (222.0, 238.0),
            (222.0, 110.0),
            (256.0, 110.0),
        ];
        let pts: Vec<Pos2> = pts_raw.into_iter().map(|(x, y)| transform(x, y)).collect();
        fill_nonconvex(&painter, pts, fill_color);
    }

    // Right button polygon -> triangulate
    if mouse_buttons[1] {
        let pts_raw = [
            (256.0, 48.0),
            (416.0, 128.0),
            (416.0, 256.0),
            (256.0, 300.0),
            (256.0, 238.0),
            (288.0, 238.0),
            (288.0, 110.0),
            (256.0, 110.0),
        ];
        let pts: Vec<Pos2> = pts_raw.into_iter().map(|(x, y)| transform(x, y)).collect();
        fill_nonconvex(&painter, pts, fill_color);
    }

    // Middle button rectangle (still convex, but reuse mesh path for consistency)
    if mouse_buttons[2] {
        let pts_raw = [
            (222.0, 110.0),
            (288.0, 110.0),
            (288.0, 238.0),
            (222.0, 238.0),
        ];
        let pts: Vec<Pos2> = pts_raw.into_iter().map(|(x, y)| transform(x, y)).collect();
        fill_nonconvex(&painter, pts, fill_color);
    }

    // --- STROKES (always drawn) ---
    // Outer outline
    let outline_pts = [
        (256.0, 48.0),
        (96.0, 128.0),
        (96.0, 360.0),
        (256.0, 464.0),
        (416.0, 360.0),
        (416.0, 128.0),
        (256.0, 48.0),
    ];
    let outline: Vec<Pos2> = outline_pts
        .into_iter()
        .map(|(x, y)| transform(x, y))
        .collect();
    painter.add(egui::Shape::line(outline, stroke));

    // Horizontal-ish internal poly (the "shoulder" line)
    let shoulder_pts = [(96.0, 256.0), (256.0, 300.0), (416.0, 256.0)];
    let shoulder: Vec<Pos2> = shoulder_pts
        .into_iter()
        .map(|(x, y)| transform(x, y))
        .collect();
    painter.add(egui::Shape::line(shoulder, stroke));

    // Top middle vertical (between top and start of wheel area)
    painter.line_segment([transform(256.0, 48.0), transform(256.0, 110.0)], stroke);

    // Bottom middle vertical (below wheel area)
    painter.line_segment([transform(256.0, 300.0), transform(256.0, 238.0)], stroke);

    // Rectangle around middle button
    let rect_pts = [
        (222.0, 110.0),
        (288.0, 110.0),
        (288.0, 238.0),
        (222.0, 238.0),
        (222.0, 110.0),
    ];
    let rect_path: Vec<Pos2> = rect_pts.into_iter().map(|(x, y)| transform(x, y)).collect();
    painter.add(egui::Shape::line(rect_path, stroke));
}
