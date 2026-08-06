//! First-run wizard — welcome / save dir / budget / shortcuts (Phase 3 chrome).
//!
//! One question per step, progress dots, skip link. No new deps.

use eframe::egui;
use egui::{Align2, Frame, Margin, RichText, Stroke, Vec2};
use rfd::FileDialog;

use crate::app::{load_budget, save_budget, BudgetConfig};
use crate::ui::theme;
use crate::VibecapApp;

pub const WIZARD_STEPS: u8 = 4;

/// Overlay wizard. Returns true if still open (caller should skip main chrome).
pub fn show(app: &mut VibecapApp, ctx: &egui::Context) -> bool {
    if !app.wizard_open {
        return false;
    }

    let mut finish = false;
    let mut skip = false;
    let mut next = false;
    let mut back = false;
    let step = app.wizard_step.min(WIZARD_STEPS - 1);

    // Dim behind the card (Middle order so the card sits above).
    egui::Area::new(egui::Id::new("vibecap_wizard_dim"))
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .interactable(false)
        .show(ctx, |ui| {
            let rect = ui.ctx().screen_rect();
            ui.painter().rect_filled(rect, 0.0, theme::OVERLAY_DIM());
        });

    egui::Area::new(egui::Id::new("vibecap_wizard"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let card_w = 520.0_f32.min(ui.ctx().screen_rect().width() - 40.0);
            Frame::none()
                .fill(theme::SURFACE())
                .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                .rounding(theme::rounding_lg())
                .inner_margin(Margin::symmetric(28.0, 24.0))
                .show(ui, |ui| {
                    ui.set_width(card_w);

                    // Header: brand + skip
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("◉  Vibecap")
                                .size(14.0)
                                .strong()
                                .color(theme::TEXT()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Skip for now")
                                            .size(12.0)
                                            .color(theme::TEXT_MUTED()),
                                    )
                                    .fill(theme::CANVAS())
                                    .stroke(Stroke::NONE),
                                )
                                .clicked()
                            {
                                skip = true;
                            }
                        });
                    });
                    ui.add_space(theme::SP_3);

                    // Progress dots
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for i in 0..WIZARD_STEPS {
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                            let fill = if i < step {
                                theme::SUCCESS()
                            } else if i == step {
                                theme::PRIMARY()
                            } else {
                                theme::SURFACE_3()
                            };
                            ui.painter().circle_filled(r.center(), 4.5, fill);
                        }
                        ui.add_space(theme::SP_2);
                        ui.label(
                            RichText::new(format!("Step {} of {}", step + 1, WIZARD_STEPS))
                                .size(11.0)
                                .color(theme::TEXT_DIM()),
                        );
                    });
                    ui.add_space(theme::SP_3);

                    match step {
                        0 => step_welcome(ui),
                        1 => step_save_dir(app, ui),
                        2 => step_budget(app, ui),
                        _ => step_shortcuts(ui),
                    }

                    ui.add_space(theme::SP_4);

                    // Nav
                    ui.horizontal(|ui| {
                        if step > 0 {
                            if ui.button("Back").clicked() {
                                back = true;
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let label = if step + 1 >= WIZARD_STEPS {
                                "Get started"
                            } else {
                                "Continue"
                            };
                            let btn = egui::Button::new(
                                RichText::new(label).color(theme::PRIMARY_INK()).strong(),
                            )
                            .fill(theme::PRIMARY())
                            .rounding(theme::rounding_md());
                            if ui.add(btn).clicked() {
                                if step + 1 >= WIZARD_STEPS {
                                    finish = true;
                                } else {
                                    next = true;
                                }
                            }
                        });
                    });
                });
        });

    if skip || finish {
        complete_wizard(app);
        return false;
    }
    if next {
        app.wizard_step = (app.wizard_step + 1).min(WIZARD_STEPS - 1);
    }
    if back {
        app.wizard_step = app.wizard_step.saturating_sub(1);
    }
    true
}

fn complete_wizard(app: &mut VibecapApp) {
    // Persist budget choice if user touched the tier step
    if app.wizard_budget_touched {
        let frames: u32 = app.budget_frames_input.trim().parse().unwrap_or(0);
        let mb: f64 = app.budget_mb_input.trim().parse().unwrap_or(0.0);
        let mins: u32 = app.budget_minutes_input.trim().parse().unwrap_or(0);
        let cfg = BudgetConfig {
            max_frames: frames,
            max_mb: if mb.is_finite() && mb >= 0.0 { mb } else { 0.0 },
            max_minutes: mins,
            analysis_tier: app.budget_tier.clone(),
        };
        let _ = save_budget(&cfg);
    }
    app.wizard_open = false;
    app.wizard_done = true;
    app.persist_session();
}

fn step_welcome(ui: &mut egui::Ui) {
    ui.label(
        RichText::new("Capture. Annotate. Answer your agent.")
            .size(22.0)
            .strong()
            .color(theme::TEXT()),
    );
    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new(
            "Vibecap is the room where you and an agent look at evidence together. \
             Screenshot, record, mark up, then reply in the Inbox — agents poll for your answers.",
        )
        .size(14.0)
        .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_3);
    for (title, body) in [
        ("Shutter", "Screenshot, record, or GIF from the dock or hotkeys."),
        ("Library", "Review captures with loop badges (Capture → Answer)."),
        ("Inbox", "When an agent asks, reply with chips, text, voice, or markup."),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new("▸").color(theme::TEXT_DIM()));
            ui.vertical(|ui| {
                ui.label(RichText::new(title).strong().color(theme::TEXT()));
                ui.label(RichText::new(body).small().color(theme::TEXT_MUTED()));
            });
        });
        ui.add_space(6.0);
    }
}

fn step_save_dir(app: &mut VibecapApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new("Where should captures live?")
            .size(22.0)
            .strong()
            .color(theme::TEXT()),
    );
    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new("Stills, clips, and GIFs land here. You can change this later in Settings.")
            .size(14.0)
            .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_3);
    Frame::none()
        .fill(theme::SURFACE_2())
        .rounding(theme::rounding_md())
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("SAVE LOCATION")
                    .size(11.0)
                    .strong()
                    .color(theme::TEXT_DIM()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(app.save_dir.display().to_string())
                    .size(13.0)
                    .color(theme::TEXT()),
            );
            ui.add_space(8.0);
            if ui.button("Choose folder…").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    app.save_dir = path;
                    app.refresh_library();
                }
            }
        });
}

fn step_budget(app: &mut VibecapApp, ui: &mut egui::Ui) {
    if !app.budget_loaded {
        let cfg = load_budget();
        app.budget_frames_input = cfg.max_frames.to_string();
        app.budget_mb_input = format!("{:.1}", cfg.max_mb);
        app.budget_minutes_input = cfg.max_minutes.to_string();
        app.budget_tier = cfg.analysis_tier.clone();
        app.budget_loaded = true;
    }

    ui.label(
        RichText::new("How heavy should agent analysis be?")
            .size(22.0)
            .strong()
            .color(theme::TEXT()),
    );
    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new(
            "Caps control how much live inspection agents can pull. 0 = no limit. Change anytime in Settings.",
        )
        .size(14.0)
        .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_3);

    for (id, title, desc, cost) in [
        (
            "eco",
            "Eco",
            "A still every few seconds. Cheap exploration and design callouts.",
            "~low cost",
        ),
        (
            "standard",
            "Standard",
            "Short GIF clips (~3s). Sweet spot for agent workflows.",
            "~balanced",
        ),
        (
            "intensive",
            "Intensive",
            "1s frame cadence. Demos, deep QA — watch the bill.",
            "~expensive",
        ),
    ] {
        let picked = app.budget_tier == id;
        let stroke = if picked {
            Stroke::new(1.5_f32, theme::PRIMARY())
        } else {
            Stroke::new(1.0_f32, theme::BORDER())
        };
        let fill = if picked {
            theme::SURFACE_3()
        } else {
            theme::SURFACE_2()
        };
        let resp = Frame::none()
            .fill(fill)
            .stroke(stroke)
            .rounding(theme::rounding_md())
            .inner_margin(Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(title)
                            .strong()
                            .size(14.0)
                            .color(theme::TEXT()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(cost)
                                .size(11.0)
                                .color(theme::TEXT_DIM()),
                        );
                    });
                });
                ui.label(RichText::new(desc).size(12.0).color(theme::TEXT_MUTED()));
            })
            .response
            .interact(egui::Sense::click());
        if resp.clicked() {
            app.budget_tier = id.to_string();
            app.wizard_budget_touched = true;
            // Sensible defaults per tier
            match id {
                "eco" => {
                    app.budget_frames_input = "120".into();
                    app.budget_mb_input = "50.0".into();
                    app.budget_minutes_input = "15".into();
                }
                "intensive" => {
                    app.budget_frames_input = "0".into();
                    app.budget_mb_input = "500.0".into();
                    app.budget_minutes_input = "60".into();
                }
                _ => {
                    app.budget_frames_input = "300".into();
                    app.budget_mb_input = "150.0".into();
                    app.budget_minutes_input = "30".into();
                }
            }
        }
        ui.add_space(6.0);
    }
}

fn step_shortcuts(ui: &mut egui::Ui) {
    ui.label(
        RichText::new("You're ready.")
            .size(22.0)
            .strong()
            .color(theme::TEXT()),
    );
    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new("A few shortcuts worth muscle-memory. Full list lives in Settings.")
            .size(14.0)
            .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_3);

    for (keys, action) in [
        ("S", "Screenshot (window focused)"),
        ("R", "Start / stop recording"),
        ("⌘K / Ctrl+K", "Command palette"),
        ("Ctrl+Shift+3", "Screenshot (global / tray)"),
        ("Ctrl+Shift+2", "Record (global / tray)"),
    ] {
        ui.horizontal(|ui| {
            Frame::none()
                .fill(theme::SURFACE_2())
                .rounding(theme::rounding_sm())
                .inner_margin(Margin::symmetric(8.0, 3.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(keys)
                            .size(12.0)
                            .strong()
                            .color(theme::TEXT()),
                    );
                });
            ui.label(RichText::new(action).size(13.0).color(theme::TEXT_MUTED()));
        });
        ui.add_space(6.0);
    }
    ui.add_space(theme::SP_2);
    ui.label(
        RichText::new("Next: take a screenshot, or wire your agent to vibecap MCP.")
            .small()
            .color(theme::TEXT_DIM()),
    );
}

