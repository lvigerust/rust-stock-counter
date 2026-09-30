//! The application shell: loads fonts, builds the menu bar, opens the window
//! and hands it to the stocktake feature. Feature logic doesn't belong here.

use std::borrow::Cow;

use gpui_kit::component::Theme;
use gpui_kit::*;
use stocktake_ui::{ExportStocktake, FocusSearch, ImportStockList, StocktakeView};

/// Font files from `assets/fonts`, embedded by `build.rs`.
mod fonts {
    include!(concat!(env!("OUT_DIR"), "/fonts.rs"));
}

/// Family name of the bundled UI font. It must match the name inside the
/// font files, or GPUI falls back to another font.
const UI_FONT_FAMILY: &str = "SF Pro Text";

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

        if !fonts::FONTS.is_empty() {
            cx.text_system()
                .add_fonts(
                    fonts::FONTS
                        .iter()
                        .map(|font| Cow::Borrowed(*font))
                        .collect(),
                )
                .expect("failed to load bundled fonts");
            Theme::update(cx, |theme| theme.font_family = UI_FONT_FAMILY.into());
        }

        // Counting is the only thing done on this laptop while it runs, so
        // the window opens full screen. Leaving full screen restores the
        // windowed bounds. Window geometry is a platform boundary, so
        // physical pixels are intentional here.
        let windowed = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(windowed)),
            window_min_size: Some(size(px(960.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Varetelling".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        open_window(options, cx, |window, cx| {
            cx.new(|cx| StocktakeView::new(window, cx))
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}

/// The native menu bar: every command is reachable from it, with its
/// shortcut shown beside it.
fn set_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new("Varetelling").items([MenuItem::action("Avslutt Varetelling", Quit)]),
        Menu::new("Fil").items([
            MenuItem::action("Importer vareliste…", ImportStockList),
            MenuItem::action("Eksporter telling…", ExportStocktake),
            MenuItem::separator(),
            MenuItem::action("Søk", FocusSearch),
        ]),
    ]);
}
