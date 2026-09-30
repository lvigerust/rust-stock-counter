use gpui_kit::{AbsoluteLength, img};

use crate::assets::{LOGO_RED, LOGO_WHITE};
use crate::prelude::*;

/// The Scala Bad logo: white on the dark theme, red on the light one, where
/// white would vanish into the background.
#[derive(IntoElement)]
pub struct Logo {
    height: AbsoluteLength,
}

impl Logo {
    /// A logo this tall; the width follows the logo's proportions.
    pub fn new(height: impl Into<AbsoluteLength>) -> Self {
        Self {
            height: height.into(),
        }
    }
}

impl RenderOnce for Logo {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let path = if cx.theme().is_dark() {
            LOGO_WHITE
        } else {
            LOGO_RED
        };
        // The SVGs declare four times their drawn size, because `img`
        // rasterizes an SVG at its declared size and scales the bitmap. At
        // 1× it would blur on a Retina display.
        img(path).flex_none().h(self.height)
    }
}
