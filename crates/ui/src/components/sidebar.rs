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
//! container spaces its children with a gap instead. [`SidebarItem`] and
//! [`SidebarHeading`] have modules of their own.
//!
//! [`SidebarItem`]: crate::SidebarItem
//! [`SidebarHeading`]: crate::SidebarHeading

use gpui_kit::StyleRefinement;

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
    /// under a [`SidebarHeading`](crate::SidebarHeading). Its items sit
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
