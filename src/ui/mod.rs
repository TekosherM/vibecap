//! Phase 1 UI layer: Graphite theme, Loop rail, toasts, icons, Shutter strip, tabs.

pub mod capture_hud;
pub mod capture_tab;
pub mod clip_tab;
pub mod components;
pub mod icons;
pub mod inbox_tab;
pub mod library_tab;
pub mod palette;
pub mod settings_tab;
pub mod still_tab;
pub mod theme;
pub mod wizard;

pub use capture_hud::{show_countdown_bubble, show_region_selector, RegionHudResult};
pub use components::{
    agent_dot_color, empty_state, loop_position_badge, loop_rail, show_capture_toast,
    show_toast_card, shutter_strip, status_strip, CaptureToastAction, LoopStage, ShutterAction,
    StatusSnapshot, ToastLevel,
};
pub use palette::{show_palette, PaletteAction};
pub use theme::{apply_current_theme, apply_graphite_theme, Density, ThemeMode};
