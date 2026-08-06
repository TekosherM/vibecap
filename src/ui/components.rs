//! Shared UI components: toast cards, empty states, Loop rail, Shutter strip.

use egui::{Align, Color32, Frame, Layout, Margin, RichText, Rounding, Sense, Stroke, Ui, Vec2};

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
    Reveal,
    Dismiss,
}

/// Bottom-right capture card: Annotate · Copy · Reveal · dismiss (does not auto-open studio).
pub fn show_capture_toast(
    ctx: &egui::Context,
    path: &std::path::Path,
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
                                RichText::new("Captured")
                                    .strong()
                                    .color(theme::TEXT())
                                    .size(14.0),
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
                            .on_hover_text("Open annotation studio")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::Annotate);
                        }
                        if ui.button("Copy").on_hover_text("Copy image to clipboard").clicked() {
                            action = Some(CaptureToastAction::Copy);
                        }
                        if ui.button("Reveal").on_hover_text("Show in Finder/Explorer").clicked() {
                            action = Some(CaptureToastAction::Reveal);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("✕").on_hover_text("Dismiss").clicked() {
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
    Media,
    Clip,
    Still,
    Inbox,
    Settings,
}

impl LoopStage {
    pub fn all() -> [Self; 6] {
        [
            Self::Shutter,
            Self::Media,
            Self::Clip,
            Self::Still,
            Self::Inbox,
            Self::Settings,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Shutter => "Shutter",
            Self::Media => "Media",
            Self::Clip => "Clip",
            Self::Still => "Still",
            Self::Inbox => "Inbox",
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Shutter => Icon::Shutter,
            Self::Media => Icon::Media,
            Self::Clip => Icon::Clip,
            Self::Still => Icon::Still,
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
) -> Option<LoopStage> {
    let mut picked = None;
    let rail_w = 72.0;

    ui.allocate_ui_with_layout(
        Vec2::new(rail_w, ui.available_height()),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add_space(theme::SP_3);
            // Brand mark
            ui.label(
                RichText::new("VC")
                    .color(theme::TEXT_MUTED())
                    .size(11.0)
                    .strong(),
            );
            ui.add_space(theme::SP_4);

            for stage in LoopStage::all() {
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

                if resp.clicked() {
                    picked = Some(stage);
                }
                ui.add_space(theme::SP_1);
            }

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(theme::SP_3);
                ui.label(
                    RichText::new("Loop")
                        .color(theme::TEXT_DIM())
                        .size(9.0),
                );
            });
        },
    );

    picked
}

// ── Loop-position badge ─────────────────────────────────────────────

/// Small chip showing loop stage (Capture / Review / Annotate / Ask / Answered).
pub fn loop_position_badge(ui: &mut Ui, pos: crate::app::LoopPosition) {
    let (fill, stroke, text_c) = match pos {
        crate::app::LoopPosition::Capture => (theme::SURFACE_2(), theme::TEXT_DIM(), theme::TEXT_MUTED()),
        crate::app::LoopPosition::Review => (theme::LOOP_REVIEW_FILL(), theme::INFO(), theme::INFO()),
        crate::app::LoopPosition::Annotate => (
            theme::LOOP_ANNOTATE_FILL(),
            theme::LOOP_ANNOTATE(),
            theme::LOOP_ANNOTATE_TEXT(),
        ),
        crate::app::LoopPosition::Ask => (theme::LOOP_ASK_FILL(), theme::ACCENT(), theme::ACCENT()),
        crate::app::LoopPosition::Answered => {
            (theme::LOOP_ANSWERED_FILL(), theme::SUCCESS(), theme::SUCCESS())
        }
    };
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke))
        .rounding(Rounding::same(theme::R_SM))
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(pos.label())
                    .size(10.0)
                    .strong()
                    .color(text_c),
            );
        });
}

// ── Status strip ────────────────────────────────────────────────────

/// Read-only chrome for storage / budget / ffmpeg / inbox / live.
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
pub fn status_strip(ui: &mut Ui, snap: &StatusSnapshot) {
    Frame::none()
        .fill(theme::SURFACE_GLASS_DIM())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
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
                if snap.ffmpeg_ok {
                    ui.label(
                        RichText::new("ffmpeg ok")
                            .size(11.0)
                            .color(theme::SUCCESS()),
                    );
                } else {
                    ui.label(
                        RichText::new("ffmpeg missing")
                            .size(11.0)
                            .color(theme::DANGER()),
                    );
                }
                ui.separator();
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

// ── Shutter strip ───────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShutterAction {
    Screenshot,
    RecordToggle,
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
                ui.label(
                    RichText::new("SHUTTER")
                        .color(theme::TEXT_DIM())
                        .size(11.0)
                        .strong(),
                );
                ui.add_space(theme::SP_3);

                // Screenshot — secondary outline (not live accent)
                let shot = egui::Button::new(
                    RichText::new("  Screenshot  (S)  ")
                        .color(theme::TEXT())
                        .size(14.0)
                        .strong(),
                )
                .fill(theme::SURFACE_2())
                .stroke(Stroke::new(1.0_f32, theme::TEXT_MUTED()))
                .rounding(theme::rounding_md());
                if ui
                    .add_sized([148.0, 44.0], shot)
                    .on_hover_text("S · Ctrl+Shift+3")
                    .clicked()
                {
                    action = Some(ShutterAction::Screenshot);
                }

                ui.add_space(theme::SP_2);

                // Record — accent only when live / primary CTA when idle
                let (fill, stroke_c, text_c) = if is_recording {
                    (theme::DANGER(), theme::DANGER(), theme::ON_SOLID())
                } else if is_arming {
                    (theme::WARN(), theme::WARN(), theme::ACCENT_INK())
                } else {
                    // Idle record uses accent as the one intentional live-capable CTA
                    (theme::ACCENT(), theme::ACCENT(), theme::ACCENT_INK())
                };
                let rec = egui::Button::new(
                    RichText::new(format!("  {}  ", rec_label))
                        .color(text_c)
                        .size(14.0)
                        .strong(),
                )
                .fill(fill)
                .stroke(Stroke::new(1.0_f32, stroke_c))
                .rounding(theme::rounding_md());
                if ui
                    .add_sized([148.0, 44.0], rec)
                    .on_hover_text("R · Ctrl+Shift+2 · tray")
                    .clicked()
                {
                    action = Some(ShutterAction::RecordToggle);
                }
            });
        });

    action
}
