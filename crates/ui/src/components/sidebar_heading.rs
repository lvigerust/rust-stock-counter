use gpui_kit::ClickEvent;

use crate::RowButton;
use crate::prelude::*;

/// What a press on a heading that opens and closes its section does.
type ToggleHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The title of a group in a section of the sidebar, such as `Tellinger`
/// above the stocktakes. Muted, so the items under it lead, and inset like
/// an item's content so the two line up.
///
/// With [`Self::on_toggle`] it opens and closes the items under it, the way
/// the trigger of a gpui-kit `Collapsible` does: the whole heading is a
/// [`RowButton`], with a chevron against its trailing edge.
#[derive(IntoElement)]
pub struct SidebarHeading {
    label: SharedString,
    toggle: Option<(ElementId, bool, ToggleHandler)>,
}

impl SidebarHeading {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            toggle: None,
        }
    }

    /// Makes the heading the button that opens and closes its section.
    /// `open` turns the chevron; the caller keeps it and flips it in
    /// `handler`. `id` keeps the button's focus from frame to frame, so it
    /// must be unique in the window.
    pub fn on_toggle(
        mut self,
        id: impl Into<ElementId>,
        open: bool,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.toggle = Some((id.into(), open, Box::new(handler)));
        self
    }
}

impl RenderOnce for SidebarHeading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = div().min_w_0().truncate().child(self.label);
        let Some((id, open, handler)) = self.toggle else {
            return div()
                .min_w_0()
                .px_2()
                .mb_1()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label)
                .into_any_element();
        };
        let chevron = if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        RowButton::new(id, handler)
            .hover_colors(theme.sidebar_accent, theme.sidebar_accent_foreground)
            .justify_between()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(label)
            .child(Icon::new(chevron).xsmall().flex_none())
            .into_any_element()
    }
}
