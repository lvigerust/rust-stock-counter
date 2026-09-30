//! The screen before any stock list is imported.

use std::time::Duration;

use gpui_kit::component::{alert::Alert, button::Button};
use ui::{KeyHint, prelude::*};

use crate::{CONTEXT, ImportStockList};

/// How far apart the welcome's parts arrive, so they read top to bottom.
const STAGGER: Duration = Duration::from_millis(70);

/// Invites the counter to import a stock list: the only thing to do here.
///
/// Importing is the owner's job; this only asks for it through
/// [`ImportStockList`], the same action as the menu shortcut.
#[derive(IntoElement)]
pub(crate) struct Welcome {
    resume_error: Option<SharedString>,
}

impl Welcome {
    pub fn new() -> Self {
        Self { resume_error: None }
    }

    /// Why the saved stocktake couldn't be resumed, shown above the invitation.
    pub fn resume_error(mut self, error: Option<SharedString>) -> Self {
        self.resume_error = error;
        self
    }
}

impl RenderOnce for Welcome {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let import_hint = KeyHint::for_action(&ImportStockList, Some(CONTEXT), "Snarvei", window);

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .when_some(self.resume_error, |this, error| {
                this.child(Alert::error("resume-error", error))
            })
            .child(
                // The dashed boundary says "drop a file here" before the
                // text does. The whole window accepts the drop.
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_6()
                    .border_2()
                    .border_dashed()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .child(
                        Appear::new("welcome-media").child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_16()
                                .rounded_full()
                                .bg(cx.theme().muted)
                                .child(
                                    Icon::new(IconName::FileSpreadsheet)
                                        .size_8()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                    )
                    .child(
                        Appear::new("welcome-text").delay(STAGGER).child(
                            v_flex()
                                .items_center()
                                .gap_2()
                                .max_w_96()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_semibold()
                                        .child("Ingen varetelling pågår"),
                                )
                                .child(
                                    div()
                                        .text_center()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            "Importer varelisten fra MultiCase for å begynne å \
                                             telle. Du kan også slippe Excel-filen her.",
                                        ),
                                ),
                        ),
                    )
                    .child(
                        Appear::new("welcome-actions").delay(STAGGER * 2).child(
                            v_flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    Button::new("import-first")
                                        .large()
                                        .icon(IconName::FileInput)
                                        .label("Importer vareliste…")
                                        .on_click(|_, window, cx| {
                                            window.dispatch_action(Box::new(ImportStockList), cx)
                                        }),
                                )
                                .children(import_hint),
                        ),
                    ),
            )
    }
}
