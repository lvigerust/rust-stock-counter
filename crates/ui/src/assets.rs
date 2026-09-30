//! The app's own images, served alongside gpui-kit's icons.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// The Scala Bad logo in white, for dark backgrounds.
pub(crate) const LOGO_WHITE: &str = "logo/scala-bad-white.svg";

/// The Scala Bad logo in red, for light backgrounds.
pub(crate) const LOGO_RED: &str = "logo/scala-bad-red.svg";

/// Every image the app ships, by the path it's loaded from.
const FILES: &[(&str, &[u8])] = &[
    (
        LOGO_WHITE,
        include_bytes!("../assets/logo/scala-bad-white.svg"),
    ),
    (LOGO_RED, include_bytes!("../assets/logo/scala-bad-red.svg")),
];

/// The app's images, then gpui-kit's full icon catalog: register it with
/// `application().with_assets(ui::Assets)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match FILES.iter().find(|(file, _)| *file == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_kit::assets::AllAssets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = FILES
            .iter()
            .map(|(file, _)| *file)
            .filter(|file| file.starts_with(path))
            .map(Into::into)
            .collect();
        paths.extend(gpui_kit::assets::AllAssets.list(path)?);
        Ok(paths)
    }
}
