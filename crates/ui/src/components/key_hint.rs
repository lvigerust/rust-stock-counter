use gpui_kit::component::kbd::Kbd;
use gpui_kit::{Action, Keystroke};

use crate::prelude::*;

/// A key and what it does, such as `⏎ Lagre antall`, for teaching the
/// keyboard path in a status bar or beside a control.
///
/// Prefer [`KeyHint::for_action`], which shows whatever the keymap binds, in
/// the platform's notation. Use [`KeyHint::new`] only for keys that aren't
/// actions, such as Enter in a text field.
#[derive(IntoElement)]
pub struct KeyHint {
    kbd: Kbd,
    label: SharedString,
}

impl KeyHint {
    /// `keystroke` uses GPUI's keymap syntax, such as `"enter"` or
    /// `"secondary-o"`.
    ///
    /// # Panics
    ///
    /// If `keystroke` isn't valid keymap syntax; pass literals only.
    pub fn new(keystroke: &str, label: impl Into<SharedString>) -> Self {
        let keystroke = Keystroke::parse(keystroke).expect("key hints use valid keystrokes");
        Self {
            kbd: Kbd::new(keystroke),
            label: label.into(),
        }
    }

    /// The hint for `action`'s key binding in `context`, or `None` when
    /// nothing binds it.
    pub fn for_action(
        action: &dyn Action,
        context: Option<&str>,
        label: impl Into<SharedString>,
        window: &Window,
    ) -> Option<Self> {
        Some(Self {
            kbd: Kbd::binding_for_action(action, context, window)?,
            label: label.into(),
        })
    }
}

impl RenderOnce for KeyHint {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_1p5()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(self.kbd)
            .child(self.label)
    }
}
