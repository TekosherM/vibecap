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

/// Which bucket the thread list shows. Snoozed requests are their own bucket —
/// without this they vanished from the UI entirely until the snooze expired.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum InboxFilter {
    #[default]
    All,
    Pending,
    Snoozed,
    Closed,
}

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    if !app.feedback_scanned {
        app.scan_feedback_requests();
        app.feedback_scanned = true;
    }

    // j/k move the thread list; a selects the first pending.
    let typing = ctx.wants_keyboard_input();
    let jump = ctx.input(|i| {
        if typing {
            0
        } else if i.key_pressed(egui::Key::J) {
            1
        } else if i.key_pressed(egui::Key::K) {
            -1
        } else {
            0
        }
    });
    // Esc returns from a thread detail to the list; `a` picks the first
    // choice chip so a reply can be driven entirely from the keyboard.
    if !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.feedback_selected = None;
        // Mark as an explicit user choice so auto-select doesn't immediately
        // re-pick the first pending thread underneath the deselect.
        app.feedback_user_picked = true;
        app.feedback_new_arrived = false;
    }
    if !typing && ctx.input(|i| i.key_pressed(egui::Key::A)) {
        if let Some(id) = &app.feedback_selected {
            if let Some(req) = app.feedback_requests.iter().find(|r| &r.id == id) {
                if let Some(opt) = req.options.first().cloned() {
                    app.feedback_choice = opt.clone();
                    if app.feedback_draft.trim().is_empty() {
                        app.feedback_draft = opt;
                    }
                }
            }
        }
    }
    if jump != 0 && !app.feedback_requests.is_empty() {
        let ids: Vec<String> = app.feedback_requests.iter().map(|r| r.id.clone()).collect();
        let cur = app
            .feedback_selected
            .as_ref()
            .and_then(|id| ids.iter().position(|x| x == id))
            .unwrap_or(0);
        let next = (cur as i32 + jump).clamp(0, ids.len() as i32 - 1) as usize;
        app.feedback_selected = Some(ids[next].clone());
        app.feedback_user_picked = true;
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
            let quiet_label = if app.inbox_quiet {
                RichText::new("Quiet ●").color(theme::ACCENT())
            } else {
                RichText::new("Quiet ○").color(theme::TEXT_MUTED())
            };
            if ui
                .button(quiet_label)
                .on_hover_text(
                    "Suppress notify/toast/auto-open on new questions — badge still counts",
                )
                .clicked()
            {
                app.inbox_quiet = !app.inbox_quiet;
                app.persist_session();
            }
        });
    });
    let poll_note = crate::app::feedback_last_poll_secs()
        .map(|s| format!("Agent last polled {s}s ago"))
        .unwrap_or_else(|| "Agent has not polled yet".into());
    ui.label(
        RichText::new(format!(
            "Agent questions land here. Reply with chips, text, voice, or mark-up. {poll_note}."
        ))
        .small()
        .color(theme::TEXT_MUTED()),
    );
    ui.label(
        RichText::new("j/k move · a picks first choice · Esc deselects · Ctrl+Enter sends")
            .size(10.0)
            .color(theme::TEXT_DIM()),
    );
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search").small().color(theme::TEXT_DIM()));
        ui.add(
            egui::TextEdit::singleline(&mut app.inbox_search)
                .hint_text("question or answer")
                .desired_width(220.0),
        );
    });
    if !app.inbox_snippets.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Snippets").small().color(theme::TEXT_DIM()));
            let snaps = app.inbox_snippets.clone();
            for s in snaps {
                if ui.small_button(&s).clicked() {
                    if !app.feedback_draft.is_empty() {
                        app.feedback_draft.push(' ');
                    }
                    app.feedback_draft.push_str(&s);
                }
            }
        });
    }
    ui.add_space(theme::SP_2);

    let q = app.inbox_search.trim().to_ascii_lowercase();
    let now = std::time::Instant::now();
    let matches_q = |r: &FeedbackRequest| {
        q.is_empty()
            || r.question.to_ascii_lowercase().contains(&q)
            || r.id.to_ascii_lowercase().contains(&q)
    };
    let snoozed_now = |r: &FeedbackRequest| {
        app.feedback_snooze_until
            .get(&r.id)
            .map(|t| *t > now)
            .unwrap_or(false)
    };
    let pending: Vec<FeedbackRequest> = app
        .feedback_requests
        .iter()
        .filter(|r| r.status == "pending" && !snoozed_now(r))
        .filter(|r| matches_q(r))
        .cloned()
        .collect();
    let snoozed: Vec<FeedbackRequest> = app
        .feedback_requests
        .iter()
        .filter(|r| r.status == "pending" && snoozed_now(r))
        .filter(|r| matches_q(r))
        .cloned()
        .collect();
    let mut pending = pending;
    pending.sort_by_key(|r| {
        if app.feedback_pinned.contains(&r.id) {
            0
        } else {
            1
        }
    });
    let closed: Vec<FeedbackRequest> = app
        .feedback_requests
        .iter()
        .filter(|r| r.status != "pending")
        .filter(|r| matches_q(r))
        .cloned()
        .collect();

    // Filter chips — which buckets the list shows.
    let show_p = matches!(app.inbox_filter, InboxFilter::All | InboxFilter::Pending);
    let show_s = matches!(app.inbox_filter, InboxFilter::All | InboxFilter::Snoozed);
    let show_c = matches!(app.inbox_filter, InboxFilter::All | InboxFilter::Closed);
    ui.horizontal_wrapped(|ui| {
        for (filter, label, count) in [
            (
                InboxFilter::All,
                "All",
                pending.len() + snoozed.len() + closed.len(),
            ),
            (InboxFilter::Pending, "Pending", pending.len()),
            (InboxFilter::Snoozed, "Snoozed", snoozed.len()),
            (InboxFilter::Closed, "Closed", closed.len()),
        ] {
            if crate::ui::components::chip(
                ui,
                &format!("{label} {count}"),
                app.inbox_filter == filter,
            ) {
                app.inbox_filter = filter;
            }
        }
    });
    ui.add_space(theme::SP_2);

    if pending.is_empty() && snoozed.is_empty() && closed.is_empty() {
        empty_state(
            ui,
            Icon::Inbox,
            "No agent questions yet",
            "When an agent calls vibecap_request_feedback, it shows up here.",
        );
        return;
    }

    // Auto-select first pending only while selection is untouched; an explicit
    // user pick suppresses silent jumps until a brand-new request arrives.
    let composing = !app.feedback_draft.trim().is_empty() || !app.feedback_choice.is_empty();
    if app.feedback_selected.is_none()
        && (!app.feedback_user_picked || app.feedback_new_arrived)
        && !composing
    {
        if let Some(first) = pending.first() {
            app.feedback_selected = Some(first.id.clone());
            app.feedback_new_arrived = false;
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
                                if show_p && !pending.is_empty() {
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

                                if show_s && !snoozed.is_empty() {
                                    ui.add_space(theme::SP_2);
                                    ui.label(
                                        RichText::new(format!("Snoozed ({})", snoozed.len()))
                                            .strong()
                                            .size(12.0)
                                            .color(theme::TEXT_MUTED()),
                                    );
                                    ui.add_space(theme::SP_2);
                                    for req in &snoozed {
                                        thread_row(app, ui, req, true);
                                        ui.add_space(4.0);
                                    }
                                }

                                if show_c && !closed.is_empty() {
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
                                    if let Some(req) = pending
                                        .iter()
                                        .chain(snoozed.iter())
                                        .find(|r| r.id == sel_id)
                                        .cloned()
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

/// Seconds since a request was filed (`created_at` is `%Y-%m-%d %H:%M:%S`
/// local). None when the stamp doesn't parse — treat as no tint.
fn thread_age_secs(created_at: &str) -> Option<i64> {
    let naive =
        chrono::NaiveDateTime::parse_from_str(created_at.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
    let age = chrono::Local::now().naive_local() - naive;
    Some(age.num_seconds().max(0))
}

/// Compact age label — "3m", "1h", "2d".
fn rel_age(secs: i64) -> String {
    if secs < 3600 {
        format!("{}m", (secs / 60).max(1))
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
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
                ui.label(RichText::new(agent).strong().size(12.0).color(if pending {
                    theme::TEXT()
                } else {
                    theme::TEXT_MUTED()
                }));
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
            ui.label(RichText::new(q).size(12.0).color(if pending {
                theme::TEXT()
            } else {
                theme::TEXT_MUTED()
            }));
            // SLA age tint: pending threads go amber after 10 min, red after
            // 30 — a stale question should be visible at a glance.
            let age_color = if pending {
                match thread_age_secs(&req.created_at) {
                    Some(s) if s > 30 * 60 => theme::DANGER(),
                    Some(s) if s > 10 * 60 => theme::WARN(),
                    _ => theme::TEXT_DIM(),
                }
            } else {
                theme::TEXT_DIM()
            };
            let stamp = match thread_age_secs(&req.created_at) {
                Some(s) if pending => format!("{} · {}", req.created_at, rel_age(s)),
                _ => req.created_at.clone(),
            };
            ui.label(RichText::new(stamp).size(10.0).color(age_color));
        })
        .response
        .interact(egui::Sense::click());

    if resp.clicked() {
        app.feedback_selected = Some(req.id.clone());
        // Explicit navigation wins over auto-select.
        app.feedback_user_picked = true;
        app.feedback_new_arrived = false;
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let id = req.id.clone();
                    let pinned = app.feedback_pinned.contains(&id);
                    if ui
                        .small_button(if pinned { "Unpin" } else { "Pin" })
                        .clicked()
                    {
                        if pinned {
                            app.feedback_pinned.remove(&id);
                        } else {
                            app.feedback_pinned.insert(id.clone());
                        }
                    }
                    egui::menu::menu_button(ui, "Snooze ▾", |ui| {
                        for (label, secs) in [
                            ("15 minutes", 15 * 60),
                            ("1 hour", 3600),
                            ("4 hours", 4 * 3600),
                        ] {
                            if ui.button(label).clicked() {
                                app.feedback_snooze_until.insert(
                                    id.clone(),
                                    std::time::Instant::now()
                                        + std::time::Duration::from_secs(secs),
                                );
                                ui.close_menu();
                            }
                        }
                    });
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
                            ui.label(RichText::new(fname).size(11.0).color(theme::TEXT_MUTED()));
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
                    .hint_text("Reply to the agent… (`code`, https://link) — Ctrl+Enter sends")
                    .desired_width(f32::INFINITY),
            );
            if ctx.input(|i| {
                (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Enter)
            }) {
                app.submit_feedback_response(&req.id);
            }
            ui.label(
                RichText::new("Markdown-lite: backticks and one URL are passed through as-is.")
                    .small()
                    .color(theme::TEXT_DIM()),
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

#[cfg(test)]
mod tests {
    use super::{rel_age, thread_age_secs};

    #[test]
    fn rel_age_buckets() {
        assert_eq!(rel_age(30), "1m");
        assert_eq!(rel_age(599), "9m");
        assert_eq!(rel_age(3600), "1h");
        assert_eq!(rel_age(172800), "2d");
    }

    #[test]
    fn thread_age_parses_stamp() {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        assert!(thread_age_secs(&now).unwrap() < 5);
        assert!(thread_age_secs("not a date").is_none());
    }
}
