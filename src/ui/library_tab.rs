//! Library tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Vec2};
use std::path::PathBuf;

use crate::app::{
    category_bytes, date_group_label, default_live_dir, get_dir_size_bytes, MediaCategory,
    MediaItem, LIBRARY_PAGE_SIZE,
};
use crate::platform::open_path;
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{empty_state, loop_position_badge};
use crate::{AppTab, VibecapApp};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Media Library").color(theme::TEXT()).strong());
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
    let (live_bytes, live_n) = get_dir_size_bytes(&live);
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

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut open_edit: Option<PathBuf> = None;
        let mut open_still: Option<PathBuf> = None;
        let mut do_copy: Option<PathBuf> = None;
        let mut do_delete: Option<PathBuf> = None;
        let mut do_reveal: Option<PathBuf> = None;
        let mut do_open: Option<PathBuf> = None;
        let mut last_group = "";

        for item in &visible {
            let group = date_group_label(item.modified_secs);
            if group != last_group {
                last_group = group;
                ui.add_space(6.0);
                ui.label(
                    RichText::new(group)
                        .small()
                        .strong()
                        .color(theme::TEXT_MUTED()),
                );
            }
            let selected = app.library_selected.contains(&item.path);
            let row = ui.group(|ui| {
                ui.horizontal(|ui| {
                    let mut checked = selected;
                    if ui.checkbox(&mut checked, "").changed() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            if let Some(prev) = &app.library_last_click {
                                let paths: Vec<PathBuf> =
                                    visible.iter().map(|i| i.path.clone()).collect();
                                if let (Some(a), Some(b)) = (
                                    paths.iter().position(|p| p == prev),
                                    paths.iter().position(|p| p == &item.path),
                                ) {
                                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
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

                    let thumb = crate::app::thumbs::thumb_file(&item.path);
                    let img_src = if thumb.exists() {
                        thumb
                    } else {
                        item.path.clone()
                    };
                    if matches!(
                        item.category,
                        MediaCategory::Screenshot | MediaCategory::Gif | MediaCategory::Video
                    ) {
                        ui.add(
                            egui::Image::new(format!("file://{}", img_src.display()))
                                .fit_to_exact_size(Vec2::new(96.0, 54.0)),
                        );
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&item.name)
                                    .strong()
                                    .color(theme::TEXT()),
                            );
                            ui.add_space(theme::SP_2);
                            loop_position_badge(ui, item.loop_position());
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                item.category.label(),
                                item.size_str
                            ))
                            .small()
                            .color(theme::TEXT_DIM()),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Delete").on_hover_text("Delete").clicked() {
                            do_delete = Some(item.path.clone());
                        }
                        if ui
                            .button("Reveal")
                            .on_hover_text("Reveal in Explorer / Finder")
                            .clicked()
                        {
                            do_reveal = Some(item.path.clone());
                        }
                        if ui
                            .button("Open")
                            .on_hover_text("Open with the default app")
                            .clicked()
                        {
                            do_open = Some(item.path.clone());
                        }
                        match item.category {
                            MediaCategory::Video => {
                                if ui
                                    .button("Clip")
                                    .on_hover_text("Open in Clip studio")
                                    .clicked()
                                {
                                    open_edit = Some(item.path.clone());
                                }
                            }
                            MediaCategory::Gif => {
                                if ui
                                    .button("Still")
                                    .on_hover_text("Open GIF in Still studio")
                                    .clicked()
                                {
                                    open_still = Some(item.path.clone());
                                }
                                if ui
                                    .button("Clip")
                                    .on_hover_text("Trim GIF as a clip")
                                    .clicked()
                                {
                                    open_edit = Some(item.path.clone());
                                }
                            }
                            MediaCategory::Screenshot => {
                                if ui
                                    .button("Still")
                                    .on_hover_text("Open in Still studio")
                                    .clicked()
                                {
                                    open_still = Some(item.path.clone());
                                }
                                if ui
                                    .button("Copy")
                                    .on_hover_text("Copy to clipboard")
                                    .clicked()
                                {
                                    do_copy = Some(item.path.clone());
                                }
                            }
                            _ => {}
                        }
                    });
                });
            });
            if row.response.hovered() && matches!(item.category, MediaCategory::Video) {
                row.response.on_hover_text("Hover-scrub: open in Clip for the filmstrip timeline");
            }
            ui.add_space(3.0);
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
