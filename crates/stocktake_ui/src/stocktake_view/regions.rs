//! The regions of the counting screen. Each reads the view's state and
//! renders it; none of them change it.
//!
//! The screen is flat and runs edge to edge, like a pane in an editor:
//! bands separated by hairlines, no cards, no gaps.
//!
//! ```text
//! ┌ toolbar ─ ● ● ●  [ search ]  31 av 43 telt ▬▬ │ 12 gjenstår │ 3 med differanse   Importer…  Eksporter… ┐
//! ├ hint or completion, when there is one ─────────────────────────────────────────────────────────┤
//! │ table                                                                                           │
//! ├ status bar ────────────────────────────────────────────────────────────────────────────────────┤
//! ```

use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::Input,
    kbd::Kbd,
    progress::Progress,
    status_bar::StatusBar,
    table::DataTable,
};
use gpui_kit::{MouseButton, rems};
use ui::{KeyHint, Stat, WindowBar, prelude::*};

use super::{SaveState, Session, StocktakeView, files::ImportSource};
use crate::{
    CONTEXT, COUNT_CELL_CONTEXT, CancelCount, ExportStocktake, FocusSearch, ImportStockList,
};

impl StocktakeView {
    /// Search on the leading edge, where the scanner's text lands; progress
    /// in the middle; commands for the whole stocktake on the trailing edge.
    ///
    /// It is also the window's title bar: the traffic lights sit in it, and
    /// its background drags the window. The search and the buttons keep
    /// their presses to themselves so they never move it.
    fn render_toolbar(
        &self,
        session: &Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let search_key = Kbd::binding_for_action(&FocusSearch, Some(CONTEXT), window);
        WindowBar::new()
            .gap_5()
            .border_b_1()
            .border_color(cx.theme().border)
            // The search keeps its width first: it's where every scan lands.
            // Only when the window is too narrow does it give some up.
            .child(
                div()
                    .w(rems(36.))
                    .min_w(rems(16.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&self.search)
                            .id("search")
                            .prefix(Icon::new(IconName::ScanBarcode).small())
                            .when_some(search_key, |input, key| input.suffix(key))
                            .cleanable(true),
                    ),
            )
            .child(self.render_stats(session, cx))
            .child(div().flex_1().min_w_0())
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("import")
                            .ghost()
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

    /// The three numbers a counter checks between scans, as inline
    /// metadata rather than as headline figures.
    fn render_stats(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let stocktake = session.stocktake.read(cx);
        let (counted, total) = (stocktake.counted_len(), stocktake.len());
        let uncounted = stocktake.uncounted_len();
        let differing = stocktake.differing_len();
        let percent = counted as f32 / total.max(1) as f32 * 100.;
        let divider = || div().w_px().h_4().bg(cx.theme().border);

        h_flex()
            .flex_none()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(Stat::new(
                        "stat-counted",
                        counted,
                        format!("av {total} telt"),
                    ))
                    .child(
                        div().w_20().child(
                            Progress::new("progress")
                                .xsmall()
                                .value(percent)
                                .when(uncounted == 0, |this| this.color(cx.theme().success))
                                .accessibility_label(format!("{counted} av {total} varer telt")),
                        ),
                    ),
            )
            .child(divider())
            .child(Stat::new("stat-uncounted", uncounted, "gjenstår"))
            .child(divider())
            .child(Stat::new("stat-differing", differing, "med differanse"))
    }

    fn render_search_hint(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (hint, generation) = self.search_hint.clone()?;
        Some(
            Appear::new(("search-hint", generation)).child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Info).small())
                    .child(hint),
            ),
        )
    }

    /// The band shown once every product is counted: the result, and the one
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
        let summary = match stocktake.differing_len() {
            0 => "Ingen varer har differanse.".to_string(),
            1 => {
                "1 vare har differanse. Eksporter tellingen og før den inn i MultiCase.".to_string()
            }
            n => {
                format!("{n} varer har differanse. Eksporter tellingen og før dem inn i MultiCase.")
            }
        };
        let theme = cx.theme();
        Some(
            Appear::new(("complete", session.generation)).child(
                h_flex()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.background.blend(theme.success.opacity(0.08)))
                    .text_sm()
                    .child(Icon::new(IconName::CircleCheck).text_color(theme.success))
                    .child(div().font_semibold().child("Alle varene er telt"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(summary),
                    )
                    .child(
                        Button::new("complete-export")
                            .small()
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
    fn render_status_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let theme = cx.theme();
        let (save_icon, save_color, save_label) = match self.save_state {
            SaveState::Saved => (IconName::Check, theme.muted_foreground, "Alt er lagret"),
            SaveState::Failed => (IconName::TriangleAlert, theme.danger, "Ikke lagret"),
        };

        StatusBar::new()
            .px_4()
            .left(h_flex().gap_5().children(hints.into_iter().flatten()))
            .right(
                h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(save_color)
                    .child(Icon::new(save_icon).xsmall())
                    .child(save_label),
            )
    }

    pub(super) fn render_counting(
        &self,
        session: &Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.render_toolbar(session, window, cx))
            .children(self.render_search_hint(cx))
            .children(self.render_completion(session, cx))
            .child(
                Appear::new(("session", session.generation))
                    .flex_1()
                    .min_h_0()
                    // The table fills the pane; the bands around it draw
                    // the hairlines, so it doesn't draw its own frame.
                    .child(DataTable::new(&session.table).small().bordered(false)),
            )
            .child(self.render_status_bar(window, cx))
    }
}
