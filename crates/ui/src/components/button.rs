//! The app's button: gpui-kit's, with the app's type on its label.
//!
//! gpui-kit gives a medium button 16px text, a step up from the 14px the
//! rest of the app reads in. It sets that size on a row of its own inside
//! the button, so a `text_sm()` on the button doesn't reach the label. This
//! button lays out its icon, label and children in a row of ours instead,
//! with sizes that match the app's text:
//!
//! ```ignore
//! Button::new("export").icon(IconName::Share).label("Eksporter telling")
//! Button::new("save").primary().label("Lagre").on_click(save)
//! ```
//!
//! Styles set on the button go where they would on gpui-kit's, with two
//! exceptions that go on the row: text styles other than the color (size,
//! weight, line height), and the gap and alignment of the children. So
//! `.text_xs().justify_start()` still does what it reads as. The color stays
//! on the button, as gpui-kit changes it there on hover and when disabled.

use std::mem;

use gpui_kit::component::button::{Button as ComponentButton, ButtonVariant, ButtonVariants};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::{FocusableExt, Selectable, Size};
use gpui_kit::{Action, ClickEvent, Interactivity, StyleRefinement};

use crate::prelude::*;

#[derive(IntoElement)]
pub struct Button {
    base: ComponentButton,
    size: Size,
    icon: Option<Icon>,
    label: Option<SharedString>,
    accessibility_label: Option<SharedString>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl Button {
    /// `id` keeps the button's focus from frame to frame, so it must be
    /// unique in the window.
    pub fn new(id: impl Into<ElementId>) -> Self {
        ComponentButton::new(id).into()
    }

    /// The text on the button, after its icon. It's also what assistive
    /// tech names the button, unless [`Self::accessibility_label`] says
    /// otherwise.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// An icon before the label. Without a label or children, the button is
    /// a square around it.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// What assistive tech names the button, when its label doesn't say,
    /// or it has none.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Outlined, in the variant's color.
    pub fn outline(mut self) -> Self {
        self.base = self.base.outline();
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.base = self.base.tooltip(tooltip);
        self
    }

    /// A tooltip that shows the shortcut for `action` beside the text.
    pub fn tooltip_with_action(
        mut self,
        tooltip: impl Into<SharedString>,
        action: &dyn Action,
        context: Option<&str>,
    ) -> Self {
        self.base = self.base.tooltip_with_action(tooltip, action, context);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self.base.on_click(handler);
        self
    }
}

/// Takes over a button gpui-kit made, such as the one
/// `DialogClose::trigger` passes in, with its behavior.
impl From<ComponentButton> for Button {
    fn from(base: ComponentButton) -> Self {
        Self {
            base,
            size: Size::default(),
            icon: None,
            label: None,
            accessibility_label: None,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ButtonVariants for Button {
    fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.base = self.base.with_variant(variant);
        self
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self.base = self.base.with_size(self.size);
        self
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.base = self.base.disabled(disabled);
        self
    }
}

impl Selectable for Button {
    fn selected(mut self, selected: bool) -> Self {
        self.base = self.base.selected(selected);
        self
    }

    fn is_selected(&self) -> bool {
        self.base.is_selected()
    }

    fn open(mut self, open: bool) -> Self {
        self.base = self.base.open(open);
        self
    }

    fn is_open(&self) -> bool {
        self.base.is_open()
    }
}

impl FocusableExt for Button {
    fn focus_ring(mut self, enabled: bool) -> Self {
        self.base = self.base.focus_ring(enabled);
        self
    }

    fn is_focus_ring_enabled(&self) -> bool {
        self.base.is_focus_ring_enabled()
    }
}

impl DropdownMenu for Button {}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self {
            base,
            size,
            icon,
            label,
            accessibility_label,
            mut style,
            children,
        } = self;
        let base = base.when_some(accessibility_label.or(label.clone()), |base, label| {
            base.accessibility_label(label)
        });

        // Nothing but an icon: gpui-kit's square icon button.
        if label.is_none() && children.is_empty() {
            return base
                .when_some(icon, |base, icon| base.icon(icon))
                .refine_style(&style);
        }

        let mut row_style = StyleRefinement {
            text: mem::take(&mut style.text),
            gap: mem::take(&mut style.gap),
            align_items: style.align_items.take(),
            justify_content: style.justify_content.take(),
            ..StyleRefinement::default()
        };
        style.text.color = row_style.text.color.take();

        let row = h_flex()
            .flex_1()
            .min_w_0()
            .justify_center()
            .font_medium()
            .map(|row| match size {
                Size::XSmall => row.gap_1().text_xs(),
                Size::Small => row.gap_1().text_sm(),
                Size::Medium | Size::Size(_) => row.gap_2().text_sm(),
                Size::Large => row.gap_2().text_base(),
            })
            .refine_style(&row_style)
            .children(icon.map(|icon| icon.with_size(size)))
            .children(label.map(|label| div().min_w_0().truncate().child(label)))
            .children(children);
        base.refine_style(&style).child(row)
    }
}
