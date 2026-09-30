use crate::prelude::*;

/// The title of a group in a section of the sidebar, such as `Tellinger`
/// above the stocktakes. Muted, so the items under it lead, and inset like
/// an item's content so the two line up.
#[derive(IntoElement)]
pub struct SidebarHeading {
    label: SharedString,
}

impl SidebarHeading {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl RenderOnce for SidebarHeading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .min_w_0()
            .truncate()
            .px_2()
            .mb_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(self.label)
    }
}
