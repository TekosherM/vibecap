//! Software annotation baker — rasterizes pen, arrow, rect, text, blur, and step badges
//! directly onto an `image::DynamicImage` RGBA pixel buffer.

use eframe::egui::{Color32, Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationTool {
    #[default]
    Pen,
    Arrow,
    Rectangle,
    Highlight,
    Text,
    Blur,
    StepBadge,
}

#[derive(Debug, Clone)]
pub struct AnnotationAction {
    pub tool: AnnotationTool,
    pub color: Color32,
    pub stroke_width: f32,
    pub points: Vec<Pos2>,
    pub text_content: String,
    pub badge_number: usize,
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
                let draw_color = image::Rgba([color.r(), color.g(), color.b(), (alpha * 255.0) as u8]);

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
                fill_circle(&mut rgba, cx, cy, badge_r, bg_color);
                draw_badge_number(&mut rgba, cx, cy, action.badge_number, badge_r);
            }
            AnnotationTool::Text => {
                let (x, y) = map_pos(action.points[0]);
                draw_text_box(&mut rgba, x, y, &action.text_content, color, iw / cw);
            }
        }
    }

    *img = image::DynamicImage::ImageRgba8(rgba);
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

fn draw_badge_number(rgba: &mut image::RgbaImage, cx: i32, cy: i32, num: usize, badge_r: i32) {
    let text = num.to_string();
    let font_scale = (badge_r as f32 / 12.0).max(1.0);
    draw_simple_text(rgba, cx - (text.len() as i32 * 4 * font_scale as i32), cy - (4.0 * font_scale) as i32, &text, image::Rgba([0, 0, 0, 255]), font_scale);
}

fn draw_text_box(rgba: &mut image::RgbaImage, x: i32, y: i32, text: &str, color: Color32, scale: f32) {
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

fn draw_simple_text(rgba: &mut image::RgbaImage, x: i32, y: i32, text: &str, color: image::Rgba<u8>, scale: f32) {
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
                                if px >= 0 && px < rgba.width() as i32 && py >= 0 && py < rgba.height() as i32 {
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
