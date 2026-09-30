use std::rc::Rc;

use gpui_kit::{ClickEvent, MouseButton, Role, StyleRefinement};

use crate::prelude::*;

/// One item in a section of the sidebar, such as the app's name in its
/// header: a row padded by 0.5rem, its children side by side.
///
/// With [`Self::on_click`] the whole row is the button: it lights up on
/// hover, takes Tab like any control, and Enter or Space press it. The
/// padding stays the same, so a clickable item lines up with a plain one.
#[derive(IntoElement)]
pub struct SidebarItem {
    id: Option<ElementId>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    disabled: bool,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl SidebarItem {
    pub fn new() -> Self {
        Self {
            id: None,
            on_click: None,
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
        self.id = Some(id.into());
        self.on_click = Some(Rc::new(handler));
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let row = h_flex()
            .min_w_0()
            .gap_2()
            .p_2()
            .children(self.children)
            .refine_style(&self.style);
        let (Some(id), Some(on_click)) = (self.id, self.on_click) else {
            return row.into_any_element();
        };
        if self.disabled {
            return row.id(id).opacity(0.5).into_any_element();
        }

        let focus_handle = window
            .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let theme = cx.theme();
        let (hover_bg, hover_fg) = (theme.sidebar_accent, theme.sidebar_accent_foreground);
        row.id(id)
            .role(Role::Button)
            .track_focus(&focus_handle)
            .tab_index(0)
            .rounded(theme.radius)
            .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
            .subtle_focus_ring(cx)
            // A click shouldn't leave the focus ring behind.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .into_any_element()
    }
}
