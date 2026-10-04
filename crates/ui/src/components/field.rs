//! A labelled control, after Catalyst's `Field`:
//!
//! ```ignore
//! Field::new()
//!     .label("Talt antall")
//!     .child(Input::new(&quantity))
//!     .description("Allerede talt 6 her. Enter legger til.")
//! ```
//!
//! The label is a [`Label`], the description a [`Text`], and Catalyst's
//! spacing sits between them: 0.75rem from label to control and from control
//! to description, so every field in a dialog reads the same.

use gpui_kit::StyleRefinement;

use crate::prelude::*;
use crate::{Label, Text};

#[derive(IntoElement)]
pub struct Field {
    label: Option<SharedString>,
    descriptions: Vec<SharedString>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl Field {
    pub fn new() -> Self {
        Self {
            label: None,
            descriptions: Vec::new(),
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// What the control is for, over it.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A line of help under the control. Several sit close together, as
    /// lines of one note.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.descriptions.push(description.into());
        self
    }
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Field {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Field {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Field {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .gap_3()
            .when_some(self.label, |this, label| this.child(Label::new(label)))
            .children(self.children)
            .when(!self.descriptions.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_1()
                        .children(self.descriptions.into_iter().map(Text::new)),
                )
            })
            .refine_style(&self.style)
    }
}
