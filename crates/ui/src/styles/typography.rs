//! Typography helpers that the theme's type scale doesn't cover.

use std::sync::Arc;

use gpui_kit::FontFeatures;

use crate::prelude::*;

pub trait StyledTypography: Styled + Sized {
    /// Digits of equal width (OpenType `tnum`), so quantities line up down a
    /// column and a number doesn't jitter sideways as it changes.
    fn tabular_nums(self) -> Self {
        self.font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
    }
}

impl<E: Styled> StyledTypography for E {}
