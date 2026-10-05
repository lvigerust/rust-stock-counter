//! A short mark beside a value, after Catalyst's `Badge`:
//!
//! ```ignore
//! Badge::new().child("+2")
//! ```
//!
//! Neutral, in the secondary fill, with the theme's small corner tier and
//! medium weight. A badge is for a count or a state that's scanned for, not
//! for every piece of metadata; and the Design Guides keep the colored
//! variants for states that mean success, warning or danger, which this app
//! marks with an icon and a word (the feature's `CountStatus`) instead. So
//! there is one badge.

use gpui_kit::StyleRefinement;
use gpui_kit::component::tag::Tag;

use crate::prelude::*;

#[derive(IntoElement)]
pub struct Badge {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl Badge {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for Badge {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Badge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Badge {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // gpui-kit's small tag already has Catalyst's padding, text size and
        // the theme's small corners; the weight is Catalyst's.
        Tag::secondary()
            .small()
            .font_medium()
            .refine_style(&self.style)
            .children(self.children)
    }
}
