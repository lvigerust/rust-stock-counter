//! The application shell: builds the menu bar, sets the theme, opens the window
//! and hands it to the stocktake feature. Feature logic doesn't belong here.

use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig};
use gpui_kit::*;
use stocktake_ui::{
    APP_NAME, ExportStocktake, FocusSearch, ImportStockList, StocktakeView, ToggleSidebar,
};

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

actions!(varetelling, [Quit]);

fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        gpui_kit::init(cx);
        stocktake_ui::init(cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        // After every key binding: the menu bar reads shortcuts from the
        // keymap when it's built.
        set_menus(cx);

        set_theme_colors(cx);

        // Counting is the only thing done on this laptop while it runs, so
        // the window opens maximized: it fills the screen but keeps the menu
        // bar, the Dock and its traffic lights. Un-zooming restores the
        // windowed bounds. Window geometry is a platform boundary, so
        // physical pixels are intentional here.
        let windowed = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(windowed)),
            window_min_size: Some(size(px(960.), px(560.))),
            // The toolbar is the title bar: the traffic lights sit in it and
            // it moves the window itself, so AppKit doesn't claim its top
            // strip for dragging.
            titlebar: Some(ui::WindowBar::titlebar_options(APP_NAME)),
            app_owns_titlebar_drag: true,
            ..Default::default()
        };

        open_window(options, cx, |window, cx| {
            cx.new(|cx| StocktakeView::new(window, cx))
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}

/// Replaces the theme colors that differ from gpui-kit's. The themes are
/// swapped rather than their colors edited, so following the system
/// appearance loads them again.
fn set_theme_colors(cx: &mut App) {
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

/// The native menu bar: every command is reachable from it, with its
/// shortcut shown beside it.
fn set_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new(APP_NAME).items([MenuItem::action(format!("Avslutt {APP_NAME}"), Quit)]),
        Menu::new("Fil").items([
            MenuItem::action("Importer vareliste…", ImportStockList),
            MenuItem::action("Eksporter telling…", ExportStocktake),
            MenuItem::separator(),
            MenuItem::action("Søk", FocusSearch),
        ]),
        Menu::new("Vis").items([MenuItem::action("Vis/skjul sidepanel", ToggleSidebar)]),
    ]);
}
