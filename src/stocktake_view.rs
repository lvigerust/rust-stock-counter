//! The window: importing a stock list, counting it, and exporting the result.

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, Theme, WindowExt as _,
    alert::Alert,
    button::{Button, ButtonVariant, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    progress::Progress,
    table::{DataTable, TableEvent, TableState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    KeyBinding, ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _,
    Subscription, Window, actions, div, prelude::FluentBuilder as _,
};

use crate::{
    export,
    product_table::{COUNT_CELL_CONTEXT, ProductTable},
    stock_list,
    stocktake::{Lookup, ProductId, Stocktake},
    store,
};

actions!(
    stocktake,
    [
        ImportStockList,
        ExportStocktake,
        CancelCount,
        FocusNext,
        FocusPrevious
    ]
);

const CONTEXT: &str = "Stocktake";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", ImportStockList, Some(CONTEXT)),
        KeyBinding::new("cmd-e", ExportStocktake, Some(CONTEXT)),
        KeyBinding::new("escape", CancelCount, Some(COUNT_CELL_CONTEXT)),
        // The table binds Tab to moving between columns, which traps focus in
        // it. These replace that, and also take precedence over the window's
        // own Tab handling so focus can skip the table.
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some("DataTable")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("DataTable")),
    ]);
}

/// How a second count of an already-counted product combines with the first.
#[derive(Clone, Copy)]
enum Recount {
    Replace,
    Add,
}

/// The stocktake in progress and the table showing it.
struct Session {
    stocktake: Entity<Stocktake>,
    table: Entity<TableState<ProductTable>>,
    _table_events: Subscription,
}

pub struct StocktakeView {
    store_path: PathBuf,
    session: Option<Session>,
    search: Entity<InputState>,
    /// The counted-quantity cell of the product being counted.
    count_input: Entity<InputState>,
    /// The quantity field of the dialog for counting a product again.
    recount_input: Entity<InputState>,
    /// Explains why Enter in the search didn't select a product.
    search_hint: Option<SharedString>,
    /// The already-counted product the recount dialog is open for.
    pending_recount: Option<ProductId>,
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
            cx.subscribe_in(&recount_input, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.apply_recount(Recount::Add, window, cx);
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
            search,
            count_input,
            recount_input,
            search_hint: None,
            pending_recount: None,
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
        self.session = Some(Session {
            stocktake,
            table,
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

    fn save(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        if let Err(error) = store::save(&self.store_path, session.stocktake.read(cx)) {
            window.push_notification(
                Notification::error(format!(
                    "Tellingen ble ikke lagret: {error}. Den går tapt hvis appen lukkes."
                ))
                .autohide(false),
                cx,
            );
        }
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
            Lookup::Ambiguous(count) => {
                self.search_hint = Some(
                    format!("{count} varer passer. Skriv mer, eller velg varen i tabellen.").into(),
                );
                cx.notify();
            }
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
                .description("Varen står ikke på varelisten, og blir ikke registrert.")
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
        self.search_hint = None;
        let product = session.stocktake.read(cx).product(id).clone();
        match product.counted_quantity() {
            Some(counted) => self.open_recount_dialog(id, product.name(), counted, window, cx),
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
        let (Some(session), Some(id)) = (&self.session, self.counting(cx)) else {
            return;
        };
        let Some(quantity) = parse_quantity(&self.count_input.read(cx).value()) else {
            return;
        };
        session.stocktake.update(cx, |stocktake, cx| {
            stocktake.set_counted_quantity(id, quantity);
            cx.notify();
        });
        self.save(window, cx);
        self.finish_count(window, cx);
    }

    fn cancel_count(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.counting(cx).is_none() {
            return;
        }
        self.finish_count(window, cx);
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

    fn open_recount_dialog(
        &mut self,
        id: ProductId,
        name: &str,
        counted: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recount_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        let title: SharedString = format!("«{name}» er allerede telt").into();
        let description: SharedString = format!(
            "Telt antall er {counted}. Legg til det du har funnet nå, eller erstatt tellingen."
        )
        .into();
        let input = self.recount_input.clone();
        let view = cx.entity().downgrade();
        self.pending_recount = Some(id);
        window.open_dialog(cx, move |dialog, _, cx| {
            let replace = view.clone();
            let add = view.clone();
            dialog
                .title(title.clone())
                .w(gpui_kit::px(420.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(description.clone()),
                        )
                        .child(Input::new(&input).id("recount")),
                )
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|button| button.label("Avbryt")))
                        .child(Button::new("recount-replace").label("Erstatt").on_click(
                            move |_, window, cx| {
                                replace
                                    .update(cx, |this, cx| {
                                        this.apply_recount(Recount::Replace, window, cx)
                                    })
                                    .ok();
                            },
                        ))
                        .child(
                            Button::new("recount-add")
                                .primary()
                                .label("Legg til")
                                .on_click(move |_, window, cx| {
                                    add.update(cx, |this, cx| {
                                        this.apply_recount(Recount::Add, window, cx)
                                    })
                                    .ok();
                                }),
                        ),
                )
        });
        let input = self.recount_input.clone();
        cx.defer_in(window, move |_, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    fn apply_recount(&mut self, recount: Recount, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(session), Some(id)) = (&self.session, self.pending_recount) else {
            return;
        };
        let Some(quantity) = parse_quantity(&self.recount_input.read(cx).value()) else {
            return;
        };
        session.stocktake.update(cx, |stocktake, cx| {
            let counted = stocktake.product(id).counted_quantity().unwrap_or(0);
            let quantity = match recount {
                Recount::Replace => quantity,
                Recount::Add => counted + quantity,
            };
            stocktake.set_counted_quantity(id, quantity);
            cx.notify();
        });
        self.pending_recount = None;
        self.save(window, cx);
        window.close_dialog(cx);
        self.finish_count(window, cx);
    }

    fn import_stock_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let in_progress = self
            .session
            .as_ref()
            .map(|session| session.stocktake.read(cx))
            .filter(|stocktake| stocktake.counted_len() > 0)
            .map(|stocktake| (stocktake.counted_len(), stocktake.len()));
        let Some((counted, total)) = in_progress else {
            self.choose_stock_list(window, cx);
            return;
        };

        let description: SharedString = format!(
            "{counted} av {total} varer er telt. En ny vareliste starter en ny varetelling, og tellingen som pågår forkastes."
        )
        .into();
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .confirm()
                .title("Forkaste varetellingen som pågår?")
                .description(description.clone())
                .ok_text("Forkast og importer…")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("Avbryt")
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| this.choose_stock_list(window, cx))
                        .ok();
                    true
                })
        });
    }

    fn choose_stock_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importer".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let products = cx
                .background_spawn(async move { stock_list::read(&path) })
                .await;
            this.update_in(cx, |this, window, cx| match products {
                Ok(products) => {
                    this.start_session(Stocktake::new(products), window, cx);
                    this.save(window, cx);
                }
                Err(error) => {
                    let description: SharedString = error.to_string().into();
                    window.open_alert_dialog(cx, move |dialog, _, _| {
                        dialog
                            .title("Varelisten kunne ikke importeres")
                            .description(description.clone())
                            .ok_text("OK")
                    });
                }
            })
            .ok();
        })
        .detach();
    }

    fn export_stocktake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let uncounted = session.stocktake.read(cx).uncounted_len();
        if uncounted == 0 {
            self.choose_export_path(window, cx);
            return;
        }

        let title: SharedString = match uncounted {
            1 => "1 vare er ikke telt".into(),
            n => format!("{n} varer er ikke telt").into(),
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .confirm()
                .title(title.clone())
                .description(format!(
                    "De merkes med «{}» i filen.",
                    export::UNCOUNTED_MARK
                ))
                .ok_text("Eksporter…")
                .cancel_text("Avbryt")
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| this.choose_export_path(window, cx))
                        .ok();
                    true
                })
        });
    }

    fn choose_export_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let stocktake = session.stocktake.read(cx).clone();
        let directory = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let path = cx.prompt_for_new_path(&directory, Some("Varetelling.xlsx"));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            let written = cx
                .background_spawn({
                    let path = path.clone();
                    async move { export::write(&stocktake, &path) }
                })
                .await;
            this.update_in(cx, |_, window, cx| match written {
                Ok(()) => window.push_notification(
                    Notification::success(format!("Eksportert til {}", file_name(&path))),
                    cx,
                ),
                Err(error) => {
                    let description: SharedString = error.to_string().into();
                    window.open_alert_dialog(cx, move |dialog, _, _| {
                        dialog
                            .title("Varetellingen kunne ikke eksporteres")
                            .description(description.clone())
                            .ok_text("OK")
                    });
                }
            })
            .ok();
        })
        .detach();
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

    fn render_toolbar(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let stocktake = session.stocktake.read(cx);
        let (counted, total) = (stocktake.counted_len(), stocktake.len());
        let progress = counted as f32 / total.max(1) as f32 * 100.;

        h_flex()
            .gap_4()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .id("search")
                        .prefix(Icon::new(LucideIcon::ScanBarcode).small())
                        .cleanable(true),
                ),
            )
            .child(
                v_flex()
                    .w_40()
                    .gap_1()
                    .child(div().text_sm().child(format!("{counted} av {total} telt")))
                    .child(
                        Progress::new("progress")
                            .value(progress)
                            .accessibility_label(format!("{counted} av {total} varer telt")),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("import")
                            .outline()
                            .label("Importer…")
                            .tooltip("Importer ny vareliste (⌘O)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.import_stock_list(window, cx)
                            })),
                    )
                    .child(
                        Button::new("export")
                            .label("Eksporter…")
                            .tooltip("Eksporter tellingen til Excel (⌘E)")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.export_stocktake(window, cx)
                                }),
                            ),
                    ),
            )
    }

    fn render_counting(&self, session: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(self.render_toolbar(session, cx))
            .when_some(self.search_hint.clone(), |this, hint| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(DataTable::new(&session.table).small()),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .when_some(self.resume_error.clone(), |this, error| {
                this.child(Alert::error("resume-error", error))
            })
            .child(
                div().flex_1().flex().items_center().justify_center().child(
                    Empty::new()
                        .header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new().child(
                                        Icon::new(LucideIcon::FileSpreadsheet)
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                                )
                                .title(EmptyTitle::new().child("Ingen varetelling pågår"))
                                .description(EmptyDescription::new().child(
                                    "Importer varelisten fra MultiCase for å begynne å telle.",
                                )),
                        )
                        .content(
                            EmptyContent::new().child(
                                Button::new("import-first")
                                    .large()
                                    .icon(LucideIcon::FileSpreadsheet)
                                    .label("Importer vareliste…")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_stock_list(window, cx)
                                    })),
                            ),
                        ),
                ),
            )
    }
}

impl Render for StocktakeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &ImportStockList, window, cx| {
                this.import_stock_list(window, cx)
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
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .map(|this| match &self.session {
                Some(session) => this.child(self.render_counting(session, cx)),
                None => this.child(self.render_empty(cx)),
            })
    }
}

/// A single-line input that accepts whole, non-negative numbers.
fn quantity_input(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    InputState::new(window, cx).validate(|text, _| text.chars().all(|c| c.is_ascii_digit()))
}

fn parse_quantity(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::test::{ElementSnapshot, TestWindowExt as _};
    use gpui_kit::{AnyWindowHandle, TestAppContext, px, size};

    use super::*;
    use crate::stocktake::Product;

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
            init(cx);
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
            assert!(counter.is_focused(id), "Shift-Tab should move focus to {id}");
        }

        std::fs::remove_dir_all(dir).ok();
    }
}
