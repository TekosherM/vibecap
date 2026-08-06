//! Simple vector-ish icons drawn with egui shapes (no emoji, cross-platform).

use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

use super::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Shutter,
    Media,
    Studio,
    Clip,
    Still,
    Inbox,
    Settings,
    Camera,
    Record,
    Stop,
    Check,
    Warn,
    Error,
    Info,
    EmptyFilm,
}

/// Paint a monochrome icon into `rect`. Returns response for hit-testing.
pub fn icon_button(ui: &mut Ui, icon: Icon, color: Color32, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        paint_icon(ui, rect, icon, color);
    }
    response
}

pub fn paint_icon(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let painter = ui.painter();
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.5;
    let stroke = Stroke::new((s * 0.18).clamp(1.2, 2.2), color);

    match icon {
        Icon::Shutter | Icon::Camera => {
            // Camera body
            let body = Rect::from_center_size(c + Vec2::new(0.0, s * 0.08), Vec2::new(s * 1.5, s * 1.05));
            painter.rect_stroke(body, 2.0, stroke);
            painter.circle_stroke(c + Vec2::new(0.0, s * 0.1), s * 0.32, stroke);
            // Viewfinder bump
            let bump = Rect::from_center_size(
                c + Vec2::new(0.0, -s * 0.42),
                Vec2::new(s * 0.55, s * 0.28),
            );
            painter.rect_stroke(bump, 1.5, stroke);
        }
        Icon::Record => {
            painter.circle_filled(c, s * 0.55, color);
        }
        Icon::Stop => {
            let r = Rect::from_center_size(c, Vec2::splat(s * 0.9));
            painter.rect_filled(r, 2.0, color);
        }
        Icon::Media => {
            // Stacked cards
            let a = Rect::from_center_size(c + Vec2::new(-s * 0.12, -s * 0.08), Vec2::new(s * 1.35, s * 1.0));
            let b = Rect::from_center_size(c + Vec2::new(s * 0.12, s * 0.12), Vec2::new(s * 1.35, s * 1.0));
            painter.rect_stroke(b, 2.0, stroke);
            painter.rect_stroke(a, 2.0, stroke);
        }
        Icon::Studio | Icon::Clip => {
            // Film strip / trim: frame + center cut
            let frame = Rect::from_center_size(c, Vec2::new(s * 1.45, s * 1.0));
            painter.rect_stroke(frame, 2.0, stroke);
            painter.line_segment(
                [Pos2::new(c.x, frame.top() + 2.0), Pos2::new(c.x, frame.bottom() - 2.0)],
                stroke,
            );
            for dy in [-0.32_f32, 0.32] {
                painter.rect_filled(
                    Rect::from_center_size(
                        c + Vec2::new(-s * 0.48, s * dy),
                        Vec2::new(s * 0.18, s * 0.14),
                    ),
                    1.0,
                    color,
                );
                painter.rect_filled(
                    Rect::from_center_size(
                        c + Vec2::new(s * 0.48, s * dy),
                        Vec2::new(s * 0.18, s * 0.14),
                    ),
                    1.0,
                    color,
                );
            }
        }
        Icon::Still => {
            // Photo frame with mountain
            let frame = Rect::from_center_size(c, Vec2::new(s * 1.45, s * 1.1));
            painter.rect_stroke(frame, 2.0, stroke);
            painter.circle_filled(c + Vec2::new(s * 0.28, -s * 0.22), s * 0.12, color);
            painter.add(Shape::closed_line(
                vec![
                    c + Vec2::new(-s * 0.55, s * 0.35),
                    c + Vec2::new(-s * 0.1, -s * 0.05),
                    c + Vec2::new(s * 0.15, s * 0.18),
                    c + Vec2::new(s * 0.55, s * 0.35),
                ],
                stroke,
            ));
        }
        Icon::Inbox => {
            let tray = [
                Pos2::new(c.x - s * 0.7, c.y - s * 0.15),
                Pos2::new(c.x - s * 0.45, c.y + s * 0.55),
                Pos2::new(c.x + s * 0.45, c.y + s * 0.55),
                Pos2::new(c.x + s * 0.7, c.y - s * 0.15),
            ];
            painter.add(Shape::closed_line(tray.to_vec(), stroke));
            painter.line_segment(
                [Pos2::new(c.x - s * 0.35, c.y - s * 0.15), Pos2::new(c.x, c.y + s * 0.15)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + s * 0.35, c.y - s * 0.15), Pos2::new(c.x, c.y + s * 0.15)],
                stroke,
            );
        }
        Icon::Settings => {
            painter.circle_stroke(c, s * 0.35, stroke);
            for i in 0..6 {
                let a = (i as f32) * std::f32::consts::TAU / 6.0;
                let dir = Vec2::new(a.cos(), a.sin());
                let inner = c + dir * (s * 0.45);
                let outer = c + dir * (s * 0.75);
                painter.line_segment([inner, outer], stroke);
            }
        }
        Icon::Check => {
            painter.line_segment(
                [c + Vec2::new(-s * 0.45, 0.05), c + Vec2::new(-0.08, s * 0.4)],
                Stroke::new(stroke.width + 0.4, color),
            );
            painter.line_segment(
                [c + Vec2::new(-0.08, s * 0.4), c + Vec2::new(s * 0.5, -s * 0.4)],
                Stroke::new(stroke.width + 0.4, color),
            );
        }
        Icon::Warn => {
            let top = c + Vec2::new(0.0, -s * 0.65);
            let bl = c + Vec2::new(-s * 0.55, s * 0.5);
            let br = c + Vec2::new(s * 0.55, s * 0.5);
            painter.add(Shape::closed_line(vec![top, br, bl], stroke));
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.25), c + Vec2::new(0.0, s * 0.1)],
                stroke,
            );
            painter.circle_filled(c + Vec2::new(0.0, s * 0.28), s * 0.08, color);
        }
        Icon::Error => {
            painter.circle_stroke(c, s * 0.65, stroke);
            painter.line_segment(
                [c + Vec2::new(-s * 0.3, -s * 0.3), c + Vec2::new(s * 0.3, s * 0.3)],
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(s * 0.3, -s * 0.3), c + Vec2::new(-s * 0.3, s * 0.3)],
                stroke,
            );
        }
        Icon::Info => {
            painter.circle_stroke(c, s * 0.65, stroke);
            painter.circle_filled(c + Vec2::new(0.0, -s * 0.28), s * 0.1, color);
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.05), c + Vec2::new(0.0, s * 0.35)],
                Stroke::new(stroke.width + 0.3, color),
            );
        }
        Icon::EmptyFilm => {
            let frame = Rect::from_center_size(c, Vec2::new(s * 1.5, s * 1.1));
            painter.rect_stroke(frame, 2.0, stroke);
            for i in 0..4 {
                let x = frame.left() + 4.0 + i as f32 * (frame.width() - 8.0) / 3.0;
                let hole = Rect::from_min_size(
                    Pos2::new(x, frame.top() + 3.0),
                    Vec2::new(3.0, 4.0),
                );
                painter.rect_filled(hole, 0.5, color);
                let hole2 = Rect::from_min_size(
                    Pos2::new(x, frame.bottom() - 7.0),
                    Vec2::new(3.0, 4.0),
                );
                painter.rect_filled(hole2, 0.5, color);
            }
            painter.line_segment(
                [
                    Pos2::new(frame.left() + 8.0, c.y),
                    Pos2::new(frame.right() - 8.0, c.y),
                ],
                Stroke::new(1.0_f32, theme::TEXT_DIM()),
            );
        }
    }
}
