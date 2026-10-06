//! The counting loop: a scan or a typed search finds a product, its count
//! dialog takes the quantity, the count is saved, and focus returns to the
//! search for the next scan.

use std::rc::Rc;

use gpui_kit::component::{
    WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
    table::TableState,
};
use stocktake::{Lookup, ProductId};
use ui::{StyledDialog as _, prelude::*};

use super::{Count, Mode, StocktakeView};
use crate::{
    count_dialog::{self, CountActions, Save, location_input, parse_quantity, quantity_input},
    product_table::{LastCounted, ProductTable},
};

impl StocktakeView {
    /// Enter in the search field: a scan or a typed search picks one product.
    pub(super) fn find_product(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let query = open.query(cx);
        if query.is_empty() {
            return;
        }
        match open.session.read(cx).stocktake().lookup(&query) {
            Lookup::Found(id) => self.select_product(id, window, cx),
            // The table already shows the matches to pick from; this says
            // why Enter opened nothing.
            Lookup::Ambiguous(len) => window.push_notification(
                Notification::warning(format!(
                    "{len} varer passer «{query}». Velg riktig i tabellen."
                )),
                cx,
            ),
            Lookup::NotFound => self.show_not_found(&query, window, cx),
        }
    }

    /// `pasted` was pasted into the search. If that made up the whole search
    /// and is a whole barcode or item number, the count dialog opens, as a
    /// scan's Enter would, when the counter has that turned on. A search
    /// that merely contains it, or a name, is left to Enter: only a complete
    /// number is certain enough to act on unasked.
    pub(super) fn search_pasted(
        &mut self,
        pasted: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = &self.open else {
            return;
        };
        let pasted = pasted.trim();
        if !self.settings.read(cx).open_count_on_paste() || open.query(cx) != pasted {
            return;
        }
        let found = open
            .session
            .read(cx)
            .stocktake()
            .product_matching_exactly(pasted);
        // Not through the table's selection: the search's own change may not
        // have narrowed the rows yet, and would clear a selection made first.
        if let Some(id) = found {
            self.begin_count(id, window, cx);
        }
    }

    fn show_not_found(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        // Selected, so the next scan replaces the text instead of appending to it.
        self.focus_search(window, cx);
        let title: SharedString = format!("Fant ingen vare for «{query}»").into();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            dialog
                .alert_frame(cx)
                .title(title.clone())
                .description(
                    "Varen står ikke på varelisten, og blir ikke registrert. \
                     Sjekk at det var riktig strekkode.",
                )
                .ok_text("OK")
        });
    }

    /// Selects the product's row, which opens its count dialog, scrolling it
    /// into view first.
    fn select_product(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let table = open.table.clone();
        match table.read(cx).delegate().row_of(id) {
            Some(row_ix) => table.update(cx, |table, cx| {
                table.set_selected_row(row_ix, cx);
                table.scroll_to_row(row_ix, cx);
            }),
            // An exact barcode match can sit outside the filtered rows.
            None => self.begin_count(id, window, cx),
        }
    }

    /// A row of `table` was selected, by a click, the keyboard or
    /// [`Self::select_product`].
    pub(super) fn select_row(
        &mut self,
        table: &Entity<TableState<ProductTable>>,
        row_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = table.read(cx).delegate().product_at(row_ix) {
            self.begin_count(id, window, cx);
        }
    }

    /// Opens the count dialog for the product, with its location field
    /// filled in with the pick location and its quantity field empty and
    /// focused.
    fn begin_count(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.count.is_some() {
            return;
        }
        let product = open.session.read(cx).stocktake().product(id).clone();
        let location = cx.new(|cx| location_input(&product, window, cx));
        let input = cx.new(|cx| quantity_input(window, cx));
        // The dialog's buttons follow both fields. Enter reaches the dialog
        // as its confirm action, so it isn't handled here.
        let refresh = |_: &mut Self,
                       _: &Entity<InputState>,
                       event: &InputEvent,
                       window: &mut Window,
                       _: &mut Context<Self>| {
            if let InputEvent::Change = event {
                window.refresh();
            }
        };
        open.count = Some(Count {
            product: id,
            location: location.clone(),
            input: input.clone(),
            _input_events: [
                cx.subscribe_in(&location, window, refresh),
                cx.subscribe_in(&input, window, refresh),
            ],
        });

        let view = cx.entity().downgrade();
        let actions = CountActions {
            on_save: Rc::new({
                let view = view.clone();
                move |save, moves_pick_location, window, cx| {
                    view.update(cx, |this, cx| {
                        this.save_count(save, moves_pick_location, window, cx)
                    })
                    .ok();
                }
            }),
            on_finish: Rc::new({
                let view = view.clone();
                move |finished, window, cx| {
                    view.update(cx, |this, cx| this.finish_product(finished, window, cx))
                        .ok();
                }
            }),
            on_pick_listed: Rc::new({
                let view = view.clone();
                move |location, window, cx| {
                    view.update(cx, |this, cx| {
                        this.pick_listed_location(location, window, cx)
                    })
                    .ok();
                }
            }),
            on_cancel: Rc::new(move |window, cx| {
                view.update(cx, |this, cx| this.dismiss_count(window, cx))
                    .ok();
            }),
        };
        count_dialog::open(product, location, input.clone(), actions, window, cx);
        // After the dialog has taken focus for itself.
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    /// Enter or a save button in the count dialog: saves the quantity at
    /// the location, replacing or adding to any earlier count there, and
    /// makes it the pick location if the counter asked for that.
    fn save_count(
        &mut self,
        save: Save,
        moves_pick_location: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = &self.open else {
            return;
        };
        let Some(count) = &open.count else {
            return;
        };
        let (id, location, text) = (
            count.product,
            count.location.read(cx).value(),
            count.input.read(cx).value(),
        );
        if self.take_scan_from_quantity(&text, window, cx) {
            return;
        }
        let Some(quantity) = parse_quantity(&text) else {
            return;
        };
        let Some(open) = &mut self.open else {
            return;
        };
        let product = open.session.read(cx).stocktake().product(id);
        let earlier = if moves_pick_location {
            product.count_at_moved(&location)
        } else {
            product.count_at(&location)
        };
        let quantity = match (save, earlier) {
            (Save::Add, Some(earlier)) => earlier + quantity,
            _ => quantity,
        };
        // Taken before closing, so the dialog's own cancel finds nothing to
        // dismiss.
        open.count = None;
        window.close_dialog(cx);
        open.session.update(cx, |session, cx| {
            session.record_count(id, &location, quantity, moves_pick_location, cx)
        });
        self.finish_count(window, cx);
        self.reveal_counted(id, cx);
    }

    /// The Ferdig talt button in the count dialog: marks the product
    /// finished, or unmarks it, and ends the count without recording one.
    fn finish_product(&mut self, finished: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(count) = open.count.take() else {
            return;
        };
        window.close_dialog(cx);
        open.session.update(cx, |session, cx| {
            session.set_finished(count.product, finished, cx)
        });
        self.finish_count(window, cx);
        self.reveal_counted(count.product, cx);
    }

    /// One of the locations the stock list also listed the product at was
    /// picked in the count dialog: it becomes the pick location, and the
    /// count ends, since the dialog was built for the old one.
    fn pick_listed_location(
        &mut self,
        location: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(count) = open.count.take() else {
            return;
        };
        window.close_dialog(cx);
        open.session.update(cx, |session, cx| {
            session.pick_listed_location(count.product, location, cx)
        });
        self.finish_count(window, cx);
        self.reveal_counted(count.product, cx);
    }

    /// A scanner types the barcode and then presses Enter. If the counter
    /// scans the next product before confirming this one, the barcode lands
    /// in the quantity field and would be saved as billions of units. A
    /// listed product's exact barcode is never a plausible quantity, so it's
    /// taken as the scan it was: nothing is saved and the scanned product is
    /// counted next. Returns whether that happened.
    fn take_scan_from_quantity(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(open) = &mut self.open else {
            return false;
        };
        let Some(scanned) = open.session.read(cx).stocktake().product_with_barcode(text) else {
            return false;
        };
        if open.count.take().is_some() {
            window.close_dialog(cx);
        }
        self.finish_count(window, cx);
        self.select_product(scanned, window, cx);
        true
    }

    /// The count dialog was dismissed: nothing is saved, and the search is
    /// cleared so the next scan doesn't append to the last.
    fn dismiss_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.count.take().is_some() {
            self.finish_count(window, cx);
        }
    }

    /// Ends the count and gets ready for the next scan. Showing the
    /// differences, there's no search to go back to, so focus returns to the
    /// window, where its shortcuts still work.
    fn finish_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let counting = self.mode == Mode::Counting;
        open.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            if counting {
                search.focus(window, cx);
            }
        });
        if !counting {
            self.focus_handle.focus(window, cx);
        }
        open.refresh_rows(cx);
        open.refresh_differences(cx);
        cx.notify();
    }

    /// Scrolls the just-counted product into view and flashes its row, so
    /// the counter sees where the count landed. Counted again from the
    /// differences, a product that now matches has left them, so there's
    /// no row to show there.
    fn reveal_counted(&mut self, id: ProductId, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let last_counted = LastCounted {
            id,
            at: cx.background_executor().now(),
        };
        for table in open.tables() {
            table.update(cx, |table, cx| {
                table.delegate_mut().set_last_counted(last_counted);
                if let Some(row_ix) = table.delegate().row_of(id) {
                    table.scroll_to_row(row_ix, cx);
                }
                cx.notify();
            });
        }
    }

    /// Back to the search, with its text selected so typing replaces it. A
    /// scan is for counting, so this brings the stock list back from any
    /// other mode first.
    pub(super) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.is_none() {
            return;
        }
        self.set_mode(Mode::Counting, window, cx);
        let Some(open) = &self.open else {
            return;
        };
        open.search.update(cx, |search, cx| {
            search.focus(window, cx);
            search.select_all(window, cx);
        });
    }
}
