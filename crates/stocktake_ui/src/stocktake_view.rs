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
// Not rendered while the screens are rebuilt; kept to draw from.
#[allow(dead_code)]
mod regions;

use std::path::PathBuf;

use gpui_kit::component::{
    FocusableExt as _, Size, Theme, WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    progress::Progress,
    separator::Separator,
    status_bar::StatusBar,
    table::{DataTable, TableEvent, TableState},
};
use gpui_kit::{
    ExternalPaths, FocusHandle, Focusable, MouseButton, Pixels, Rems, Subscription, px,
};
use stocktake::{Lookup, ProductId, Stocktake, store};
use ui::{SidebarHeading, SidebarItem, WindowBar, prelude::*};

use crate::{
    APP_NAME, CONTEXT, ExportStocktake, FocusNext, FocusPrevious, FocusSearch, ImportStockList,
    ToggleSidebar, count_dialog,
    product_table::{LastCounted, ProductTable, ROW_HEIGHT},
    quantity::{parse_quantity, quantity_input},
};
use files::ImportSource;

/// Wide enough for a product name beside an icon, and a bar that clears the
/// traffic lights.
const SIDEBAR_WIDTH: Pixels = px(256.);

/// Padding around the content of the main pane.
const MAIN_PADDING: Rems = Rems(3.);

/// The search field's height, a size up from a medium input.
const SEARCH_HEIGHT: Rems = Rems(2.5);

/// Space above and below the search field in the main pane's bar.
const SEARCH_PADDING: Rems = Rems(1.5);

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
    /// The window's own focus, so its commands and shortcuts work while no
    /// control inside it has focus.
    focus_handle: FocusHandle,
    store_path: PathBuf,
    session: Option<Session>,
    sessions_started: usize,
    search: Entity<InputState>,
    /// The quantity field of the count dialog.
    count_input: Entity<InputState>,
    /// Explains why Enter in the search didn't select a product, and how many
    /// hints came before it, so a repeated hint arrives again.
    search_hint: Option<(SharedString, usize)>,
    hints_shown: usize,
    /// The product the count dialog is open for.
    counting: Option<ProductId>,
    save_state: SaveState,
    /// Why the saved stocktake couldn't be resumed.
    resume_error: Option<SharedString>,
    sidebar_collapsed: bool,
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
        let count_input = cx.new(|cx| quantity_input(window, cx).placeholder("Antall"));

        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                InputEvent::Change => this.on_search_changed(cx),
                InputEvent::PressEnter { .. } => this.find_product(window, cx),
                _ => {}
            }),
            // The dialog enables Lagre from the field. Enter reaches the
            // dialog as its confirm action, so it isn't handled here.
            cx.subscribe_in(&count_input, window, |_, _, event, window, _| {
                if let InputEvent::Change = event {
                    window.refresh();
                }
            }),
            // The product column takes the width the others leave.
            cx.observe_window_bounds(window, |this, window, cx| this.fit_columns(window, cx)),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];
        Theme::sync_system_appearance(Some(window), cx);

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let mut this = Self {
            focus_handle,
            store_path,
            session: None,
            sessions_started: 0,
            search,
            count_input,
            search_hint: None,
            hints_shown: 0,
            counting: None,
            save_state: SaveState::Saved,
            resume_error: None,
            sidebar_collapsed: false,
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
        let delegate = ProductTable::new(stocktake.clone(), cx);
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .col_selectable(false)
                .col_movable(false)
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
        self.fit_columns(window, cx);
        self.resume_error = None;
        self.search_hint = None;
        self.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        cx.notify();
    }

    /// Gives the product column whatever width the other columns leave, so
    /// a full-screen window shows long names instead of empty space.
    fn fit_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        // The table spans the main pane.
        let sidebar = if self.sidebar_collapsed {
            px(0.)
        } else {
            SIDEBAR_WIDTH
        };
        let width = window.viewport_size().width - sidebar;
        session.table.update(cx, |table, cx| {
            table.delegate_mut().set_width(width);
            table.refresh(cx);
            cx.notify();
        });
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
            table.delegate_mut().set_rows(rows, cx);
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

    /// Opens the count dialog for the product, with its quantity field
    /// filled in and selected: Enter keeps what's there, typing replaces it.
    /// That's the earlier count if there is one, or else the system quantity,
    /// so confirming it takes a single Enter.
    fn begin_count(&mut self, id: ProductId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        if self.counting.is_some() {
            return;
        }
        let product = session.stocktake.read(cx).product(id).clone();
        let prefill = product
            .counted_quantity()
            .unwrap_or(product.system_quantity());
        self.count_input.update(cx, |input, cx| {
            input.set_value(prefill.to_string(), window, cx)
        });
        self.counting = Some(id);
        count_dialog::open(
            cx.entity().downgrade(),
            self.count_input.clone(),
            &product,
            window,
            cx,
        );
        // After the dialog has taken focus for itself.
        let input = self.count_input.clone();
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        });
    }

    /// Enter or Lagre in the count dialog: saves the quantity, replacing any
    /// earlier count.
    pub(crate) fn save_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.counting else {
            return;
        };
        let text = self.count_input.read(cx).value();
        if self.take_scan_from_quantity(&text, window, cx) {
            return;
        }
        let Some(quantity) = parse_quantity(&text) else {
            return;
        };
        self.counting = None;
        window.close_dialog(cx);
        self.record_count(id, quantity, window, cx);
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
        if self.counting.take().is_some() {
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

    /// Back to the search, with its text selected so typing replaces it.
    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| {
            search.focus(window, cx);
            search.select_all(window, cx);
        });
    }

    /// The count dialog was dismissed: nothing is saved, and the search is
    /// cleared so the next scan doesn't append to the last.
    pub(crate) fn dismiss_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.counting.take().is_some() {
            self.finish_count(window, cx);
        }
    }

    /// Saves a product's counted quantity, then shows where it landed and
    /// gets ready for the next scan.
    fn record_count(
        &mut self,
        id: ProductId,
        quantity: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = &self.session else {
            return;
        };
        session.stocktake.update(cx, |stocktake, cx| {
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
        let last_counted = LastCounted {
            id,
            at: cx.background_executor().now(),
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

impl StocktakeView {
    /// The button that was pressed goes away with its bar and its twin
    /// appears in the other one. Focus on it would go with it, leaving
    /// nothing to take Tab or the shortcuts, so it returns to the window
    /// instead; the next Tab reaches the twin, the first stop in either
    /// state. Focus in the search or the table stays where it is.
    fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.fit_columns(window, cx);
        let stays = self.search.focus_handle(cx).contains_focused(window, cx)
            || self
                .session
                .as_ref()
                .is_some_and(|session| session.table.focus_handle(cx).contains_focused(window, cx));
        if !stays {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// The pane along the leading edge, full height, in four sections: the
    /// top bar with the traffic lights and the button that hides the
    /// sidebar, a header, a body that takes the remaining height, and a
    /// footer. Each is padded by 1rem, and holds its content as
    /// [`SidebarItem`]s. A rule sets the top bar apart.
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = Self::render_sidebar_toggle(true, cx);
        let theme = cx.theme();
        v_flex()
            .id("sidebar")
            .flex_none()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(WindowBar::new().justify_end().child(toggle))
            .child(Separator::horizontal().color(theme.sidebar_border))
            .child(Self::render_sidebar_header())
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .child(SidebarHeading::new("Tellinger")),
            )
            .child(v_flex().flex_none().p_4())
    }

    /// The app's name.
    fn render_sidebar_header() -> impl IntoElement {
        v_flex().flex_none().p_4().child(
            SidebarItem::new().child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_semibold()
                    .child(APP_NAME),
            ),
        )
    }

    /// The pane beside the sidebar: its bar, holding the search, then the
    /// shell the screens' content goes in, then the status bar. For now the
    /// shell holds the stock list, once one is imported.
    fn render_main(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.as_ref();
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                // With the sidebar hidden, the traffic lights move into this
                // bar, followed by the way back. With it shown, the search
                // is inset by the pane's padding, in line with the table's
                // content below.
                WindowBar::new()
                    .traffic_lights(self.sidebar_collapsed)
                    // With the sidebar shown, taller than its bar: the search
                    // gets [`SEARCH_PADDING`] above and below. Hidden, the
                    // bar keeps its height, so the search stays level with
                    // the traffic lights.
                    .when(!self.sidebar_collapsed, |this| {
                        this.h(SEARCH_HEIGHT + SEARCH_PADDING * 2.)
                    })
                    .gap_4()
                    .when(!self.sidebar_collapsed, |this| this.px(MAIN_PADDING))
                    .when(self.sidebar_collapsed, |this| {
                        this.child(Self::render_sidebar_toggle(false, cx))
                    })
                    .when(session.is_some(), |this| {
                        // A press here is the field's, not the start of a
                        // window drag.
                        this.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(self.render_search(cx)),
                        )
                    }),
            )
            .child(
                // The table runs to the pane's edges, so it shows as many
                // rows as fit and its scrollbar sits against the window. Its
                // outer columns inset their content by the pane's padding.
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .when_some(session, |this, session| {
                        // The rows set their own height; the text stays small.
                        this.child(
                            div().flex_1().min_h_0().text_sm().child(
                                DataTable::new(&session.table)
                                    .with_size(Size::Size(ROW_HEIGHT))
                                    // No frame: it would end at the window's
                                    // edge. The rows' own lines separate them.
                                    .bordered(false),
                            ),
                        )
                    }),
            )
            .child(self.render_status(cx))
    }

    /// Along the bottom of the main pane, whatever it shows: how much of the
    /// stock list is counted at the leading edge, and whether every count is
    /// on disk at the trailing edge. With no stock list there is nothing to
    /// show, so it's empty.
    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let progress = self.session.as_ref().map(|session| {
            let stocktake = session.stocktake.read(cx);
            let (counted, total) = (stocktake.counted_len(), stocktake.len());
            // Rounded down, so 100 % means every product is counted.
            let percent = counted * 100 / total.max(1);
            h_flex()
                .gap_2p5()
                .child(
                    div().w_48().child(
                        Progress::new("progress")
                            .xsmall()
                            .color(cx.theme().muted_foreground)
                            .value(percent as f32)
                            .accessibility_label(format!("{counted} av {total} varer telt")),
                    ),
                )
                .child(div().tabular_nums().child(format!("{percent} %")))
        });
        let theme = cx.theme();
        let (icon, color, label) = match self.save_state {
            SaveState::Saved => (IconName::Check, theme.muted_foreground, "Lagret"),
            SaveState::Failed => (IconName::TriangleAlert, theme.danger, "Ikke lagret"),
        };
        StatusBar::new()
            .flex_none()
            // The same height empty as with the save state in it.
            .h_8()
            .pl(MAIN_PADDING)
            .pr(Rems(1.5))
            .when_some(progress, |this, progress| this.left(progress))
            .when(self.session.is_some(), |this| {
                this.right(
                    h_flex()
                        .gap_2()
                        .text_color(color)
                        .child(Icon::new(icon).xsmall())
                        .child(label),
                )
            })
    }

    /// Where every scan lands, and where products are looked up by number or
    /// name. A filled field, quieter than the table below it.
    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        Input::new(&self.search)
            .id("search")
            .prefix(
                Icon::new(IconName::ScanBarcode)
                    .small()
                    .text_color(theme.muted_foreground),
            )
            .cleanable(true)
            // `Input::h` sizes multi-line inputs only; this sets the field.
            .map(|input| Styled::h(input, SEARCH_HEIGHT))
            .bg(theme.muted)
            .border_color(theme.border)
            // Focus doesn't recolor the border: the caret is enough, and
            // the field is focused nearly all the time anyway.
            .focus_bordered(false)
    }

    /// Hides the sidebar from its own bar, or shows it again from the main
    /// pane's bar once it's hidden.
    fn render_sidebar_toggle(expanded: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (id, tooltip) = if expanded {
            ("hide-sidebar", "Skjul sidepanel")
        } else {
            ("show-sidebar", "Vis sidepanel")
        };
        // A press here is the button's, not the start of a window drag.
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(id)
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeft)
                    // Quieter than the content at rest; hover brings it up.
                    .text_color(cx.theme().muted_foreground)
                    // gpui-kit would recolor a border for focus; this draws
                    // a faint ring around the button instead.
                    .focus_ring(false)
                    .subtle_focus_ring(cx)
                    .tooltip_with_action(tooltip, &ToggleSidebar, Some(CONTEXT))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_sidebar(window, cx))),
            )
    }
}

impl Focusable for StocktakeView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StocktakeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ImportStockList, window, cx| {
                this.import_stock_list(ImportSource::Choose, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExportStocktake, window, cx| {
                this.export_stocktake(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusNext, window, cx| this.move_focus(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleSidebar, window, cx| this.toggle_sidebar(window, cx)),
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
            // Stripped down while the screens are rebuilt piece by piece.
            // The regions and the welcome screen are kept, unrendered, to
            // draw from.
            .child(
                h_flex()
                    .size_full()
                    .when(!self.sidebar_collapsed, |this| {
                        this.child(self.render_sidebar(cx))
                    })
                    .child(self.render_main(cx)),
            )
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

        fn click(&mut self, id: &'static str) {
            self.find(id);
            self.step(|window, cx| window.click(id, cx));
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
    #[ignore = "tabs to the import and export buttons, which aren't rendered while the UI is rebuilt"]
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

        // Scanning opens the count dialog, pre-filled with the system quantity.
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

        // Scanning a counted product again pre-fills its count, and the new
        // quantity replaces it.
        counter.input("7043811520629");
        counter.press("enter");
        assert_eq!(counter.value("count").as_deref(), Some("47"));
        counter.input("50");
        counter.press("enter");
        assert!(counter.find("count").is_none());
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
        // That's Burano 120 Hvit. Escape leaves it, with the search cleared
        // for the next scan.
        assert_eq!(counter.value("count").as_deref(), Some("33"));
        counter.press("escape");
        assert!(counter.find("count").is_none());
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

    #[gpui_kit::test]
    fn hides_and_shows_the_sidebar(cx: &mut TestAppContext) {
        let store_path = std::env::temp_dir()
            .join(format!("stocktake-ui-sidebar-{}", std::process::id()))
            .join("varetelling.json");
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let window = cx.open_window(size(px(1040.), px(720.)), |window, cx| {
            let view = cx.new(|cx| StocktakeView::with_store_path(store_path, window, cx));
            Root::new(view, window, cx)
        });
        let mut counter = Counter {
            cx,
            window: window.into(),
        };

        // Each state offers only the way to the other: hiding from the
        // sidebar's bar, showing from the main pane's.
        let shown = |counter: &mut Counter| {
            let hide = counter.find("hide-sidebar").is_some();
            let show = counter.find("show-sidebar").is_some();
            assert_ne!(hide, show);
            hide
        };

        assert!(shown(&mut counter));
        counter.click("hide-sidebar");
        assert!(!shown(&mut counter));
        counter.click("show-sidebar");
        assert!(shown(&mut counter));

        // The shortcut does the same.
        counter.press("cmd-b");
        assert!(!shown(&mut counter));
        counter.press("cmd-b");
        assert!(shown(&mut counter));

        // From the keyboard, focus survives its button going away: the next
        // Tab reaches the twin in the other bar.
        counter.press("tab");
        assert!(counter.is_focused("hide-sidebar"));
        counter.press("space");
        assert!(!shown(&mut counter));
        counter.press("tab");
        assert!(counter.is_focused("show-sidebar"));
        counter.press("space");
        assert!(shown(&mut counter));
        counter.press("tab");
        assert!(counter.is_focused("hide-sidebar"));
    }
}
