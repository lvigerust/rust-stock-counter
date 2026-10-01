//! The app's colors where they differ from gpui-kit's, and following the
//! system's light or dark appearance.

use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig};
use gpui_kit::{App, Window};

/// The window background in dark mode.
const DARK_BACKGROUND: &str = "#18181a";

/// The sidebar's background in dark mode, a shade lighter than the window.
const DARK_SIDEBAR: &str = "#1e1e20";

/// Muted text, such as placeholders, one step further from the text than
/// gpui-kit's (`neutral-500` in light mode, `neutral-400` in dark).
const LIGHT_MUTED_FOREGROUND: &str = "neutral-400";
const DARK_MUTED_FOREGROUND: &str = "neutral-500";

/// The hovered table row: the accent at half of gpui-kit's 60% opacity.
const LIGHT_TABLE_HOVER: &str = "neutral-100/30";
const DARK_TABLE_HOVER: &str = "neutral-800/30";

/// The border of the focused control, in both appearances.
const FOCUS_RING: &str = "#2b7fff";

/// No color at all; the theme parser reads hex only.
const TRANSPARENT: &str = "#00000000";

/// Replaces the theme colors that differ from gpui-kit's. The themes are
/// swapped rather than their colors edited, so following the system
/// appearance loads them again.
pub fn init(cx: &mut App) {
    Theme::update(cx, |theme| {
        // Focus shows as a colored border alone, without a glow around it.
        theme.focus_ring = false;

        let mut light = (*theme.light_theme).clone();
        light.colors.ring = Some(FOCUS_RING.into());
        light.colors.muted_foreground = Some(LIGHT_MUTED_FOREGROUND.into());
        light.colors.table_hover = Some(LIGHT_TABLE_HOVER.into());
        clear_table_colors(&mut light);
        theme.light_theme = Rc::new(light);

        let mut dark = (*theme.dark_theme).clone();
        dark.colors.background = Some(DARK_BACKGROUND.into());
        dark.colors.sidebar = Some(DARK_SIDEBAR.into());
        dark.colors.ring = Some(FOCUS_RING.into());
        dark.colors.muted_foreground = Some(DARK_MUTED_FOREGROUND.into());
        // Tooltips take the popover color; in dark mode they share the
        // muted fill instead of gpui-kit's near-black.
        dark.colors.popover = dark.colors.muted.clone();
        dark.colors.table_hover = Some(DARK_TABLE_HOVER.into());
        clear_table_colors(&mut dark);
        theme.dark_theme = Rc::new(dark);
    });
}

/// Tables take the color of what they sit on: no fill of their own behind
/// the header or the rows.
fn clear_table_colors(config: &mut ThemeConfig) {
    config.colors.table = Some(TRANSPARENT.into());
    config.colors.table_head = Some(TRANSPARENT.into());
}

/// Matches the window's appearance now, and again whenever the system
/// switches between light and dark, for as long as the window is open.
pub fn follow_system_appearance(window: &mut Window, cx: &mut App) {
    Theme::sync_system_appearance(Some(window), cx);
    window
        .observe_window_appearance(|window, cx| Theme::sync_system_appearance(Some(window), cx))
        .detach();
}
