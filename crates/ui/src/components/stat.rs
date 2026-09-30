use gpui_kit::base::motion::{Transition, transition};

use crate::{Card, prelude::*};

/// One key number on its own card: a muted label, the number large, and a
/// line of context beneath. Like the metric cards on a Shadcn dashboard.
///
/// When the number changes it counts to the new value instead of jumping,
/// so a change is noticed without being announced.
///
/// ```ignore
/// Stat::new("remaining", "Gjenstår", 12)
///     .icon(IconName::CircleDashed)
///     .detail("varer er ikke telt")
/// ```
#[derive(IntoElement)]
pub struct Stat {
    id: ElementId,
    label: SharedString,
    value: usize,
    suffix: Option<SharedString>,
    detail: Option<SharedString>,
    icon: Option<IconName>,
    footer: Option<AnyElement>,
}

impl Stat {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, value: usize) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
            suffix: None,
            detail: None,
            icon: None,
            footer: None,
        }
    }

    /// Small text right after the number, such as `%` or `av 43`.
    pub fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// A line of context under the number.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A muted icon at the trailing edge of the label row.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Content at the bottom of the card, such as a progress bar.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
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
        let muted = cx.theme().muted_foreground;

        Card::new().child(
            v_flex()
                .id(self.id)
                .p_5()
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .text_sm()
                        .font_medium()
                        .text_color(muted)
                        .child(self.label)
                        .children(self.icon.map(|icon| Icon::new(icon).small())),
                )
                .child(
                    h_flex()
                        .items_baseline()
                        .gap_1p5()
                        .child(
                            div()
                                .text_3xl()
                                .font_semibold()
                                .tabular_nums()
                                .child(shown.to_string()),
                        )
                        .when_some(self.suffix, |this, suffix| {
                            this.child(div().text_sm().text_color(muted).child(suffix))
                        }),
                )
                .when_some(self.detail, |this, detail| {
                    this.child(div().text_xs().text_color(muted).child(detail))
                })
                .when_some(self.footer, |this, footer| {
                    this.child(div().pt_3().child(footer))
                }),
        )
    }
}
