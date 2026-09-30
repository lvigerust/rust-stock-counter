//! The screen before any stock list is imported.

use std::time::Duration;

use gpui_kit::component::{alert::Alert, button::Button};
use gpui_kit::rems;
use ui::{Card, KeyHint, Surface, prelude::*};

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
        let theme = cx.theme();
        let muted = theme.muted_foreground;

        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .bg(Surface::Page.bg(cx))
            .when_some(self.resume_error, |this, error| {
                this.child(Alert::error("resume-error", error))
            })
            .child(
                v_flex().flex_1().items_center().justify_center().child(
                    Card::new().w(rems(30.)).child(
                        v_flex()
                            .items_center()
                            .gap_6()
                            .p_8()
                            .child(
                                Appear::new("welcome-media").child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size_12()
                                        .rounded_full()
                                        .bg(theme.muted)
                                        .child(Icon::new(IconName::ClipboardCheck).size_6().text_color(muted)),
                                ),
                            )
                            .child(
                                Appear::new("welcome-text").delay(STAGGER).child(
                                    v_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .text_center()
                                        .child(
                                            div()
                                                .text_xl()
                                                .font_semibold()
                                                .child("Start en varetelling"),
                                        )
                                        .child(div().text_sm().text_color(muted).child(
                                            "Importer varelisten fra MultiCase for å begynne å telle.",
                                        )),
                                ),
                            )
                            .child(
                                Appear::new("welcome-drop")
                                    .delay(STAGGER * 2)
                                    .w_full()
                                    .child(Self::render_drop_zone(cx)),
                            )
                            .children(import_hint),
                    ),
                ),
            )
    }
}

impl Welcome {
    /// Where the export can be dropped, with the file dialog as the other
    /// way in. The whole window accepts the drop; this makes it discoverable.
    fn render_drop_zone(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w_full()
            .items_center()
            .gap_3()
            .py_6()
            .px_4()
            .rounded_lg()
            .border_1()
            .border_dashed()
            // Stronger than a hairline, so the dashes read as "drop here".
            .border_color(theme.muted_foreground.opacity(0.35))
            .child(
                Icon::new(IconName::FileSpreadsheet)
                    .size_6()
                    .text_color(theme.muted_foreground),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_0p5()
                    .child(div().text_sm().font_medium().child("Slipp Excel-filen her"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Vareliste-eksporten fra MultiCase, som .xlsx"),
                    ),
            )
            .child(
                Button::new("import-first")
                    .icon(IconName::FolderOpen)
                    .label("Velg fil…")
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(ImportStockList), cx)
                    }),
            )
    }
}
