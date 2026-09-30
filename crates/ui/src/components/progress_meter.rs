use gpui_kit::component::progress::Progress;

use crate::prelude::*;

/// A labelled progress bar that turns into a success state when complete.
///
/// The bar animates between values on its own. The caller writes the label
/// (`31 av 43 telt`), since only it knows the words.
#[derive(IntoElement)]
pub struct ProgressMeter {
    id: ElementId,
    fraction: f32,
    label: SharedString,
    detail: Option<SharedString>,
}

impl ProgressMeter {
    /// `fraction` is how much is done, from 0 to 1.
    pub fn new(id: impl Into<ElementId>, fraction: f32, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            fraction: fraction.clamp(0.0, 1.0),
            label: label.into(),
            detail: None,
        }
    }

    /// Secondary text at the trailing edge, such as a percentage.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn is_complete(&self) -> bool {
        self.fraction >= 1.0
    }
}

impl RenderOnce for ProgressMeter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let complete = self.is_complete();
        let accessibility_label = self.label.clone();
        v_flex()
            .w_full()
            .gap_1p5()
            .child(
                h_flex()
                    .gap_1p5()
                    .text_sm()
                    .when(complete, |this| {
                        this.child(
                            Icon::new(IconName::CircleCheck)
                                .small()
                                .text_color(cx.theme().success),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .tabular_nums()
                            .child(self.label),
                    )
                    .when_some(self.detail, |this, detail| {
                        this.child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .tabular_nums()
                                .child(detail),
                        )
                    }),
            )
            .child(
                Progress::new(self.id)
                    .small()
                    .value(self.fraction * 100.0)
                    .when(complete, |this| this.color(cx.theme().success))
                    .accessibility_label(accessibility_label),
            )
    }
}
