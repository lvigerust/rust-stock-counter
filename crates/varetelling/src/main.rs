//! The application shell: sets the theme, builds the menu bar, opens the
//! window and hands it to the stocktake feature. Feature logic doesn't
//! belong here.

mod menus;
mod theme;

use gpui_kit::*;
use stocktake_ui::{APP_NAME, StocktakeView};

actions!(
    varetelling,
    [
        /// Quits the application.
        Quit
    ]
);

fn main() {
    application().with_assets(ui::Assets).run(|cx| {
        gpui_kit::init(cx);
        stocktake_ui::init(cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        // After every key binding, so the menu shows their shortcuts.
        menus::init(cx);
        theme::init(cx);

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

        // `open_window` puts gpui-kit's `Root` above the view, for dialogs
        // and notifications.
        open_window(options, cx, |window, cx| {
            theme::follow_system_appearance(window, cx);
            cx.new(|cx| StocktakeView::new(window, cx))
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}
