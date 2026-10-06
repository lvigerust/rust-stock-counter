//! An alert's buttons, after Catalyst's `AlertActions`: a ghost Avbryt, when
//! there's a choice, then the button that confirms.
//!
//! gpui-kit's alert draws its own OK and Cancel, with its 16px labels and its
//! outlined Cancel. This footer draws them as the app's [`Button`]s, and
//! leaves what they do to gpui-kit: Avbryt and Escape cancel, the confirm
//! button and Enter run the alert's `on_ok`.
//!
//! ```ignore
//! dialog
//!     .alert_frame(cx)
//!     .title("Forkaste varetellingen som pågår?")
//!     .on_ok(discard)
//!     .footer(AlertActions::new("Forkast og importer").danger().cancel("Avbryt"))
//! ```

use gpui_kit::component::button::{ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::{Confirm, DialogClose, DialogFooter};
use gpui_kit::{Rems, rems};

use crate::prelude::*;
use crate::{Button, FocusRing};

/// A touch shorter than a medium button's 2.25rem, so the buttons sit
/// lighter in an alert's short decision.
const BUTTON_HEIGHT: Rems = rems(2.125);

#[derive(IntoElement)]
pub struct AlertActions {
    confirm: SharedString,
    variant: ButtonVariant,
    cancel: Option<SharedString>,
}

impl AlertActions {
    /// An alert with one button, labeled `confirm`, primary unless a variant
    /// says otherwise.
    pub fn new(confirm: impl Into<SharedString>) -> Self {
        Self {
            confirm: confirm.into(),
            variant: ButtonVariant::Primary,
            cancel: None,
        }
    }

    /// A button before the confirm one that leaves without doing anything.
    pub fn cancel(mut self, cancel: impl Into<SharedString>) -> Self {
        self.cancel = Some(cancel.into());
        self
    }
}

/// The confirm button's variant: `.danger()` for one that throws work away.
impl ButtonVariants for AlertActions {
    fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl RenderOnce for AlertActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The confirm button dispatches from a focus node inside itself, as
        // gpui-kit's OK does, so it reaches the alert it sits in whatever
        // holds focus. gpui-kit's `DialogAction` would only take clicks on
        // its wrapper, leaving Space on the focused button inert.
        let anchor = window
            .use_keyed_state("alert-confirm-anchor", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let confirm = Button::new("alert-confirm")
            .with_variant(self.variant)
            .label(self.confirm)
            .h(BUTTON_HEIGHT)
            .child(div().absolute().size_0().track_focus(&anchor))
            .on_click(move |_, window, cx| {
                anchor.dispatch_action(&Confirm { secondary: false }, window, cx)
            });

        // `DialogClose` fills its container; this keeps Avbryt as wide as its
        // label. It takes the solid ring of the button beside it, so focus
        // looks the same on both.
        DialogFooter::new()
            .gap_3()
            .when_some(self.cancel, |footer, cancel| {
                footer.child(
                    div()
                        .flex_none()
                        .child(DialogClose::new().trigger(|button| {
                            Button::from(button)
                                .ghost()
                                .label(cancel)
                                .h(BUTTON_HEIGHT)
                                .with_focus_ring(FocusRing::Solid)
                        })),
                )
            })
            .child(confirm)
    }
}
