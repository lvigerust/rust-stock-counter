//! The window: importing a stock list, counting it, and exporting the result.
//!
//! [`StocktakeView`] owns the workflow — which product is being counted, where
//! focus goes next, when to save — and composes the regions that render it.
//!
//! One type, several files, split by concern the way Zed splits its editor:
//! this file holds the state and the counting workflow, `files` getting stock
//! lists in and results out, and `regions` the parts of the counting screen.
//! They are child modules, so they share the view's private fields without
//! making any of them public.

mod files;
mod regions;

use std::path::PathBuf;

use gpui_kit::component::{
    Theme, WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
    table::{TableEvent, TableState},
};
use gpui_kit::{ExternalPaths, Focusable as _, Subscription};
use stocktake::{Lookup, ProductId, Stocktake, store};
use ui::prelude::*;

use crate::{
    CONTEXT, CancelCount, ExportStocktake, FocusNext, FocusPrevious, ImportStockList,
    product_table::{LastCounted, ProductTable},
    quantity::{parse_quantity, quantity_input},
    recount_dialog::{self, Recount},
    welcome::Welcome,
};
use files::ImportSource;

/// Whether the last change reached the disk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveState {
    Saved,
    Failed,
}

/// The stocktake in progress and the table showing it.
struct Session {
    stocktake: Entity<Stocktake>,
    table: Entity<TableState<ProductTable>>,
    /// Tells this session's entrance apart from the previous one's.
    generation: usize,
    _table_events: Subscription,
}

pub struct StocktakeView {
    store_path: PathBuf,
    session: Option<Session>,
    sessions_started: usize,
    search: Entity<InputState>,
    /// The counted-quantity cell of the product being counted.
    count_input: Entity<InputState>,
    /// The quantity field of the dialog for counting a product again.
    recount_input: Entity<InputState>,
    /// Explains why Enter in the search didn't select a product, and how many
    /// hints came before it, so a repeated hint arrives again.
    search_hint: Option<(SharedString, usize)>,
    hints_shown: usize,
    /// The already-counted product the recount dialog is open for.
    pending_recount: Option<ProductId>,
    counts_confirmed: usize,
    save_state: SaveState,
    /// Why the saved stocktake couldn't be resumed.
    resume_error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl StocktakeView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_store_path(store::default_path(), window, cx)
    }

    fn with_store_path(store_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Skann strekkode, eller søk på varenummer eller navn")
        });
        let count_input = cx.new(|cx| quantity_input(window, cx));
        let recount_input = cx.new(|cx| quantity_input(window, cx).placeholder("Antall"));

        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                InputEvent::Change => this.on_search_changed(cx),
                InputEvent::PressEnter { .. } => this.find_product(window, cx),
                _ => {}
            }),
            cx.subscribe_in(&count_input, window, |this, _, event, window, cx| {
                match event {
                    InputEvent::PressEnter { .. } => this.confirm_count(window, cx),
                    // Clicking elsewhere abandons the count without saving it.
                    InputEvent::Blur => this.cancel_count(window, cx),
                    _ => {}
                }
            }),
            // The dialog previews each choice's result from the field. Enter
            // reaches the dialog as its confirm action, so it isn't handled here.
            cx.subscribe_in(&recount_input, window, |_, _, event, window, _| {
                if let InputEvent::Change = event {
                    window.refresh();
                }
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];
        Theme::sync_system_appearance(Some(window), cx);

        let mut this = Self {
            store_path,
            session: None,
            sessions_started: 0,
            search,
            count_input,
            recount_input,
            search_hint: None,
            hints_shown: 0,
            pending_recount: None,
            counts_confirmed: 0,
            save_state: SaveState::Saved,
            resume_error: None,
            _subscriptions: subscriptions,
        };
        match store::load(&this.store_path) {
            Ok(Some(stocktake)) => this.start_session(stocktake, window, cx),
            Ok(None) => {}
            Err(error) => {
                this.resume_error =
                    Some(format!("Den lagrede varetellingen kunne ikke åpnes: {error}").into())
            }
        }
        this
    }

    fn start_session(&mut self, stocktake: Stocktake, window: &mut Window, cx: &mut Context<Self>) {
        let stocktake = cx.new(|_| stocktake);
        let delegate = ProductTable::new(stocktake.clone(), self.count_input.clone(), cx);
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .col_selectable(false)
                .col_movable(false)
                .sortable(false)
        });
        let table_events = cx.subscribe_in(&table, window, |this, _, event, window, cx| {
            if let TableEvent::SelectRow(row_ix) = event {
                this.select_row(*row_ix, window, cx);
            }
        });
        self.sessions_started += 1;
        self.session = Some(Session {
            stocktake,
            table,
            generation: self.sessions_started,
            _table_events: table_events,
        });
        self.resume_error = None;
        self.search_hint = None;
        self.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let result = store::save(&self.store_path, session.stocktake.read(cx));
        self.save_state = match result {
            Ok(()) => SaveState::Saved,
            Err(error) => {
                window.push_notification(
                    Notification::error(format!(
                        "Tellingen ble ikke lagret: {error}. Den går tapt hvis appen lukkes."
                    ))
                    .autohide(false),
                    cx,
                );
                SaveState::Failed
            }
        };
        cx.notify();
    }

    fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_string()
    }

    fn on_search_changed(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let rows = session.stocktake.read(cx).search(&self.query(cx));
        session.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows);
            // Row indices now point at different products.
            table.clear_selection(cx);
        });
        self.search_hint = None;
        cx.notify();
    }

    fn show_search_hint(&mut self, hint: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.hints_shown += 1;
        self.search_hint = Some((hint.into(), self.hints_shown));
        cx.notify();
    }

    /// Enter in the search field: a scan or a typed search picks one product.
    fn find_product(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let query = self.query(cx);
        if query.is_empty() {
            return;
        }
        match session.stocktake.read(cx).lookup(&query) {
            Lookup::Found(id) => self.select_product(id, window, cx),
            Lookup::Ambiguous(count) => self.show_search_hint(
                format!("{count} varer passer. Skriv mer, eller velg varen i tabellen."),
                cx,
            ),
            Lookup::NotFound => self.show_not_found(query, window, cx),
        }
    }

    fn show_not_found(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        // Selected, so the next scan replaces the text instead of appending to it.
        self.search
            .update(cx, |search, cx| search.select_all(window, cx));
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

    fn select_product(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let table = session.table.clone();
        let row_ix = table.read(cx).delegate().row_of(id);
        match row_ix {
            Some(row_ix) => table.update(cx, |table, cx| {
                table.set_selected_row(row_ix, cx);
                table.scroll_to_row(row_ix, cx);
            }),
            // An exact barcode match can sit outside the filtered rows.
            None => self.begin_count(id, window, cx),
        }
    }

    fn select_row(&mut self, row_ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        if let Some(id) = session.table.read(cx).delegate().product_at(row_ix) {
            self.begin_count(id, window, cx);
        }
    }

    fn counting(&self, cx: &App) -> Option<ProductId> {
        self.session
            .as_ref()
            .and_then(|session| session.table.read(cx).delegate().counting())
    }

    fn begin_count(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        // Clicking into the cell being edited selects its row again.
        if self.counting(cx) == Some(id) {
            return;
        }
        let product = session.stocktake.read(cx).product(id).clone();
        match product.counted_quantity() {
            Some(counted) => {
                self.recount_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.pending_recount = Some(id);
                recount_dialog::open(
                    cx.entity().downgrade(),
                    self.recount_input.clone(),
                    product.name(),
                    counted,
                    window,
                    cx,
                );
                let input = self.recount_input.clone();
                cx.defer_in(window, move |_, window, cx| {
                    input.update(cx, |input, cx| input.focus(window, cx));
                });
            }
            None => self.edit_count(id, product.system_quantity(), window, cx),
        }
    }

    /// Moves focus to the product's counted-quantity cell, pre-filled and
    /// selected so typing a number overwrites it.
    fn edit_count(
        &mut self,
        id: ProductId,
        prefill: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = &self.session else {
            return;
        };
        session.table.update(cx, |table, cx| {
            table.delegate_mut().set_counting(Some(id));
            cx.notify();
        });
        self.count_input.update(cx, |input, cx| {
            input.set_value(prefill.to_string(), window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn confirm_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.counting(cx) else {
            return;
        };
        let text = self.count_input.read(cx).value();
        if self.take_scan_from_quantity(&text, window, cx) {
            return;
        }
        let Some(quantity) = parse_quantity(&text) else {
            return;
        };
        self.record_count(id, |_| quantity, window, cx);
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
        let Some(session) = &self.session else {
            return false;
        };
        let stocktake = session.stocktake.read(cx);
        let Some(scanned) = stocktake.product_with_barcode(text) else {
            return false;
        };
        let name = stocktake.product(scanned).name().to_string();
        if self.pending_recount.take().is_some() {
            window.close_dialog(cx);
        }
        self.finish_count(window, cx);
        self.select_product(scanned, window, cx);
        self.show_search_hint(
            format!("Antallet ble ikke lagret, fordi du skannet «{name}». Tell den nå."),
            cx,
        );
        true
    }

    fn cancel_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.counting(cx).is_none() {
            return;
        }
        self.finish_count(window, cx);
    }

    pub(crate) fn apply_recount(
        &mut self,
        recount: Recount,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.pending_recount else {
            return;
        };
        let text = self.recount_input.read(cx).value();
        if self.take_scan_from_quantity(&text, window, cx) {
            return;
        }
        let Some(found) = parse_quantity(&text) else {
            return;
        };
        self.pending_recount = None;
        window.close_dialog(cx);
        self.record_count(
            id,
            |counted| recount.apply(counted.unwrap_or(0), found),
            window,
            cx,
        );
    }

    /// The recount dialog was dismissed without a choice: nothing is saved,
    /// and the search is cleared so the next scan doesn't append to the last.
    pub(crate) fn dismiss_recount(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_recount.take().is_some() {
            self.finish_count(window, cx);
        }
    }

    /// Saves a product's new counted quantity, computed from its current
    /// one, then shows where it landed and gets ready for the next scan.
    fn record_count(
        &mut self,
        id: ProductId,
        quantity: impl FnOnce(Option<i64>) -> i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = &self.session else {
            return;
        };
        session.stocktake.update(cx, |stocktake, cx| {
            let quantity = quantity(stocktake.product(id).counted_quantity());
            stocktake.set_counted_quantity(id, quantity);
            cx.notify();
        });
        self.save(window, cx);
        self.finish_count(window, cx);
        self.reveal_counted(id, cx);
    }

    /// Scrolls the just-counted product into view and flashes its row.
    fn reveal_counted(&mut self, id: ProductId, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        self.counts_confirmed += 1;
        let last_counted = LastCounted {
            id,
            generation: self.counts_confirmed,
        };
        session.table.update(cx, |table, cx| {
            table.delegate_mut().set_last_counted(last_counted);
            if let Some(row_ix) = table.delegate().row_of(id) {
                table.scroll_to_row(row_ix, cx);
            }
            cx.notify();
        });
    }

    /// Ends the count and gets ready for the next scan.
    fn finish_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            session.table.update(cx, |table, cx| {
                table.delegate_mut().set_counting(None);
                cx.notify();
            });
        }
        self.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        self.on_search_changed(cx);
    }

    /// Tab and Shift-Tab, skipping the table. Rows are reached by scanning,
    /// searching or clicking, and the table shows no focus ring, so a Tab
    /// stop on it would look like lost focus.
    fn move_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let table_focus = self
            .session
            .as_ref()
            .map(|session| session.table.focus_handle(cx));
        // Twice at most: the table is a single Tab stop.
        for _ in 0..2 {
            if forward {
                window.focus_next(cx);
            } else {
                window.focus_prev(cx);
            }
            if !table_focus
                .as_ref()
                .is_some_and(|table| table.is_focused(window))
            {
                break;
            }
        }
    }
}

impl Render for StocktakeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &ImportStockList, window, cx| {
                this.import_stock_list(ImportSource::Choose, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExportStocktake, window, cx| {
                this.export_stocktake(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &CancelCount, window, cx| this.cancel_count(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusNext, window, cx| this.move_focus(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPrevious, window, cx| {
                    this.move_focus(false, window, cx)
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.on_drop_files(paths, window, cx)
            }))
            .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().drop_target))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .map(|this| match &self.session {
                Some(session) => this.child(self.render_counting(session, window, cx)),
                None => this.child(Welcome::new().resume_error(self.resume_error.clone())),
            })
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::test::{ElementSnapshot, TestWindowExt as _};
    use gpui_kit::{AnyWindowHandle, TestAppContext, px, size};
    use stocktake::Product;

    use super::*;

    /// Drives the window like a counter at the keyboard. Each step is its own
    /// update, so subscriptions to input events run before the next step.
    struct Counter<'a> {
        cx: &'a mut TestAppContext,
        window: AnyWindowHandle,
    }

    impl Counter<'_> {
        fn input(&mut self, text: &str) {
            self.step(|window, cx| window.input(text, cx));
        }

        fn press(&mut self, key: &str) {
            self.step(|window, cx| window.press(key, cx));
        }

        fn step(&mut self, f: impl FnOnce(&mut Window, &mut App)) {
            self.cx
                .update_window(self.window, |_, window, cx| f(window, cx))
                .unwrap();
            self.cx.run_until_parked();
        }

        fn find(&mut self, id: &'static str) -> Option<ElementSnapshot> {
            self.cx
                .update_window(self.window, |_, window, cx| {
                    window.render_frame(cx);
                    window.try_find(id)
                })
                .unwrap()
        }

        fn is_focused(&mut self, id: &'static str) -> bool {
            self.find(id).and_then(|element| element.focused()) == Some(true)
        }

        fn value(&mut self, id: &'static str) -> Option<String> {
            self.find(id)
                .and_then(|element| element.value().map(str::to_string))
        }
    }

    #[gpui_kit::test]
    fn counts_scanned_products(cx: &mut TestAppContext) {
        let dir = std::env::temp_dir().join(format!("stocktake-ui-{}", std::process::id()));
        let store_path = dir.join("varetelling.json");
        let stocktake = Stocktake::new(vec![
            Product::new("152066", "Burano 120 Hvit", "C4-7", "7043811520667", 33),
            Product::new("152062", "Burano 120 Sort", "C4-7", "7043811520629", 49),
            Product::new("150765", "Veneto 75", "", "", 16),
        ]);
        store::save(&store_path, &stocktake).unwrap();

        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let window = cx.open_window(size(px(1040.), px(720.)), |window, cx| {
            let view = cx.new(|cx| StocktakeView::with_store_path(store_path.clone(), window, cx));
            Root::new(view, window, cx)
        });
        let mut counter = Counter {
            cx,
            window: window.into(),
        };
        // Reads what was saved, which is also what a restarted app resumes.
        let saved = || {
            store::load(&store_path)
                .unwrap()
                .unwrap()
                .products()
                .map(|(_, product)| product.counted_quantity())
                .collect::<Vec<_>>()
        };

        // Scanning selects the product and pre-fills its counted quantity.
        assert!(counter.is_focused("search"));
        counter.input("7043811520629");
        counter.press("enter");
        assert!(counter.is_focused("count"));
        assert_eq!(counter.value("count").as_deref(), Some("49"));

        // Typing overwrites the pre-filled value; Enter confirms and saves.
        counter.input("47");
        counter.press("enter");
        assert!(counter.is_focused("search"));
        assert_eq!(counter.value("search").as_deref(), Some(""));
        assert_eq!(saved(), [None, Some(47), None]);

        // Enter alone confirms the system quantity.
        counter.input("152066");
        counter.press("enter");
        counter.press("enter");
        assert_eq!(saved(), [Some(33), Some(47), None]);

        // Scanning a counted product again adds to its count.
        counter.input("7043811520629");
        counter.press("enter");
        assert!(counter.is_focused("recount"));
        counter.input("3");
        counter.press("enter");
        assert!(counter.find("recount").is_none());
        assert!(counter.is_focused("search"));
        assert_eq!(saved(), [Some(33), Some(50), None]);

        // Products without a barcode are found by name; Escape abandons the
        // count without saving it.
        counter.input("veneto");
        counter.press("enter");
        assert_eq!(counter.value("count").as_deref(), Some("16"));
        counter.press("escape");
        assert!(counter.find("count").is_none());
        assert!(counter.is_focused("search"));
        assert_eq!(saved(), [Some(33), Some(50), None]);

        // Scanning the next product before confirming this one doesn't save
        // the barcode as a quantity; the scanned product is counted instead.
        counter.input("veneto");
        counter.press("enter");
        counter.input("7043811520667");
        counter.press("enter");
        assert_eq!(saved(), [Some(33), Some(50), None]);
        // That's Burano 120 Hvit, already counted, so it offers a recount.
        // Escape leaves it, with the search cleared for the next scan.
        assert!(counter.is_focused("recount"));
        counter.press("escape");
        assert!(counter.find("recount").is_none());
        assert_eq!(counter.value("search").as_deref(), Some(""));
        assert_eq!(saved(), [Some(33), Some(50), None]);

        // An unlisted product isn't recorded.
        counter.input("999");
        counter.press("enter");
        assert!(counter.find("count").is_none());
        assert_eq!(saved(), [Some(33), Some(50), None]);

        // Tab cycles through the controls, skipping the table, and wraps
        // back to the search field.
        counter.press("escape");
        assert!(counter.is_focused("search"));
        for id in ["import", "export", "search"] {
            counter.press("tab");
            assert!(counter.is_focused(id), "Tab should move focus to {id}");
        }
        for id in ["export", "import", "search"] {
            counter.press("shift-tab");
            assert!(
                counter.is_focused(id),
                "Shift-Tab should move focus to {id}"
            );
        }

        std::fs::remove_dir_all(dir).ok();
    }
}
