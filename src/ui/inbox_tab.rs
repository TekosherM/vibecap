//! Agent Inbox — dual-pane conversation UI (Phase 2).

use eframe::egui;
use egui::{Frame, Margin, RichText, Stroke, Vec2};
use std::path::PathBuf;

use crate::app::{
    feedback_responses_dir, format_feedback_answer, FeedbackRequest, FeedbackResponse,
};
use crate::platform::open_path;
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{agent_dot_color, empty_state};
use crate::VibecapApp;

const LIST_WIDTH: f32 = 280.0;

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    if !app.feedback_scanned {
        app.scan_feedback_requests();
        app.feedback_scanned = true;
    }

    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("Inbox")
                .size(22.0)
                .color(theme::TEXT())
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear closed").clicked() {
                app.clear_answered_feedback();
            }
            if ui.button("Refresh").clicked() {
                app.scan_feedback_requests();
            }
        });
    });
    ui.label(
        RichText::new(
            "Agent questions land here. Reply with chips, text, voice, or mark-up. Agents poll — nothing is pushed into chat.",
        )
        .small()
        .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_2);

    let pending: Vec<FeedbackRequest> = app
        .feedback_requests
        .iter()
        .filter(|r| r.status == "pending")
        .cloned()
        .collect();
    let closed: Vec<FeedbackRequest> = app
        .feedback_requests
        .iter()
        .filter(|r| r.status != "pending")
        .cloned()
        .collect();

    if pending.is_empty() && closed.is_empty() {
        empty_state(
            ui,
            Icon::Inbox,
            "No agent questions yet",
            "When an agent calls vibecap_request_feedback, it shows up here.",
        );
        return;
    }

    // Auto-select first pending if nothing selected
    if app.feedback_selected.is_none() {
        if let Some(first) = pending.first() {
            app.feedback_selected = Some(first.id.clone());
        }
    }

    let avail_h = ui.available_height().max(200.0);

    ui.horizontal(|ui| {
        // ── Left: thread list ────────────────────────────────────
        ui.allocate_ui_with_layout(
            Vec2::new(LIST_WIDTH, avail_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_width(LIST_WIDTH);
                ui.set_max_width(LIST_WIDTH);
                Frame::none()
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                    .rounding(theme::rounding_md())
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("inbox_list")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if !pending.is_empty() {
                                    ui.label(
                                        RichText::new(format!("Waiting ({})", pending.len()))
                                            .strong()
                                            .size(12.0)
                                            .color(theme::ACCENT()),
                                    );
                                    ui.add_space(theme::SP_2);
                                    for req in &pending {
                                        thread_row(app, ui, req, true);
                                        ui.add_space(4.0);
                                    }
                                }

                                if !closed.is_empty() {
                                    ui.add_space(theme::SP_2);
                                    ui.label(
                                        RichText::new(format!("Closed ({})", closed.len()))
                                            .strong()
                                            .size(12.0)
                                            .color(theme::TEXT_MUTED()),
                                    );
                                    ui.add_space(theme::SP_2);
                                    for req in &closed {
                                        thread_row(app, ui, req, false);
                                        ui.add_space(4.0);
                                    }
                                }
                            });
                    });
            },
        );

        ui.add_space(theme::SP_2);

        // ── Right: conversation detail ───────────────────────────
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), avail_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Frame::none()
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0_f32, theme::BORDER()))
                    .rounding(theme::rounding_md())
                    .inner_margin(Margin::symmetric(14.0, 12.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("inbox_detail")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let sel = app.feedback_selected.clone();
                                if let Some(sel_id) = sel {
                                    if let Some(req) =
                                        pending.iter().find(|r| r.id == sel_id).cloned()
                                    {
                                        show_conversation_detail(app, ui, ctx, &req);
                                    } else if let Some(req) =
                                        closed.iter().find(|r| r.id == sel_id).cloned()
                                    {
                                        show_closed_detail(app, ui, &req);
                                    } else {
                                        empty_state(
                                            ui,
                                            Icon::Inbox,
                                            "Select a thread",
                                            "Pick a request from the list to reply.",
                                        );
                                    }
                                } else {
                                    empty_state(
                                        ui,
                                        Icon::Inbox,
                                        "Select a thread",
                                        "Pick a request from the list to reply.",
                                    );
                                }
                            });
                    });
            },
        );
    });
}

fn thread_row(app: &mut VibecapApp, ui: &mut egui::Ui, req: &FeedbackRequest, pending: bool) {
    let selected = app.feedback_selected.as_deref() == Some(req.id.as_str());
    let agent = if req.agent_label.is_empty() {
        "Agent"
    } else {
        req.agent_label.as_str()
    };
    let dot = agent_dot_color(agent);
    let pri = req.priority.as_str();
    let pri_color = match pri {
        "high" => theme::DANGER(),
        "low" => theme::TEXT_DIM(),
        _ => theme::TEXT_MUTED(),
    };

    let stroke = if selected {
        Stroke::new(1.5_f32, theme::ACCENT())
    } else {
        Stroke::new(1.0_f32, theme::BORDER())
    };

    let resp = Frame::none()
        .fill(if selected {
            theme::SURFACE_3()
        } else {
            theme::SURFACE_2()
        })
        .stroke(stroke)
        .rounding(theme::rounding_sm())
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, dot);
                ui.label(
                    RichText::new(agent)
                        .strong()
                        .size(12.0)
                        .color(if pending {
                            theme::TEXT()
                        } else {
                            theme::TEXT_MUTED()
                        }),
                );
                if pending {
                    Frame::none()
                        .fill(match pri {
                            "high" => theme::PRI_HIGH_FILL(),
                            "low" => theme::SURFACE(),
                            _ => theme::PRI_NORMAL_FILL(),
                        })
                        .rounding(theme::rounding_sm())
                        .inner_margin(Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(pri.to_uppercase())
                                    .size(9.0)
                                    .strong()
                                    .color(pri_color),
                            );
                        });
                } else {
                    ui.label(
                        RichText::new(&req.status)
                            .size(10.0)
                            .color(theme::TEXT_DIM()),
                    );
                }
            });
            ui.add_space(3.0);
            let q = if req.question.len() > 72 {
                format!("{}…", &req.question[..72])
            } else {
                req.question.clone()
            };
            ui.label(
                RichText::new(q)
                    .size(12.0)
                    .color(if pending {
                        theme::TEXT()
                    } else {
                        theme::TEXT_MUTED()
                    }),
            );
            ui.label(
                RichText::new(&req.created_at)
                    .size(10.0)
                    .color(theme::TEXT_DIM()),
            );
        })
        .response
        .interact(egui::Sense::click());

    if resp.clicked() {
        app.feedback_selected = Some(req.id.clone());
        if pending {
            app.feedback_choice.clear();
        }
    }
}

fn show_closed_detail(app: &mut VibecapApp, ui: &mut egui::Ui, req: &FeedbackRequest) {
    let agent = if req.agent_label.is_empty() {
        "Agent"
    } else {
        req.agent_label.as_str()
    };
    ui.label(
        RichText::new(format!("{} · {}", agent.to_uppercase(), req.status))
            .size(12.0)
            .strong()
            .color(theme::TEXT_MUTED()),
    );
    ui.add_space(theme::SP_2);
    ui.label(RichText::new(&req.question).size(15.0).color(theme::TEXT()));
    ui.add_space(theme::SP_2);

    if !app.feedback_reply_cache.contains_key(&req.id) {
        if let Ok(s) =
            std::fs::read_to_string(feedback_responses_dir().join(format!("{}.json", req.id)))
        {
            if let Ok(resp) = serde_json::from_str::<FeedbackResponse>(&s) {
                app.feedback_reply_cache
                    .insert(req.id.clone(), format_feedback_answer(&req.id, &resp));
            }
        }
    }
    if let Some(reply) = app.feedback_reply_cache.get(&req.id).cloned() {
        Frame::none()
            .fill(theme::SURFACE_2())
            .rounding(theme::rounding_md())
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("YOUR REPLY")
                        .size(10.0)
                        .strong()
                        .color(theme::TEXT_DIM()),
                );
                ui.label(RichText::new(reply).size(13.0).color(theme::TEXT_MUTED()));
            });
    }
}

fn show_conversation_detail(
    app: &mut VibecapApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    req: &FeedbackRequest,
) {
    let agent = if req.agent_label.is_empty() {
        "Agent"
    } else {
        req.agent_label.as_str()
    };
    let dot = agent_dot_color(agent);

    // Agent message bubble
    Frame::none()
        .fill(theme::SURFACE_2())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(theme::rounding_lg())
        .inner_margin(Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 10.0, dot);
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    agent
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "?".into()),
                    egui::FontId::proportional(11.0),
                    theme::CANVAS(),
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("{} · {}", agent.to_uppercase(), req.id))
                            .size(11.0)
                            .strong()
                            .color(theme::TEXT_MUTED()),
                    );
                    ui.label(
                        RichText::new(&req.created_at)
                            .size(10.0)
                            .color(theme::TEXT_DIM()),
                    );
                });
            });
            ui.add_space(theme::SP_2);
            ui.label(RichText::new(&req.question).size(15.0).color(theme::TEXT()));
            if !req.context.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Context: {}", req.context))
                        .small()
                        .color(theme::TEXT_MUTED()),
                );
            }

            // Media card
            if !req.media_path.is_empty() {
                ui.add_space(theme::SP_2);
                let lower = req.media_path.to_lowercase();
                let is_image = [".jpg", ".jpeg", ".png", ".gif", ".webp"]
                    .iter()
                    .any(|e| lower.ends_with(e));
                Frame::none()
                    .fill(theme::SURFACE())
                    .rounding(theme::rounding_md())
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        if is_image {
                            ui.add(
                                egui::Image::new(format!("file://{}", req.media_path))
                                    .max_width(ui.available_width().min(420.0))
                                    .max_height(180.0),
                            );
                        }
                        let fname = std::path::Path::new(&req.media_path)
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| req.media_path.clone());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(fname).size(11.0).color(theme::TEXT_MUTED()),
                            );
                            if ui.small_button("Open").clicked() {
                                let _ = open_path(std::path::Path::new(&req.media_path));
                            }
                            if is_image && ui.small_button("Mark up").clicked() {
                                app.annotating_feedback_id = Some(req.id.clone());
                                app.annotate_media(ctx, PathBuf::from(&req.media_path));
                            }
                        });
                    });
            }

            // Choice chips
            if !req.options.is_empty() {
                ui.add_space(theme::SP_2);
                ui.horizontal_wrapped(|ui| {
                    for opt in &req.options {
                        let selected = app.feedback_choice == *opt;
                        let fill = if selected {
                            theme::PRIMARY()
                        } else {
                            theme::SURFACE()
                        };
                        let text_c = if selected {
                            theme::PRIMARY_INK()
                        } else {
                            theme::TEXT()
                        };
                        let btn = egui::Button::new(RichText::new(opt).color(text_c).strong())
                            .fill(fill)
                            .stroke(Stroke::new(
                                1.0_f32,
                                if selected {
                                    theme::PRIMARY()
                                } else {
                                    theme::BORDER()
                                },
                            ))
                            .rounding(theme::rounding_md());
                        if ui.add(btn).clicked() {
                            app.feedback_choice = opt.clone();
                            if app.feedback_draft.trim().is_empty() {
                                app.feedback_draft = opt.clone();
                            }
                        }
                    }
                });
            }
        });

    ui.add_space(theme::SP_3);

    // Your reply composer
    Frame::none()
        .fill(theme::SURFACE_2())
        .stroke(Stroke::new(1.0_f32, theme::BORDER()))
        .rounding(theme::rounding_lg())
        .inner_margin(Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new("YOU · REPLY")
                    .size(11.0)
                    .strong()
                    .color(theme::TEXT_MUTED()),
            );
            if !app.feedback_choice.is_empty() {
                ui.label(
                    RichText::new(format!("Choice: {}", app.feedback_choice))
                        .small()
                        .color(theme::SUCCESS()),
                );
            }
            ui.add(
                egui::TextEdit::multiline(&mut app.feedback_draft)
                    .hint_text("Reply to the agent…")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(theme::SP_2);
            ui.horizontal(|ui| {
                let is_image = !req.media_path.is_empty()
                    && [".jpg", ".jpeg", ".png", ".gif", ".webp"]
                        .iter()
                        .any(|e| req.media_path.to_lowercase().ends_with(e));
                if is_image
                    && ui
                        .button("Annotate")
                        .on_hover_text("Draw on media and send")
                        .clicked()
                {
                    app.annotating_feedback_id = Some(req.id.clone());
                    app.annotate_media(ctx, PathBuf::from(&req.media_path));
                }
                let voice_label = if app.is_recording_voice_memo {
                    RichText::new("Stop voice").color(theme::DANGER()).strong()
                } else {
                    RichText::new("Voice").color(theme::SUCCESS())
                };
                if ui.button(voice_label).clicked() {
                    let was = app.is_recording_voice_memo;
                    app.toggle_voice_memo();
                    if !was {
                        app.feedback_voice_note = app.active_voice_memo_path.clone();
                    }
                }
                if let Some(p) = &app.feedback_voice_note {
                    ui.label(
                        RichText::new(format!(
                            "🎙 {}",
                            p.file_name().unwrap_or_default().to_string_lossy()
                        ))
                        .small()
                        .color(theme::TEXT_MUTED()),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("Send").color(theme::ACCENT_INK()).strong())
                        .clicked()
                    {
                        app.submit_feedback_response(&req.id);
                    }
                    if ui.button("Dismiss").clicked() {
                        app.dismiss_feedback_request(&req.id);
                    }
                });
            });
        });
}
