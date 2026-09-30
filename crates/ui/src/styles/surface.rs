//! Surfaces: the few background levels the app is built from.
//!
//! Hierarchy comes from contrast between layers, not from lines or shadows:
//! a faintly tinted page, with content on plain cards above it. This is the
//! same idea as Zed's `ElevationIndex`, cut down to the levels this app uses.

use gpui_kit::Hsla;

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The window behind the content: a quiet tint that lets cards stand out.
    Page,
    /// Content regions, and chrome that frames the page such as the header.
    Card,
}

impl Surface {
    pub fn bg(self, cx: &App) -> Hsla {
        let theme = cx.theme();
        match self {
            // Mixed from theme colors rather than picked, so the tint follows
            // light, dark and custom themes alike.
            Self::Page => theme.background.blend(theme.muted.opacity(0.85)),
            Self::Card => theme.background,
        }
    }
}
