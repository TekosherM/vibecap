//! Mono / Celestial design tokens (ported from the Browmie mono-ui spec).
//!
//! **Gate G4:** raw `Color32::from_rgb` belongs here only (plus rare TRANSPARENT).
//! Accent is reserved for **live** states (recording, agent waiting, pending inbox).
//!
//! Color tokens are **functions** so dark/light can switch at runtime without
//! rewriting every paint call. Spacing stays const.

use std::cell::Cell;

use egui::{Color32, Rounding, Stroke, Visuals};

// ── Theme mode ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    /// Celestial Pathfinder — dark cosmic surfaces, pink↔teal aurora accents.
    Celestial,
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
    install_ui_fonts(ctx);
    install_chrome_style(ctx);
    match theme_mode() {
        ThemeMode::Dark => apply_graphite_theme(ctx),
        ThemeMode::Light => apply_light_theme(ctx),
        ThemeMode::Celestial => apply_celestial_theme(ctx),
    }
}

/// Load real UI fonts — egui's bundled font is a big part of the toolbox feel.
/// Picks the OS-native UI face where available; falls back to egui defaults.
fn install_ui_fonts(ctx: &egui::Context) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let proportional: &[&str] = if cfg!(windows) {
            &[
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
            ]
        } else if cfg!(target_os = "macos") {
            &[
                "/System/Library/Fonts/SFNS.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
            ]
        } else {
            &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            ]
        };
        let monospace: &[&str] = if cfg!(windows) {
            &[r"C:\Windows\Fonts\CascadiaMono.ttf", r"C:\Windows\Fonts\consola.ttf"]
        } else {
            &[]
        };

        let mut fonts = egui::FontDefinitions::default();
        let mut installed = false;
        for (i, path) in proportional.iter().enumerate() {
            if let Ok(bytes) = std::fs::read(path) {
                let name = format!("ui-sans-{i}");
                fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes));
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(i, name);
                installed = true;
            }
        }
        for (i, path) in monospace.iter().enumerate() {
            if let Ok(bytes) = std::fs::read(path) {
                let name = format!("ui-mono-{i}");
                fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes));
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(i, name);
            }
        }
        if installed {
            ctx.set_fonts(fonts);
        }
    });
}

/// Roomier chrome — bigger hit targets and breathing room read as "app",
/// egui's defaults read as "toolbox".
fn install_chrome_style(ctx: &egui::Context) {
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::Vec2::new(10.0, 8.0);
        s.spacing.button_padding = egui::Vec2::new(12.0, 6.0);
        s.spacing.indent = 20.0;
        s.spacing.slider_width = 140.0;
        s.spacing.text_edit_width = 240.0;
    });
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

/// Three-way token: dark / light / celestial. `dual!` tokens automatically
/// give Celestial the dark value (it is a dark-class theme).
macro_rules! tri {
    ($name:ident, $dark:expr, $light:expr, $celestial:expr) => {
        #[inline]
        #[allow(non_snake_case)]
        pub fn $name() -> Color32 {
            match theme_mode() {
                ThemeMode::Light => $light,
                ThemeMode::Celestial => $celestial,
                ThemeMode::Dark => $dark,
            }
        }
    };
}

// ── Canvas (Mono zinc neutrals; Celestial cosmic indigo) ────────────

tri!(
    CANVAS,
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x0a, 0x08, 0x20)
);
tri!(
    SURFACE,
    Color32::from_rgb(0x14, 0x14, 0x16),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x16, 0x14, 0x3b)
);
tri!(
    SURFACE_2,
    Color32::from_rgb(0x1a, 0x1a, 0x1d),
    Color32::from_rgb(0xfa, 0xfa, 0xfa),
    Color32::from_rgb(0x1d, 0x1a, 0x4c)
);
tri!(
    SURFACE_3,
    Color32::from_rgb(0x22, 0x22, 0x25),
    Color32::from_rgb(0xf0, 0xf0, 0xf2),
    Color32::from_rgb(0x26, 0x21, 0x5c)
);

// ── Text ────────────────────────────────────────────────────────────

tri!(
    TEXT,
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xf4, 0xf1, 0xff)
);
tri!(
    TEXT_MUTED,
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x52, 0x52, 0x5b),
    Color32::from_rgb(0xc7, 0xc2, 0xe6)
);
tri!(
    TEXT_DIM,
    Color32::from_rgb(0x6b, 0x6b, 0x72),
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x8f, 0x89, 0xbc)
);

// ── Brand / live accent ─────────────────────────────────────────────
// Mono: color is reserved for *data* — the accent is ink. Celestial:
// brand pink carries live state; teal is the secondary data hue.

tri!(
    ACCENT,
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xec, 0x4f, 0x8e)
);
tri!(
    ACCENT_INK,
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xf4, 0xf1, 0xff)
);
tri!(
    PRIMARY,
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xf4, 0xf1, 0xff)
);
tri!(
    PRIMARY_INK,
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x0f, 0x0d, 0x29)
);
tri!(
    BORDER,
    Color32::from_rgb(0x23, 0x23, 0x27),
    Color32::from_rgb(0xe5, 0xe5, 0xe7),
    Color32::from_rgba_premultiplied(173, 195, 255, 31)
);
tri!(
    BORDER_STRONG,
    Color32::from_rgb(0x30, 0x30, 0x34),
    Color32::from_rgb(0xd7, 0xd7, 0xda),
    Color32::from_rgba_premultiplied(173, 195, 255, 56)
);
tri!(
    SELECTION_FILL,
    Color32::from_rgba_premultiplied(244, 244, 245, 38),
    Color32::from_rgba_premultiplied(24, 24, 27, 26),
    Color32::from_rgba_premultiplied(236, 79, 142, 60)
);
tri!(
    OVERLAY_DIM,
    Color32::from_black_alpha(100),
    Color32::from_black_alpha(90),
    Color32::from_black_alpha(120)
);
tri!(
    OVERLAY_LABEL,
    Color32::from_black_alpha(180),
    Color32::from_rgba_premultiplied(28, 30, 36, 200),
    Color32::from_rgba_premultiplied(15, 13, 41, 210)
);
tri!(
    OVERLAY_BLUR,
    Color32::from_black_alpha(220),
    Color32::from_rgba_premultiplied(28, 30, 36, 220),
    Color32::from_rgba_premultiplied(10, 8, 32, 230)
);
tri!(
    NEUTRAL_STROKE,
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x52, 0x52, 0x5b),
    Color32::from_rgb(0xc7, 0xc2, 0xe6)
);

// ── Semantic (shared hues; celestial uses its palette's ok/danger) ──

tri!(
    SUCCESS,
    Color32::from_rgb(0x4a, 0xde, 0x80),
    Color32::from_rgb(0x16, 0xa3, 0x4a),
    Color32::from_rgb(0x4a, 0xde, 0x80)
);
tri!(
    WARN,
    Color32::from_rgb(0xea, 0xb3, 0x08),
    Color32::from_rgb(0xb8, 0x86, 0x0b),
    Color32::from_rgb(0xfb, 0xbf, 0x24)
);
tri!(
    DANGER,
    Color32::from_rgb(0xf8, 0x71, 0x71),
    Color32::from_rgb(0xdc, 0x26, 0x26),
    Color32::from_rgb(0xf8, 0x71, 0x71)
);
tri!(
    DANGER_SOFT,
    Color32::from_rgb(0xef, 0x44, 0x44),
    Color32::from_rgb(0xe0, 0x31, 0x31),
    Color32::from_rgb(0xef, 0x44, 0x44)
);
tri!(
    INFO,
    Color32::from_rgb(0x60, 0xa5, 0xfa),
    Color32::from_rgb(0x3b, 0x82, 0xf6),
    Color32::from_rgb(0x67, 0xa2, 0xd9)
);
tri!(
    AGENT_TEAL,
    Color32::from_rgb(0x2d, 0xd4, 0xbf),
    Color32::from_rgb(0x0d, 0x94, 0x88),
    Color32::from_rgb(0x26, 0xd6, 0xc0)
);
dual!(
    ON_SOLID,
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xff, 0xff, 0xff)
);

// ── Annotation / loop (shared hues) ─────────────────────────────────

dual!(
    LOOP_ANNOTATE,
    Color32::from_rgb(0xa8, 0x55, 0xf7),
    Color32::from_rgb(0x7c, 0x4d, 0xbf)
);
tri!(
    PRI_HIGH_FILL,
    Color32::from_rgba_premultiplied(0xf8, 0x71, 0x71, 40),
    Color32::from_rgba_premultiplied(0xdc, 0x26, 0x26, 36),
    Color32::from_rgba_premultiplied(0xf8, 0x71, 0x71, 44)
);
tri!(
    PRI_NORMAL_FILL,
    Color32::from_rgba_premultiplied(0xa1, 0xa1, 0xaa, 40),
    Color32::from_rgba_premultiplied(0x52, 0x52, 0x5b, 36),
    Color32::from_rgba_premultiplied(0xc7, 0xc2, 0xe6, 40)
);
tri!(
    SURFACE_GLASS,
    Color32::from_rgba_premultiplied(0x14, 0x14, 0x16, 230),
    Color32::from_rgba_premultiplied(0xff, 0xff, 0xff, 235),
    Color32::from_rgba_premultiplied(0x16, 0x14, 0x3b, 235)
);
tri!(
    SURFACE_GLASS_DIM,
    Color32::from_rgba_premultiplied(0x14, 0x14, 0x16, 220),
    Color32::from_rgba_premultiplied(0xff, 0xff, 0xff, 220),
    Color32::from_rgba_premultiplied(0x16, 0x14, 0x3b, 225)
);

// ── Celestial aurora stops (signature teal→pink gradient) ───────────

pub const AURORA_TEAL: Color32 = Color32::from_rgb(0x26, 0xd6, 0xc0);
pub const AURORA_MID: Color32 = Color32::from_rgb(0x67, 0xa2, 0xd9);
pub const AURORA_PINK: Color32 = Color32::from_rgb(0xec, 0x4f, 0x8e);
pub const CELESTIAL_STAR: Color32 = Color32::from_rgb(0xff, 0xe7, 0xb2);

// ── Spacing (8pt grid) ──────────────────────────────────────────────

pub const SP_1: f32 = 4.0;
pub const SP_2: f32 = 8.0;
pub const SP_3: f32 = 12.0;
pub const SP_4: f32 = 16.0;
#[allow(dead_code)]
pub const SP_5: f32 = 24.0;
pub const SP_6: f32 = 32.0;

// ── Radius (mono-ui spec: 12 / 9 / 6) ───────────────────────────────

pub const R_SM: f32 = 6.0;
pub const R_MD: f32 = 9.0;
pub const R_LG: f32 = 12.0;

pub fn rounding_sm() -> Rounding {
    Rounding::same(R_SM)
}
pub fn rounding_md() -> Rounding {
    Rounding::same(R_MD)
}
pub fn rounding_lg() -> Rounding {
    Rounding::same(R_LG)
}

/// Pulsing REC indicator color (`t` = sin abs 0..1).
pub fn danger_pulse(t: f32) -> Color32 {
    Color32::from_rgb((180.0 + t.clamp(0.0, 1.0) * 75.0) as u8, 50, 50)
}

/// Soft white for third-lines / faint HUD guides.
#[allow(non_snake_case)]
pub fn HUD_GUIDE() -> Color32 {
    Color32::from_rgba_premultiplied(70, 70, 70, 70)
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

/// Apply Mono dark visuals to the egui context.
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
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG());
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

/// Mono light — full token parity with dark chrome.
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
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG());
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

/// Celestial Pathfinder — same chrome as dark, cosmic tokens.
pub fn apply_celestial_theme(ctx: &egui::Context) {
    set_theme_mode(ThemeMode::Celestial);
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
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG());
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

// ── Celestial flourish ──────────────────────────────────────────────

/// Deterministic starfield + soft aurora glows for the Celestial canvas.
/// Paint first inside the root panel so all widgets draw on top.
pub fn paint_celestial_sky(painter: &egui::Painter, rect: egui::Rect) {
    // Normalized star positions (x, y, radius, color) — fixed so the sky
    // doesn't twinkle between frames.
    const STARS: &[(f32, f32, f32, Color32)] = &[
        (0.04, 0.07, 1.0, Color32::from_rgba_premultiplied(178, 178, 178, 178)),
        (0.15, 0.21, 1.0, Color32::from_rgba_premultiplied(128, 128, 128, 128)),
        (0.28, 0.12, 1.5, CELESTIAL_STAR),
        (0.39, 0.33, 1.0, Color32::from_rgba_premultiplied(153, 153, 153, 153)),
        (0.52, 0.08, 1.0, Color32::from_rgba_premultiplied(38, 214, 192, 217)),
        (0.67, 0.29, 1.0, Color32::from_rgba_premultiplied(140, 140, 140, 140)),
        (0.80, 0.46, 1.5, Color32::from_rgba_premultiplied(236, 79, 142, 217)),
        (0.92, 0.19, 1.0, Color32::from_rgba_premultiplied(128, 128, 128, 128)),
        (0.07, 0.40, 1.0, Color32::from_rgba_premultiplied(102, 102, 102, 102)),
        (0.23, 0.56, 1.0, Color32::from_rgba_premultiplied(140, 140, 140, 140)),
        (0.44, 0.67, 1.0, Color32::from_rgba_premultiplied(115, 115, 115, 115)),
        (0.62, 0.78, 1.0, Color32::from_rgba_premultiplied(128, 128, 128, 128)),
        (0.14, 0.84, 1.0, Color32::from_rgba_premultiplied(153, 153, 153, 153)),
        (0.85, 0.92, 1.0, Color32::from_rgba_premultiplied(102, 102, 102, 102)),
        (0.36, 0.90, 1.0, Color32::from_rgba_premultiplied(38, 214, 192, 140)),
        (0.74, 0.62, 1.0, Color32::from_rgba_premultiplied(236, 79, 142, 140)),
    ];
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    // Soft aurora glows: teal top-left, pink lower-right, violet center.
    let dim = w.min(h);
    painter.circle_filled(
        egui::pos2(rect.left() + w * 0.18, rect.top() + h * 0.08),
        dim * 0.42,
        Color32::from_rgba_premultiplied(38, 214, 192, 7),
    );
    painter.circle_filled(
        egui::pos2(rect.left() + w * 0.82, rect.top() + h * 0.86),
        dim * 0.46,
        Color32::from_rgba_premultiplied(236, 79, 142, 8),
    );
    painter.circle_filled(
        egui::pos2(rect.left() + w * 0.55, rect.top() + h * 0.45),
        dim * 0.55,
        Color32::from_rgba_premultiplied(103, 76, 209, 6),
    );
    for &(fx, fy, r, color) in STARS {
        painter.circle_filled(
            egui::pos2(rect.left() + fx * w, rect.top() + fy * h),
            r,
            color,
        );
    }
}

/// Teal→blue→pink aurora gradient strip (celestial signature accent).
/// Drawn as a 3-stop vertex-colored mesh — square-edged by design for
/// hairline accents; callers place it under headers or on rail ticks.
pub fn paint_aurora_strip(painter: &egui::Painter, rect: egui::Rect) {
    paint_aurora(painter, rect, true);
}

/// Vertical aurora gradient (pink top → teal bottom) for rail ticks.
pub fn paint_aurora_strip_v(painter: &egui::Painter, rect: egui::Rect) {
    paint_aurora(painter, rect, false);
}

fn paint_aurora(painter: &egui::Painter, rect: egui::Rect, horizontal: bool) {
    let mut mesh = egui::epaint::Mesh::default();
    let cols = if horizontal {
        [AURORA_TEAL, AURORA_MID, AURORA_PINK]
    } else {
        [AURORA_PINK, AURORA_MID, AURORA_TEAL]
    };
    for (i, c) in cols.iter().enumerate() {
        let t = i as f32 / 2.0;
        let (a, b) = if horizontal {
            let x = rect.left() + rect.width() * t;
            (egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom()))
        } else {
            let y = rect.top() + rect.height() * t;
            (egui::pos2(rect.left(), y), egui::pos2(rect.right(), y))
        };
        mesh.colored_vertex(a, *c);
        mesh.colored_vertex(b, *c);
    }
    mesh.add_triangle(0, 2, 1);
    mesh.add_triangle(1, 2, 3);
    mesh.add_triangle(2, 4, 3);
    mesh.add_triangle(3, 4, 5);
    painter.add(egui::Shape::mesh(mesh));
}

/// Preview swatch colors for the theme picker (canvas, surface, ink).
pub fn preview_colors(mode: ThemeMode) -> (Color32, Color32, Color32) {
    match mode {
        ThemeMode::Dark => (
            Color32::from_rgb(0x0b, 0x0b, 0x0c),
            Color32::from_rgb(0x14, 0x14, 0x16),
            Color32::from_rgb(0xf4, 0xf4, 0xf5),
        ),
        ThemeMode::Light => (
            Color32::from_rgb(0xf4, 0xf4, 0xf5),
            Color32::from_rgb(0xff, 0xff, 0xff),
            Color32::from_rgb(0x18, 0x18, 0x1b),
        ),
        ThemeMode::Celestial => (
            Color32::from_rgb(0x0a, 0x08, 0x20),
            Color32::from_rgb(0x16, 0x14, 0x3b),
            Color32::from_rgb(0xec, 0x4f, 0x8e),
        ),
    }
}

pub fn theme_mode_label(m: ThemeMode) -> &'static str {
    match m {
        ThemeMode::Dark => "Mono Dark",
        ThemeMode::Light => "Mono Light",
        ThemeMode::Celestial => "Celestial",
    }
}

pub fn theme_mode_from_str(s: &str) -> ThemeMode {
    match s {
        "light" => ThemeMode::Light,
        "celestial" => ThemeMode::Celestial,
        _ => ThemeMode::Dark,
    }
}

pub fn theme_mode_to_str(m: ThemeMode) -> &'static str {
    match m {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
        ThemeMode::Celestial => "celestial",
    }
}
