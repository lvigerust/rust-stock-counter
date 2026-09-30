use gpui_kit::StyleRefinement;

use crate::prelude::*;

/// One item in a section of the sidebar, such as the app's name in its
/// header: a row padded by 0.5rem, its children side by side.
#[derive(IntoElement)]
pub struct SidebarItem {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl SidebarItem {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for SidebarItem {
    fn default() -> Self {
        Self::new()
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
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .min_w_0()
            .gap_2()
            .p_2()
            .children(self.children)
            .refine_style(&self.style)
    }
}
