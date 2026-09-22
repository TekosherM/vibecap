//! Shared UI components: toast cards, empty states, Loop rail, Shutter strip.

use egui::{
    Align, Color32, Frame, Layout, Margin, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

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
        if t.starts_with('❌')
            || t.contains("failed")
            || t.contains("Failed")
            || t.contains("Could not")
        {
            Self::Error
        } else if t.starts_with('⚠') || t.starts_with("⚠️") {
            Self::Warn
        } else if t.starts_with('✅')
            || t.starts_with("💾")
            || t.starts_with("📸")
            || t.starts_with("🎨")
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
                        icons::icon_button(ui, level.icon(), accent, theme::ICON_SM + 2.0);
                        ui.add_space(theme::SP_2);
                        ui.label(RichText::new(message).color(theme::TEXT()).size(13.0));
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
                        icons::icon_button(ui, Icon::Camera, theme::ACCENT(), theme::ICON_MD);
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
                            ui.label(RichText::new(&name).small().color(theme::TEXT_MUTED()));
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
                        if ui
                            .button("Copy Image")
                            .on_hover_text("Copy image to clipboard")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::Copy);
                        }
                        if ui
                            .button("Copy Path")
                            .on_hover_text("Copy file path")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::CopyPath);
                        }
                        if ui
                            .button("Reveal")
                            .on_hover_text("Show in Finder / Explorer")
                            .clicked()
                        {
                            action = Some(CaptureToastAction::Reveal);
                        }
                        if ui
                            .button("Discard")
                            .on_hover_text("Move to undo trash")
                            .clicked()
                        {
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
        // E21 — a soft disc (aurora ring on celestial) behind the glyph
        // turns "no rows" into an intentional empty-state plate.
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
        let p = ui.painter();
        p.circle_filled(rect.center(), 31.0, theme::SURFACE_2());
        if theme::is_celestial() {
            p.circle_stroke(
                rect.center(),
                31.0,
                Stroke::new(1.0_f32, theme::ACCENT().gamma_multiply(0.25)),
            );
        } else {
            p.circle_stroke(rect.center(), 31.0, Stroke::new(1.0_f32, theme::BORDER()));
        }
        icons::paint_icon(ui, rect.shrink(8.0), icon, theme::TEXT_DIM());
        ui.add_space(theme::SP_3);
        ui.label(
            RichText::new(title)
                .color(theme::TEXT_MUTED())
                .size(15.0)
                .strong(),
        );
        ui.add_space(theme::SP_1);
        ui.label(RichText::new(subtitle).color(theme::TEXT_DIM()).size(12.0));
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
    collapsed: &mut bool,
) -> Option<LoopStage> {
    let mut picked = None;
    // E26 — icon-only collapse: 48px rail, labels/divider text hidden,
    // tooltips carry the names. `is_collapsed` is a snapshot so the
    // inner closures don't hold a borrow across the toggle's write.
    let is_collapsed = *collapsed;
    let rail_w = if is_collapsed { 48.0 } else { 72.0 };

    ui.allocate_ui_with_layout(
        Vec2::new(rail_w, ui.available_height()),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add_space(theme::SP_3);
            // Brand mark — texture so Windows (and everyone) sees the real logo.
            if let Some(logo) = logo {
                let logo_sz = if is_collapsed { 28.0 } else { 36.0 };
                let (r, _) = ui.allocate_exact_size(Vec2::splat(logo_sz), Sense::hover());
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
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::splat(theme::ICON_LG), Sense::hover());
                            icons::paint_icon(ui, r, stage.icon(), icon_color);
                            if !is_collapsed {
                                ui.add_space(2.0);
                                let label_color = if is_active {
                                    theme::TEXT()
                                } else {
                                    theme::TEXT_DIM()
                                };
                                ui.label(
                                    RichText::new(stage.label()).color(label_color).size(10.0),
                                );
                            }
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
                        ui.painter()
                            .rect_filled(tick, Rounding::same(1.5), theme::ACCENT());
                    }
                }

                resp
            };

            // E28 — group dividers label the rail's two zones; a hairline
            // stands in when collapsed.
            let divider = |ui: &mut Ui, label: &str| {
                ui.add_space(theme::SP_1);
                if is_collapsed {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(24.0, 1.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.5, theme::BORDER());
                } else {
                    ui.label(
                        RichText::new(label)
                            .size(8.0)
                            .color(theme::TEXT_DIM())
                            .strong(),
                    );
                }
                ui.add_space(2.0);
            };
            // Funnel: Capture → Review ↓, then the archive legs, Settings pinned
            // to the bottom like a normal app.
            for (i, stage) in LoopStage::all().iter().enumerate() {
                let stage = *stage;
                if matches!(stage, LoopStage::Settings) {
                    continue;
                }
                if i == 0 {
                    divider(ui, "CAPTURE");
                } else if matches!(stage, LoopStage::Media) {
                    divider(ui, "KEEP");
                } else if matches!(stage, LoopStage::Inbox) {
                    divider(ui, "AGENT");
                }
                if stage_button(ui, stage).clicked() {
                    picked = Some(stage);
                }
                // Flow hint between the first two funnel steps.
                if i == 0 && !is_collapsed {
                    ui.label(RichText::new("↓").color(theme::TEXT_DIM()).size(11.0));
                }
                ui.add_space(theme::SP_1);
            }

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(theme::SP_3);
                // E26 — collapse toggle rides the bottom of the rail.
                let arrow = if is_collapsed { "»" } else { "«" };
                let hint = if is_collapsed {
                    "Expand rail"
                } else {
                    "Collapse rail (icons only)"
                };
                if ui
                    .button(RichText::new(arrow).size(12.0).color(theme::TEXT_MUTED()))
                    .on_hover_text(hint)
                    .clicked()
                {
                    *collapsed = !*collapsed;
                }
                ui.add_space(theme::SP_1);
                divider(ui, "APP");
                if stage_button(ui, LoopStage::Settings).clicked() {
                    picked = Some(LoopStage::Settings);
                }
            });
        },
    );

    picked
}

/// E46 — horizontal top-tab alternative to the left rail, for users who
/// want the Snagit-style strip. Same stages, same badges — laid out in a
/// row with an underline accent on the active tab.
pub fn loop_tabs(
    ui: &mut Ui,
    active: LoopStage,
    inbox_badge: usize,
    rec_live: bool,
    logo: Option<&egui::TextureHandle>,
) -> Option<LoopStage> {
    let mut picked = None;
    ui.horizontal_centered(|ui| {
        ui.add_space(theme::SP_2);
        if let Some(logo) = logo {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
            ui.painter_at(r).image(
                logo.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.add_space(theme::SP_3);
        }
        for stage in LoopStage::all() {
            let is_active = stage == active;
            let is_live_stage = matches!(stage, LoopStage::Shutter) && rec_live
                || matches!(stage, LoopStage::Inbox) && inbox_badge > 0;
            let ink = if is_live_stage {
                theme::ACCENT()
            } else if is_active {
                theme::TEXT()
            } else {
                theme::TEXT_MUTED()
            };
            let label = match stage {
                LoopStage::Inbox if inbox_badge > 0 => {
                    format!("{} {}", stage.label(), inbox_badge.min(99))
                }
                LoopStage::Shutter if rec_live => "Capture ●".to_string(),
                _ => stage.label().to_string(),
            };
            let resp = Frame::none()
                .inner_margin(Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::splat(theme::ICON_SM), Sense::hover());
                        icons::paint_icon(ui, r, stage.icon(), ink);
                        ui.label(RichText::new(label).color(ink).size(11.5));
                    });
                })
                .response
                .interact(Sense::click());
            if is_active {
                let r = resp.rect;
                let tick = Rect::from_min_size(
                    Pos2::new(r.left() + 8.0, r.bottom() - 2.0),
                    Vec2::new(r.width() - 16.0, 2.0),
                );
                if theme::is_celestial() {
                    theme::paint_aurora_strip_for(ui.painter(), tick, theme::theme_mode());
                } else {
                    ui.painter()
                        .rect_filled(tick, Rounding::same(1.0), theme::ACCENT());
                }
            }
            if resp.clicked() {
                picked = Some(stage);
            }
            resp.on_hover_text(stage.label());
            ui.add_space(theme::SP_1);
        }
    });
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

/// Where a status-strip segment wants to take you (E37).
#[derive(Clone, Copy, PartialEq)]
pub enum StatusJump {
    Library,
    Settings,
    Inbox,
}

/// Bottom status strip (mock language: storage · tier · ffmpeg · inbox).
/// `details = false` on the Shutter stage — the funnel home stays clean;
/// storage/budget trivia lives on the workspace stages. E37 — each segment
/// is a jump: storage → Library, tier → Settings, inbox count → Inbox,
/// ffmpeg-missing → Settings.
pub fn status_strip(ui: &mut Ui, snap: &StatusSnapshot, details: bool) -> Option<StatusJump> {
    let mut jump = None;
    // E36 — +2 px vertical margin: the strip is a click/drag surface, not
    // a hairline. Dragging empty strip space moves the window.
    let inner = Frame::none()
        .fill(theme::SURFACE_GLASS_DIM())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if details {
                    if ui
                        .selectable_label(
                            false,
                            RichText::new(&snap.storage_label)
                                .size(11.0)
                                .color(theme::TEXT_MUTED()),
                        )
                        .on_hover_text("Open Library")
                        .clicked()
                    {
                        jump = Some(StatusJump::Library);
                    }
                    ui.separator();
                    if ui
                        .selectable_label(
                            false,
                            RichText::new(format!("{} · {}", snap.budget_tier, snap.budget_usage))
                                .size(11.0)
                                .color(theme::TEXT_MUTED()),
                        )
                        .on_hover_text("Open Settings › Agent budget")
                        .clicked()
                    {
                        jump = Some(StatusJump::Settings);
                    }
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
                    if ui
                        .selectable_label(
                            false,
                            RichText::new("ffmpeg missing")
                                .size(11.0)
                                .color(theme::DANGER()),
                        )
                        .on_hover_text("Open Settings › Recording — install ffmpeg")
                        .clicked()
                    {
                        jump = Some(StatusJump::Settings);
                    }
                    ui.separator();
                }
                let inbox_c = if snap.pending_inbox > 0 {
                    theme::ACCENT()
                } else {
                    theme::TEXT_DIM()
                };
                if ui
                    .selectable_label(
                        false,
                        RichText::new(format!("inbox {}", snap.pending_inbox))
                            .size(11.0)
                            .color(inbox_c),
                    )
                    .on_hover_text("Open Inbox")
                    .clicked()
                {
                    jump = Some(StatusJump::Inbox);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if snap.rec_live {
                        // E15 — mono face: tabular digits keep the REC
                        // clock width stable as it ticks.
                        ui.label(
                            RichText::new(format!("● {}", snap.rec_label))
                                .font(theme::mono_font(11.0))
                                .color(theme::DANGER()),
                        );
                    } else {
                        ui.label(RichText::new("idle").size(11.0).color(theme::TEXT_DIM()));
                    }
                });
            });
        });
    // E36 — empty strip space drags the whole window (dead-space chrome).
    let strip_resp = ui.interact(
        inner.response.rect,
        egui::Id::new("status_strip_drag"),
        Sense::drag(),
    );
    if strip_resp.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    jump
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
        .inner_margin(Margin::same(14.0))
        // Tokens E2 — cards sit on the raised elevation tier (was
        // celestial-only inline shadow; now every theme has one).
        .shadow(theme::elevation_raised());
    let inner = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        theme::caps_label(ui, title);
        ui.add_space(theme::SP_2);
        add(ui);
    });
    // E19 — celestial inner-glow: a 1px top inner highlight so cards
    // read as lit glass over the cosmic canvas, not flat slabs.
    if theme::is_celestial() {
        let r = inner.response.rect;
        ui.painter().hline(
            (r.left() + 10.0)..=(r.right() - 10.0),
            r.top() + 1.0,
            Stroke::new(1.0_f32, Color32::from_white_alpha(14)),
        );
    }
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
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), theme::TEXT());
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let (fill, stroke, text) = match kind {
        BtnKind::Primary => (theme::PRIMARY(), theme::PRIMARY(), theme::PRIMARY_INK()),
        BtnKind::Secondary => (theme::SURFACE(), theme::BORDER_STRONG(), theme::TEXT()),
        BtnKind::Small => (
            theme::SURFACE(),
            theme::BORDER_STRONG(),
            theme::TEXT_MUTED(),
        ),
        BtnKind::Danger => (
            theme::DANGER_SOFT(),
            theme::DANGER_SOFT(),
            theme::ON_SOLID(),
        ),
    };
    let down = resp.is_pointer_button_down_on();
    // E10 — 80–120 ms ease on hover (mockup's --ease), flattened to a snap
    // when reduced-motion is on.
    let hov_t = if theme::reduce_motion() {
        if resp.hovered() {
            1.0
        } else {
            0.0
        }
    } else {
        ui.ctx().animate_bool(resp.id, resp.hovered() || down)
    };
    let hover_fill = match kind {
        BtnKind::Primary => theme::PRIMARY_HOVER(),
        BtnKind::Secondary | BtnKind::Small => theme::SURFACE_3(),
        BtnKind::Danger => theme::DANGER(),
    };
    let mix = |a: Color32, b: Color32, t: f32| -> Color32 {
        let t = t.clamp(0.0, 1.0);
        let (a, b) = (egui::Rgba::from(a), egui::Rgba::from(b));
        egui::Rgba::from_rgba_unmultiplied(
            a.r() + (b.r() - a.r()) * t,
            a.g() + (b.g() - a.g()) * t,
            a.b() + (b.b() - a.b()) * t,
            a.a() + (b.a() - a.a()) * t,
        )
        .into()
    };
    let fill = if down {
        match kind {
            BtnKind::Primary => theme::PRIMARY_DOWN(),
            BtnKind::Danger => theme::DANGER(),
            _ => theme::SURFACE_3(),
        }
    } else {
        mix(fill, hover_fill, hov_t)
    };
    let text = if matches!(kind, BtnKind::Small) && resp.hovered() {
        theme::TEXT()
    } else {
        text
    };

    let p = ui.painter();
    let r = theme::rounding_md();
    // E11 — pressed-state scale: primary CTAs shrink ~2% for tactile feel.
    let rect = if down && matches!(kind, BtnKind::Primary | BtnKind::Danger) {
        rect.shrink(1.2)
    } else {
        rect
    };
    // Chromie --cta-gradient: celestial primaries are the aurora itself.
    if matches!(kind, BtnKind::Primary) && theme::is_celestial() {
        theme::paint_aurora_button(p, rect, r.nw);
        if down {
            p.rect_filled(rect, r, egui::Color32::from_black_alpha(60));
        } else if resp.hovered() {
            p.rect_filled(rect, r, egui::Color32::from_black_alpha(28));
        }
        p.rect_stroke(
            rect,
            r,
            Stroke::new(1.0_f32, egui::Color32::from_white_alpha(30)),
        );
    } else {
        p.rect_filled(rect, r, fill);
        p.rect_stroke(rect, r, Stroke::new(1.0_f32, stroke));
    }
    // E8 — keyboard-focus ring: hand-drawn widgets must opt in (egui only
    // rings its own widgets). Click claims focus; Tab order picks it up.
    if resp.clicked() {
        resp.request_focus();
    }
    if resp.has_focus() {
        p.rect_stroke(
            rect.expand(2.0),
            egui::Rounding::same(r.nw + 2.0),
            Stroke::new(1.5_f32, theme::FOCUS_RING()),
        );
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
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), theme::TEXT_MUTED());
    let (rect, resp) =
        ui.allocate_exact_size(galley.size() + Vec2::new(22.0, 10.0), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let (fill, stroke, text) = if active {
        (theme::PRIMARY(), theme::PRIMARY(), theme::PRIMARY_INK())
    } else {
        (
            if resp.hovered() {
                theme::SURFACE_3()
            } else {
                theme::SURFACE()
            },
            theme::BORDER(),
            theme::TEXT_MUTED(),
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(rect.height() / 2.0), fill);
    p.rect_stroke(
        rect,
        Rounding::same(rect.height() / 2.0),
        Stroke::new(1.0_f32, stroke),
    );
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
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
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
                    let (rect, resp) = ui
                        .allocate_exact_size(galley.size() + Vec2::new(24.0, 10.0), Sense::click());
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
        let bg = if *on {
            theme::PRIMARY()
        } else {
            theme::SURFACE_3()
        };
        paint.rect_filled(rect, r, bg);
        let knob_x = if *on {
            rect.right() - r
        } else {
            rect.left() + r
        };
        let knob_c = if *on {
            theme::PRIMARY_INK()
        } else {
            theme::TEXT_MUTED()
        };
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
                    .font(theme::mono_font(11.0))
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
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
        Stroke::new(
            1.0_f32,
            if hovered {
                theme::ACCENT()
            } else {
                theme::BORDER()
            },
        ),
    );
    icons::paint_icon(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 24.0, rect.center().y),
            Vec2::splat(theme::ICON_MD),
        ),
        icon,
        if hovered {
            theme::ACCENT()
        } else {
            theme::TEXT_MUTED()
        },
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
    /// E52 — one-shot variant from the split-menu: capture once with this
    /// target without changing the persisted From selection.
    ShotFull,
    ShotRegion,
    ShotWindow,
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

                // E52 — split-menu: one-shot Region/Window/Full variants
                // without touching the From selector.
                let chev =
                    egui::Button::new(RichText::new("▾").color(theme::TEXT_MUTED()).size(13.0))
                        .fill(theme::SURFACE_2())
                        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                        .rounding(theme::rounding_md());
                let chev_resp = ui
                    .add_sized([26.0, 48.0], chev)
                    .on_hover_text("Capture once as…");
                egui::popup::popup_below_widget(
                    ui,
                    ui.make_persistent_id("shot_split_menu"),
                    &chev_resp,
                    egui::popup::PopupCloseBehavior::CloseOnClick,
                    |ui| {
                        for (label, act) in [
                            ("Full screen", ShutterAction::ShotFull),
                            ("Region…", ShutterAction::ShotRegion),
                            ("Window…", ShutterAction::ShotWindow),
                        ] {
                            if ui.button(label).clicked() {
                                action = Some(act);
                                ui.close_menu();
                            }
                        }
                    },
                );

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
                    // E15 — mono face keeps the shutter timer's digit
                    // widths stable while recording.
                    RichText::new(format!("  {}  ", rec_label))
                        .color(text_c)
                        .font(theme::mono_font(15.0)),
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
                let gif =
                    egui::Button::new(RichText::new("  GIF  ").color(theme::TEXT()).size(13.0))
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
