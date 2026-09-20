//! ⌘K / Ctrl+K command palette — filtered action list (chrome only).

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Vec2};

use super::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    GoShutter,
    GoMedia,
    GoReview,
    GoInbox,
    GoSettings,
    Screenshot,
    RepeatLast,
    CopyLastMarkdown,
    CopyLastPath,
    ToggleRecord,
    RefreshLibrary,
    ToggleDensity,
    ToggleTheme,
    ToggleRetro,
    SaveRetro,
    BugReport,
    OpenPaletteHelp,
    QuitApp,
}

impl PaletteAction {
    pub fn all() -> &'static [(Self, &'static str, &'static str)] {
        &[
            (Self::Screenshot, "Screenshot", "Capture full screen (S)"),
            (
                Self::RepeatLast,
                "Repeat last capture",
                "Re-fire the same region/window/fullscreen",
            ),
            (
                Self::CopyLastMarkdown,
                "Copy last capture as Markdown",
                "![](path) — paste-ready for docs",
            ),
            (
                Self::CopyLastPath,
                "Copy last capture path",
                "File path for still or clip",
            ),
            (Self::ToggleRecord, "Start / stop recording", "R · Ctrl+Shift+2"),
            (Self::GoShutter, "Go to Capture", "Take a screenshot or record"),
            (Self::GoMedia, "Go to Library", "All your captures"),
            (Self::GoReview, "Go to Review", "Annotate still · trim clip"),
            (Self::GoInbox, "Go to Inbox", "Agent feedback"),
            (Self::GoSettings, "Go to Settings", "Budget · shortcuts"),
            (Self::RefreshLibrary, "Refresh library", "Rescan media folder"),
            (Self::ToggleDensity, "Toggle density", "Comfortable ↔ Compact"),
            (Self::ToggleTheme, "Toggle theme", "Cycle all five themes"),
            (
                Self::ToggleRetro,
                "Toggle retro buffer",
                "Rolling last-N-seconds capture (off by default)",
            ),
            (
                Self::SaveRetro,
                "Save retro buffer as GIF",
                "Export the ring buffer to Media",
            ),
            (
                Self::BugReport,
                "Bug report pack",
                "Screenshot + retro GIF (if buffer on)",
            ),
            (Self::OpenPaletteHelp, "Palette help", "This list"),
            (
                Self::QuitApp,
                "Quit Vibecap",
                "Fully exit (the X button hides to tray)",
            ),
        ]
    }
}

/// Subsequence fuzzy score — higher is better. Word-start and consecutive
/// matches rank up; `None` when `q` isn't a subsequence of `text`.
fn fuzzy_score(q: &str, text: &str) -> Option<i32> {
    let t = text.to_lowercase();
    let tb = t.as_bytes();
    let mut score = 0i32;
    let mut ti = 0usize;
    let mut last: Option<usize> = None;
    for qc in q.chars() {
        let pos = t[ti..].find(qc).map(|p| ti + p)?;
        score += 10;
        if pos == 0 || tb[pos - 1] == b' ' {
            score += 6; // word-start hit
        }
        if last == Some(pos.wrapping_sub(1)) && pos > 0 {
            score += 6; // consecutive run
        }
        last = Some(pos);
        ti = pos + 1;
    }
    // Shorter candidates win ties.
    Some(score - t.len() as i32 / 8)
}

/// Modal command palette. Returns selected action when user confirms.
/// `mru` lists recently-run actions — surfaced as a Recent group when the
/// query is empty.
pub fn show_palette(
    ctx: &egui::Context,
    query: &mut String,
    selected: &mut usize,
    open: &mut bool,
    mru: &[PaletteAction],
) -> Option<PaletteAction> {
    if !*open {
        return None;
    }

    let mut chosen = None;
    let q = query.trim().to_lowercase();
    let filtered: Vec<_> = if q.is_empty() {
        // MRU first (deduped, in recency order), then the rest.
        let mut v: Vec<(PaletteAction, &'static str, &'static str, bool)> = Vec::new();
        for &a in mru.iter().take(3) {
            if let Some(&(_, l, h)) =
                PaletteAction::all().iter().find(|(x, _, _)| *x == a)
            {
                v.push((a, l, h, true));
            }
        }
        for &(a, l, h) in PaletteAction::all() {
            if !mru.iter().take(3).any(|m| *m == a) {
                v.push((a, l, h, false));
            }
        }
        v
    } else {
        // Fuzzy: label match beats hint match; sort by score.
        let mut scored: Vec<(i32, (PaletteAction, &'static str, &'static str, bool))> =
            PaletteAction::all()
                .iter()
                .filter_map(|&(a, l, h)| {
                    let s = fuzzy_score(&q, l)
                        .map(|s| s + 20)
                        .or_else(|| fuzzy_score(&q, h))?;
                    Some((s, (a, l, h, false)))
                })
                .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, x)| x).collect()
    };
    let filtered: Vec<(PaletteAction, &'static str, &'static str, bool)> = filtered;


    if *selected >= filtered.len() && !filtered.is_empty() {
        *selected = 0;
    }

    // Dim backdrop
    egui::Area::new(egui::Id::new("palette_backdrop"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, theme::OVERLAY_DIM());
            if resp.clicked() {
                *open = false;
            }
        });

    egui::Area::new(egui::Id::new("palette_panel"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::SURFACE())
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
                .rounding(theme::rounding_lg())
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_min_width(420.0);
                    ui.set_max_width(480.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("⌘K")
                                .size(12.0)
                                .color(theme::TEXT_DIM())
                                .strong(),
                        );
                        let te = TextEdit::singleline(query)
                            .hint_text("Type a command…")
                            .desired_width(360.0)
                            .font(egui::TextStyle::Body);
                        let r = ui.add(te);
                        r.request_focus();
                    });
                    ui.add_space(theme::SP_2);

                    ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            if filtered.is_empty() {
                                ui.label(
                                    RichText::new("No matches")
                                        .color(theme::TEXT_DIM())
                                        .size(13.0),
                                );
                                return;
                            }
                            for (i, (action, label, hint, is_mru)) in filtered.iter().enumerate() {
                                // "Recent" divider above the first MRU row.
                                if *is_mru && (i == 0 || !filtered[i - 1].3) {
                                    theme::caps_label(ui, "Recent");
                                }
                                let sel = i == *selected;
                                let fill = if sel {
                                    theme::SURFACE_3()
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                let resp = egui::Frame::none()
                                    .fill(fill)
                                    .rounding(theme::rounding_sm())
                                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                    .show(ui, |ui| {
                                        ui.set_min_width(400.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(*label)
                                                    .color(if sel {
                                                        theme::TEXT()
                                                    } else {
                                                        theme::TEXT_MUTED()
                                                    })
                                                    .strong(),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(*hint)
                                                            .size(11.0)
                                                            .color(theme::TEXT_DIM()),
                                                    );
                                                },
                                            );
                                        });
                                    })
                                    .response
                                    .interact(Sense::click());
                                if resp.clicked() {
                                    chosen = Some(*action);
                                    *open = false;
                                }
                                if resp.hovered() {
                                    *selected = i;
                                }
                            }
                        });

                    ui.add_space(theme::SP_1);
                    ui.label(
                        RichText::new("↑↓ navigate · Enter run · Esc close")
                            .size(11.0)
                            .color(theme::TEXT_DIM()),
                    );
                });
        });

    // Keyboard while open
    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            *open = false;
        }
        if i.key_pressed(Key::ArrowDown) && !filtered.is_empty() {
            *selected = (*selected + 1) % filtered.len();
        }
        if i.key_pressed(Key::ArrowUp) && !filtered.is_empty() {
            *selected = (*selected + filtered.len() - 1) % filtered.len();
        }
        if i.key_pressed(Key::Enter) && !filtered.is_empty() {
            if let Some((action, _, _, _)) = filtered.get(*selected) {
                chosen = Some(*action);
                *open = false;
            }
        }
    });

    let _ = Vec2::ZERO; // keep import useful if layout changes
    chosen
}

/// `?` / F1 modal — every shortcut, grouped. Esc / click-outside closes.
pub fn show_cheatsheet(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Area::new(egui::Id::new("cheatsheet_backdrop"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, theme::OVERLAY_DIM());
            if resp.clicked() {
                *open = false;
            }
        });

    const GROUPS: &[(&str, &[(&str, &str)])] = &[
        (
            "In the app",
            &[
                ("S", "Screenshot"),
                ("R", "Start / stop record"),
                ("Ctrl+C", "Copy last capture"),
                ("Z", "Undo delete"),
                ("Ctrl+1–5", "Jump to a stage"),
                ("Alt+←/→", "Stage back / forward"),
            ],
        ),
        (
            "Go",
            &[
                ("Ctrl+K", "Command palette"),
                ("Ctrl+I", "Inbox"),
                ("Ctrl+B", "Toggle rail"),
                ("?  ·  F1", "This sheet"),
            ],
        ),
        (
            "Region overlay",
            &[
                ("drag / release", "Select · capture"),
                ("Shift · Alt", "Square · 16:9 while dragging"),
                ("WASD · arrows", "Nudge (Shift = 10 px)"),
                ("Enter · R", "Confirm · repeat last region"),
                ("scroll", "Cycle overlapping windows (pick)"),
                ("Esc · right-click", "Cancel"),
            ],
        ),
        (
            "Global",
            &[
                ("Ctrl+Shift+3", "Screenshot"),
                ("Ctrl+Shift+2", "Record toggle"),
                ("Ctrl+Alt+V", "Show / hide window"),
            ],
        ),
    ];

    egui::Area::new(egui::Id::new("cheatsheet_panel"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::SURFACE())
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
                .rounding(theme::rounding_lg())
                .inner_margin(egui::Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_min_width(520.0);
                    ui.label(
                        RichText::new("Shortcuts")
                            .size(16.0)
                            .color(theme::TEXT())
                            .strong(),
                    );
                    ui.add_space(theme::SP_3);
                    ui.columns(2, |cols| {
                        for (i, (title, rows)) in GROUPS.iter().enumerate() {
                            let ui = &mut cols[i % 2];
                            ui.label(
                                RichText::new(*title)
                                    .size(11.0)
                                    .color(theme::TEXT_DIM())
                                    .strong(),
                            );
                            ui.add_space(theme::SP_1);
                            for (key, action) in *rows {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(*key)
                                            .size(11.0)
                                            .color(theme::ACCENT())
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new(*action)
                                            .size(12.0)
                                            .color(theme::TEXT_MUTED()),
                                    );
                                });
                            }
                            ui.add_space(theme::SP_3);
                        }
                    });
                    ui.label(
                        RichText::new("Esc or click outside to close")
                            .size(11.0)
                            .color(theme::TEXT_DIM()),
                    );
                });
        });

    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            *open = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_subsequence_scores_and_rejects() {
        assert!(fuzzy_score("gif", "Export GIF").is_some());
        assert!(fuzzy_score("rl", "Repeat last capture").is_some());
        assert!(fuzzy_score("xyz", "Screenshot").is_none());
        // Consecutive word-start hits beat scattered matches.
        let good = fuzzy_score("go lib", "Go to Library").unwrap();
        let weak = fuzzy_score("go lib", "Toggle density — comfortable ↔ compact").unwrap_or(0);
        assert!(good > weak);
    }
}
