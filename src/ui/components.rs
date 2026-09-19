//! Shared UI components: toast cards, empty states, Loop rail, Shutter strip.

use egui::{Align, Color32, Frame, Layout, Margin, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};

use super::icons::{self, Icon};
use super::theme;

// ── Toast ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastLevel {
    #[default]
    Info,
    Success,
    Warn,
    Error,
}

impl ToastLevel {
    pub fn from_message(msg: &str) -> Self {
        let t = msg.trim_start();
        if t.starts_with('❌') || t.contains("failed") || t.contains("Failed") || t.contains("Could not")
        {
            Self::Error
        } else if t.starts_with('⚠') || t.starts_with("⚠️") {
            Self::Warn
        } else if t.starts_with('✅') || t.starts_with("💾") || t.starts_with("📸") || t.starts_with("🎨")
        {
            Self::Success
        } else {
            Self::Info
        }
    }

    pub fn accent(self) -> Color32 {
        match self {
            Self::Success => theme::SUCCESS(),
            Self::Warn => theme::WARN(),
            Self::Error => theme::DANGER(),
            Self::Info => theme::INFO(),
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Success => Icon::Check,
            Self::Warn => Icon::Warn,
            Self::Error => Icon::Error,
            Self::Info => Icon::Info,
        }
    }
}

/// Compact severity-tinted toast card (bottom-right overlay style via Area).
pub fn show_toast_card(ctx: &egui::Context, message: &str, level: ToastLevel) {
    let accent = level.accent();
    egui::Area::new(egui::Id::new("vibecap_toast"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme::SURFACE())
                .stroke(Stroke::new(1.0_f32, accent))
                .rounding(theme::rounding_md())
                .inner_margin(Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.set_max_width(360.0);
                    ui.horizontal(|ui| {
                        // Left severity bar
                        let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 22.0), Sense::hover());
                        ui.painter().rect_filled(bar, 1.0, accent);
                        ui.add_space(theme::SP_2);
                        icons::icon_button(ui, level.icon(), accent, 16.0);
                        ui.add_space(theme::SP_2);
                        ui.label(
                            RichText::new(message)
                                .color(theme::TEXT())
                                .size(13.0),
                        );
                    });
                });
        });
}

// ── Post-capture action toast ───────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CaptureToastAction {
    Annotate,
    Copy,
    CopyPath,
    Reveal,
    Discard,
    Dismiss,
}

/// Bottom-right capture card: Annotate · Copy · Reveal · dismiss (does not auto-open studio).
/// `copied` — auto-copy already landed on the clipboard, so the title can say so.
pub fn show_capture_toast(
    ctx: &egui::Context,
    path: &std::path::Path,
    copied: bool,
) -> Option<CaptureToastAction> {
    let mut action = None;
    let name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "capture".into());

    egui::Area::new(egui::Id::new("vibecap_capture_toast"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme::SURFACE_GLASS())
                .stroke(Stroke::new(1.0_f32, theme::ACCENT()))
                .rounding(theme::rounding_lg())
                .inner_margin(Margin::symmetric(14.0, 12.0))
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.set_max_width(360.0);
                    ui.horizontal(|ui| {
                        icons::icon_button(ui, Icon::Camera, theme::ACCENT(), 18.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(if copied {
                                    "Copied to clipboard"
                                } else {
                                    "Captured"
                                })
                                .font(egui::FontId::new(14.0, theme::font_semibold()))
                                .color(theme::TEXT()),
                            );
                            ui.label(
                                RichText::new(&name)
                                    .small()
                                    .color(theme::TEXT_MUTED()),
                            );
                        });
                    });
                    ui.add_space(theme::SP_2);
                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new("Annotate").strong())
                            .on_hover_text("Open in Review to mark it up")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::Annotate);
                        }
                        if ui.button("Copy Image").on_hover_text("Copy image to clipboard").clicked() {
                            action = Some(CaptureToastAction::Copy);
                        }
                        if ui.button("Copy Path").on_hover_text("Copy file path").clicked() {
                            action = Some(CaptureToastAction::CopyPath);
                        }
                        if ui
                            .button("Reveal")
                            .on_hover_text("Show in Finder / Explorer")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::Reveal);
                        }
                        if ui.button("Discard").on_hover_text("Move to undo trash").clicked() {
                            action = Some(CaptureToastAction::Discard);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_btn(ui, "✕", "Dismiss") {
                                action = Some(CaptureToastAction::Dismiss);
                            }
                        });
                    });
                });
        });
    action
}

/// Stable color for an agent label (Codex / Claude / …).
pub fn agent_dot_color(label: &str) -> Color32 {
    if label.is_empty() {
        return theme::TEXT_MUTED();
    }
    let mut h = 0u32;
    for b in label.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    let palette = [
        theme::INFO(),
        theme::SUCCESS(),
        theme::ACCENT(),
        theme::LOOP_ANNOTATE(),
        theme::WARN(),
        theme::AGENT_TEAL(),
    ];
    palette[(h as usize) % palette.len()]
}

// ── Empty state ─────────────────────────────────────────────────────

pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
    ui.add_space(theme::SP_6);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
        icons::paint_icon(ui, rect, icon, theme::TEXT_DIM());
        ui.add_space(theme::SP_3);
        ui.label(
            RichText::new(title)
                .color(theme::TEXT_MUTED())
                .size(15.0)
                .strong(),
        );
        ui.add_space(theme::SP_1);
        ui.label(
            RichText::new(subtitle)
                .color(theme::TEXT_DIM())
                .size(12.0),
        );
    });
}

// ── Loop rail ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoopStage {
    Shutter,
    Review,
    Media,
    Inbox,
    Settings,
}

impl LoopStage {
    pub fn all() -> [Self; 5] {
        [
            Self::Shutter,
            Self::Review,
            Self::Media,
            Self::Inbox,
            Self::Settings,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Shutter => "Capture",
            Self::Review => "Review",
            Self::Media => "Library",
            Self::Inbox => "Inbox",
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Shutter => Icon::Shutter,
            Self::Review => Icon::Still,
            Self::Media => Icon::Media,
            Self::Inbox => Icon::Inbox,
            Self::Settings => Icon::Settings,
        }
    }
}

/// Left Loop rail. Returns newly selected stage if the user clicked.
pub fn loop_rail(
    ui: &mut Ui,
    active: LoopStage,
    inbox_badge: usize,
    rec_live: bool,
    logo: Option<&egui::TextureHandle>,
) -> Option<LoopStage> {
    let mut picked = None;
    let rail_w = 72.0;

    ui.allocate_ui_with_layout(
        Vec2::new(rail_w, ui.available_height()),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add_space(theme::SP_3);
            // Brand mark — texture so Windows (and everyone) sees the real logo.
            if let Some(logo) = logo {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                ui.painter_at(r).image(
                    logo.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.label(
                    RichText::new("VC")
                        .color(theme::TEXT_MUTED())
                        .size(11.0)
                        .strong(),
                );
            }
            ui.add_space(theme::SP_4);

            let stage_button = |ui: &mut Ui, stage: LoopStage| {
                let is_active = stage == active;
                let is_live_stage = matches!(stage, LoopStage::Shutter) && rec_live
                    || matches!(stage, LoopStage::Inbox) && inbox_badge > 0;

                let icon_color = if is_live_stage {
                    theme::ACCENT()
                } else if is_active {
                    theme::TEXT()
                } else {
                    theme::TEXT_MUTED()
                };

                let fill = if is_active {
                    theme::SURFACE_3()
                } else {
                    Color32::TRANSPARENT
                };

                let resp = Frame::none()
                    .fill(fill)
                    .rounding(theme::rounding_md())
                    .inner_margin(Margin::symmetric(6.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(rail_w - 12.0);
                        ui.vertical_centered(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                            icons::paint_icon(ui, r, stage.icon(), icon_color);
                            ui.add_space(2.0);
                            let label_color = if is_active {
                                theme::TEXT()
                            } else {
                                theme::TEXT_DIM()
                            };
                            ui.label(
                                RichText::new(stage.label())
                                    .color(label_color)
                                    .size(10.0),
                            );
                            if matches!(stage, LoopStage::Inbox) && inbox_badge > 0 {
                                ui.label(
                                    RichText::new(format!("{}", inbox_badge.min(99)))
                                        .color(theme::ACCENT())
                                        .size(10.0)
                                        .strong(),
                                );
                            }
                            if matches!(stage, LoopStage::Shutter) && rec_live {
                                ui.label(
                                    RichText::new("REC")
                                        .color(theme::DANGER())
                                        .size(9.0)
                                        .strong(),
                                );
                            }
                        });
                    })
                    .response
                    .on_hover_text(stage.label())
                    .interact(Sense::click());

                // Accent tick on the active stage — reads as "you are here".
                if is_active {
                    let r = resp.rect;
                    let tick = Rect::from_min_size(
                        Pos2::new(r.left() + 2.0, r.top() + 8.0),
                        Vec2::new(3.0, r.height() - 16.0),
                    );
                    if theme::is_celestial() {
                        theme::paint_aurora_strip_v(ui.painter(), tick);
                    } else {
                        ui.painter().rect_filled(tick, Rounding::same(1.5), theme::ACCENT());
                    }
                }

                resp
            };

            // Funnel: Capture → Review ↓, then the archive legs, Settings pinned
            // to the bottom like a normal app.
            for (i, stage) in LoopStage::all().iter().enumerate() {
                let stage = *stage;
                if matches!(stage, LoopStage::Settings) {
                    continue;
                }
                if stage_button(ui, stage).clicked() {
                    picked = Some(stage);
                }
                // Flow hint between the first two funnel steps.
                if i == 0 {
                    ui.label(
                        RichText::new("↓")
                            .color(theme::TEXT_DIM())
                            .size(11.0),
                    );
                }
                ui.add_space(theme::SP_1);
            }

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(theme::SP_3);
                if stage_button(ui, LoopStage::Settings).clicked() {
                    picked = Some(LoopStage::Settings);
                }
            });
        },
    );

    picked
}

// ── Status strip ────────────────────────────────────────────────────

/// Read-only chrome for storage / budget / ffmpeg / inbox / live.
#[derive(Clone)]
pub struct StatusSnapshot {
    pub storage_label: String,
    pub budget_tier: String,
    pub budget_usage: String,
    pub ffmpeg_ok: bool,
    pub pending_inbox: usize,
    pub rec_live: bool,
    pub rec_label: String,
}

/// Bottom status strip (mock language: storage · tier · ffmpeg · inbox).
/// `details = false` on the Shutter stage — the funnel home stays clean;
/// storage/budget trivia lives on the workspace stages.
pub fn status_strip(ui: &mut Ui, snap: &StatusSnapshot, details: bool) {
    Frame::none()
        .fill(theme::SURFACE_GLASS_DIM())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if details {
                    ui.label(
                        RichText::new(&snap.storage_label)
                            .size(11.0)
                            .color(theme::TEXT_MUTED()),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} · {}", snap.budget_tier, snap.budget_usage))
                            .size(11.0)
                            .color(theme::TEXT_MUTED()),
                    );
                    ui.separator();
                }
                if snap.ffmpeg_ok {
                    if details {
                        ui.label(
                            RichText::new("ffmpeg ok")
                                .size(11.0)
                                .color(theme::SUCCESS()),
                        );
                        ui.separator();
                    }
                } else {
                    // Missing ffmpeg is a functional warning — always show it.
                    ui.label(
                        RichText::new("ffmpeg missing")
                            .size(11.0)
                            .color(theme::DANGER()),
                    );
                    ui.separator();
                }
                let inbox_c = if snap.pending_inbox > 0 {
                    theme::ACCENT()
                } else {
                    theme::TEXT_DIM()
                };
                ui.label(
                    RichText::new(format!("inbox {}", snap.pending_inbox))
                        .size(11.0)
                        .color(inbox_c),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if snap.rec_live {
                        ui.label(
                            RichText::new(format!("● {}", snap.rec_label))
                                .size(11.0)
                                .strong()
                                .color(theme::DANGER()),
                        );
                    } else {
                        ui.label(
                            RichText::new("idle")
                                .size(11.0)
                                .color(theme::TEXT_DIM()),
                        );
                    }
                });
            });
        });
}

// ── Modern controls (rail-matching language) ────────────────────────
//
// Tabs compose these instead of raw egui widgets so chrome stays consistent:
// SURFACE cards + BORDER strokes, segmented choices, switch toggles, and
// primary/secondary buttons with the 8pt grid.

/// Section card: SURFACE fill, 1px BORDER hairline, lg (12px) rounding,
/// 14px padding, letterspaced caps title — mono-ui `.card` + `.section-title`.
pub fn section_card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let frame = Frame::none()
        .fill(theme::SURFACE())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(theme::rounding_lg())
        .inner_margin(Margin::same(14.0));
    // Chromie celestial .card shadow: 0 4px 14px rgba(7,6,26,.4).
    let frame = if theme::is_celestial() {
        frame.shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 4.0),
            blur: 14.0,
            spread: 0.0,
            color: egui::Color32::from_rgba_unmultiplied(7, 6, 26, 102),
        })
    } else {
        frame
    };
    frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            theme::caps_label(ui, title);
            ui.add_space(theme::SP_2);
            add(ui);
        });
    ui.add_space(theme::SP_3);
}

#[derive(Clone, Copy)]
enum BtnKind {
    Primary,
    Secondary,
    Small,
    Danger,
}

/// Hand-painted button — egui's `Button` can't express the mockup's state
/// model (ink fill → opacity fade on hover, surface→surface-3 on outline).
fn paint_button(ui: &mut Ui, label: &str, kind: BtnKind) -> bool {
    let (size_px, pad) = match kind {
        BtnKind::Primary | BtnKind::Danger => (13.0, Vec2::new(14.0, 9.0)),
        BtnKind::Secondary => (12.5, Vec2::new(12.0, 7.0)),
        BtnKind::Small => (11.5, Vec2::new(11.0, 5.0)),
    };
    let font = egui::FontId::new(size_px, theme::font_semibold());
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font.clone(),
        theme::TEXT(),
    );
    let (rect, resp) =
        ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let (fill, stroke, text) = match kind {
        BtnKind::Primary => (theme::PRIMARY(), theme::PRIMARY(), theme::PRIMARY_INK()),
        BtnKind::Secondary => (theme::SURFACE(), theme::BORDER_STRONG(), theme::TEXT()),
        BtnKind::Small => (theme::SURFACE(), theme::BORDER_STRONG(), theme::TEXT_MUTED()),
        BtnKind::Danger => (theme::DANGER_SOFT(), theme::DANGER_SOFT(), theme::ON_SOLID()),
    };
    let down = resp.is_pointer_button_down_on();
    let fill = if down {
        match kind {
            BtnKind::Primary => theme::PRIMARY_DOWN(),
            BtnKind::Danger => theme::DANGER(),
            _ => theme::SURFACE_3(),
        }
    } else if resp.hovered() {
        match kind {
            BtnKind::Primary => theme::PRIMARY_HOVER(),
            BtnKind::Secondary | BtnKind::Small => theme::SURFACE_3(),
            BtnKind::Danger => theme::DANGER(),
        }
    } else {
        fill
    };
    let text = if matches!(kind, BtnKind::Small) && resp.hovered() {
        theme::TEXT()
    } else {
        text
    };

    let p = ui.painter();
    let r = theme::rounding_md();
    // Chromie --cta-gradient: celestial primaries are the aurora itself.
    if matches!(kind, BtnKind::Primary) && theme::is_celestial() {
        theme::paint_aurora_button(p, rect, r.nw);
        if down {
            p.rect_filled(rect, r, egui::Color32::from_black_alpha(60));
        } else if resp.hovered() {
            p.rect_filled(rect, r, egui::Color32::from_black_alpha(28));
        }
        p.rect_stroke(rect, r, Stroke::new(1.0_f32, egui::Color32::from_white_alpha(30)));
    } else {
        p.rect_filled(rect, r, fill);
        p.rect_stroke(rect, r, Stroke::new(1.0_f32, stroke));
    }
    p.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        ui.painter().layout_no_wrap(label.to_string(), font, text),
        text,
    );
    resp.clicked()
}

/// Ink primary action (mono-ui `.btn-primary` — near-black fill, fades on hover).
pub fn btn_primary(ui: &mut Ui, label: &str) -> bool {
    paint_button(ui, label, BtnKind::Primary)
}

/// Outline surface action (mono-ui `.btn-outline` — border-2, surface-3 hover).
pub fn btn_secondary(ui: &mut Ui, label: &str) -> bool {
    paint_button(ui, label, BtnKind::Secondary)
}

/// Compact outline action (mono-ui `.wake-btn`).
pub fn btn_small(ui: &mut Ui, label: &str) -> bool {
    paint_button(ui, label, BtnKind::Small)
}

/// Destructive action.
pub fn btn_danger(ui: &mut Ui, label: &str) -> bool {
    paint_button(ui, label, BtnKind::Danger)
}

/// Ghost icon/text button — no chrome at rest, a `surface-2` wash and
/// brighter glyph on hover (mono-ui topbar icon buttons). Vertically
/// centered on its glyph so it sits flush in mixed icon+text rows.
pub fn icon_btn(ui: &mut Ui, label: &str, hover: &str) -> bool {
    let font = egui::FontId::new(13.5, egui::FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), theme::TEXT_MUTED());
    let (rect, resp) = ui.allocate_exact_size(
        egui::Vec2::new((galley.size().x + 14.0).max(26.0), 26.0),
        Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(rect, theme::rounding_sm(), theme::SURFACE_3());
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        if resp.hovered() {
            theme::TEXT()
        } else {
            theme::TEXT_MUTED()
        },
    );
    let resp = if hover.is_empty() {
        resp
    } else {
        resp.on_hover_text(hover)
    };
    resp.clicked()
}

/// Ghost `⋯` overflow menu — same chrome-less look as `icon_btn`, keeping
/// egui's popup machinery by stripping the button's rest-state visuals.
pub fn icon_menu_button(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let mut vis = (*ui.visuals()).clone();
    vis.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    vis.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    vis.widgets.inactive.bg_stroke = Stroke::NONE;
    vis.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, theme::TEXT_MUTED());
    vis.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, theme::TEXT());
    ui.scope(|ui| {
        *ui.visuals_mut() = vis;
        ui.menu_button(RichText::new(label).size(15.0), add);
    });
}

/// Monospace count badge (mono-ui `.count`).
pub fn count_chip(ui: &mut Ui, label: &str) {
    Frame::none()
        .fill(theme::SURFACE_2())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(Rounding::same(5.0))
        .inner_margin(Margin::symmetric(6.0, 1.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .font(egui::FontId::new(11.0, egui::FontFamily::Monospace))
                    .color(theme::TEXT_DIM()),
            );
        });
}

/// Pill filter chip (mono-ui `.chip` — radius 999, ink fill when active).
pub fn chip(ui: &mut Ui, label: &str, active: bool) -> bool {
    let font = egui::FontId::new(11.5, theme::font_semibold());
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), font.clone(), theme::TEXT_MUTED());
    let (rect, resp) = ui.allocate_exact_size(
        galley.size() + Vec2::new(22.0, 10.0),
        Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let (fill, stroke, text) = if active {
        (theme::PRIMARY(), theme::PRIMARY(), theme::PRIMARY_INK())
    } else {
        (
            if resp.hovered() { theme::SURFACE_3() } else { theme::SURFACE() },
            theme::BORDER(),
            theme::TEXT_MUTED(),
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(rect.height() / 2.0), fill);
    p.rect_stroke(rect, Rounding::same(rect.height() / 2.0), Stroke::new(1.0_f32, stroke));
    p.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        ui.painter().layout_no_wrap(label.to_string(), font, text),
        text,
    );
    resp.clicked()
}

/// Segmented control for exclusive choices (mono-ui `.seg` — bordered
/// surface-2 track, active segment pops to `surface` with semibold text).
/// Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    Frame::none()
        .fill(theme::SURFACE_2())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(3.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (val, label) in options {
                    let active = current == val;
                    let font = egui::FontId::new(
                        12.0,
                        if active {
                            theme::font_semibold()
                        } else {
                            egui::FontFamily::Proportional
                        },
                    );
                    let galley = ui.painter().layout_no_wrap(
                        (*label).to_string(),
                        font.clone(),
                        theme::TEXT(),
                    );
                    let (rect, resp) = ui.allocate_exact_size(
                        galley.size() + Vec2::new(24.0, 10.0),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let p = ui.painter();
                    if active {
                        p.rect_filled(rect, theme::rounding_sm(), theme::SURFACE());
                    }
                    let color = if active || resp.hovered() {
                        theme::TEXT()
                    } else {
                        theme::TEXT_MUTED()
                    };
                    p.galley(
                        egui::pos2(
                            rect.center().x - galley.size().x / 2.0,
                            rect.center().y - galley.size().y / 2.0,
                        ),
                        ui.painter()
                            .layout_no_wrap((*label).to_string(), font, color),
                        color,
                    );
                    if resp.clicked() {
                        *current = *val;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// iOS-style switch + label. Returns true when toggled.
pub fn switch(ui: &mut Ui, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, 18.0), Sense::click());
        if resp.clicked() {
            *on = !*on;
            changed = true;
        }
        let paint = ui.painter_at(rect);
        let r = rect.height() / 2.0;
        let bg = if *on { theme::PRIMARY() } else { theme::SURFACE_3() };
        paint.rect_filled(rect, r, bg);
        let knob_x = if *on { rect.right() - r } else { rect.left() + r };
        let knob_c = if *on { theme::PRIMARY_INK() } else { theme::TEXT_MUTED() };
        paint.circle_filled(egui::pos2(knob_x, rect.center().y), r - 3.0, knob_c);
        resp.on_hover_text(label);
        ui.add_space(theme::SP_2);
        ui.label(RichText::new(label).size(12.0).color(theme::TEXT_MUTED()));
    });
    changed
}

/// Monospace key chip for shortcut listings.
pub fn kbd(ui: &mut Ui, key: &str) {
    Frame::none()
        .fill(theme::SURFACE_2())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(theme::rounding_sm())
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(key)
                    .font(egui::FontId::new(11.0, egui::FontFamily::Monospace))
                    .color(theme::TEXT_MUTED()),
            );
        });
}

/// Label + control row that wraps on narrow windows.
pub fn setting_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(26.0);
        ui.label(RichText::new(label).size(12.0).color(theme::TEXT_MUTED()));
        ui.add_space(theme::SP_2);
        add(ui);
    });
    ui.add_space(theme::SP_1);
}

/// Grouped control cluster with a dim caption title (tools menus).
pub fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.label(RichText::new(title).size(10.0).color(theme::TEXT_DIM()));
    ui.add_space(2.0);
    ui.horizontal_wrapped(add);
    ui.add_space(theme::SP_2);
}

/// Slim clickable bar standing in for a collapsed funnel stage.
/// Returns true when the user clicks to expand that stage.
pub fn funnel_stripe(ui: &mut Ui, icon: Icon, title: &str, hint: &str) -> bool {
    let h = 40.0;
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let hovered = resp.hovered();
    ui.painter().rect_filled(
        rect,
        theme::rounding_md(),
        if hovered {
            theme::SURFACE_3()
        } else {
            theme::SURFACE()
        },
    );
    ui.painter().rect_stroke(
        rect,
        theme::rounding_md(),
        Stroke::new(1.0_f32, if hovered { theme::ACCENT() } else { theme::BORDER() }),
    );
    icons::paint_icon(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 24.0, rect.center().y),
            Vec2::splat(18.0),
        ),
        icon,
        if hovered { theme::ACCENT() } else { theme::TEXT_MUTED() },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 48.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        theme::TEXT(),
    );
    ui.painter().text(
        Pos2::new(rect.right() - 16.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        hint,
        egui::FontId::proportional(11.0),
        theme::TEXT_DIM(),
    );
    let clicked = resp.clicked();
    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}

// ── Shutter strip ───────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShutterAction {
    Screenshot,
    RecordToggle,
    Gif,
}

/// Persistent capture dock: screenshot + record (or stop / arming).
pub fn shutter_strip(
    ui: &mut Ui,
    is_recording: bool,
    is_arming: bool,
    rec_label: &str,
) -> Option<ShutterAction> {
    let mut action = None;

    // Translucent fill (mock backdrop-blur approximated — no real blur in egui).
    Frame::none()
        .fill(theme::SURFACE_GLASS())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(theme::rounding_lg())
        .inner_margin(Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Screenshot — the primary CTA for a snipping tool.
                let shot = egui::Button::new(
                    RichText::new("  📸  Screenshot  ")
                        .color(theme::ACCENT_INK())
                        .size(15.0)
                        .strong(),
                )
                .fill(theme::ACCENT())
                .stroke(Stroke::new(1.0_f32, theme::ACCENT()))
                .rounding(theme::rounding_md());
                if ui
                    .add_sized([176.0, 48.0], shot)
                    .on_hover_text("S · Ctrl+Shift+3")
                    .clicked()
                {
                    action = Some(ShutterAction::Screenshot);
                }

                ui.add_space(theme::SP_2);

                // Record — outline at rest, danger fill only while live.
                let (fill, stroke_c, text_c) = if is_recording {
                    (theme::DANGER(), theme::DANGER(), theme::ON_SOLID())
                } else if is_arming {
                    (theme::WARN(), theme::WARN(), theme::ACCENT_INK())
                } else {
                    (theme::SURFACE_2(), theme::TEXT_MUTED(), theme::TEXT())
                };
                let rec = egui::Button::new(
                    RichText::new(format!("  {}  ", rec_label))
                        .color(text_c)
                        .size(15.0)
                        .strong(),
                )
                .fill(fill)
                .stroke(Stroke::new(1.0_f32, stroke_c))
                .rounding(theme::rounding_md());
                if ui
                    .add_sized([176.0, 48.0], rec)
                    .on_hover_text("R · Ctrl+Shift+2 · tray")
                    .clicked()
                {
                    action = Some(ShutterAction::RecordToggle);
                }

                ui.add_space(theme::SP_2);
                let gif = egui::Button::new(
                    RichText::new("  GIF  ")
                        .color(theme::TEXT())
                        .size(13.0),
                )
                .fill(theme::SURFACE_2())
                .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                .rounding(theme::rounding_md());
                if ui
                    .add_sized([88.0, 48.0], gif)
                    .on_hover_text("Record 3 seconds and export a GIF")
                    .clicked()
                {
                    action = Some(ShutterAction::Gif);
                }
            });
        });

    action
}
