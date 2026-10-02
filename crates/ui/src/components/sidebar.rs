//! The sidebar and the parts it's composed from, after Catalyst's:
//!
//! ```ignore
//! Sidebar::new()
//!     .w(SIDEBAR_WIDTH)
//!     .child(SidebarHeader::new().child(SidebarSection::new().child(title)))
//!     .child(
//!         SidebarBody::new()
//!             .child(SidebarSection::new().child(SidebarHeading::new("Lokasjoner")).children(items)),
//!     )
//!     .child(SidebarFooter::new().child(SidebarSection::new().child(clear_button)))
//! ```
//!
//! Catalyst spaces sections with sibling selectors; GPUI has none, so each
//! container spaces its children with a gap instead.

use gpui_kit::{ClickEvent, StyleRefinement};

use crate::RowButton;
use crate::prelude::*;

/// Declares a part that holds children and takes style overrides, leaving
/// only its `RenderOnce` to write.
macro_rules! container {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(IntoElement)]
        pub struct $name {
            style: StyleRefinement,
            children: Vec<AnyElement>,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    style: StyleRefinement::default(),
                    children: Vec::new(),
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }

        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.children.extend(elements);
            }
        }
    };
}

container!(
    /// The pane along the window's leading edge, full height, in the
    /// sidebar's colors with a rule along its trailing edge. Stacks its
    /// children: usually a [`WindowBar`](crate::WindowBar), then a
    /// [`SidebarHeader`], a [`SidebarBody`] and a [`SidebarFooter`]. The
    /// caller sets its width.
    ///
    /// One per window: it's found by the id `sidebar`.
    Sidebar
);

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .id("sidebar")
            .flex_none()
            .h_full()
            .min_h_0()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .children(self.children)
            .refine_style(&self.style)
    }
}

container!(
    /// The top of a [`Sidebar`], as tall as its content, padded by 1rem.
    /// Its sections sit close together.
    SidebarHeader
);

impl RenderOnce for SidebarHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .flex_none()
            .gap_2p5()
            .p_4()
            .children(self.children)
            .refine_style(&self.style)
    }
}

container!(
    /// The middle of a [`Sidebar`]: takes the height the header and footer
    /// leave, and scrolls on its own when its content is taller. Padded by
    /// 1rem, with 2rem between its sections, since each has a heading of
    /// its own to set it apart.
    ///
    /// One per window: its scroll position is kept under the id
    /// `sidebar-body`.
    SidebarBody
);

impl RenderOnce for SidebarBody {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .id("sidebar-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_8()
            .p_4()
            .children(self.children)
            .refine_style(&self.style)
    }
}

/// Empty room in a [`SidebarBody`] that takes the height its sections leave,
/// so the sections after it sit at the bottom, against the footer. Never
/// less than the 2rem between two sections: it takes back one of the two
/// gaps around it.
#[derive(IntoElement, Default)]
pub struct SidebarSpacer;

impl SidebarSpacer {
    pub fn new() -> Self {
        Self
    }
}

impl RenderOnce for SidebarSpacer {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().flex_1().mt_neg_8()
    }
}

container!(
    /// The bottom of a [`Sidebar`], set apart from the body by a rule: as
    /// tall as its content, padded by 1rem. Its sections sit close together.
    SidebarFooter
);

impl RenderOnce for SidebarFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .flex_none()
            .gap_2p5()
            .p_4()
            .border_t_1()
            .border_color(cx.theme().sidebar_border)
            .children(self.children)
            .refine_style(&self.style)
    }
}

container!(
    /// A group of items in a sidebar's header, body or footer, usually
    /// under a [`SidebarHeading`]. Its items sit
    /// almost touching, so their hover fills read as one column.
    ///
    /// As wide as its container, even where nothing stretches it: inside a
    /// `Collapsible` the reveal lays its content out on its own, and a
    /// section there would shrink to its widest item.
    SidebarSection
);

impl RenderOnce for SidebarSection {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_0p5()
            .children(self.children)
            .refine_style(&self.style)
    }
}

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
