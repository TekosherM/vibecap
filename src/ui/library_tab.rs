//! Library tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Vec2};
use std::path::PathBuf;

use crate::app::{
    category_bytes, date_group_label, default_live_dir, MediaCategory,
    MediaItem, LIBRARY_PAGE_SIZE,
};
use crate::platform::open_path;
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{empty_state, loop_position_badge};
use crate::{AppTab, VibecapApp};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Library").color(theme::TEXT()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Refresh").clicked() {
                app.refresh_library();
            }
        });
    });
    ui.add_space(4.0);

    let shot_b = category_bytes(&app.library_items, MediaCategory::Screenshot);
    let vid_b = category_bytes(&app.library_items, MediaCategory::Video)
        + category_bytes(&app.library_items, MediaCategory::Gif);
    let live = default_live_dir().display().to_string();
    let stats = app.live_stats_snapshot();
    let live_n = stats.count;
    let live_bytes = (stats.mb * 1024.0 * 1024.0) as u64;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "Stills {} · Video {} · Live frames {} ({live_n})",
                crate::app::library::format_size(shot_b),
                crate::app::library::format_size(vid_b),
                crate::app::library::format_size(live_bytes),
            ))
            .small()
            .color(theme::TEXT_MUTED()),
        );
        if live_n > 0 && ui.small_button("Free live frames").clicked() {
            let _ = std::fs::remove_dir_all(&live);
            let _ = std::fs::create_dir_all(&live);
            app.show_toast("Live frames cleared");
        }
    });
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        let cats = [
            "All",
            MediaCategory::Screenshot.label(),
            MediaCategory::Video.label(),
            MediaCategory::Gif.label(),
            MediaCategory::Audio.label(),
            MediaCategory::Note.label(),
        ];
        for cat in cats {
            let selected = app.library_filter == cat;
            let label = if cat == "All" {
                format!("All ({})", app.library_items.len())
            } else {
                let n = app
                    .library_items
                    .iter()
                    .filter(|i| i.category.label() == cat)
                    .count();
                format!("{cat} ({n})")
            };
            if ui
                .selectable_label(selected, RichText::new(label).small())
                .clicked()
            {
                app.library_filter = cat.to_string();
                app.library_show_limit = LIBRARY_PAGE_SIZE;
                app.library_confirm_clear = false;
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search").small().color(theme::TEXT_MUTED()));
        ui.add(
            egui::TextEdit::singleline(&mut app.library_search)
                .hint_text("name or note")
                .desired_width(220.0),
        );
    });
    ui.add_space(4.0);

    let filtered: Vec<MediaItem> = app.library_filtered().into_iter().cloned().collect();
    let total_filtered = filtered.len();
    let show_n = app.library_show_limit.min(total_filtered);
    let visible: Vec<MediaItem> = filtered.into_iter().take(show_n).collect();
    let selected_count = app.library_selected.len();

    ui.horizontal(|ui| {
        if ui.button("Select all shown").clicked() {
            for item in &visible {
                app.library_selected.insert(item.path.clone());
            }
        }
        if ui.button("Clear selection").clicked() {
            app.library_selected.clear();
        }
        ui.label(
            RichText::new(format!(
                "{selected_count} selected · showing {show_n} of {total_filtered}"
            ))
            .small()
            .color(theme::TEXT_DIM()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if selected_count > 0 {
                if ui
                    .button(
                        RichText::new(format!("Delete ({selected_count})"))
                            .color(theme::DANGER_SOFT()),
                    )
                    .clicked()
                {
                    let paths: Vec<_> = app.library_selected.iter().cloned().collect();
                    app.delete_library_paths(&paths);
                }
                if ui.button(format!("Reveal ({selected_count})")).clicked() {
                    let paths: Vec<_> = app.library_selected.iter().cloned().collect();
                    app.reveal_paths(&paths);
                }
            }
            if !app.library_confirm_clear {
                if ui
                    .button("Clear list…")
                    .on_hover_text("Delete all files in the current category from disk")
                    .clicked()
                {
                    app.library_confirm_clear = true;
                }
            } else {
                if ui
                    .button(RichText::new("Confirm clear").color(theme::DANGER_SOFT()).strong())
                    .clicked()
                {
                    let paths: Vec<_> = app
                        .library_filtered()
                        .into_iter()
                        .map(|i| i.path.clone())
                        .collect();
                    app.delete_library_paths(&paths);
                    app.library_confirm_clear = false;
                    app.library_show_limit = LIBRARY_PAGE_SIZE;
                }
                if ui.button("Cancel").clicked() {
                    app.library_confirm_clear = false;
                }
            }
        });
    });
    ui.separator();

    if total_filtered == 0 {
        empty_state(
            ui,
            Icon::EmptyFilm,
            "No media in this category",
            "Take a screenshot or recording — it lands here. Drag files in to import.",
        );
        return;
    }

    // Gallery grid: thumb cards grouped by date, click → Review, right-click → actions.
    let card_w = 176.0;
    let cols = ((ui.available_width() + 8.0) / card_w).floor().max(1.0) as usize;

    // Group visible items by date label (items arrive date-sorted).
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, item) in visible.iter().enumerate() {
        let g = date_group_label(item.modified_secs).to_string();
        match groups.last_mut() {
            Some((lg, v)) if *lg == g => v.push(i),
            _ => groups.push((g, vec![i])),
        }
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut open_edit: Option<PathBuf> = None;
        let mut open_still: Option<PathBuf> = None;
        let mut do_copy: Option<PathBuf> = None;
        let mut do_delete: Option<PathBuf> = None;
        let mut do_reveal: Option<PathBuf> = None;
        let mut do_open: Option<PathBuf> = None;
        let mut do_toggle_sel: Option<PathBuf> = None;

        for (glabel, idxs) in &groups {
            ui.add_space(6.0);
            ui.label(
                RichText::new(glabel)
                    .small()
                    .strong()
                    .color(theme::TEXT_MUTED()),
            );
            ui.add_space(2.0);
            for chunk in idxs.chunks(cols) {
                ui.horizontal(|ui| {
                    for &i in chunk {
                        let item = &visible[i];
                        let selected = app.library_selected.contains(&item.path);

                        let inner = egui::Frame::none()
                            .fill(if selected {
                                theme::SURFACE_3()
                            } else {
                                theme::SURFACE()
                            })
                            .rounding(theme::rounding_md())
                            .stroke(egui::Stroke::new(
                                if selected { 1.5_f32 } else { 1.0_f32 },
                                if selected { theme::ACCENT() } else { theme::BORDER() },
                            ))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui| {
                                ui.set_width(card_w - 20.0);
                                // Thumbnail (or icon tile for audio/note).
                                let thumb = crate::app::thumbs::thumb_file(&item.path);
                                let img_src = if thumb.exists() {
                                    thumb
                                } else {
                                    item.path.clone()
                                };
                                let thumb_size = Vec2::new(card_w - 20.0, 88.0);
                                let resp = if matches!(
                                    item.category,
                                    MediaCategory::Screenshot
                                        | MediaCategory::Gif
                                        | MediaCategory::Video
                                ) {
                                    let r = ui.add(
                                        egui::Image::new(format!(
                                            "file://{}",
                                            img_src.display()
                                        ))
                                        .fit_to_exact_size(thumb_size)
                                        .rounding(theme::rounding_sm())
                                        .sense(egui::Sense::click()),
                                    );
                                    if matches!(item.category, MediaCategory::Video | MediaCategory::Gif) {
                                        // ▶ badge on playable media
                                        let c = r.rect.center();
                                        ui.painter().circle_filled(
                                            c,
                                            13.0,
                                            egui::Color32::from_black_alpha(150),
                                        );
                                        ui.painter().text(
                                            c,
                                            egui::Align2::CENTER_CENTER,
                                            "▶",
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                    r
                                } else {
                                    // Icon tile for audio/note/etc.
                                    let (r, resp) = ui.allocate_exact_size(
                                        thumb_size,
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        r,
                                        theme::rounding_sm(),
                                        theme::SURFACE_2(),
                                    );
                                    ui.painter().text(
                                        r.center(),
                                        egui::Align2::CENTER_CENTER,
                                        match item.category {
                                            MediaCategory::Audio => "🎙",
                                            MediaCategory::Note => "📝",
                                            _ => "📄",
                                        },
                                        egui::FontId::proportional(26.0),
                                        theme::TEXT_DIM(),
                                    );
                                    resp
                                };

                                // Click → open in Review; right-click → actions menu.
                                let primary = match item.category {
                                    MediaCategory::Screenshot | MediaCategory::Gif => {
                                        "Open in Review"
                                    }
                                    MediaCategory::Video => "Trim in Review",
                                    _ => "Open",
                                };
                                if resp.clicked() {
                                    match item.category {
                                        MediaCategory::Screenshot => {
                                            open_still = Some(item.path.clone())
                                        }
                                        MediaCategory::Video => {
                                            open_edit = Some(item.path.clone())
                                        }
                                        MediaCategory::Gif => {
                                            open_still = Some(item.path.clone())
                                        }
                                        _ => do_open = Some(item.path.clone()),
                                    }
                                }
                                resp.context_menu(|ui| {
                                    ui.set_min_width(160.0);
                                    if ui.button(primary).clicked() {
                                        match item.category {
                                            MediaCategory::Screenshot | MediaCategory::Gif => {
                                                open_still = Some(item.path.clone())
                                            }
                                            MediaCategory::Video => {
                                                open_edit = Some(item.path.clone())
                                            }
                                            _ => do_open = Some(item.path.clone()),
                                        }
                                        ui.close_menu();
                                    }
                                    if matches!(
                                        item.category,
                                        MediaCategory::Screenshot | MediaCategory::Gif
                                    ) && ui.button("Copy image").clicked()
                                    {
                                        do_copy = Some(item.path.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("Open with default app").clicked() {
                                        do_open = Some(item.path.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("Reveal in Explorer").clicked() {
                                        do_reveal = Some(item.path.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui
                                        .button(if selected { "Deselect" } else { "Select" })
                                        .clicked()
                                    {
                                        do_toggle_sel = Some(item.path.clone());
                                        ui.close_menu();
                                    }
                                    if ui
                                        .button(
                                            RichText::new("Delete").color(theme::DANGER_SOFT()),
                                        )
                                        .clicked()
                                    {
                                        do_delete = Some(item.path.clone());
                                        ui.close_menu();
                                    }
                                });

                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let mut checked = selected;
                                    if ui.checkbox(&mut checked, "").changed() {
                                        let shift = ui.input(|i| i.modifiers.shift);
                                        if shift {
                                            if let Some(prev) = &app.library_last_click {
                                                let paths: Vec<PathBuf> = visible
                                                    .iter()
                                                    .map(|i| i.path.clone())
                                                    .collect();
                                                if let (Some(a), Some(b)) = (
                                                    paths.iter().position(|p| p == prev),
                                                    paths.iter().position(|p| p == &item.path),
                                                ) {
                                                    let (lo, hi) =
                                                        if a <= b { (a, b) } else { (b, a) };
                                                    for p in &paths[lo..=hi] {
                                                        app.library_selected.insert(p.clone());
                                                    }
                                                }
                                            }
                                        } else if checked {
                                            app.library_selected.insert(item.path.clone());
                                        } else {
                                            app.library_selected.remove(&item.path);
                                        }
                                        app.library_last_click = Some(item.path.clone());
                                    }
                                    ui.vertical(|ui| {
                                        let name = if item.name.chars().count() > 26 {
                                            format!(
                                                "{}…",
                                                item.name.chars().take(25).collect::<String>()
                                            )
                                        } else {
                                            item.name.clone()
                                        };
                                        ui.label(
                                            RichText::new(name)
                                                .size(11.0)
                                                .strong()
                                                .color(theme::TEXT()),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} · {}",
                                                    item.category.label(),
                                                    item.size_str
                                                ))
                                                .size(9.5)
                                                .color(theme::TEXT_DIM()),
                                            );
                                            loop_position_badge(ui, item.loop_position());
                                        });
                                    });
                                });
                            });
                        let _ = inner;
                        ui.add_space(8.0);
                    }
                });
                ui.add_space(2.0);
            }
        }

        if show_n < total_filtered {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if ui
                    .button(format!("Show more ({} hidden)", total_filtered - show_n))
                    .clicked()
                {
                    app.library_show_limit =
                        app.library_show_limit.saturating_add(LIBRARY_PAGE_SIZE);
                }
            });
        }

        if let Some(p) = do_delete {
            app.delete_library_paths(&[p]);
        }
        if let Some(p) = do_reveal {
            app.reveal_paths(&[p]);
        }
        if let Some(p) = do_open {
            if let Err(e) = open_path(&p) {
                app.show_toast(format!("Open failed: {e}"));
            }
        }
        if let Some(p) = do_copy {
            app.copy_image_to_clipboard(&p);
        }
        if let Some(p) = do_toggle_sel {
            if !app.library_selected.remove(&p) {
                app.library_selected.insert(p);
            }
        }
        if let Some(p) = open_edit {
            app.edit_file = Some(p.clone());
            app.current_tab = AppTab::Clip;
            app.load_filmstrip(ctx, p);
        }
        if let Some(p) = open_still {
            app.open_still_from_path(p);
        }
    });
}
