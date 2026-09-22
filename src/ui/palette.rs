//! ⌘K / Ctrl+K command palette — filtered action list (chrome only).

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Vec2};

use crate::app::library::{MediaCategory, MediaItem};

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
    ToggleZen,
    ToggleTheme,
    ToggleRetro,
    SaveRetro,
    BugReport,
    OpenPaletteHelp,
    QuitApp,
    /// Jump to a media item's review stage — index into the `media` slice
    /// passed to `show_palette`.
    OpenMedia(usize),
    /// E22 — capture a named saved region — index into the `regions`
    /// slice passed to `show_palette`.
    ApplyRegion(usize),
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
            (
                Self::ToggleRecord,
                "Start / stop recording",
                "R · Ctrl+Shift+2",
            ),
            (
                Self::GoShutter,
                "Go to Capture",
                "Take a screenshot or record",
            ),
            (Self::GoMedia, "Go to Library", "All your captures"),
            (Self::GoReview, "Go to Review", "Annotate still · trim clip"),
            (Self::GoInbox, "Go to Inbox", "Agent feedback"),
            (Self::GoSettings, "Go to Settings", "Budget · shortcuts"),
            (
                Self::RefreshLibrary,
                "Refresh library",
                "Rescan media folder",
            ),
            (
                Self::ToggleDensity,
                "Toggle density",
                "Comfortable ↔ Compact",
            ),
            (
                Self::ToggleZen,
                "Toggle zen mode",
                "Hide rail + status strip — palette + hotkeys only",
            ),
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

    /// Right-aligned kbd hint shown on the row — mirrors the real binding.
    pub fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Self::Screenshot => "S",
            Self::ToggleRecord => "R",
            Self::CopyLastPath => "Ctrl+C",
            Self::GoShutter => "Ctrl+1",
            Self::GoMedia => "Ctrl+2",
            Self::GoReview => "Ctrl+3",
            Self::GoInbox => "Ctrl+I",
            Self::GoSettings => "Ctrl+5",
            Self::OpenPaletteHelp => "?",
            _ => return None,
        })
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
/// query is empty. `media` is the library list — recent captures render as
/// their own group when idle, and filenames fuzzy-match the query (#45/#66).
pub fn show_palette(
    ctx: &egui::Context,
    query: &mut String,
    selected: &mut usize,
    open: &mut bool,
    mru: &[PaletteAction],
    media: &[MediaItem],
    regions: &[(String, [i32; 4])],
) -> Option<PaletteAction> {
    if !*open {
        return None;
    }

    // Row kind: 0 = action, 1 = MRU action, 2 = media item.
    let mut chosen = None;
    let q = query.trim().to_lowercase();
    let media_hint = |m: &MediaItem| {
        format!(
            "{} — opens review · copies path",
            match m.category {
                MediaCategory::Video | MediaCategory::Gif => "clip",
                MediaCategory::Audio => "audio",
                _ => "still",
            }
        )
    };
    let filtered: Vec<(PaletteAction, String, String, u8)> = if q.is_empty() {
        // MRU first (deduped, in recency order), then recent captures,
        // then the rest of the verbs.
        let mut v: Vec<(PaletteAction, String, String, u8)> = Vec::new();
        for &a in mru.iter().take(3) {
            if let Some(&(_, l, h)) = PaletteAction::all().iter().find(|(x, _, _)| *x == a) {
                v.push((a, l.to_string(), h.to_string(), 1));
            }
        }
        for (i, m) in media.iter().take(10).enumerate() {
            v.push((
                PaletteAction::OpenMedia(i),
                m.name.clone(),
                media_hint(m),
                2,
            ));
        }
        // E22 — saved regions surface in the idle list right after media.
        for (i, (name, r)) in regions.iter().enumerate() {
            v.push((
                PaletteAction::ApplyRegion(i),
                format!("▦ {name}"),
                format!("saved region · {}×{} — captures it now", r[0], r[1]),
                3,
            ));
        }
        for &(a, l, h) in PaletteAction::all() {
            if !mru.iter().take(3).any(|m| *m == a) {
                v.push((a, l.to_string(), h.to_string(), 0));
            }
        }
        v
    } else {
        // Fuzzy: label match beats hint match; sort by score.
        let mut scored: Vec<(i32, (PaletteAction, String, String, u8))> = PaletteAction::all()
            .iter()
            .filter_map(|&(a, l, h)| {
                let s = fuzzy_score(&q, l)
                    .map(|s| s + 20)
                    .or_else(|| fuzzy_score(&q, h))?;
                Some((s, (a, l.to_string(), h.to_string(), 0)))
            })
            .collect();
        for (i, m) in media.iter().enumerate() {
            if let Some(s) = fuzzy_score(&q, &m.name) {
                scored.push((
                    s + 5,
                    (
                        PaletteAction::OpenMedia(i),
                        m.name.clone(),
                        media_hint(m),
                        2,
                    ),
                ));
            }
        }
        for (i, (name, r)) in regions.iter().enumerate() {
            if let Some(s) = fuzzy_score(&q, name) {
                scored.push((
                    s + 3,
                    (
                        PaletteAction::ApplyRegion(i),
                        format!("▦ {name}"),
                        format!("saved region · {}×{} — captures it now", r[0], r[1]),
                        3,
                    ),
                ));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, x)| x).collect()
    };

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
            ui.painter().rect_filled(screen, 0.0, theme::OVERLAY_DIM());
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

                    ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No matches")
                                    .color(theme::TEXT_DIM())
                                    .size(13.0),
                            );
                            return;
                        }
                        for (i, (action, label, hint, kind)) in filtered.iter().enumerate() {
                            // Group dividers above the first row of each kind.
                            let prev = i.checked_sub(1).map(|p| filtered[p].3);
                            if *kind != 0 && prev != Some(*kind) {
                                theme::caps_label(
                                    ui,
                                    if *kind == 1 { "Recent" } else { "Captures" },
                                );
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
                                        if *kind == 2 {
                                            ui.label(
                                                RichText::new("🖼")
                                                    .size(11.0)
                                                    .color(theme::TEXT_DIM()),
                                            );
                                        }
                                        ui.label(
                                            RichText::new(label.as_str())
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
                                                if let Some(kbd) = action.shortcut() {
                                                    ui.label(
                                                        RichText::new(kbd)
                                                            .size(10.0)
                                                            .color(theme::ACCENT())
                                                            .monospace(),
                                                    );
                                                }
                                                ui.label(
                                                    RichText::new(hint.as_str())
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
/// E223 — the Global group is generated from the live binding fields so
/// a rebound digit (or the optional Pause/PrtScn slots) can never drift
/// from what the sheet claims.
pub fn show_cheatsheet(
    ctx: &egui::Context,
    open: &mut bool,
    shot_digit: u8,
    rec_digit: u8,
    pause_digit: Option<u8>,
    prtscn: bool,
) {
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
            ui.painter().rect_filled(screen, 0.0, theme::OVERLAY_DIM());
            if resp.clicked() {
                *open = false;
            }
        });

    let mut global_rows: Vec<(String, &'static str)> = vec![
        (format!("Ctrl+Shift+{shot_digit}"), "Screenshot"),
        (format!("Ctrl+Shift+{rec_digit}"), "Record toggle"),
        ("Ctrl+Alt+V".to_string(), "Show / hide window"),
    ];
    if let Some(d) = pause_digit {
        global_rows.push((format!("Ctrl+Shift+{d}"), "Pause / resume record"));
    }
    if prtscn {
        global_rows.push(("PrtScn".to_string(), "Screenshot"));
    }

    let groups: Vec<(&str, Vec<(String, &str)>)> = vec![
        (
            "In the app",
            [
                ("S", "Screenshot"),
                ("R", "Start / stop record"),
                ("Ctrl+C", "Copy last capture"),
                ("Z", "Undo delete"),
                ("Ctrl+1–5", "Jump to a stage"),
                ("Alt+←/→", "Stage back / forward"),
            ]
            .iter()
            .map(|(k, a)| (k.to_string(), *a))
            .collect(),
        ),
        (
            "Go",
            [
                ("Ctrl+K", "Command palette"),
                ("Ctrl+I", "Inbox"),
                ("Ctrl+B", "Toggle rail"),
                ("?  ·  F1", "This sheet"),
            ]
            .iter()
            .map(|(k, a)| (k.to_string(), *a))
            .collect(),
        ),
        (
            "Region overlay",
            [
                ("drag / release", "Select · capture"),
                ("Shift · Alt", "Square · 16:9 while dragging"),
                ("WASD · arrows", "Nudge (Shift = 10 px)"),
                ("Enter · R", "Confirm · repeat last region"),
                ("scroll", "Cycle overlapping windows (pick)"),
                ("Esc · right-click", "Cancel"),
            ]
            .iter()
            .map(|(k, a)| (k.to_string(), *a))
            .collect(),
        ),
        ("Global", global_rows),
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
                        for (i, (title, rows)) in groups.iter().enumerate() {
                            let ui = &mut cols[i % 2];
                            ui.label(
                                RichText::new(*title)
                                    .size(11.0)
                                    .color(theme::TEXT_DIM())
                                    .strong(),
                            );
                            ui.add_space(theme::SP_1);
                            for (key, action) in rows {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(key)
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
