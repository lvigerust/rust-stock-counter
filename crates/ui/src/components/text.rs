//! The type roles, after Catalyst's `Subheading`, `Label`, `Text` and
//! `SidebarHeading`. A view names the role and gets its size, weight and
//! color; it never spells them out itself, so one change here reaches every
//! screen:
//!
//! ```ignore
//! Heading::new("Ingen differanse")          // sm, semibold, foreground
//! Label::new("Lokasjon")                    // sm, medium, foreground: names a control
//! Text::new("Allerede talt 6 her.")         // sm, muted: body and help
//! SectionHeading::new("Status")             // xs, medium, muted: over a group of rows
//! ```
//!
//! Each takes `Styled`, so a caller can still truncate it, pad it or, where a
//! row is set smaller, size it down: the role sets the defaults, not a cage.

use gpui_kit::StyleRefinement;

use crate::prelude::*;

/// Declares a type role: a piece of text with a style of its own.
macro_rules! text_role {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(IntoElement)]
        pub struct $name {
            text: SharedString,
            style: StyleRefinement,
        }

        impl $name {
            pub fn new(text: impl Into<SharedString>) -> Self {
                Self {
                    text: text.into(),
                    style: StyleRefinement::default(),
                }
            }
        }

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                &mut self.style
            }
        }
    };
}

text_role!(
    /// The title of a region or a state: the mode atop the sidebar, an empty
    /// state's first line. Small and semibold, in the text color, as
    /// Catalyst's `Subheading` is; a desktop window has no page title above
    /// it.
    Heading
);

impl RenderOnce for Heading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .font_semibold()
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(self.text)
    }
}

text_role!(
    /// Names a control or a group of values, as Catalyst's `Label` does
    /// inside a field: small and medium, in the text color.
    Label
);

impl RenderOnce for Label {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .font_medium()
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(self.text)
    }
}

text_role!(
    /// Body and help text: what was already counted, why a list is empty.
    /// Small and muted, as Catalyst's `Text` is, so it reads after the
    /// values and controls around it.
    Text
);

impl RenderOnce for Text {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .refine_style(&self.style)
            .child(self.text)
    }
}

text_role!(
    /// The heading over a group of rows: a sidebar section, a menu's items,
    /// a tooltip's list. Extra small, medium and muted, as Catalyst's
    /// `SidebarHeading` and `DropdownHeading` are, so the rows lead.
    SectionHeading
);

impl RenderOnce for SectionHeading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_xs()
            .font_medium()
            .text_color(cx.theme().muted_foreground)
            .refine_style(&self.style)
            .child(self.text)
    }
}
