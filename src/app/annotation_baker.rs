//! Software annotation baker — rasterizes pen, arrow, rect, ellipse, text, blur, and step badges
//! directly onto an `image::DynamicImage` RGBA pixel buffer.

use eframe::egui::{Color32, Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationTool {
    #[default]
    Pen,
    Arrow,
    Rectangle,
    Ellipse,
    Highlight,
    Text,
    Blur,
    StepBadge,
    /// Dim everything outside the dragged rect (E29/E109).
    Spotlight,
    /// Drag a line; bakes the segment + a "N px" label (E43/E110).
    Measure,
    /// Pasted bitmap anchored at `points[0]` (E119) — not a draw tool.
    Sticker,
}

/// Pasted bitmap layer (E119): pixels for baking + the uploaded texture for
/// the live preview. Both halves are Arc-backed so undo snapshots stay cheap.
#[derive(Clone)]
pub struct Sticker {
    pub rgba: std::sync::Arc<image::RgbaImage>,
    pub tex: eframe::egui::TextureHandle,
}

#[derive(Clone)]
pub struct AnnotationAction {
    pub tool: AnnotationTool,
    pub color: Color32,
    pub stroke_width: f32,
    pub points: Vec<Pos2>,
    pub text_content: String,
    pub badge_number: usize,
    /// E30 — badge look: 0 filled circle, 1 outline circle, 2 filled square,
    /// 3 outline square.
    pub badge_style: u8,
    pub sticker: Option<Sticker>,
}

impl std::fmt::Debug for AnnotationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnnotationAction")
            .field("tool", &self.tool)
            .field("color", &self.color)
            .field("stroke_width", &self.stroke_width)
            .field("points", &self.points)
            .field("text_content", &self.text_content)
            .field("badge_number", &self.badge_number)
            .field("badge_style", &self.badge_style)
            .field(
                "sticker",
                &self
                    .sticker
                    .as_ref()
                    .map(|s| format!("{}×{}", s.rgba.width(), s.rgba.height())),
            )
            .finish()
    }
}

/// Software rasterize all annotation shapes onto `img` based on normalized canvas coordinates.
pub fn bake_annotations(
    img: &mut image::DynamicImage,
    actions: &[AnnotationAction],
    canvas_rect: Option<Rect>,
) {
    if actions.is_empty() {
        return;
    }
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let mut rgba = img.to_rgba8();

    let rect = canvas_rect.unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::new(iw, ih)));
    let cw = rect.width().max(1.0);
    let ch = rect.height().max(1.0);

    let map_pos = |p: Pos2| -> (i32, i32) {
        let u = ((p.x - rect.min.x) / cw).clamp(0.0, 1.0);
        let v = ((p.y - rect.min.y) / ch).clamp(0.0, 1.0);
        ((u * iw) as i32, (v * ih) as i32)
    };

    for action in actions {
        if action.points.is_empty() {
            continue;
        }

        let color = action.color;
        let stroke_px = (action.stroke_width * (iw / cw)).max(1.5);

        match action.tool {
            AnnotationTool::Pen | AnnotationTool::Highlight => {
                let alpha = if action.tool == AnnotationTool::Highlight {
                    0.4
                } else {
                    (color.a() as f32) / 255.0
                };
                let draw_color =
                    image::Rgba([color.r(), color.g(), color.b(), (alpha * 255.0) as u8]);

                for i in 1..action.points.len() {
                    let (x0, y0) = map_pos(action.points[i - 1]);
                    let (x1, y1) = map_pos(action.points[i]);
                    draw_line_thick(&mut rgba, x0, y0, x1, y1, stroke_px, draw_color);
                }
            }
            AnnotationTool::Arrow => {
                let draw_color = image::Rgba([color.r(), color.g(), color.b(), color.a()]);
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    draw_line_thick(&mut rgba, x0, y0, x1, y1, stroke_px, draw_color);

                    let angle = ((y1 - y0) as f32).atan2((x1 - x0) as f32);
                    let arrow_len = (stroke_px * 3.5).max(18.0);
                    let a1_x = (x1 as f32 - arrow_len * (angle - 0.45).cos()) as i32;
                    let a1_y = (y1 as f32 - arrow_len * (angle - 0.45).sin()) as i32;
                    let a2_x = (x1 as f32 - arrow_len * (angle + 0.45).cos()) as i32;
                    let a2_y = (y1 as f32 - arrow_len * (angle + 0.45).sin()) as i32;

                    draw_line_thick(&mut rgba, x1, y1, a1_x, a1_y, stroke_px, draw_color);
                    draw_line_thick(&mut rgba, x1, y1, a2_x, a2_y, stroke_px, draw_color);
                }
            }
            AnnotationTool::Rectangle => {
                let draw_color = image::Rgba([color.r(), color.g(), color.b(), color.a()]);
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    let min_x = x0.min(x1);
                    let max_x = x0.max(x1);
                    let min_y = y0.min(y1);
                    let max_y = y0.max(y1);

                    draw_line_thick(&mut rgba, min_x, min_y, max_x, min_y, stroke_px, draw_color);
                    draw_line_thick(&mut rgba, max_x, min_y, max_x, max_y, stroke_px, draw_color);
                    draw_line_thick(&mut rgba, max_x, max_y, min_x, max_y, stroke_px, draw_color);
                    draw_line_thick(&mut rgba, min_x, max_y, min_x, min_y, stroke_px, draw_color);
                }
            }
            AnnotationTool::Ellipse => {
                let draw_color = image::Rgba([color.r(), color.g(), color.b(), color.a()]);
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    draw_ellipse_outline(&mut rgba, x0, y0, x1, y1, stroke_px, draw_color);
                }
            }
            AnnotationTool::Blur => {
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    let min_x = (x0.min(x1) as u32).min(rgba.width());
                    let max_x = (x0.max(x1) as u32).min(rgba.width());
                    let min_y = (y0.min(y1) as u32).min(rgba.height());
                    let max_y = (y0.max(y1) as u32).min(rgba.height());

                    let block_size = ((max_x - min_x) / 16).max(12).min(36);
                    pixelate_rect(&mut rgba, min_x, max_x, min_y, max_y, block_size);
                }
            }
            AnnotationTool::StepBadge => {
                let (cx, cy) = map_pos(action.points[0]);
                let badge_r = (16.0 * (iw / cw)).max(14.0) as i32;
                let bg_color = image::Rgba([color.r(), color.g(), color.b(), 255]);
                let edge = stroke_px.max(3.0);
                // E30 — style presets; outline styles draw the number in the
                // badge color, filled styles keep black on the fill.
                let num_color = match action.badge_style {
                    1 | 3 => bg_color,
                    _ => image::Rgba([0, 0, 0, 255]),
                };
                match action.badge_style {
                    1 => stroke_circle(&mut rgba, cx, cy, badge_r, edge, bg_color),
                    2 => fill_rect(
                        &mut rgba,
                        cx - badge_r,
                        cy - badge_r,
                        cx + badge_r,
                        cy + badge_r,
                        bg_color,
                    ),
                    3 => stroke_rect(
                        &mut rgba,
                        cx - badge_r,
                        cy - badge_r,
                        cx + badge_r,
                        cy + badge_r,
                        edge,
                        bg_color,
                    ),
                    _ => fill_circle(&mut rgba, cx, cy, badge_r, bg_color),
                }
                draw_badge_number(&mut rgba, cx, cy, action.badge_number, badge_r, num_color);
            }
            AnnotationTool::Spotlight => {
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    let min_x = x0.min(x1).clamp(0, rgba.width() as i32) as u32;
                    let max_x = x0.max(x1).clamp(0, rgba.width() as i32) as u32;
                    let min_y = y0.min(y1).clamp(0, rgba.height() as i32) as u32;
                    let max_y = y0.max(y1).clamp(0, rgba.height() as i32) as u32;
                    darken_outside(&mut rgba, min_x, max_x, min_y, max_y, 0.45);
                }
            }
            AnnotationTool::Measure => {
                let draw_color = image::Rgba([color.r(), color.g(), color.b(), color.a()]);
                if action.points.len() >= 2 {
                    let (x0, y0) = map_pos(action.points[0]);
                    let (x1, y1) = map_pos(*action.points.last().unwrap());
                    draw_line_thick(&mut rgba, x0, y0, x1, y1, stroke_px, draw_color);
                    let dist = ((x1 - x0).pow(2) + (y1 - y0).pow(2)) as f32;
                    let label = format!("{} px", dist.sqrt() as i32);
                    draw_text_box(&mut rgba, x1 + 8, y1 - 20, &label, color, iw / cw);
                }
            }
            AnnotationTool::Text => {
                let (x, y) = map_pos(action.points[0]);
                draw_text_box(&mut rgba, x, y, &action.text_content, color, iw / cw);
            }
            AnnotationTool::Sticker => {
                // Paste lands at native pixel size — canvas↔image scale only
                // moves the anchor, matching how text badges bake.
                if let Some(sticker) = &action.sticker {
                    let (x, y) = map_pos(action.points[0]);
                    image::imageops::overlay(&mut rgba, sticker.rgba.as_ref(), x as i64, y as i64);
                }
            }
        }
    }

    *img = image::DynamicImage::ImageRgba8(rgba);
}

/// E35 — export-time edge treatment applied post-bake: `fx` 0 = none,
/// 1 = solid border frame, 2 = soft drop shadow, 3 = torn edge. `px` is
/// border width, shadow blur radius, or tear depth.
pub fn apply_edge_fx(
    img: image::DynamicImage,
    fx: u8,
    px: u32,
    color: Color32,
) -> image::DynamicImage {
    let px = px.max(1);
    let (w, h) = (img.width(), img.height());
    match fx {
        1 => {
            let mut canvas = image::RgbaImage::from_pixel(
                w + px * 2,
                h + px * 2,
                image::Rgba([color.r(), color.g(), color.b(), 255]),
            );
            image::imageops::overlay(&mut canvas, &img.to_rgba8(), px.into(), px.into());
            image::DynamicImage::ImageRgba8(canvas)
        }
        2 => {
            let blur = px as f32;
            let off = (px / 2).max(2);
            let m = px * 2; // blur margin around all edges
            let mut layer = image::RgbaImage::from_pixel(
                w + m * 2 + off,
                h + m * 2 + off,
                image::Rgba([0, 0, 0, 0]),
            );
            for py in (m + off)..(m + off + h) {
                for px_i in (m + off)..(m + off + w) {
                    layer.put_pixel(px_i, py, image::Rgba([0, 0, 0, 150]));
                }
            }
            let shadow = image::imageops::blur(&layer, blur);
            let mut out = shadow;
            image::imageops::overlay(&mut out, &img.to_rgba8(), m.into(), m.into());
            image::DynamicImage::ImageRgba8(out)
        }
        3 => {
            // Torn edge: image sits on a transparent mat; each edge raggedly
            // recedes by a deterministic noise depth into the image.
            let depth = (px / 2).max(3).min(24);
            let mut rgba = img.to_rgba8();
            // Hash-based noise: smooth-ish jag per column/row, deterministic
            // so preview and export agree.
            let jag = |i: u32| -> u32 {
                let x = i.wrapping_mul(2654435761).rotate_left(13);
                (x >> 16) % (depth * 2).max(1)
            };
            for x in 0..w {
                let top = jag(x).min(depth);
                let bot = jag(x.wrapping_add(0x9E3779B9)).min(depth);
                for y in 0..top.min(h) {
                    rgba.get_pixel_mut(x, y).0[3] = 0;
                }
                for y in (h.saturating_sub(bot))..h {
                    rgba.get_pixel_mut(x, y).0[3] = 0;
                }
            }
            for y in 0..h {
                let left = jag(y.wrapping_add(0x85EBCA6B)).min(depth);
                let right = jag(y.wrapping_add(0xC2B2AE35)).min(depth);
                for x in 0..left.min(w) {
                    rgba.get_pixel_mut(x, y).0[3] = 0;
                }
                for x in (w.saturating_sub(right))..w {
                    rgba.get_pixel_mut(x, y).0[3] = 0;
                }
            }
            image::DynamicImage::ImageRgba8(rgba)
        }
        _ => img,
    }
}

/// Shift-drag snap: arrows lock to 15° angles, rectangles/blur become squares.
pub fn snap_annotation_point(tool: AnnotationTool, start: Pos2, pos: Pos2) -> Pos2 {
    let d = pos - start;
    match tool {
        AnnotationTool::Arrow => {
            let len = d.length();
            if len < 1.0 {
                return pos;
            }
            let step = std::f32::consts::PI / 12.0; // 15°
            let ang = (d.y.atan2(d.x) / step).round() * step;
            start + Vec2::angled(ang) * len
        }
        AnnotationTool::Rectangle
        | AnnotationTool::Blur
        | AnnotationTool::Ellipse
        | AnnotationTool::Spotlight => {
            let s = d.x.abs().max(d.y.abs());
            start + Vec2::new(d.x.signum() * s, d.y.signum() * s)
        }
        _ => pos,
    }
}

/// Collapse a near-straight freehand stroke to its two endpoints (E32).
/// Returns true when the stroke was straightened. Deviation threshold:
/// 6 % of the chord length, min 4 px, chord itself must be ≥ 24 px.
pub fn straighten_if_near_line(points: &mut Vec<Pos2>) -> bool {
    if points.len() < 4 {
        return false;
    }
    let (a, b) = (points[0], *points.last().unwrap());
    let chord = b - a;
    let len = chord.length();
    if len < 24.0 {
        return false;
    }
    let max_dev = points[1..points.len() - 1]
        .iter()
        .map(|p| {
            // Perpendicular distance from p to the a→b line.
            ((p.x - a.x) * chord.y - (p.y - a.y) * chord.x).abs() / len
        })
        .fold(0.0_f32, f32::max);
    if max_dev <= (len * 0.06).max(4.0) {
        *points = vec![a, b];
        true
    } else {
        false
    }
}

/// Multiply every pixel outside `[min_x,max_x) × [min_y,max_y)` by `factor`
/// — the spotlight bake (E29).
fn darken_outside(
    rgba: &mut image::RgbaImage,
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    factor: f32,
) {
    let (w, h) = (rgba.width(), rgba.height());
    for y in 0..h {
        for x in 0..w {
            if x < min_x || x >= max_x || y < min_y || y >= max_y {
                let p = rgba.get_pixel_mut(x, y);
                p.0[0] = (p.0[0] as f32 * factor) as u8;
                p.0[1] = (p.0[1] as f32 * factor) as u8;
                p.0[2] = (p.0[2] as f32 * factor) as u8;
            }
        }
    }
}

/// After deleting a badge, keep remaining numbers contiguous from 1.
pub fn renumber_step_badges(actions: &mut [AnnotationAction]) -> usize {
    let mut n = 1usize;
    for action in actions {
        if action.tool == AnnotationTool::StepBadge {
            action.badge_number = n;
            n += 1;
        }
    }
    n
}

fn draw_line_thick(
    rgba: &mut image::RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: f32,
    color: image::Rgba<u8>,
) {
    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let distance = (dx * dx + dy * dy).sqrt();
    let steps = (distance.ceil() as i32).max(1);
    let radius = (thickness / 2.0).max(1.0) as i32;

    for step in 0..=steps {
        let t = (step as f32) / (steps as f32);
        let cx = (x0 as f32 + t * dx) as i32;
        let cy = (y0 as f32 + t * dy) as i32;
        fill_circle(rgba, cx, cy, radius, color);
    }
}

/// Ellipse outline between two corner points, drawn as a thick polyline
/// (64 segments — consistent with the other tools' rasterized look).
fn draw_ellipse_outline(
    rgba: &mut image::RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: f32,
    color: image::Rgba<u8>,
) {
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let rx = ((x1 - x0).abs() as f32 / 2.0).max(1.0);
    let ry = ((y1 - y0).abs() as f32 / 2.0).max(1.0);
    const SEGMENTS: usize = 64;
    for i in 0..SEGMENTS {
        let t0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let t1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        draw_line_thick(
            rgba,
            (cx + rx * t0.cos()) as i32,
            (cy + ry * t0.sin()) as i32,
            (cx + rx * t1.cos()) as i32,
            (cy + ry * t1.sin()) as i32,
            thickness,
            color,
        );
    }
}

fn fill_circle(rgba: &mut image::RgbaImage, cx: i32, cy: i32, radius: i32, color: image::Rgba<u8>) {
    let w = rgba.width() as i32;
    let h = rgba.height() as i32;
    let r2 = radius * radius;

    let min_x = (cx - radius).clamp(0, w - 1);
    let max_x = (cx + radius).clamp(0, w - 1);
    let min_y = (cy - radius).clamp(0, h - 1);
    let max_y = (cy + radius).clamp(0, h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                blend_pixel(rgba, x as u32, y as u32, color);
            }
        }
    }
}

fn blend_pixel(rgba: &mut image::RgbaImage, x: u32, y: u32, src: image::Rgba<u8>) {
    let dst = rgba.get_pixel(x, y);
    if src[3] == 255 {
        rgba.put_pixel(x, y, src);
    } else if src[3] > 0 {
        let a = (src[3] as f32) / 255.0;
        let r = (src[0] as f32 * a + dst[0] as f32 * (1.0 - a)) as u8;
        let g = (src[1] as f32 * a + dst[1] as f32 * (1.0 - a)) as u8;
        let b = (src[2] as f32 * a + dst[2] as f32 * (1.0 - a)) as u8;
        rgba.put_pixel(x, y, image::Rgba([r, g, b, 255]));
    }
}

fn pixelate_rect(
    rgba: &mut image::RgbaImage,
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    block_size: u32,
) {
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let block = block_size.max(4);

    let mut by = min_y;
    while by < max_y {
        let ey = (by + block).min(max_y);
        let mut bx = min_x;
        while bx < max_x {
            let ex = (bx + block).min(max_x);

            let mut r_sum = 0u64;
            let mut g_sum = 0u64;
            let mut b_sum = 0u64;
            let mut count = 0u64;

            for y in by..ey {
                for x in bx..ex {
                    let px = rgba.get_pixel(x, y);
                    r_sum += px[0] as u64;
                    g_sum += px[1] as u64;
                    b_sum += px[2] as u64;
                    count += 1;
                }
            }

            if count > 0 {
                let avg = image::Rgba([
                    (r_sum / count) as u8,
                    (g_sum / count) as u8,
                    (b_sum / count) as u8,
                    255,
                ]);

                for y in by..ey {
                    for x in bx..ex {
                        rgba.put_pixel(x, y, avg);
                    }
                }
            }

            bx = ex;
        }
        by = ey;
    }
}

fn draw_badge_number(
    rgba: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    num: usize,
    badge_r: i32,
    num_color: image::Rgba<u8>,
) {
    let text = num.to_string();
    let font_scale = (badge_r as f32 / 12.0).max(1.0);
    draw_simple_text(
        rgba,
        cx - (text.len() as i32 * 4 * font_scale as i32),
        cy - (4.0 * font_scale) as i32,
        &text,
        num_color,
        font_scale,
    );
}

/// E30 — filled axis-aligned rect.
fn fill_rect(
    rgba: &mut image::RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: image::Rgba<u8>,
) {
    let (w, h) = (rgba.width() as i32, rgba.height() as i32);
    for py in y0.clamp(0, h)..y1.clamp(0, h) {
        for px in x0.clamp(0, w)..x1.clamp(0, w) {
            blend_pixel(rgba, px as u32, py as u32, color);
        }
    }
}

/// E30 — rect outline via four thick edges.
fn stroke_rect(
    rgba: &mut image::RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: f32,
    color: image::Rgba<u8>,
) {
    draw_line_thick(rgba, x0, y0, x1, y0, thickness, color);
    draw_line_thick(rgba, x1, y0, x1, y1, thickness, color);
    draw_line_thick(rgba, x1, y1, x0, y1, thickness, color);
    draw_line_thick(rgba, x0, y1, x0, y0, thickness, color);
}

/// E30 — circle outline as a thick 64-segment polyline.
fn stroke_circle(
    rgba: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    radius: i32,
    thickness: f32,
    color: image::Rgba<u8>,
) {
    const SEGMENTS: usize = 64;
    let r = radius as f32;
    for i in 0..SEGMENTS {
        let t0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let t1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        draw_line_thick(
            rgba,
            (cx as f32 + r * t0.cos()) as i32,
            (cy as f32 + r * t0.sin()) as i32,
            (cx as f32 + r * t1.cos()) as i32,
            (cy as f32 + r * t1.sin()) as i32,
            thickness,
            color,
        );
    }
}

fn draw_text_box(
    rgba: &mut image::RgbaImage,
    x: i32,
    y: i32,
    text: &str,
    color: Color32,
    scale: f32,
) {
    let font_scale = scale.clamp(1.0, 3.0);
    let text_w = text.len() as i32 * 8 * font_scale as i32;
    let text_h = 16 * font_scale as i32;

    // Background pill
    let bg_color = image::Rgba([20, 20, 24, 220]);
    let min_x = (x - 6).max(0) as u32;
    let max_x = (x + text_w + 6).min(rgba.width() as i32) as u32;
    let min_y = (y - 4).max(0) as u32;
    let max_y = (y + text_h + 4).min(rgba.height() as i32) as u32;

    for py in min_y..max_y {
        for px in min_x..max_x {
            blend_pixel(rgba, px, py, bg_color);
        }
    }

    let draw_color = image::Rgba([color.r(), color.g(), color.b(), 255]);
    draw_simple_text(rgba, x, y, text, draw_color, font_scale);
}

fn draw_simple_text(
    rgba: &mut image::RgbaImage,
    x: i32,
    y: i32,
    text: &str,
    color: image::Rgba<u8>,
    scale: f32,
) {
    let s = scale.max(1.0) as i32;
    let mut cur_x = x;

    for ch in text.chars() {
        if let Some(glyph) = get_5x7_glyph(ch) {
            for row in 0..7 {
                for col in 0..5 {
                    if (glyph[row] & (1 << (4 - col))) != 0 {
                        for sy in 0..s {
                            for sx in 0..s {
                                let px = cur_x + (col as i32) * s + sx;
                                let py = y + (row as i32) * s + sy;
                                if px >= 0
                                    && px < rgba.width() as i32
                                    && py >= 0
                                    && py < rgba.height() as i32
                                {
                                    blend_pixel(rgba, px as u32, py as u32, color);
                                }
                            }
                        }
                    }
                }
            }
        }
        cur_x += 6 * s;
    }
}

fn get_5x7_glyph(ch: char) -> Option<[u8; 7]> {
    match ch {
        '0' => Some([0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
        '1' => Some([0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
        '2' => Some([0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
        '3' => Some([0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E]),
        '4' => Some([0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
        '5' => Some([0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
        '6' => Some([0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
        '7' => Some([0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
        '8' => Some([0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
        '9' => Some([0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
        'A' | 'a' => Some([0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        'B' | 'b' => Some([0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
        'C' | 'c' => Some([0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
        'D' | 'd' => Some([0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C]),
        'E' | 'e' => Some([0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
        'F' | 'f' => Some([0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
        'G' | 'g' => Some([0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F]),
        'H' | 'h' => Some([0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        'I' | 'i' => Some([0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
        'J' | 'j' => Some([0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C]),
        'K' | 'k' => Some([0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
        'L' | 'l' => Some([0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
        'M' | 'm' => Some([0x11, 0x1B, 0x15, 0x11, 0x11, 0x11, 0x11]),
        'N' | 'n' => Some([0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
        'O' | 'o' => Some([0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        'P' | 'p' => Some([0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
        'Q' | 'q' => Some([0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
        'R' | 'r' => Some([0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
        'S' | 's' => Some([0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E]),
        'T' | 't' => Some([0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        'U' | 'u' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        'V' | 'v' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
        'W' | 'w' => Some([0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11]),
        'X' | 'x' => Some([0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
        'Y' | 'y' => Some([0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04]),
        'Z' | 'z' => Some([0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
        ' ' => Some([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        '-' => Some([0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
        '!' => Some([0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04]),
        '?' => Some([0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04]),
        ':' => Some([0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
        '.' => Some([0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
        _ => Some([0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn badge(n: usize) -> AnnotationAction {
        AnnotationAction {
            tool: AnnotationTool::StepBadge,
            color: Color32::WHITE,
            stroke_width: 1.0,
            points: vec![Pos2::new(1.0, 1.0)],
            text_content: String::new(),
            badge_number: n,
            badge_style: 0,
            sticker: None,
        }
    }

    #[test]
    fn edge_fx_border_and_shadow_expand_canvas() {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            40,
            20,
            image::Rgba([200, 0, 0, 255]),
        ));
        let bordered = apply_edge_fx(img.clone(), 1, 4, Color32::BLACK);
        assert_eq!((bordered.width(), bordered.height()), (48, 28));
        // Corner pixel is the border color.
        assert_eq!(bordered.to_rgba8().get_pixel(0, 0).0, [0, 0, 0, 255]);
        let shadowed = apply_edge_fx(img.clone(), 2, 8, Color32::BLACK);
        assert!(shadowed.width() > 40 && shadowed.height() > 20);
        // Shadow pixels exist past the image's right edge: with m=16, off=4
        // the image spans x<56 and the shadow rect reaches x<60 — (58, 30)
        // is outside the image but inside the blurred shadow band.
        let s = shadowed.to_rgba8();
        let px = s.get_pixel(58, 30);
        assert!(px[3] > 0);
        // Torn edge keeps the canvas size but clears ragged edge pixels.
        let torn = apply_edge_fx(img, 3, 8, Color32::BLACK);
        assert_eq!((torn.width(), torn.height()), (40, 20));
        let t = torn.to_rgba8();
        assert!(t.rows().flatten().any(|p| p.0[3] == 0));
    }

    #[test]
    fn deleting_a_badge_renumbers_the_rest() {
        let mut acts = vec![badge(1), badge(2), badge(3)];
        acts.remove(1);
        let next = renumber_step_badges(&mut acts);
        assert_eq!(acts[0].badge_number, 1);
        assert_eq!(acts[1].badge_number, 2);
        assert_eq!(next, 3);
    }

    #[test]
    fn straighten_collapses_a_wobbly_line() {
        let mut pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(50.0, 2.0),
            Pos2::new(100.0, -1.5),
            Pos2::new(160.0, 1.0),
            Pos2::new(200.0, 0.0),
        ];
        assert!(straighten_if_near_line(&mut pts));
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], Pos2::new(0.0, 0.0));
        assert_eq!(pts[1], Pos2::new(200.0, 0.0));
    }

    #[test]
    fn straighten_keeps_a_real_curve() {
        // L-shaped stroke: the corner deviates far from the chord.
        let mut pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(50.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 50.0),
            Pos2::new(100.0, 100.0),
        ];
        assert!(!straighten_if_near_line(&mut pts));
        assert_eq!(pts.len(), 5);
    }
}
