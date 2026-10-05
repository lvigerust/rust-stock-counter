//! A setting that is on or off, after Catalyst's `SwitchField`: its label and
//! help on the leading side, the switch on the trailing side.
//!
//! ```ignore
//! SwitchField::new("open-on-paste", on)
//!     .label("Åpne telling ved innliming")
//!     .description("Åpner tellingen når du limer inn en hel strekkode.")
//!     .on_change(|on, _, cx| set(*on, cx))
//! ```
//!
//! The label is a [`Label`] and the help a [`Text`], so the row reads like a
//! [`Field`](crate::Field). Pressing the label or the help flips the switch
//! too, as Catalyst's does.

use std::rc::Rc;

use gpui_kit::component::switch::Switch;
use gpui_kit::{MouseButton, StyleRefinement};

use crate::prelude::*;
use crate::{Label, Text};

/// What a change of the switch does, given the value it asks for.
type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SwitchField {
    id: ElementId,
    checked: bool,
    label: SharedString,
    description: Option<SharedString>,
    on_change: Option<ChangeHandler>,
    style: StyleRefinement,
}

impl SwitchField {
    /// `id` keeps the switch's focus from frame to frame, so it must be
    /// unique in the window and stable for the setting it stands for.
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: SharedString::default(),
            description: None,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// What the setting is, named by what it does when on.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// A line of help under the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Called with the value the counter asked for. The owner writes it and
    /// notifies, so the switch shows it.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for SwitchField {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SwitchField {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let on_label = self.on_change.clone();
        let on_switch = self.on_change;
        h_flex()
            .items_start()
            .justify_between()
            .gap_6()
            .child(
                v_flex()
                    .id((self.id.clone(), "text"))
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .when_some(on_label, |this, handler| {
                        this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            handler(&!checked, window, cx)
                        })
                    })
                    .child(Label::new(self.label.clone()))
                    .children(self.description.map(Text::new)),
            )
            .child(
                Switch::new(self.id)
                    .checked(checked)
                    .accessibility_label(self.label)
                    .when_some(on_switch, |this, handler| {
                        this.on_change(move |checked, window, cx| handler(checked, window, cx))
                    }),
            )
            .refine_style(&self.style)
    }
}
