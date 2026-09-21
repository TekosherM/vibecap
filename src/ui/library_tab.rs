//! Library tab UI (extracted from main for Phase 1a).

use eframe::egui;
use egui::{Rect, RichText, Vec2};
use std::path::PathBuf;

use crate::app::{
    category_bytes, date_group_label, default_live_dir, MediaCategory, MediaItem, LIBRARY_PAGE_SIZE,
};
use crate::platform::{open_path, open_with};
use crate::ui::icons::Icon;
use crate::ui::theme;
use crate::ui::{chip, empty_state};
use crate::{AppTab, VibecapApp};

/// Sink for the shared right-click menu — one `&mut` slot per deferred
/// action so tiles and list rows share one menu implementation.
struct MenuActs<'a> {
    open_still: &'a mut Option<PathBuf>,
    open_edit: &'a mut Option<PathBuf>,
    do_open: &'a mut Option<PathBuf>,
    do_copy: &'a mut Option<PathBuf>,
    do_reveal: &'a mut Option<PathBuf>,
    do_fav: &'a mut Option<String>,
    do_flag: &'a mut Option<String>,
    do_tag_edit: &'a mut Option<String>,
    do_open_with: &'a mut Option<PathBuf>,
    do_toggle_sel: &'a mut Option<PathBuf>,
    do_delete: &'a mut Option<PathBuf>,
}

fn item_menu(
    ui: &mut egui::Ui,
    app: &VibecapApp,
    item: &MediaItem,
    selected: bool,
    acts: MenuActs<'_>,
) {
    let is_fav = app.library_favorites.contains(&item.name);
    let is_flagged = app.library_flagged.contains(&item.name);
    ui.set_min_width(160.0);
    let primary = match item.category {
        MediaCategory::Screenshot | MediaCategory::Gif => "Open in Review",
        MediaCategory::Video => "Trim in Review",
        _ => "Open",
    };
    if ui.button(primary).clicked() {
        match item.category {
            MediaCategory::Screenshot | MediaCategory::Gif => {
                *acts.open_still = Some(item.path.clone())
            }
            MediaCategory::Video => *acts.open_edit = Some(item.path.clone()),
            _ => *acts.do_open = Some(item.path.clone()),
        }
        ui.close_menu();
    }
    if matches!(
        item.category,
        MediaCategory::Screenshot | MediaCategory::Gif
    ) && ui.button("Copy image").clicked()
    {
        *acts.do_copy = Some(item.path.clone());
        ui.close_menu();
    }
    // GIFs are both a still and a clip — offer the trim/export surface too,
    // not just the editor.
    if matches!(item.category, MediaCategory::Gif) && ui.button("Trim as clip").clicked() {
        *acts.open_edit = Some(item.path.clone());
        ui.close_menu();
    }
    if ui.button("Open with default app").clicked() {
        *acts.do_open = Some(item.path.clone());
        ui.close_menu();
    }
    // E82 — OS "open with" chooser (OpenAs dialog on Windows; Finder reveal
    // on macOS).
    if ui.button("Open with…").clicked() {
        *acts.do_open_with = Some(item.path.clone());
        ui.close_menu();
    }
    if ui.button("Reveal in Explorer").clicked() {
        *acts.do_reveal = Some(item.path.clone());
        ui.close_menu();
    }
    if ui
        .button(if is_fav {
            "★ Unfavorite"
        } else {
            "☆ Favorite"
        })
        .clicked()
    {
        *acts.do_fav = Some(item.name.clone());
        ui.close_menu();
    }
    // E83 — flag/unflag for the review queue.
    if ui
        .button(if is_flagged {
            "⚑ Remove review flag"
        } else {
            "⚑ Flag for review"
        })
        .clicked()
    {
        *acts.do_flag = Some(item.name.clone());
        ui.close_menu();
    }
    // E76 — comma-separated tag editor (applies to the whole selection
    // when the item is part of one).
    if ui.button("🏷 Tags…").clicked() {
        *acts.do_tag_edit = Some(item.name.clone());
        ui.close_menu();
    }
    ui.separator();
    if ui
        .button(if selected { "Deselect" } else { "Select" })
        .clicked()
    {
        *acts.do_toggle_sel = Some(item.path.clone());
        ui.close_menu();
    }
    if ui
        .button(RichText::new("Delete").color(theme::DANGER_SOFT()))
        .clicked()
    {
        *acts.do_delete = Some(item.path.clone());
        ui.close_menu();
    }
}

pub fn show(app: &mut VibecapApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let live = default_live_dir().display().to_string();
    let stats = app.live_stats_snapshot();
    let live_n = stats.count;

    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("Library")
                .color(theme::TEXT())
                .font(egui::FontId::new(20.0, theme::font_semibold())),
        );
        ui.add_space(4.0);
        crate::ui::count_chip(ui, &format!("{}", app.library_filtered().len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Tile size — S / M / L segmented, matches the chip look.
            egui::Frame::none()
                .fill(theme::SURFACE_2())
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER()))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(2.0))
                .show(ui, |ui| {
                    for (i, lab) in ["S", "M", "L"].iter().enumerate() {
                        let on = app.library_tile_size == i as u8;
                        let resp = egui::Frame::none()
                            .fill(if on {
                                theme::SURFACE()
                            } else {
                                egui::Color32::TRANSPARENT
                            })
                            .rounding(theme::rounding_sm())
                            .inner_margin(egui::Margin::symmetric(7.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(*lab).size(10.0).color(if on {
                                    theme::TEXT()
                                } else {
                                    theme::TEXT_MUTED()
                                }));
                            })
                            .response
                            .interact(egui::Sense::click())
                            .on_hover_text("Tile size");
                        if resp.clicked() {
                            app.library_tile_size = i as u8;
                        }
                    }
                });
            ui.add_space(4.0);
            // E157 — grid / list view toggle.
            if chip(ui, if app.library_list_view { "≡" } else { "▦" }, false) {
                app.library_list_view = !app.library_list_view;
                app.persist_session();
            }
            ui.add_space(4.0);
            // Sort menu — date sorts keep the group headers; others go flat.
            crate::ui::icon_menu_button(ui, &format!("⇅ {}", app.library_sort.label()), |ui| {
                ui.set_min_width(120.0);
                for s in crate::app::LibrarySort::ALL {
                    let on = app.library_sort == s;
                    if ui
                        .selectable_label(
                            on,
                            RichText::new(s.label()).color(if on {
                                theme::ACCENT()
                            } else {
                                theme::TEXT()
                            }),
                        )
                        .clicked()
                    {
                        app.library_sort = s;
                        ui.close_menu();
                    }
                }
            });
            ui.add_space(4.0);
            crate::ui::icon_menu_button(ui, "⋯", |ui| {
                ui.set_min_width(180.0);
                if ui.button("Refresh").clicked() {
                    app.refresh_library();
                    ui.close_menu();
                }
                if ui.button("Repair thumbnails").clicked() {
                    // E170 — drop zero-byte thumbs, regenerate the missing.
                    let media: Vec<PathBuf> =
                        app.library_items.iter().map(|i| i.path.clone()).collect();
                    crate::app::thumbs::repair_thumbs(app.save_dir.clone(), media);
                    app.show_toast("Repairing thumbnails…");
                    ui.close_menu();
                }
                if live_n > 0 && ui.button("Free live frames").clicked() {
                    let _ = std::fs::remove_dir_all(&live);
                    let _ = std::fs::create_dir_all(&live);
                    app.show_toast("Live frames cleared");
                    ui.close_menu();
                }
                ui.separator();
                if !app.library_confirm_clear {
                    if ui
                        .button(RichText::new("Clear list…").color(theme::DANGER_SOFT()))
                        .on_hover_text("Delete all files in the current category from disk")
                        .clicked()
                    {
                        app.library_confirm_clear = true;
                        ui.close_menu();
                    }
                } else {
                    if ui
                        .button(
                            RichText::new("Confirm clear — deletes files")
                                .color(theme::DANGER_SOFT())
                                .strong(),
                        )
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
                        ui.close_menu();
                    }
                    if ui.button("Cancel").clicked() {
                        app.library_confirm_clear = false;
                        ui.close_menu();
                    }
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut app.library_search)
                    .hint_text("Search…")
                    .desired_width(160.0),
            );
        });
    });
    ui.add_space(4.0);

    let shot_b = category_bytes(&app.library_items, MediaCategory::Screenshot);
    let vid_b = category_bytes(&app.library_items, MediaCategory::Video)
        + category_bytes(&app.library_items, MediaCategory::Gif);
    let live_bytes = (stats.mb * 1024.0 * 1024.0) as u64;
    ui.label(
        RichText::new(format!(
            "Stills {} · Video {} · Live frames {} ({live_n})",
            crate::app::library::format_size(shot_b),
            crate::app::library::format_size(vid_b),
            crate::app::library::format_size(live_bytes),
        ))
        .font(egui::FontId::new(11.0, egui::FontFamily::Monospace))
        .color(theme::TEXT_DIM()),
    );
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
                format!("All  {}", app.library_items.len())
            } else {
                let n = app
                    .library_items
                    .iter()
                    .filter(|i| i.category.label() == cat)
                    .count();
                format!("{cat}  {n}")
            };
            if chip(ui, &label, selected) {
                app.library_filter = cat.to_string();
                app.library_show_limit = LIBRARY_PAGE_SIZE;
                app.library_confirm_clear = false;
            }
        }
        // E154 — ★ Favorites pseudo-category (file names, session-persisted).
        let fav_n = app.library_favorites.len();
        if chip(
            ui,
            &format!("★  {fav_n}"),
            app.library_filter == "★ Favorites",
        ) {
            app.library_filter = "★ Favorites".into();
            app.library_show_limit = LIBRARY_PAGE_SIZE;
            app.library_confirm_clear = false;
        }
        // E83 — ⚑ review-queue pseudo-category (same persistence as ★).
        let flag_n = app.library_flagged.len();
        if flag_n > 0
            && chip(
                ui,
                &format!("⚑  {flag_n}"),
                app.library_filter == "⚑ Review",
            )
        {
            app.library_filter = "⚑ Review".into();
            app.library_show_limit = LIBRARY_PAGE_SIZE;
            app.library_confirm_clear = false;
        }
    });

    // E76 — tag chips: a second row, only while any tags exist. Clicking a
    // chip ANDs the tag onto the category filter; clicking again clears it.
    let tag_index = app.library_tag_index();
    if !tag_index.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("🏷").size(11.0).color(theme::TEXT_MUTED()));
            for (tag, n) in tag_index.iter().take(12) {
                let on = app.library_tag_filter.as_deref() == Some(tag.as_str());
                if chip(ui, &format!("{tag} {n}"), on) {
                    app.library_tag_filter = if on { None } else { Some(tag.clone()) };
                    app.library_show_limit = LIBRARY_PAGE_SIZE;
                    app.library_confirm_clear = false;
                }
            }
            if tag_index.len() > 12 {
                ui.label(
                    RichText::new(format!("+{}", tag_index.len() - 12))
                        .small()
                        .color(theme::TEXT_DIM()),
                );
            }
        });
    }
    ui.add_space(6.0);

    let filtered: Vec<MediaItem> = app.library_filtered().into_iter().cloned().collect();
    let total_filtered = filtered.len();
    let show_n = app.library_show_limit.min(total_filtered);
    let visible: Vec<MediaItem> = filtered.into_iter().take(show_n).collect();
    let selected_count = app.library_selected.len();

    // Selection bar — only appears once you've picked something; otherwise the
    // grid is the whole story.
    if selected_count > 0 {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{selected_count} selected"))
                    .small()
                    .strong()
                    .color(theme::ACCENT()),
            );
            if ui.button(format!("Reveal ({selected_count})")).clicked() {
                let paths: Vec<_> = app.library_selected.iter().cloned().collect();
                app.reveal_paths(&paths);
            }
            if ui.button("Export ZIP").clicked() {
                app.export_selection_zip();
            }
            if ui
                .button(
                    RichText::new(format!("Delete ({selected_count})")).color(theme::DANGER_SOFT()),
                )
                .clicked()
            {
                let paths: Vec<_> = app.library_selected.iter().cloned().collect();
                app.delete_library_paths(&paths);
            }
            if ui.button("Clear selection").clicked() {
                app.library_selected.clear();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Select all shown").clicked() {
                    for item in &visible {
                        app.library_selected.insert(item.path.clone());
                    }
                }
            });
        });
        ui.separator();
    } else if show_n < total_filtered || total_filtered > LIBRARY_PAGE_SIZE {
        ui.label(
            RichText::new(format!("Showing {show_n} of {total_filtered}"))
                .small()
                .color(theme::TEXT_DIM()),
        );
    }

    // E294 — stats line: captures this week, bytes, consecutive-day streak.
    {
        let now_day = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86_400)
            .unwrap_or(0);
        let mut week_n = 0u32;
        let mut week_bytes = 0u64;
        let mut active_days = std::collections::HashSet::new();
        for item in &app.library_items {
            let d = item.modified_secs / 86_400;
            if d <= now_day {
                active_days.insert(d);
                if now_day - d < 7 {
                    week_n += 1;
                    week_bytes += item.size_bytes;
                }
            }
        }
        // Streak counts back from today (or yesterday if today is empty).
        let mut streak = 0u32;
        let mut d = if active_days.contains(&now_day) {
            now_day
        } else {
            now_day.saturating_sub(1)
        };
        while active_days.contains(&d) {
            streak += 1;
            d = d.saturating_sub(1);
        }
        if week_n > 0 {
            ui.label(
                RichText::new(format!(
                    "This week: {week_n} · {} · {}-day streak",
                    crate::app::library::format_size(week_bytes),
                    streak
                ))
                .small()
                .color(theme::TEXT_DIM()),
            );
        }
    }

    // E168 — recently-deleted shelf: the undo window (12 s) is easy to miss
    // in a toast; pin it under the toolbar while it's still live.
    let trash_n = app
        .undo_trash
        .as_ref()
        .filter(|(_, at, _)| at.elapsed() < std::time::Duration::from_secs(12))
        .map(|(paths, _, _)| paths.len())
        .unwrap_or(0);
    if trash_n > 0 {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("🗑 {trash_n} file(s) deleted"))
                    .size(11.0)
                    .color(theme::TEXT_MUTED()),
            );
            if ui.small_button("Undo").clicked() {
                app.undo_last_delete();
            }
            if ui.small_button("Dismiss").clicked() {
                app.undo_trash = None;
            }
        });
        ui.add_space(4.0);
    }

    // E167 — storage bar: per-type usage across the media root plus the
    // reclaimable cache segment (thumbs + scratch dirs regenerate).
    {
        let segs: [(MediaCategory, egui::Color32); 5] = [
            (MediaCategory::Screenshot, theme::PRIMARY()),
            (MediaCategory::Video, theme::ACCENT()),
            (MediaCategory::Gif, theme::SUCCESS()),
            (MediaCategory::Audio, theme::WARN()),
            (MediaCategory::Note, theme::TEXT_DIM()),
        ];
        let parts: Vec<(&'static str, u64, egui::Color32)> = segs
            .iter()
            .map(|(cat, color)| {
                (
                    cat.label(),
                    crate::app::library::category_bytes(&app.library_items, *cat),
                    *color,
                )
            })
            .collect();
        let media_total: u64 = parts.iter().map(|(_, b, _)| b).sum();
        let cache = app.library_reclaimable;
        let total = media_total + cache;
        if total > 0 {
            ui.horizontal(|ui| {
                let bar_w = (ui.available_width() - 210.0).max(120.0);
                let (bar_rect, bar_resp) =
                    ui.allocate_exact_size(Vec2::new(bar_w, 6.0), egui::Sense::hover());
                let bp = ui.painter_at(bar_rect);
                bp.rect_filled(bar_rect, 3.0, theme::SURFACE_2());
                let mut x = bar_rect.left();
                for (_label, bytes, color) in &parts {
                    if *bytes == 0 {
                        continue;
                    }
                    let w = (*bytes as f32 / total as f32) * bar_rect.width();
                    let r = Rect::from_min_size(
                        egui::pos2(x, bar_rect.top()),
                        Vec2::new(w, bar_rect.height()),
                    );
                    bp.rect_filled(r, 3.0, *color);
                    x += w;
                }
                if cache > 0 {
                    let w = (cache as f32 / total as f32) * bar_rect.width();
                    let r = Rect::from_min_size(
                        egui::pos2(x, bar_rect.top()),
                        Vec2::new(w, bar_rect.height()),
                    );
                    // Hatch-free dim segment — cache reads as "not real media".
                    bp.rect_filled(r, 3.0, theme::TEXT_DIM().gamma_multiply(0.45));
                }
                bar_resp.on_hover_text(format!(
                    "Screenshots {} · Videos {} · GIFs {} · Audio {} · cache {}",
                    crate::app::library::format_size(parts[0].1),
                    crate::app::library::format_size(parts[1].1),
                    crate::app::library::format_size(parts[2].1),
                    crate::app::library::format_size(parts[3].1),
                    crate::app::library::format_size(cache),
                ));
                ui.label(
                    RichText::new(crate::app::library::format_size(total))
                        .size(10.5)
                        .color(theme::TEXT_MUTED()),
                );
                if cache > 0
                    && ui
                        .small_button(format!("Clean {}", crate::app::library::format_size(cache)))
                        .on_hover_text("Delete thumbs cache + capture scratch dirs (regenerable)")
                        .clicked()
                {
                    let freed = crate::app::library::clean_reclaimable(&app.save_dir);
                    app.library_reclaimable = 0;
                    app.show_toast(format!("Freed {}", crate::app::library::format_size(freed)));
                }
            });
            ui.add_space(4.0);
        }
    }

    if total_filtered == 0 {
        empty_state(
            ui,
            Icon::EmptyFilm,
            "No media in this category",
            "Take a screenshot or recording — it lands here. Drag files in to import.",
        );
        ui.vertical_centered(|ui| {
            ui.add_space(theme::SP_2);
            if app.library_items.is_empty() {
                // G175: first-run CTA routes straight to the shutter.
                if crate::ui::components::btn_primary(ui, "Take a screenshot") {
                    app.current_tab = crate::AppTab::Capture;
                }
            } else if chip(ui, "Clear filters & search", false) {
                app.library_filter = "All".into();
                app.library_search.clear();
            }
        });
        return;
    }

    // Gallery grid: thumb cards grouped by date, click → Review, right-click → actions.
    let card_w = match app.library_tile_size {
        0 => 132.0,
        2 => 224.0,
        _ => 176.0,
    };
    let cols = ((ui.available_width() + 8.0) / card_w).floor().max(1.0) as usize;

    // Group by date label only for date sorts — Name/Size/Type go flat.
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    let by_date = app.library_sort.groups_by_date();
    for (i, item) in visible.iter().enumerate() {
        let g = if by_date {
            date_group_label(item.modified_secs).to_string()
        } else {
            String::new()
        };
        match groups.last_mut() {
            Some((lg, v)) if *lg == g => v.push(i),
            _ => groups.push((g, vec![i])),
        }
    }
    // E154 — favorites float to the top of their own group (stable sort,
    // keeps the date ordering inside each half).
    for (_, idxs) in groups.iter_mut() {
        idxs.sort_by_key(|&i| !app.library_favorites.contains(&visible[i].name));
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut open_edit: Option<PathBuf> = None;
        let mut open_still: Option<PathBuf> = None;
        let mut do_copy: Option<PathBuf> = None;
        let mut do_delete: Option<PathBuf> = None;
        let mut do_reveal: Option<PathBuf> = None;
        let mut do_open: Option<PathBuf> = None;
        let mut do_toggle_sel: Option<PathBuf> = None;
        let mut do_fav: Option<String> = None;
        let mut do_flag: Option<String> = None;
        let mut do_tag_edit: Option<String> = None;
        let mut do_open_with: Option<PathBuf> = None;
        let mut do_copy_path: Option<PathBuf> = None;

        // Right-click menu — shared by grid tiles and list rows (E157).
        // Macro builds a per-call MenuActs so the &mut borrows live only
        // inside each context_menu closure.
        macro_rules! menu {
            ($ui:expr, $item:expr, $selected:expr) => {
                item_menu(
                    $ui,
                    app,
                    $item,
                    $selected,
                    MenuActs {
                        open_still: &mut open_still,
                        open_edit: &mut open_edit,
                        do_open: &mut do_open,
                        do_copy: &mut do_copy,
                        do_reveal: &mut do_reveal,
                        do_fav: &mut do_fav,
                        do_flag: &mut do_flag,
                        do_tag_edit: &mut do_tag_edit,
                        do_open_with: &mut do_open_with,
                        do_toggle_sel: &mut do_toggle_sel,
                        do_delete: &mut do_delete,
                    },
                )
            };
        }

        // Row height is uniform across the grid — used for virtualization.
        let thumb_w = card_w - 12.0;
        let thumb_h = thumb_w * 9.0 / 16.0;
        let card_h = 6.0 + thumb_h + 4.0 + 14.0 + 12.0 + 6.0;

        if app.library_list_view {
            // E157 — compact list rows: glyph · name · tags · size, same
            // click/select/context/drag semantics as the tiles.
            let row_w = ui.available_width();
            for (glabel, idxs) in &groups {
                ui.add_space(4.0);
                if !glabel.is_empty() {
                    theme::caps_label(ui, glabel);
                    ui.add_space(2.0);
                }
                for &i in idxs {
                    let item = &visible[i];
                    let selected = app.library_selected.contains(&item.path);
                    let (rect, resp) = ui
                        .allocate_exact_size(Vec2::new(row_w, 26.0), egui::Sense::click_and_drag());
                    // Rows fully outside the scroll viewport skip paint —
                    // same virtualization trick as the grid.
                    let clip = ui.clip_rect();
                    if rect.max.y < clip.min.y - 26.0 || rect.min.y > clip.max.y + 26.0 {
                        continue;
                    }
                    let paint = ui.painter_at(rect);
                    if selected {
                        paint.rect_filled(rect, theme::rounding_sm(), theme::SURFACE_3());
                    } else if resp.hovered() {
                        paint.rect_filled(rect, theme::rounding_sm(), theme::SURFACE_2());
                    }
                    let is_fav = app.library_favorites.contains(&item.name);
                    let is_flagged = app.library_flagged.contains(&item.name);
                    let mut x = rect.min.x + 8.0;
                    let cy = rect.center().y;
                    paint.text(
                        egui::pos2(x, cy),
                        egui::Align2::LEFT_CENTER,
                        match item.category {
                            MediaCategory::Video => "🎬",
                            MediaCategory::Gif => "🌀",
                            MediaCategory::Audio => "🎙",
                            MediaCategory::Note => "📝",
                            _ => "🖼",
                        },
                        egui::FontId::proportional(12.0),
                        theme::TEXT_MUTED(),
                    );
                    x += 22.0;
                    if selected {
                        paint.circle_filled(egui::pos2(x, cy), 5.0, theme::ACCENT());
                    }
                    x += 14.0;
                    paint.text(
                        egui::pos2(x, cy),
                        egui::Align2::LEFT_CENTER,
                        &item.name,
                        egui::FontId::proportional(12.0),
                        if selected {
                            theme::ACCENT()
                        } else {
                            theme::TEXT()
                        },
                    );
                    // Right-side meta: badges · tags · size.
                    let mut rx = rect.max.x - 8.0;
                    paint.text(
                        egui::pos2(rx, cy),
                        egui::Align2::RIGHT_CENTER,
                        &item.size_str,
                        egui::FontId::new(9.5, egui::FontFamily::Monospace),
                        theme::TEXT_DIM(),
                    );
                    rx -= 68.0;
                    if let Some(tags) = app.library_tags.get(&item.name) {
                        let t = tags.join(", ");
                        paint.text(
                            egui::pos2(rx, cy),
                            egui::Align2::RIGHT_CENTER,
                            &t,
                            egui::FontId::new(9.5, egui::FontFamily::Monospace),
                            theme::ACCENT(),
                        );
                        rx -= (t.len() as f32 * 6.0 + 16.0).min(140.0);
                    }
                    let mut badges = String::new();
                    if is_fav {
                        badges.push('★');
                    }
                    if is_flagged {
                        badges.push('⚑');
                    }
                    if item.dupe {
                        badges.push('≡');
                    }
                    if !badges.is_empty() {
                        paint.text(
                            egui::pos2(rx, cy),
                            egui::Align2::RIGHT_CENTER,
                            &badges,
                            egui::FontId::proportional(11.0),
                            theme::WARN(),
                        );
                    }

                    // Same click semantics as tiles: open · ctrl select ·
                    // shift range.
                    if resp.clicked() {
                        let (multi, range) = ui.input(|i| {
                            (i.modifiers.ctrl || i.modifiers.command, i.modifiers.shift)
                        });
                        if range {
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
                            app.library_last_click = Some(item.path.clone());
                        } else if multi {
                            if selected {
                                app.library_selected.remove(&item.path);
                            } else {
                                app.library_selected.insert(item.path.clone());
                            }
                            app.library_last_click = Some(item.path.clone());
                        } else {
                            match item.category {
                                MediaCategory::Screenshot | MediaCategory::Gif => {
                                    open_still = Some(item.path.clone())
                                }
                                MediaCategory::Video => open_edit = Some(item.path.clone()),
                                _ => do_open = Some(item.path.clone()),
                            }
                        }
                    }
                    resp.context_menu(|ui| menu!(ui, item, selected));
                    // E163 — drag a row out → OS file drag (selection-aware).
                    if resp.drag_started() {
                        let paths: Vec<PathBuf> = if app.library_selected.contains(&item.path) {
                            app.library_selected.iter().cloned().collect()
                        } else {
                            vec![item.path.clone()]
                        };
                        let _ = crate::platform::start_file_drag(&paths);
                    }
                    resp.on_hover_text(format!(
                        "{}\nClick to open · Ctrl+click select · drag out to share",
                        item.name
                    ));
                    ui.add_space(2.0);
                }
            }
        } else {
            for (glabel, idxs) in &groups {
                ui.add_space(6.0);
                if !glabel.is_empty() {
                    theme::caps_label(ui, glabel);
                    ui.add_space(2.0);
                }
                for chunk in idxs.chunks(cols) {
                    // Virtualized grid (J229): rows fully outside the scroll
                    // viewport allocate their space but skip tile widgets — no
                    // image-loader calls, no hit-test rects for off-screen items.
                    let row_top = ui.cursor().min.y;
                    let clip = ui.clip_rect();
                    if row_top + card_h < clip.min.y - card_h || row_top > clip.max.y + card_h {
                        ui.allocate_space(Vec2::new(card_w * cols as f32, card_h));
                        continue;
                    }
                    ui.horizontal(|ui| {
                        for &i in chunk {
                            let item = &visible[i];
                            let selected = app.library_selected.contains(&item.path);
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(card_w, card_h),
                                egui::Sense::click_and_drag(),
                            );
                            let hovered = resp.hovered();
                            let paint = ui.painter_at(rect);
                            if selected {
                                paint.rect_filled(rect, theme::rounding_md(), theme::SURFACE_3());
                                paint.rect_stroke(
                                    rect,
                                    theme::rounding_md(),
                                    egui::Stroke::new(1.5_f32, theme::ACCENT()),
                                );
                            } else if hovered {
                                paint.rect_filled(rect, theme::rounding_md(), theme::SURFACE_2());
                            }

                            // Thumbnail — only feed the loader a decodable image
                            // (mp4 → ⚠). Videos use their .jpg thumb; missing
                            // thumbs fall back to an icon tile.
                            let thumb = crate::app::thumbs::thumb_file(&item.path);
                            let img_src: Option<PathBuf> = match item.category {
                                // Prefer the small .vibecap thumb for every media
                                // kind; fall back to the source file for images
                                // when no thumb exists yet.
                                _ if thumb.exists() => Some(thumb),
                                MediaCategory::Screenshot | MediaCategory::Gif => {
                                    Some(item.path.clone())
                                }
                                _ => None,
                            };
                            let thumb_rect = Rect::from_min_size(
                                rect.min + Vec2::new(6.0, 6.0),
                                Vec2::new(thumb_w, thumb_h),
                            );
                            if let Some(src) = img_src {
                                let uri = format!("file://{}", src.display().to_string());
                                ui.allocate_ui_at_rect(thumb_rect, |ui| {
                                    ui.add(
                                        egui::Image::new(uri)
                                            .fit_to_exact_size(thumb_rect.size())
                                            .rounding(theme::rounding_sm())
                                            .sense(egui::Sense::hover()),
                                    );
                                });
                                // Hairline so white/near-white thumbs still read
                                // as tiles on the dark canvas.
                                paint.rect_stroke(
                                    thumb_rect,
                                    theme::rounding_sm(),
                                    egui::Stroke::new(1.0_f32, theme::BORDER()),
                                );
                                if matches!(
                                    item.category,
                                    MediaCategory::Video | MediaCategory::Gif
                                ) {
                                    // ▶ badge on playable media
                                    let c = thumb_rect.center();
                                    paint.circle_filled(
                                        c,
                                        13.0,
                                        egui::Color32::from_black_alpha(150),
                                    );
                                    paint.text(
                                        c,
                                        egui::Align2::CENTER_CENTER,
                                        "▶",
                                        egui::FontId::proportional(14.0),
                                        egui::Color32::WHITE,
                                    );
                                }
                            } else {
                                // Icon tile for audio/note/etc.
                                paint.rect_filled(
                                    thumb_rect,
                                    theme::rounding_sm(),
                                    theme::SURFACE_2(),
                                );
                                paint.text(
                                    thumb_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    match item.category {
                                        MediaCategory::Audio => "🎙",
                                        MediaCategory::Note => "📝",
                                        _ => "📄",
                                    },
                                    egui::FontId::proportional(26.0),
                                    theme::TEXT_DIM(),
                                );
                            }

                            // E159 — hover-scrub: sweep the pointer across a
                            // clip tile to preview its frames (extracted
                            // off-thread, cached per path).
                            if hovered
                                && matches!(
                                    item.category,
                                    MediaCategory::Video | MediaCategory::Gif
                                )
                            {
                                if let Some(strip) = app.scrub_cache.get(&item.path) {
                                    if let Some(pos) = resp.hover_pos() {
                                        let frac = ((pos.x - thumb_rect.left())
                                            / thumb_rect.width())
                                        .clamp(0.0, 0.999);
                                        let tex = &strip[(frac * strip.len() as f32) as usize];
                                        paint.image(
                                            tex.id(),
                                            thumb_rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                } else {
                                    app.request_scrub(item.path.clone(), ui.ctx());
                                }
                            }

                            // Selection badge: accent check when selected, a faint
                            // ring hint on hover (Ctrl+click toggles).
                            let badge = Rect::from_center_size(
                                thumb_rect.left_top() + Vec2::new(11.0, 11.0),
                                Vec2::splat(16.0),
                            );
                            if selected {
                                paint.circle_filled(badge.center(), 8.0, theme::ACCENT());
                                paint.text(
                                    badge.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "✓",
                                    egui::FontId::proportional(11.0),
                                    theme::ACCENT_INK(),
                                );
                            } else if hovered {
                                paint.circle_stroke(
                                    badge.center(),
                                    7.0,
                                    egui::Stroke::new(1.5_f32, theme::TEXT_DIM()),
                                );
                            }

                            // Hover quick-actions (E160): favorite / copy path /
                            // reveal — a ghost strip pinned to the thumb's top-right.
                            let is_fav = app.library_favorites.contains(&item.name);
                            let is_flagged = app.library_flagged.contains(&item.name);
                            if hovered && !selected {
                                let strip_w = 3.0 * 22.0 + 8.0;
                                let strip = Rect::from_center_size(
                                    thumb_rect.right_top() + Vec2::new(-strip_w / 2.0 - 2.0, 13.0),
                                    Vec2::new(strip_w, 22.0),
                                );
                                let r2 = ui.allocate_ui_at_rect(strip, |ui| {
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_black_alpha(140))
                                        .rounding(theme::rounding_sm())
                                        .inner_margin(egui::Margin::same(3.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let star = ui.add(
                                                    egui::Label::new(
                                                        RichText::new(if is_fav {
                                                            "★"
                                                        } else {
                                                            "☆"
                                                        })
                                                        .size(12.0)
                                                        .color(theme::WARN()),
                                                    )
                                                    .sense(egui::Sense::click()),
                                                );
                                                if star.on_hover_text("Favorite").clicked() {
                                                    do_fav = Some(item.name.clone());
                                                }
                                                let cp = ui.add(
                                                    egui::Label::new(
                                                        RichText::new("⧉")
                                                            .size(12.0)
                                                            .color(theme::TEXT()),
                                                    )
                                                    .sense(egui::Sense::click()),
                                                );
                                                if cp.on_hover_text("Copy path").clicked() {
                                                    do_copy_path = Some(item.path.clone());
                                                }
                                                let rv = ui.add(
                                                    egui::Label::new(
                                                        RichText::new("↗")
                                                            .size(12.0)
                                                            .color(theme::TEXT()),
                                                    )
                                                    .sense(egui::Sense::click()),
                                                );
                                                if rv.on_hover_text("Reveal in folder").clicked() {
                                                    do_reveal = Some(item.path.clone());
                                                }
                                            });
                                        })
                                        .response
                                });
                                let _ = r2;
                            }
                            // Persistent ★ on favorited tiles when not hovered.
                            if is_fav && !hovered {
                                paint.text(
                                    thumb_rect.right_top() + Vec2::new(-11.0, 4.0),
                                    egui::Align2::CENTER_TOP,
                                    "★",
                                    egui::FontId::proportional(13.0),
                                    theme::WARN(),
                                );
                            }
                            // E83 — ⚑ review flag pinned top-left (always shown so
                            // the queue is scannable without hovering).
                            if is_flagged {
                                let frect = Rect::from_center_size(
                                    thumb_rect.left_top() + Vec2::new(13.0, 13.0),
                                    Vec2::new(20.0, 16.0),
                                );
                                paint.rect_filled(frect, 3.0, egui::Color32::from_black_alpha(160));
                                paint.text(
                                    frect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "⚑",
                                    egui::FontId::proportional(11.0),
                                    theme::ACCENT(),
                                );
                            }
                            // E165 — duplicate badge: same size + same head/tail
                            // fingerprint as another library item.
                            if item.dupe {
                                let drect = Rect::from_center_size(
                                    thumb_rect.left_bottom() + Vec2::new(14.0, -13.0),
                                    Vec2::new(22.0, 16.0),
                                );
                                paint.rect_filled(drect, 3.0, egui::Color32::from_black_alpha(160));
                                paint.text(
                                    drect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "≡",
                                    egui::FontId::proportional(11.0),
                                    theme::WARN(),
                                );
                            }

                            // Name + meta painted directly — no nested rows.
                            let name = if item.name.chars().count() > 26 {
                                format!("{}…", item.name.chars().take(25).collect::<String>())
                            } else {
                                item.name.clone()
                            };
                            paint.text(
                                rect.min + Vec2::new(8.0, 6.0 + thumb_h + 4.0),
                                egui::Align2::LEFT_TOP,
                                name,
                                egui::FontId::new(11.5, theme::font_semibold()),
                                theme::TEXT(),
                            );
                            let pos = item.loop_position();
                            let meta = if matches!(pos, crate::app::LoopPosition::Capture) {
                                format!("{} · {}", item.category.label(), item.size_str)
                            } else {
                                format!(
                                    "{} · {} · {}",
                                    item.category.label(),
                                    item.size_str,
                                    pos.label()
                                )
                            };
                            paint.text(
                                rect.min + Vec2::new(8.0, 6.0 + thumb_h + 4.0 + 14.0),
                                egui::Align2::LEFT_TOP,
                                meta,
                                egui::FontId::new(9.5, egui::FontFamily::Monospace),
                                theme::TEXT_DIM(),
                            );

                            // Click → open in Review; Ctrl+click toggles selection,
                            // Shift+click range-selects (Explorer semantics);
                            // right-click → actions menu.
                            if resp.clicked() {
                                let (multi, range) = ui.input(|i| {
                                    (i.modifiers.ctrl || i.modifiers.command, i.modifiers.shift)
                                });
                                if range {
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
                                    app.library_last_click = Some(item.path.clone());
                                } else if multi {
                                    if selected {
                                        app.library_selected.remove(&item.path);
                                    } else {
                                        app.library_selected.insert(item.path.clone());
                                    }
                                    app.library_last_click = Some(item.path.clone());
                                } else {
                                    match item.category {
                                        MediaCategory::Screenshot | MediaCategory::Gif => {
                                            open_still = Some(item.path.clone())
                                        }
                                        MediaCategory::Video => open_edit = Some(item.path.clone()),
                                        _ => do_open = Some(item.path.clone()),
                                    }
                                }
                            }
                            resp.context_menu(|ui| menu!(ui, item, selected));
                            // E163 — drag a tile out of the window → OS file drag
                            // (OLE CF_HDROP on Windows). Drags the whole selection
                            // when the tile is part of one.
                            if resp.drag_started() {
                                let paths: Vec<PathBuf> =
                                    if app.library_selected.contains(&item.path) {
                                        app.library_selected.iter().cloned().collect()
                                    } else {
                                        vec![item.path.clone()]
                                    };
                                let _ = crate::platform::start_file_drag(&paths);
                            }
                            resp.on_hover_text(format!(
                                "{}\nClick to open · Ctrl+click select · right-click actions",
                                item.name
                            ));
                            ui.add_space(8.0);
                        }
                    });
                    ui.add_space(2.0);
                }
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
        if let Some(name) = do_fav {
            app.toggle_library_favorite(&name);
        }
        if let Some(name) = do_flag {
            app.toggle_library_flag(&name);
        }
        if let Some(name) = do_tag_edit {
            // Prefill with the item's current tags.
            app.library_tag_edit_buf = app
                .library_tags
                .get(&name)
                .map(|t| t.join(", "))
                .unwrap_or_default();
            app.library_tag_edit = Some(name);
        }
        if let Some(p) = do_open_with {
            match open_with(&p) {
                Ok(()) => app.show_toast("Pick an app"),
                Err(e) => app.show_toast(format!("Open-with failed: {e}")),
            }
        }
        if let Some(p) = do_copy_path {
            if let Ok(mut board) = arboard::Clipboard::new() {
                if board.set_text(p.display().to_string()).is_ok() {
                    app.show_toast("Path copied");
                }
            }
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

    // E76 — tag editor: small modal. When the edited item is part of a
    // multi-selection, the tags apply to every selected file.
    if let Some(target) = app.library_tag_edit.clone() {
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new("Tags")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new(&target).small().color(theme::TEXT_MUTED()));
                ui.label(
                    RichText::new("Comma-separated — e.g. bug, release, demo")
                        .small()
                        .color(theme::TEXT_DIM()),
                );
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.library_tag_edit_buf).desired_width(260.0),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save = true;
                    }
                    if ui.button("Clear all").clicked() {
                        app.library_tag_edit_buf.clear();
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }
        if save {
            // Bulk: apply to the whole selection when the target is in it.
            let in_selection = app.library_selected.iter().any(|p| {
                p.file_name()
                    .map(|f| f.to_string_lossy() == target)
                    .unwrap_or(false)
            });
            let names: Vec<String> = if in_selection && app.library_selected.len() > 1 {
                app.library_selected
                    .iter()
                    .filter_map(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
                    .collect()
            } else {
                vec![target]
            };
            let buf = app.library_tag_edit_buf.clone();
            app.set_library_tags(&names, &buf);
            app.library_tag_edit = None;
            app.library_tag_edit_buf.clear();
        } else if !open {
            app.library_tag_edit = None;
            app.library_tag_edit_buf.clear();
        }
    }
}
