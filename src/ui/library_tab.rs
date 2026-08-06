//! Library tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{RichText, Vec2};
use std::path::PathBuf;

use crate::app::{MediaCategory, MediaItem, LIBRARY_PAGE_SIZE};
use crate::platform::{open_path, reveal_in_file_manager};
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{empty_state, loop_position_badge};
use crate::{AppTab, VibecapApp};

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {

                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("Media Library").color(theme::ACCENT()).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🔄 Refresh").clicked() {
                                app.refresh_library();
                            }
                        });
                    });
                    ui.add_space(4.0);

                    // Category chips
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
                                let n = app.library_items.iter().filter(|i| i.category.label() == cat).count();
                                format!("{} ({})", cat, n)
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

                    let filtered: Vec<MediaItem> = app
                        .library_filtered()
                        .into_iter()
                        .cloned()
                        .collect();
                    let total_filtered = filtered.len();
                    let show_n = app.library_show_limit.min(total_filtered);
                    let visible: Vec<MediaItem> = filtered.into_iter().take(show_n).collect();
                    let selected_count = app.library_selected.len();

                    // Bulk toolbar
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
                            RichText::new(format!("{selected_count} selected · showing {show_n} of {total_filtered}"))
                                .small()
                                .color(theme::TEXT_DIM()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if selected_count > 0 {
                                if ui
                                    .button(RichText::new(format!("🗑 Delete ({selected_count})")).color(theme::DANGER_SOFT()))
                                    .clicked()
                                {
                                    let paths: Vec<_> = app.library_selected.iter().cloned().collect();
                                    app.delete_library_paths(&paths);
                                }
                                if ui.button(format!("📂 Open in Finder ({selected_count})")).clicked() {
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
                            "Capture a screenshot or recording from Shutter — it lands here.",
                        );
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut open_edit: Option<PathBuf> = None;
                            let mut open_still: Option<PathBuf> = None;
                            let mut do_copy: Option<PathBuf> = None;
                            let mut do_delete: Option<PathBuf> = None;
                            let mut do_reveal: Option<PathBuf> = None;

                            for item in &visible {
                                let selected = app.library_selected.contains(&item.path);
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        let mut checked = selected;
                                        if ui.checkbox(&mut checked, "").changed() {
                                            if checked {
                                                app.library_selected.insert(item.path.clone());
                                            } else {
                                                app.library_selected.remove(&item.path);
                                            }
                                        }

                                        if matches!(item.category, MediaCategory::Screenshot | MediaCategory::Gif) {
                                            ui.add(
                                                egui::Image::new(format!("file://{}", item.path.display()))
                                                    .fit_to_exact_size(Vec2::new(64.0, 36.0)),
                                            );
                                        }

                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} {}",
                                                        item.category.icon(),
                                                        item.name
                                                    ))
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
                                            if ui.button("🗑").on_hover_text("Delete").clicked() {
                                                do_delete = Some(item.path.clone());
                                            }
                                            if ui.button("📂").on_hover_text("Reveal in Finder").clicked() {
                                                do_reveal = Some(item.path.clone());
                                            }
                                            match item.category {
                                                MediaCategory::Video | MediaCategory::Gif => {
                                                    if ui.button("Clip").on_hover_text("Open in Clip studio").clicked() {
                                                        open_edit = Some(item.path.clone());
                                                    }
                                                }
                                                MediaCategory::Screenshot => {
                                                    if ui.button("Still").on_hover_text("Open in Still studio").clicked() {
                                                        open_still = Some(item.path.clone());
                                                    }
                                                    if ui.button("Copy").on_hover_text("Copy to clipboard").clicked() {
                                                        do_copy = Some(item.path.clone());
                                                    }
                                                }
                                                _ => {
                                                    if ui.button("Open").clicked() {
                                                        let _ = open_path(&item.path);
                                                    }
                                                }
                                            }
                                        });
                                    });
                                });
                                ui.add_space(3.0);
                            }

                            if show_n < total_filtered {
                                ui.add_space(8.0);
                                ui.vertical_centered(|ui| {
                                    if ui
                                        .button(format!(
                                            "Show more ({} hidden)",
                                            total_filtered - show_n
                                        ))
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
                            if let Some(p) = do_copy {
                                app.copy_image_to_clipboard(&p);
                            }
                            if let Some(p) = open_edit {
                                app.edit_file = Some(p.clone());
                                app.current_tab = AppTab::Clip;
                                app.load_filmstrip(ctx, p);
                            }
                            if let Some(p) = open_still {
                                app.img_source_dims = image::image_dimensions(&p)
                                    .map(|(w, h)| format!("{}×{}", w, h))
                                    .unwrap_or_default();
                                app.img_preview_params.clear();
                                app.img_preview_on = true;
                                app.img_edit_file = Some(p);
                                app.current_tab = AppTab::Still;
                            }
                        });
                    }
                
}
