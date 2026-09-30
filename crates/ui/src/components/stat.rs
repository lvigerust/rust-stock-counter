use gpui_kit::base::motion::{Transition, transition};

use crate::prelude::*;

/// A number followed by a muted label on one line, such as `12 gjenstår`.
/// Compact enough to sit several in a toolbar, separated by dividers.
///
/// When the number changes it counts to the new value instead of jumping,
/// so a change is noticed without being announced.
#[derive(IntoElement)]
pub struct Stat {
    id: ElementId,
    value: usize,
    label: SharedString,
}

impl Stat {
    pub fn new(id: impl Into<ElementId>, value: usize, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value,
            label: label.into(),
        }
    }
}

impl RenderOnce for Stat {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens().clone();
        let shown = transition(
            (self.id.clone(), "value"),
            self.value as f32,
            Transition::new(motion.duration_slow).easing(motion.easing_move),
            window,
            cx,
        )
        .round() as usize;

        h_flex()
            .id(self.id)
            .items_baseline()
            .gap_1p5()
            .whitespace_nowrap()
            .child(
                div()
                    .font_semibold()
                    .tabular_nums()
                    .child(shown.to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.label),
            )
    }
}
