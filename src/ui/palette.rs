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
            (Self::ToggleTheme, "Toggle theme", "Dark ↔ Light"),
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

/// Modal command palette. Returns selected action when user confirms.
pub fn show_palette(
    ctx: &egui::Context,
    query: &mut String,
    selected: &mut usize,
    open: &mut bool,
) -> Option<PaletteAction> {
    if !*open {
        return None;
    }

    let mut chosen = None;
    let q = query.to_lowercase();
    let filtered: Vec<_> = PaletteAction::all()
        .iter()
        .filter(|(_, label, hint)| {
            q.is_empty()
                || label.to_lowercase().contains(&q)
                || hint.to_lowercase().contains(&q)
        })
        .copied()
        .collect();

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
                            for (i, (action, label, hint)) in filtered.iter().enumerate() {
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
            if let Some((action, _, _)) = filtered.get(*selected) {
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
