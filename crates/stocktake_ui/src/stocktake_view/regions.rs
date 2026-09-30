//! The regions of the counting screen. Each reads the view's state and
//! renders it; none of them change it.
//!
//! ```text
//! ┌ header ─────────────────────────────── Importer…  Eksporter… ┐
//! ├ page ────────────────────────────────────────────────────────┤
//! │ [ Telt ]  [ Gjenstår ]  [ Differanse ]         stat cards    │
//! │ ( all counted )                                 when done    │
//! │ ┌ products card ──────────────────────────────────────────┐  │
//! │ │ search                                                   │  │
//! │ │ table                                                    │  │
//! │ └──────────────────────────────────────────────────────────┘  │
//! ├ status bar ──────────────────────────────────────────────────┤
//! ```

use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::Input,
    progress::Progress,
    status_bar::StatusBar,
    table::DataTable,
};
use ui::{Card, KeyHint, Stat, Surface, prelude::*};

use super::{SaveState, Session, StocktakeView, files::ImportSource};
use crate::{CONTEXT, COUNT_CELL_CONTEXT, CancelCount, ExportStocktake, ImportStockList};

impl StocktakeView {
    /// What this window is, and the commands that apply to all of it.
    /// Export leads; importing starts over, so it stays quiet.
    fn render_header(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let total = session.stocktake.read(cx).len();
        h_flex()
            .gap_4()
            .px_6()
            .py_4()
            .bg(Surface::Card.bg(cx))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(div().text_lg().font_semibold().child("Varetelling"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{total} varer fra varelisten i MultiCase")),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
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

    /// The three numbers a counter checks between scans.
    fn render_stats(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let stocktake = session.stocktake.read(cx);
        let (counted, total) = (stocktake.counted_len(), stocktake.len());
        let uncounted = stocktake.uncounted_len();
        let differing = stocktake.differing_len();
        let percent = counted as f32 / total.max(1) as f32 * 100.;
        let complete = uncounted == 0;

        h_flex()
            .items_stretch()
            .gap_4()
            .child(
                div().flex_1().child(
                    Stat::new("stat-counted", "Telt", counted)
                        .suffix(format!("av {total}"))
                        .icon(IconName::ListChecks)
                        .footer(
                            Progress::new("progress")
                                .small()
                                .value(percent)
                                .when(complete, |this| this.color(cx.theme().success))
                                .accessibility_label(format!("{counted} av {total} varer telt")),
                        ),
                ),
            )
            .child(
                div().flex_1().child(
                    Stat::new("stat-uncounted", "Gjenstår", uncounted)
                        .icon(IconName::CircleDashed)
                        .detail(match uncounted {
                            0 => "Alle varene er telt".to_string(),
                            1 => "vare er ikke telt ennå".to_string(),
                            _ => "varer er ikke telt ennå".to_string(),
                        }),
                ),
            )
            .child(
                div().flex_1().child(
                    Stat::new("stat-differing", "Differanse", differing)
                        .icon(IconName::ArrowRightLeft)
                        .detail(match differing {
                            0 => "Ingen avvik så langt".to_string(),
                            1 => "vare må justeres i MultiCase".to_string(),
                            _ => "varer må justeres i MultiCase".to_string(),
                        }),
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

    /// The stock list: the search that drives it, over the table it filters.
    fn render_products(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        Card::new()
            .flex_1()
            .min_h_0()
            .child(
                v_flex()
                    .p_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search)
                            .id("search")
                            .prefix(Icon::new(IconName::ScanBarcode).small())
                            .cleanable(true),
                    )
                    .children(self.render_search_hint(cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    // The card draws the frame, so the table doesn't draw another.
                    .child(DataTable::new(&session.table).small().bordered(false)),
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
                    .gap_4()
                    .px_5()
                    .py_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.success.opacity(0.3))
                    .bg(theme.background.blend(theme.success.opacity(0.07)))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size_10()
                            .rounded_full()
                            .bg(theme.success.opacity(0.15))
                            .child(Icon::new(IconName::PartyPopper).text_color(theme.success)),
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
            .px_6()
            .bg(Surface::Card.bg(cx))
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
            .bg(Surface::Page.bg(cx))
            .child(self.render_header(session, cx))
            .child(
                Appear::new(("session", session.generation))
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .size_full()
                            .px_6()
                            .py_5()
                            .gap_4()
                            .child(self.render_stats(session, cx))
                            .children(self.render_completion(session, cx))
                            .child(self.render_products(session, cx)),
                    ),
            )
            .child(self.render_status_bar(window, cx))
    }
}
