//! Safelight / Graphite design tokens.
//!
//! **Gate G4:** raw `Color32::from_rgb` belongs here only (plus rare TRANSPARENT).
//! Accent is reserved for **live** states (recording, agent waiting, pending inbox).
//!
//! Color tokens are **functions** so dark/light can switch at runtime without
//! rewriting every paint call. Spacing stays const.

use std::cell::Cell;

use egui::{Color32, FontId, Rounding, Stroke, Visuals};

// ── Theme mode ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

thread_local! {
    static THEME_MODE: Cell<ThemeMode> = const { Cell::new(ThemeMode::Dark) };
}

pub fn theme_mode() -> ThemeMode {
    THEME_MODE.with(|c| c.get())
}

pub fn set_theme_mode(mode: ThemeMode) {
    THEME_MODE.with(|c| c.set(mode));
}

pub fn is_light() -> bool {
    theme_mode() == ThemeMode::Light
}

/// Apply current mode's egui visuals + token table.
pub fn apply_current_theme(ctx: &egui::Context) {
    match theme_mode() {
        ThemeMode::Dark => apply_graphite_theme(ctx),
        ThemeMode::Light => apply_light_theme(ctx),
    }
}

macro_rules! dual {
    ($name:ident, $dark:expr, $light:expr) => {
        #[inline]
        #[allow(non_snake_case)]
        pub fn $name() -> Color32 {
            if is_light() {
                $light
            } else {
                $dark
            }
        }
    };
}

// ── Canvas ──────────────────────────────────────────────────────────

dual!(
    CANVAS,
    Color32::from_rgb(0x12, 0x13, 0x16),
    Color32::from_rgb(0xf6, 0xf5, 0xf2)
);
dual!(
    SURFACE,
    Color32::from_rgb(0x1a, 0x1c, 0x21),
    Color32::from_rgb(0xff, 0xff, 0xff)
);
dual!(
    SURFACE_2,
    Color32::from_rgb(0x22, 0x25, 0x2c),
    Color32::from_rgb(0xee, 0xed, 0xe8)
);
dual!(
    SURFACE_3,
    Color32::from_rgb(0x2c, 0x30, 0x38),
    Color32::from_rgb(0xe4, 0xe2, 0xda)
);

// ── Text ────────────────────────────────────────────────────────────

dual!(
    TEXT,
    Color32::from_rgb(0xe8, 0xea, 0xef),
    Color32::from_rgb(0x1c, 0x1e, 0x24)
);
dual!(
    TEXT_MUTED,
    Color32::from_rgb(0x9a, 0xa0, 0xad),
    Color32::from_rgb(0x5c, 0x62, 0x70)
);
dual!(
    TEXT_DIM,
    Color32::from_rgb(0x6b, 0x72, 0x80),
    Color32::from_rgb(0x8a, 0x90, 0x9c)
);

// ── Brand / live accent (same in both modes — live always reads) ────

dual!(
    ACCENT,
    Color32::from_rgb(0xf5, 0x9e, 0x4b),
    Color32::from_rgb(0xd4, 0x7a, 0x20)
);
dual!(
    ACCENT_INK,
    Color32::from_rgb(0x14, 0x12, 0x0e),
    Color32::from_rgb(0x14, 0x12, 0x0e)
);
dual!(
    PRIMARY,
    Color32::from_rgb(0xe8, 0xea, 0xef),
    Color32::from_rgb(0x1c, 0x1e, 0x24)
);
dual!(
    PRIMARY_INK,
    Color32::from_rgb(0x14, 0x16, 0x1a),
    Color32::from_rgb(0xf6, 0xf5, 0xf2)
);
dual!(
    BORDER,
    Color32::from_rgba_premultiplied(232, 234, 239, 28),
    Color32::from_rgba_premultiplied(28, 30, 36, 40)
);
dual!(
    SELECTION_FILL,
    Color32::from_rgba_premultiplied(245, 158, 75, 60),
    Color32::from_rgba_premultiplied(212, 122, 32, 50)
);
dual!(
    OVERLAY_DIM,
    Color32::from_black_alpha(100),
    Color32::from_black_alpha(90)
);
dual!(
    OVERLAY_LABEL,
    Color32::from_black_alpha(180),
    Color32::from_rgba_premultiplied(28, 30, 36, 200)
);
dual!(
    OVERLAY_BLUR,
    Color32::from_black_alpha(220),
    Color32::from_rgba_premultiplied(28, 30, 36, 220)
);
dual!(
    NEUTRAL_STROKE,
    Color32::from_rgb(0x9a, 0xa0, 0xad),
    Color32::from_rgb(0x5c, 0x62, 0x70)
);

// ── Semantic (shared) ───────────────────────────────────────────────

dual!(
    SUCCESS,
    Color32::from_rgb(0x5e, 0xc2, 0x6a),
    Color32::from_rgb(0x2f, 0x9e, 0x44)
);
dual!(
    WARN,
    Color32::from_rgb(0xd8, 0xa4, 0x41),
    Color32::from_rgb(0xb8, 0x86, 0x0b)
);
dual!(
    DANGER,
    Color32::from_rgb(0xe0, 0x55, 0x55),
    Color32::from_rgb(0xc9, 0x2a, 0x2a)
);
dual!(
    DANGER_SOFT,
    Color32::from_rgb(0xe8, 0x3b, 0x3b),
    Color32::from_rgb(0xe0, 0x31, 0x31)
);
dual!(
    INFO,
    Color32::from_rgb(0x6b, 0xa3, 0xe8),
    Color32::from_rgb(0x3b, 0x7d, 0xd8)
);
dual!(
    AGENT_TEAL,
    Color32::from_rgb(0x7e, 0xc8, 0xe3),
    Color32::from_rgb(0x0c, 0x85, 0x9a)
);
dual!(
    ON_SOLID,
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xff, 0xff, 0xff)
);

// ── Annotation / loop (shared hues) ─────────────────────────────────

dual!(
    ANNO_BUG,
    Color32::from_rgb(0xe0, 0x55, 0x55),
    Color32::from_rgb(0xc9, 0x2a, 0x2a)
);
#[inline]
#[allow(non_snake_case)]
pub fn ANNO_QUESTION() -> Color32 {
    ACCENT()
}
#[inline]
#[allow(non_snake_case)]
pub fn ANNO_APPROVE() -> Color32 {
    SUCCESS()
}
#[inline]
#[allow(non_snake_case)]
pub fn ANNO_NOTE() -> Color32 {
    INFO()
}
dual!(
    LOOP_ANNOTATE,
    Color32::from_rgb(0xb4, 0x8c, 0xdc),
    Color32::from_rgb(0x7c, 0x4d, 0xbf)
);
dual!(
    LOOP_ANNOTATE_TEXT,
    Color32::from_rgb(0xd0, 0xb8, 0xee),
    Color32::from_rgb(0x5f, 0x3d, 0x9b)
);
dual!(
    LOOP_ANNOTATE_FILL,
    Color32::from_rgba_premultiplied(180, 140, 220, 40),
    Color32::from_rgba_premultiplied(124, 77, 191, 36)
);
dual!(
    LOOP_REVIEW_FILL,
    Color32::from_rgba_premultiplied(107, 163, 232, 40),
    Color32::from_rgba_premultiplied(59, 125, 216, 36)
);
dual!(
    LOOP_ASK_FILL,
    Color32::from_rgba_premultiplied(245, 158, 75, 40),
    Color32::from_rgba_premultiplied(212, 122, 32, 36)
);
dual!(
    LOOP_ANSWERED_FILL,
    Color32::from_rgba_premultiplied(94, 194, 106, 40),
    Color32::from_rgba_premultiplied(47, 158, 68, 36)
);
dual!(
    PRI_HIGH_FILL,
    Color32::from_rgba_premultiplied(0xe0, 0x55, 0x55, 40),
    Color32::from_rgba_premultiplied(0xc9, 0x2a, 0x2a, 36)
);
dual!(
    PRI_NORMAL_FILL,
    Color32::from_rgba_premultiplied(0x9a, 0xa0, 0xad, 40),
    Color32::from_rgba_premultiplied(0x5c, 0x62, 0x70, 36)
);
dual!(
    SURFACE_GLASS,
    Color32::from_rgba_premultiplied(0x1a, 0x1c, 0x21, 230),
    Color32::from_rgba_premultiplied(0xff, 0xff, 0xff, 235)
);
dual!(
    SURFACE_GLASS_DIM,
    Color32::from_rgba_premultiplied(0x1a, 0x1c, 0x21, 220),
    Color32::from_rgba_premultiplied(0xff, 0xff, 0xff, 220)
);

// ── Spacing (8pt grid) ──────────────────────────────────────────────

pub const SP_1: f32 = 4.0;
pub const SP_2: f32 = 8.0;
pub const SP_3: f32 = 12.0;
pub const SP_4: f32 = 16.0;
pub const SP_5: f32 = 24.0;
pub const SP_6: f32 = 32.0;

// ── Radius ──────────────────────────────────────────────────────────

pub const R_SM: f32 = 6.0;
pub const R_MD: f32 = 10.0;
pub const R_LG: f32 = 14.0;

pub fn rounding_sm() -> Rounding {
    Rounding::same(R_SM)
}
pub fn rounding_md() -> Rounding {
    Rounding::same(R_MD)
}
pub fn rounding_lg() -> Rounding {
    Rounding::same(R_LG)
}

// ── Type scale ──────────────────────────────────────────────────────

pub fn font_xs() -> FontId {
    FontId::proportional(12.0)
}
pub fn font_sm() -> FontId {
    FontId::proportional(13.0)
}
pub fn font_md() -> FontId {
    FontId::proportional(15.0)
}
pub fn font_lg() -> FontId {
    FontId::proportional(18.0)
}
pub fn font_xl() -> FontId {
    FontId::proportional(24.0)
}
pub fn font_2xl() -> FontId {
    FontId::proportional(32.0)
}

pub fn heading_color() -> Color32 {
    TEXT()
}

/// Pulsing REC indicator color (`t` = sin abs 0..1).
pub fn danger_pulse(t: f32) -> Color32 {
    Color32::from_rgb((180.0 + t.clamp(0.0, 1.0) * 75.0) as u8, 50, 50)
}

/// Soft white for third-lines / faint HUD guides.
#[allow(non_snake_case)]
pub fn HUD_GUIDE() -> Color32 {
    Color32::from_white_alpha(70)
}

// ── Density ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub fn scale(self) -> f32 {
        match self {
            Density::Comfortable => 1.0,
            Density::Compact => 0.85,
        }
    }

    pub fn sp(self, base: f32) -> f32 {
        base * self.scale()
    }
}

// ── Apply themes ────────────────────────────────────────────────────

/// Apply Graphite dark visuals to the egui context.
pub fn apply_graphite_theme(ctx: &egui::Context) {
    set_theme_mode(ThemeMode::Dark);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = CANVAS();
    visuals.window_fill = CANVAS();
    visuals.extreme_bg_color = SURFACE();
    visuals.faint_bg_color = SURFACE_2();
    visuals.override_text_color = Some(TEXT());

    visuals.widgets.noninteractive.bg_fill = SURFACE();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.noninteractive.rounding = rounding_md();

    visuals.widgets.inactive.bg_fill = SURFACE_2();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER());
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.inactive.rounding = rounding_md();

    visuals.widgets.hovered.bg_fill = SURFACE_3();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, TEXT_MUTED());
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.hovered.rounding = rounding_md();

    visuals.widgets.active.bg_fill = PRIMARY();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PRIMARY_INK());
    visuals.widgets.active.rounding = rounding_md();

    visuals.selection.bg_fill = SELECTION_FILL();
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT());

    visuals.hyperlink_color = INFO();
    visuals.warn_fg_color = WARN();
    visuals.error_fg_color = DANGER();

    ctx.set_visuals(visuals);
}

/// Light / paper theme — full token parity with dark chrome.
pub fn apply_light_theme(ctx: &egui::Context) {
    set_theme_mode(ThemeMode::Light);
    let mut visuals = Visuals::light();
    visuals.panel_fill = CANVAS();
    visuals.window_fill = CANVAS();
    visuals.extreme_bg_color = SURFACE();
    visuals.faint_bg_color = SURFACE_2();
    visuals.override_text_color = Some(TEXT());

    visuals.widgets.noninteractive.bg_fill = SURFACE();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.noninteractive.rounding = rounding_md();

    visuals.widgets.inactive.bg_fill = SURFACE_2();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER());
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.inactive.rounding = rounding_md();

    visuals.widgets.hovered.bg_fill = SURFACE_3();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, TEXT_MUTED());
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT());
    visuals.widgets.hovered.rounding = rounding_md();

    visuals.widgets.active.bg_fill = PRIMARY();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PRIMARY_INK());
    visuals.widgets.active.rounding = rounding_md();

    visuals.selection.bg_fill = SELECTION_FILL();
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT());
    visuals.hyperlink_color = INFO();
    visuals.warn_fg_color = WARN();
    visuals.error_fg_color = DANGER();

    ctx.set_visuals(visuals);
}

pub fn theme_mode_from_str(s: &str) -> ThemeMode {
    match s {
        "light" => ThemeMode::Light,
        _ => ThemeMode::Dark,
    }
}

pub fn theme_mode_to_str(m: ThemeMode) -> &'static str {
    match m {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    }
}
