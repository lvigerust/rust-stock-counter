//! The regions of the counting screen. Each reads the view's state and
//! renders it; none of them change it.

use gpui_kit::component::{button::Button, input::Input, status_bar::StatusBar, table::DataTable};
use ui::{KeyHint, ProgressMeter, prelude::*};

use super::{SaveState, Session, StocktakeView, files::ImportSource};
use crate::{CONTEXT, COUNT_CELL_CONTEXT, CancelCount, ExportStocktake, ImportStockList};

impl StocktakeView {
    fn render_toolbar(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let stocktake = session.stocktake.read(cx);
        let (counted, total) = (stocktake.counted_len(), stocktake.len());
        let fraction = counted as f32 / total.max(1) as f32;
        let label = if counted == total {
            format!("Alle {total} telt")
        } else {
            format!("{counted} av {total} telt")
        };

        h_flex()
            .gap_4()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .id("search")
                        .prefix(Icon::new(IconName::ScanBarcode).small())
                        .cleanable(true),
                ),
            )
            .child(
                div().w_56().flex_none().child(
                    ProgressMeter::new("progress", fraction, label)
                        .detail(format!("{} %", (fraction * 100.0).round())),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("import")
                            .outline()
                            .icon(IconName::FileInput)
                            .label("Importer…")
                            .tooltip_with_action(
                                "Importer ny vareliste",
                                &ImportStockList,
                                Some(CONTEXT),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.import_stock_list(ImportSource::Choose, window, cx)
                            })),
                    )
                    .child(
                        Button::new("export")
                            .outline()
                            .icon(IconName::Download)
                            .label("Eksporter…")
                            .tooltip_with_action(
                                "Eksporter tellingen til Excel",
                                &ExportStocktake,
                                Some(CONTEXT),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.export_stocktake(window, cx)
                                }),
                            ),
                    ),
            )
    }

    fn render_search_hint(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (hint, generation) = self.search_hint.clone()?;
        Some(
            Appear::new(("search-hint", generation)).child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Info).small())
                    .child(hint),
            ),
        )
    }

    /// The summary once every product is counted: the result, and the one
    /// thing left to do with it.
    fn render_completion(
        &self,
        session: &Session,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let stocktake = session.stocktake.read(cx);
        if stocktake.uncounted_len() > 0 {
            return None;
        }
        let summary =
            match stocktake.differing_len() {
                0 => "Ingen varer har differanse. Eksporter tellingen for å ta vare på den."
                    .to_string(),
                1 => "1 vare har differanse. Eksporter tellingen og før den inn i MultiCase."
                    .to_string(),
                n => format!(
                    "{n} varer har differanse. Eksporter tellingen og før dem inn i MultiCase."
                ),
            };
        let theme = cx.theme();
        Some(
            Appear::new(("complete", session.generation)).child(
                h_flex()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.success.opacity(0.35))
                    .bg(theme.success.opacity(0.08))
                    .child(
                        Icon::new(IconName::PartyPopper)
                            .large()
                            .text_color(theme.success),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().font_semibold().child("Alle varene er telt"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(summary),
                            ),
                    )
                    .child(
                        Button::new("complete-export")
                            .icon(IconName::Download)
                            .label("Eksporter…")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.export_stocktake(window, cx)
                                }),
                            ),
                    ),
            ),
        )
    }

    /// Teaches the keys for what can be done right now, and confirms that
    /// every count is safely on disk.
    fn render_status_bar(
        &self,
        session: &Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hints: Vec<Option<KeyHint>> = if self.counting(cx).is_some() {
            vec![
                Some(KeyHint::new("enter", "Lagre antall")),
                KeyHint::for_action(&CancelCount, Some(COUNT_CELL_CONTEXT), "Avbryt", window),
            ]
        } else {
            vec![
                Some(KeyHint::new("enter", "Finn vare")),
                KeyHint::for_action(&ImportStockList, Some(CONTEXT), "Importer", window),
                KeyHint::for_action(&ExportStocktake, Some(CONTEXT), "Eksporter", window),
            ]
        };
        let differing = session.stocktake.read(cx).differing_len();
        let theme = cx.theme();
        let (save_icon, save_color, save_label) = match self.save_state {
            SaveState::Saved => (IconName::Check, theme.muted_foreground, "Lagret"),
            SaveState::Failed => (IconName::TriangleAlert, theme.danger, "Ikke lagret"),
        };

        StatusBar::new()
            .left(h_flex().gap_4().children(hints.into_iter().flatten()))
            .right(
                h_flex()
                    .gap_4()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .when(differing > 0, |this| {
                        this.child(match differing {
                            1 => "1 vare med differanse".to_string(),
                            n => format!("{n} varer med differanse"),
                        })
                    })
                    .child(
                        h_flex()
                            .gap_1()
                            .text_color(save_color)
                            .child(Icon::new(save_icon).xsmall())
                            .child(save_label),
                    ),
            )
    }

    pub(super) fn render_counting(
        &self,
        session: &Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Appear::new(("session", session.generation))
            .size_full()
            .child(
                v_flex()
                    .size_full()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .p_4()
                            .gap_3()
                            .child(self.render_toolbar(session, cx))
                            .children(self.render_search_hint(cx))
                            .children(self.render_completion(session, cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(DataTable::new(&session.table).small()),
                            ),
                    )
                    .child(self.render_status_bar(session, window, cx)),
            )
    }
}
