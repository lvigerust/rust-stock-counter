//! The counting loop: a scan or a typed search finds a product, its count
//! dialog takes the quantity, the count is saved, and focus returns to the
//! search for the next scan.

use gpui_kit::component::{WindowExt as _, input::InputEvent};
use stocktake::{Lookup, ProductId};
use ui::prelude::*;

use super::{Count, StocktakeView};
use crate::{
    count_dialog::{self, parse_quantity, quantity_input},
    product_table::LastCounted,
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
            // The table already shows the matches to pick from.
            Lookup::Ambiguous(_) => {}
            Lookup::NotFound => self.show_not_found(query, window, cx),
        }
    }

    fn show_not_found(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        // Selected, so the next scan replaces the text instead of appending to it.
        self.focus_search(window, cx);
        let title: SharedString = format!("Fant ingen vare for «{query}»").into();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            dialog
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

    /// A row was selected, by a click, the keyboard or [`Self::select_product`].
    pub(super) fn select_row(
        &mut self,
        row_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = &self.open else {
            return;
        };
        if let Some(id) = open.table.read(cx).delegate().product_at(row_ix) {
            self.begin_count(id, window, cx);
        }
    }

    /// Opens the count dialog for the product, with its quantity field
    /// filled in and selected: Enter keeps what's there, typing replaces it.
    /// That's the earlier count if there is one, or else the system quantity,
    /// so confirming it takes a single Enter.
    fn begin_count(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.count.is_some() {
            return;
        }
        let product = open.session.read(cx).stocktake().product(id).clone();
        let prefill = product
            .counted_quantity()
            .unwrap_or(product.system_quantity());
        let input = cx.new(|cx| quantity_input(window, cx));
        input.update(cx, |input, cx| {
            input.set_value(prefill.to_string(), window, cx)
        });
        // The dialog enables Lagre from the field. Enter reaches the dialog
        // as its confirm action, so it isn't handled here.
        let input_events = cx.subscribe_in(&input, window, |_, _, event, window, _| {
            if let InputEvent::Change = event {
                window.refresh();
            }
        });
        open.count = Some(Count {
            product: id,
            input: input.clone(),
            _input_events: input_events,
        });

        let view = cx.entity().downgrade();
        count_dialog::open(
            &product,
            input.clone(),
            {
                let view = view.clone();
                move |window, cx| {
                    view.update(cx, |this, cx| this.save_count(window, cx)).ok();
                }
            },
            move |window, cx| {
                view.update(cx, |this, cx| this.dismiss_count(window, cx))
                    .ok();
            },
            window,
            cx,
        );
        // After the dialog has taken focus for itself.
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        });
    }

    /// Enter or Lagre in the count dialog: saves the quantity, replacing any
    /// earlier count.
    fn save_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let Some(count) = &open.count else {
            return;
        };
        let (id, text) = (count.product, count.input.read(cx).value());
        if self.take_scan_from_quantity(&text, window, cx) {
            return;
        }
        let Some(quantity) = parse_quantity(&text) else {
            return;
        };
        let Some(open) = &mut self.open else {
            return;
        };
        // Taken before closing, so the dialog's own cancel finds nothing to
        // dismiss.
        open.count = None;
        window.close_dialog(cx);
        open.session
            .update(cx, |session, cx| session.record_count(id, quantity, cx));
        self.finish_count(window, cx);
        self.reveal_counted(id, cx);
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

    /// Ends the count and gets ready for the next scan.
    fn finish_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        open.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        open.refresh_rows(cx);
        cx.notify();
    }

    /// Scrolls the just-counted product into view and flashes its row, so
    /// the counter sees where the count landed.
    fn reveal_counted(&mut self, id: ProductId, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let last_counted = LastCounted {
            id,
            at: cx.background_executor().now(),
        };
        open.table.update(cx, |table, cx| {
            table.delegate_mut().set_last_counted(last_counted);
            if let Some(row_ix) = table.delegate().row_of(id) {
                table.scroll_to_row(row_ix, cx);
            }
            cx.notify();
        });
    }

    /// Back to the search, with its text selected so typing replaces it.
    pub(super) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        open.search.update(cx, |search, cx| {
            search.focus(window, cx);
            search.select_all(window, cx);
        });
    }
}
