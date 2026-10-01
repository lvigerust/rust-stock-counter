use gpui_kit::{ClickEvent, StyleRefinement};

use crate::RowButton;
use crate::prelude::*;

/// One item in a section of the sidebar, such as the app's name in its
/// header: a row padded by 0.5rem, its children side by side.
///
/// With [`Self::on_click`] the whole row is a [`RowButton`] in the sidebar's
/// accent. The padding stays the same, so a clickable item lines up with a
/// plain one.
#[derive(IntoElement)]
pub struct SidebarItem {
    button: Option<RowButton>,
    disabled: bool,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl SidebarItem {
    pub fn new() -> Self {
        Self {
            button: None,
            disabled: false,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// Makes the row a button. `id` keeps its focus from frame to frame, so
    /// it must be unique in the window.
    pub fn on_click(
        mut self,
        id: impl Into<ElementId>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button = Some(RowButton::new(id, handler));
        self
    }
}

impl Default for SidebarItem {
    fn default() -> Self {
        Self::new()
    }
}

/// A disabled item is dimmed and ignores clicks and Tab.
impl Disableable for SidebarItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for SidebarItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for SidebarItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SidebarItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(button) = self.button else {
            return h_flex()
                .min_w_0()
                .gap_2()
                .p_2()
                .children(self.children)
                .refine_style(&self.style)
                .into_any_element();
        };
        let theme = cx.theme();
        button
            .hover_colors(theme.sidebar_accent, theme.sidebar_accent_foreground)
            .disabled(self.disabled)
            .gap_2()
            .p_2()
            .children(self.children)
            .refine_style(&self.style)
            .into_any_element()
    }
}
