//! What a UI file almost always needs: `use ui::prelude::*;`.
//!
//! Keep this short. A name belongs here when most view files use it; anything
//! rarer is imported by path so a reader can see where it comes from.

pub use gpui_kit::prelude::*;
pub use gpui_kit::{
    AnyElement, App, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};

pub use gpui_kit::assets::IconName;
pub use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, StyledExt, h_flex, v_flex};

pub use crate::{Appear, StyledTypography};
