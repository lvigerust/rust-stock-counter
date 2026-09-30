//! The application shell: loads fonts, opens the window and hands it to the
//! stocktake feature. Feature logic doesn't belong here.

use std::borrow::Cow;

use gpui_kit::component::Theme;
use gpui_kit::*;
use stocktake_ui::StocktakeView;

/// Font files from `assets/fonts`, embedded by `build.rs`.
mod fonts {
    include!(concat!(env!("OUT_DIR"), "/fonts.rs"));
}

/// Family name of the bundled UI font. It must match the name inside the
/// font files, or GPUI falls back to another font.
const UI_FONT_FAMILY: &str = "SF Pro Text";

fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        gpui_kit::init(cx);
        stocktake_ui::init(cx);

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

        // Window geometry is a platform boundary, so physical pixels are intentional here.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1040.), px(720.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(480.))),
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
