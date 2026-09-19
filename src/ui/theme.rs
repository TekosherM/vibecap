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
    /// Chromie `dark` — Tailwind slate greys ("carbon").
    Carbon,
    #[default]
    Dark,
    Light,
    /// Celestial Pathfinder — dark cosmic surfaces, pink↔teal aurora accents.
    Celestial,
    /// Pink-forward celestial — plum surfaces, pink aurora, gradient CTAs.
    CelestialPink,
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
        ThemeMode::Carbon => apply_carbon_theme(ctx),
        ThemeMode::Dark => apply_graphite_theme(ctx),
        ThemeMode::Light => apply_light_theme(ctx),
        ThemeMode::Celestial => apply_celestial_theme(ctx),
        ThemeMode::CelestialPink => apply_celestial_pink_theme(ctx),
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
        // Real weight faces — egui's `.strong()` only brightens color, so a
        // named family is the only way to get the mockup's 550/650 weights.
        let semibold: &[&str] = if cfg!(windows) {
            &[r"C:\Windows\Fonts\seguisb.ttf", r"C:\Windows\Fonts\segoeuib.ttf"]
        } else {
            &[]
        };
        let bold: &[&str] = if cfg!(windows) {
            &[r"C:\Windows\Fonts\segoeuib.ttf", r"C:\Windows\Fonts\segoeui.ttf"]
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
        for (family_name, paths) in [("semibold", semibold), ("bold", bold)] {
            for path in paths {
                if let Ok(bytes) = std::fs::read(path) {
                    let data = format!("ui-{family_name}");
                    fonts.font_data.insert(data.clone(), egui::FontData::from_owned(bytes));
                    fonts
                        .families
                        .insert(egui::FontFamily::Name(family_name.into()), vec![data]);
                    break;
                }
            }
        }
        if installed {
            ctx.set_fonts(fonts);
        }
    });
}

/// Semibold weight face — button labels, card names, section titles
/// (the mockup's 550–650 weight range). Falls back to proportional.
pub fn font_semibold() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

/// Bold weight face — wordmarks and stat numerals.
pub fn font_bold() -> egui::FontFamily {
    egui::FontFamily::Name("bold".into())
}

/// Letterspaced uppercase section label (mono-ui `.section-title`).
/// Paints through a LayoutJob because RichText can't letter-space.
pub fn caps_label(ui: &mut egui::Ui, text: &str) {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::new(10.5, font_semibold()),
            color: TEXT_DIM(),
            extra_letter_spacing: 1.0,
            ..Default::default()
        },
    );
    ui.label(job);
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
        // mono-ui scrollbar: thin floating 8px bar.
        s.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 8.0,
            ..egui::style::ScrollStyle::thin()
        };
        // Headings get the real semibold face — `.strong()` alone only
        // brightens color in egui, which is why the type felt flat.
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(20.0, font_semibold()),
        );
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
/// give Celestial the dark value (it is a dark-class theme); Carbon and
/// CelestialPink fall through to the dark arm too.
macro_rules! tri {
    ($name:ident, $dark:expr, $light:expr, $celestial:expr) => {
        #[inline]
        #[allow(non_snake_case)]
        pub fn $name() -> Color32 {
            match theme_mode() {
                ThemeMode::Light => $light,
                ThemeMode::Celestial => $celestial,
                _ => $dark,
            }
        }
    };
}

/// Five-way token: carbon / dark / light / celestial / celestial-pink.
macro_rules! pent {
    ($name:ident, $carbon:expr, $dark:expr, $light:expr, $celestial:expr, $pink:expr) => {
        #[inline]
        #[allow(non_snake_case)]
        pub fn $name() -> Color32 {
            match theme_mode() {
                ThemeMode::Carbon => $carbon,
                ThemeMode::Dark => $dark,
                ThemeMode::Light => $light,
                ThemeMode::Celestial => $celestial,
                ThemeMode::CelestialPink => $pink,
            }
        }
    };
}

/// True for the two cosmic themes (starfield + aurora accents apply).
pub fn is_celestial() -> bool {
    matches!(theme_mode(), ThemeMode::Celestial | ThemeMode::CelestialPink)
}

// ── Canvas ──────────────────────────────────────────────────────────
// Order: carbon / mono-dark / mono-light / celestial / celestial-pink.
// Carbon = Chromie `dark` (Tailwind slate); Celestial = cosmic indigo;
// CelestialPink = plum surfaces under the same aurora.

pent!(
    CANVAS,
    Color32::from_rgb(0x11, 0x18, 0x27),
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x0a, 0x08, 0x20),
    Color32::from_rgb(0x12, 0x08, 0x1e)
);
pent!(
    SURFACE,
    Color32::from_rgb(0x1f, 0x29, 0x37),
    Color32::from_rgb(0x14, 0x14, 0x16),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x16, 0x14, 0x3b),
    Color32::from_rgb(0x1c, 0x0f, 0x30)
);
pent!(
    SURFACE_2,
    Color32::from_rgb(0x25, 0x30, 0x44),
    Color32::from_rgb(0x1a, 0x1a, 0x1d),
    Color32::from_rgb(0xfa, 0xfa, 0xfa),
    Color32::from_rgb(0x1d, 0x1a, 0x4c),
    Color32::from_rgb(0x25, 0x14, 0x3f)
);
pent!(
    SURFACE_3,
    Color32::from_rgb(0x37, 0x41, 0x51),
    Color32::from_rgb(0x22, 0x22, 0x25),
    Color32::from_rgb(0xf0, 0xf0, 0xf2),
    Color32::from_rgb(0x26, 0x21, 0x5c),
    Color32::from_rgb(0x30, 0x19, 0x4f)
);

// ── Text ────────────────────────────────────────────────────────────

pent!(
    TEXT,
    Color32::from_rgb(0xf9, 0xfa, 0xfb),
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xf4, 0xf1, 0xff),
    Color32::from_rgb(0xfd, 0xf1, 0xf8)
);
pent!(
    TEXT_MUTED,
    Color32::from_rgb(0xd1, 0xd5, 0xdb),
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x52, 0x52, 0x5b),
    Color32::from_rgb(0xc7, 0xc2, 0xe6),
    Color32::from_rgb(0xe3, 0xc2, 0xdd)
);
pent!(
    TEXT_DIM,
    Color32::from_rgb(0x6b, 0x72, 0x80),
    Color32::from_rgb(0x6b, 0x6b, 0x72),
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x8f, 0x89, 0xbc),
    Color32::from_rgb(0xa0, 0x7c, 0xb8)
);

// ── Brand / live accent ─────────────────────────────────────────────
// Mono & Carbon: color is reserved for *data* — the accent is ink.
// Celestial: brand pink carries live state; teal is the data hue.

pent!(
    ACCENT,
    Color32::from_rgb(0xf9, 0xfa, 0xfb),
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xec, 0x4f, 0x8e),
    Color32::from_rgb(0xf2, 0x6f, 0xa4)
);
pent!(
    ACCENT_INK,
    Color32::from_rgb(0x11, 0x18, 0x27),
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xf4, 0xf1, 0xff),
    Color32::from_rgb(0x12, 0x08, 0x1e)
);
pent!(
    PRIMARY,
    Color32::from_rgb(0xf9, 0xfa, 0xfb),
    Color32::from_rgb(0xf4, 0xf4, 0xf5),
    Color32::from_rgb(0x18, 0x18, 0x1b),
    Color32::from_rgb(0xf4, 0xf1, 0xff),
    Color32::from_rgb(0xfd, 0xf1, 0xf8)
);
pent!(
    PRIMARY_INK,
    Color32::from_rgb(0x11, 0x18, 0x27),
    Color32::from_rgb(0x0b, 0x0b, 0x0c),
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0x0f, 0x0d, 0x29),
    Color32::from_rgb(0x12, 0x08, 0x1e)
);
// Ink button hover / pressed — the mockup's `opacity: .88` / `.76`
// pre-blended over each canvas (egui fills are opaque).
pent!(
    PRIMARY_HOVER,
    Color32::from_rgb(0xdd, 0xdd, 0xe0),
    Color32::from_rgb(0xd8, 0xd8, 0xda),
    Color32::from_rgb(0x33, 0x33, 0x38),
    Color32::from_rgb(0xd8, 0xd5, 0xe4),
    Color32::from_rgb(0xe1, 0xd5, 0xde)
);
pent!(
    PRIMARY_DOWN,
    Color32::from_rgb(0xc1, 0xc1, 0xc5),
    Color32::from_rgb(0xbc, 0xbc, 0xbf),
    Color32::from_rgb(0x48, 0x48, 0x4e),
    Color32::from_rgb(0xbc, 0xb9, 0xca),
    Color32::from_rgb(0xc5, 0xb9, 0xc4)
);
pent!(
    BORDER,
    Color32::from_rgb(0x37, 0x41, 0x51),
    Color32::from_rgb(0x23, 0x23, 0x27),
    Color32::from_rgb(0xe5, 0xe5, 0xe7),
    Color32::from_rgba_unmultiplied(173, 195, 255, 31),
    Color32::from_rgba_unmultiplied(236, 140, 190, 31)
);
pent!(
    BORDER_STRONG,
    Color32::from_rgb(0x4b, 0x55, 0x63),
    Color32::from_rgb(0x30, 0x30, 0x34),
    Color32::from_rgb(0xd7, 0xd7, 0xda),
    Color32::from_rgba_unmultiplied(173, 195, 255, 56),
    Color32::from_rgba_unmultiplied(236, 140, 190, 58)
);
pent!(
    SELECTION_FILL,
    Color32::from_rgba_unmultiplied(249, 250, 251, 38),
    Color32::from_rgba_unmultiplied(244, 244, 245, 38),
    Color32::from_rgba_unmultiplied(24, 24, 27, 26),
    Color32::from_rgba_unmultiplied(236, 79, 142, 60),
    Color32::from_rgba_unmultiplied(242, 111, 164, 60)
);
tri!(
    OVERLAY_DIM,
    Color32::from_black_alpha(100),
    Color32::from_black_alpha(90),
    Color32::from_black_alpha(120)
);
pent!(
    OVERLAY_LABEL,
    Color32::from_rgba_unmultiplied(17, 24, 39, 210),
    Color32::from_black_alpha(180),
    Color32::from_rgba_unmultiplied(28, 30, 36, 200),
    Color32::from_rgba_unmultiplied(15, 13, 41, 210),
    Color32::from_rgba_unmultiplied(18, 8, 30, 215)
);
pent!(
    OVERLAY_BLUR,
    Color32::from_rgba_unmultiplied(17, 24, 39, 220),
    Color32::from_black_alpha(220),
    Color32::from_rgba_unmultiplied(28, 30, 36, 220),
    Color32::from_rgba_unmultiplied(10, 8, 32, 230),
    Color32::from_rgba_unmultiplied(18, 8, 30, 230)
);
pent!(
    NEUTRAL_STROKE,
    Color32::from_rgb(0xd1, 0xd5, 0xdb),
    Color32::from_rgb(0xa1, 0xa1, 0xaa),
    Color32::from_rgb(0x52, 0x52, 0x5b),
    Color32::from_rgb(0xc7, 0xc2, 0xe6),
    Color32::from_rgb(0xe3, 0xc2, 0xdd)
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
pent!(
    INFO,
    Color32::from_rgb(0x60, 0xa5, 0xfa),
    Color32::from_rgb(0x60, 0xa5, 0xfa),
    Color32::from_rgb(0x3b, 0x82, 0xf6),
    Color32::from_rgb(0x67, 0xa2, 0xd9),
    Color32::from_rgb(0xc0, 0x84, 0xfc)
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
    Color32::from_rgba_unmultiplied(0xf8, 0x71, 0x71, 40),
    Color32::from_rgba_unmultiplied(0xdc, 0x26, 0x26, 36),
    Color32::from_rgba_unmultiplied(0xf8, 0x71, 0x71, 44)
);
tri!(
    PRI_NORMAL_FILL,
    Color32::from_rgba_unmultiplied(0xa1, 0xa1, 0xaa, 40),
    Color32::from_rgba_unmultiplied(0x52, 0x52, 0x5b, 36),
    Color32::from_rgba_unmultiplied(0xc7, 0xc2, 0xe6, 40)
);
pent!(
    SURFACE_GLASS,
    Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 217),
    Color32::from_rgba_unmultiplied(0x14, 0x14, 0x16, 230),
    Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 235),
    Color32::from_rgba_unmultiplied(0x16, 0x14, 0x3b, 235),
    Color32::from_rgba_unmultiplied(0x1c, 0x0f, 0x30, 232)
);
pent!(
    SURFACE_GLASS_DIM,
    Color32::from_rgba_unmultiplied(0x1f, 0x29, 0x37, 212),
    Color32::from_rgba_unmultiplied(0x14, 0x14, 0x16, 220),
    Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 220),
    Color32::from_rgba_unmultiplied(0x16, 0x14, 0x3b, 225),
    Color32::from_rgba_unmultiplied(0x1c, 0x0f, 0x30, 222)
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
    Color32::from_rgba_unmultiplied(70, 70, 70, 70)
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

/// Shared widget chrome — all five schemes differ only in tokens + shadow.
fn apply_visuals(ctx: &egui::Context, mode: ThemeMode, shadow: egui::epaint::Shadow) {
    set_theme_mode(mode);
    let mut visuals = if mode == ThemeMode::Light {
        Visuals::light()
    } else {
        Visuals::dark()
    };
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

    visuals.popup_shadow = shadow;
    visuals.window_shadow = shadow;

    ctx.set_visuals(visuals);
}

/// Carbon — Chromie `dark` (Tailwind slate greys).
pub fn apply_carbon_theme(ctx: &egui::Context) {
    apply_visuals(
        ctx,
        ThemeMode::Carbon,
        egui::epaint::Shadow {
            offset: egui::vec2(0.0, 10.0),
            blur: 28.0,
            spread: 0.0,
            color: Color32::from_black_alpha(110),
        },
    );
}

/// Apply Mono dark visuals to the egui context.
pub fn apply_graphite_theme(ctx: &egui::Context) {
    apply_visuals(
        ctx,
        ThemeMode::Dark,
        // mono-ui --shadow-pop (dark): soft deep lift under popups/windows.
        egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 32.0,
            spread: 0.0,
            color: Color32::from_black_alpha(128),
        },
    );
}

/// Mono light — full token parity with dark chrome.
pub fn apply_light_theme(ctx: &egui::Context) {
    apply_visuals(
        ctx,
        ThemeMode::Light,
        // mono-ui --shadow-pop (light): faint, close lift.
        egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: Color32::from_black_alpha(15),
        },
    );
}

/// Celestial Pathfinder — same chrome as dark, cosmic tokens.
pub fn apply_celestial_theme(ctx: &egui::Context) {
    apply_visuals(
        ctx,
        ThemeMode::Celestial,
        // Celestial shadow — deep, faintly violet.
        egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 36.0,
            spread: 0.0,
            color: Color32::from_rgba_unmultiplied(5, 3, 26, 150),
        },
    );
}

/// Celestial Pink — plum surfaces, pink aurora, rose shadows.
pub fn apply_celestial_pink_theme(ctx: &egui::Context) {
    apply_visuals(
        ctx,
        ThemeMode::CelestialPink,
        egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 36.0,
            spread: 0.0,
            color: Color32::from_rgba_unmultiplied(40, 5, 25, 150),
        },
    );
}

// ── Celestial flourish ──────────────────────────────────────────────

/// Deterministic starfield + radial aurora glows for the Celestial canvas,
/// matching Chromie's `body` background: vertical gradient base, radial
/// teal/pink glows, scattered stars. Paint first inside a panel.
pub fn paint_celestial_sky(painter: &egui::Painter, rect: egui::Rect) {
    let pink = theme_mode() == ThemeMode::CelestialPink;
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    let dim = w.min(h);

    // Base: vertical gradient — Chromie `linear-gradient(180deg, #0F0D29, #0A0820)`.
    let top = if pink {
        Color32::from_rgb(0x20, 0x11, 0x36)
    } else {
        Color32::from_rgb(0x0f, 0x0d, 0x29)
    };
    let mut base = egui::epaint::Mesh::default();
    base.colored_vertex(rect.left_top(), top);
    base.colored_vertex(rect.right_top(), top);
    base.colored_vertex(rect.left_bottom(), CANVAS());
    base.colored_vertex(rect.right_bottom(), CANVAS());
    base.add_triangle(0, 1, 2);
    base.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(base));

    // Radial glows — Chromie `radial-gradient(60% 60% at X Y, …)`.
    // CelestialPink swaps teal for rose so the sky reads pink-forward.
    let (g_teal, g_ambient) = if pink {
        (
            Color32::from_rgba_unmultiplied(244, 114, 182, 34),
            Color32::from_rgba_unmultiplied(251, 113, 133, 22),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(38, 214, 192, 40),
            Color32::from_rgba_unmultiplied(103, 76, 209, 26),
        )
    };
    radial_glow(
        painter,
        egui::pos2(rect.left() + w * 0.18, rect.top() + h * 0.08),
        dim * 0.50,
        g_teal,
    );
    radial_glow(
        painter,
        egui::pos2(rect.left() + w * 0.82, rect.top() + h * 0.06),
        dim * 0.52,
        Color32::from_rgba_unmultiplied(236, 79, 142, 34),
    );
    radial_glow(
        painter,
        egui::pos2(rect.left() + w * 0.08, rect.top() + h * 0.94),
        dim * 0.45,
        g_ambient,
    );

    // Stars — positions from Chromie's body starfield (normalized here).
    // `let` not `const`: from_rgba_unmultiplied isn't a const fn.
    let stars: &[(f32, f32, f32, Color32)] = &[
        (0.04, 0.07, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 178)),
        (0.15, 0.19, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 128)),
        (0.28, 0.11, 1.5, CELESTIAL_STAR),
        (0.39, 0.30, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 153)),
        (0.52, 0.07, 1.0, Color32::from_rgba_unmultiplied(38, 214, 192, 217)),
        (0.67, 0.27, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
        (0.80, 0.42, 1.5, Color32::from_rgba_unmultiplied(236, 79, 142, 217)),
        (0.92, 0.17, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 128)),
        (0.07, 0.36, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 102)),
        (0.23, 0.51, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
        (0.44, 0.61, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 115)),
        (0.62, 0.72, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 128)),
        (0.14, 0.77, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 153)),
        (0.85, 0.87, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 102)),
        (0.36, 0.86, 1.0, Color32::from_rgba_unmultiplied(38, 214, 192, 140)),
        (0.74, 0.58, 1.0, Color32::from_rgba_unmultiplied(236, 79, 142, 140)),
        (0.57, 0.92, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 120)),
        (0.95, 0.62, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 100)),
    ];
    for &(fx, fy, r, color) in stars {
        // Teal-tinted stars become rose under CelestialPink.
        let color = if pink && (color.r(), color.g(), color.b()) == (38, 214, 192) {
            Color32::from_rgba_unmultiplied(244, 114, 182, color.a())
        } else {
            color
        };
        painter.circle_filled(egui::pos2(rect.left() + fx * w, rect.top() + fy * h), r, color);
    }
}

/// Soft radial glow — two-ring fan mesh, alpha falls center → edge.
/// (egui has no radial gradient; this approximates `radial-gradient`.)
fn radial_glow(painter: &egui::Painter, c: egui::Pos2, r: f32, color: Color32) {
    const SEG: usize = 32;
    let mut mesh = egui::epaint::Mesh::default();
    let mid = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a() / 3);
    mesh.colored_vertex(c, color);
    for i in 0..SEG {
        let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(
            c + egui::vec2(a.cos(), a.sin()) * r * 0.55,
            mid,
        );
    }
    for i in 0..SEG {
        let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(
            c + egui::vec2(a.cos(), a.sin()) * r,
            Color32::TRANSPARENT,
        );
    }
    for i in 0..SEG {
        let n = (i + 1) % SEG;
        // inner fan
        mesh.add_triangle(0, 1 + i as u32, 1 + n as u32);
        // outer band
        let (a1, a2, b1, b2) = (
            1 + i as u32,
            1 + n as u32,
            1 + SEG as u32 + i as u32,
            1 + SEG as u32 + n as u32,
        );
        mesh.add_triangle(a1, b1, a2);
        mesh.add_triangle(a2, b1, b2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// The aurora's three stops for a given theme. CelestialPink uses
/// Chromie's `dawn` accent palette (rose → pink → rose).
fn aurora_stops_for(mode: ThemeMode) -> [Color32; 3] {
    if mode == ThemeMode::CelestialPink {
        [
            Color32::from_rgb(0xf4, 0x72, 0xb6),
            Color32::from_rgb(0xf3, 0x72, 0x9e),
            Color32::from_rgb(0xfb, 0x71, 0x85),
        ]
    } else {
        [AURORA_TEAL, AURORA_MID, AURORA_PINK]
    }
}

fn aurora_stops() -> [Color32; 3] {
    aurora_stops_for(theme_mode())
}

/// Teal→blue→pink aurora gradient strip (celestial signature accent;
/// pink→violet→pink under CelestialPink). Square-edged by design for
/// hairline accents; callers place it under headers or on rail ticks.
pub fn paint_aurora_strip(painter: &egui::Painter, rect: egui::Rect) {
    paint_aurora(painter, rect, true);
}

/// Vertical aurora gradient (pink top → teal bottom) for rail ticks.
pub fn paint_aurora_strip_v(painter: &egui::Painter, rect: egui::Rect) {
    paint_aurora(painter, rect, false);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

fn aurora_color(t: f32) -> Color32 {
    let cols = aurora_stops();
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        lerp_color(cols[0], cols[1], t * 2.0)
    } else {
        lerp_color(cols[1], cols[2], (t - 0.5) * 2.0)
    }
}

/// Horizontal aurora strip painted with an explicit theme's stops —
/// used by the theme-picker swatches where `theme_mode()` is the active
/// theme, not the one being previewed.
pub fn paint_aurora_strip_for(painter: &egui::Painter, rect: egui::Rect, mode: ThemeMode) {
    paint_aurora_with(painter, rect, aurora_stops_for(mode), true);
}

fn paint_aurora(painter: &egui::Painter, rect: egui::Rect, horizontal: bool) {
    let s = aurora_stops();
    let cols = if horizontal { s } else { [s[2], s[1], s[0]] };
    paint_aurora_with(painter, rect, cols, horizontal);
}

fn paint_aurora_with(
    painter: &egui::Painter,
    rect: egui::Rect,
    cols: [Color32; 3],
    horizontal: bool,
) {
    let mut mesh = egui::epaint::Mesh::default();
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

/// Rounded-corner aurora fill for celestial CTAs (Chromie's
/// `--cta-gradient`). Trapezoid slices follow the corner arcs.
pub fn paint_aurora_button(painter: &egui::Painter, rect: egui::Rect, rounding: f32) {
    const SLICES: usize = 24;
    let r = rounding.min(rect.height() / 2.0).min(rect.width() / 2.0);
    // Vertical inset of the rounded edge at a given x.
    let inset = |x: f32| -> f32 {
        let dl = rect.left() + r - x; // distance inside left corner arc
        let dr = x - (rect.right() - r); // distance inside right corner arc
        let d = dl.max(dr);
        if d <= 0.0 {
            0.0
        } else if d >= r {
            r
        } else {
            r - (r * r - d * d).sqrt()
        }
    };
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..SLICES {
        let x0 = rect.left() + rect.width() * i as f32 / SLICES as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / SLICES as f32;
        let top0 = rect.top() + inset(x0);
        let top1 = rect.top() + inset(x1);
        let bot0 = rect.bottom() - inset(x0);
        let bot1 = rect.bottom() - inset(x1);
        let c0 = aurora_color(i as f32 / SLICES as f32);
        let c1 = aurora_color((i + 1) as f32 / SLICES as f32);
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(egui::pos2(x0, top0), c0);
        mesh.colored_vertex(egui::pos2(x0, bot0), c0);
        mesh.colored_vertex(egui::pos2(x1, top1), c1);
        mesh.colored_vertex(egui::pos2(x1, bot1), c1);
        mesh.add_triangle(base, base + 2, base + 1);
        mesh.add_triangle(base + 1, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Preview swatch colors for the theme picker (canvas, surface, ink).
pub fn preview_colors(mode: ThemeMode) -> (Color32, Color32, Color32) {
    match mode {
        ThemeMode::Carbon => (
            Color32::from_rgb(0x11, 0x18, 0x27),
            Color32::from_rgb(0x1f, 0x29, 0x37),
            Color32::from_rgb(0xf9, 0xfa, 0xfb),
        ),
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
        ThemeMode::CelestialPink => (
            Color32::from_rgb(0x12, 0x06, 0x1c),
            Color32::from_rgb(0x26, 0x10, 0x30),
            Color32::from_rgb(0xf2, 0x6f, 0xa4),
        ),
    }
}

/// Picker order — Carbon first, then Mono pair, then both Celestials.
pub const THEME_ORDER: [ThemeMode; 5] = [
    ThemeMode::Carbon,
    ThemeMode::Dark,
    ThemeMode::Light,
    ThemeMode::Celestial,
    ThemeMode::CelestialPink,
];

pub fn theme_mode_label(m: ThemeMode) -> &'static str {
    match m {
        ThemeMode::Carbon => "Carbon",
        ThemeMode::Dark => "Mono Dark",
        ThemeMode::Light => "Mono Light",
        ThemeMode::Celestial => "Celestial",
        ThemeMode::CelestialPink => "Celestial Pink",
    }
}

pub fn theme_mode_from_str(s: &str) -> ThemeMode {
    match s {
        "carbon" => ThemeMode::Carbon,
        "light" => ThemeMode::Light,
        "celestial" => ThemeMode::Celestial,
        "celestial-pink" => ThemeMode::CelestialPink,
        _ => ThemeMode::Dark,
    }
}

pub fn theme_mode_to_str(m: ThemeMode) -> &'static str {
    match m {
        ThemeMode::Carbon => "carbon",
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
        ThemeMode::Celestial => "celestial",
        ThemeMode::CelestialPink => "celestial-pink",
    }
}
