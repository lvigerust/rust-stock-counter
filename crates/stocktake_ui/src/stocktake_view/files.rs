//! Getting stock lists in and results out: importing (chosen, dropped or
//! opened again from the recent ones), and exporting to Excel.
//!
//! Files are read and written on the background executor, so a large or
//! slow file never stalls the window; the results come back to the view.

use std::{
    cell::RefCell,
    collections::HashSet,
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui_kit::component::{
    WindowExt as _,
    button::{ButtonVariant, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    notification::Notification,
    radio::{Radio, RadioGroup},
};
use gpui_kit::{ClickEvent, ExternalPaths, PathPromptOptions};
use stocktake::{
    Product, Stocktake, export, recent,
    stock_list::{self, ImportError, StockList},
};
use ui::{Button, FocusRing, Label, Spacing, StyledDialog as _, Text, prelude::*};

use super::StocktakeView;
use crate::{path_display::file_name, session::Session};

/// Where a new stock list comes from.
#[derive(Clone)]
pub(super) enum ImportSource {
    /// Ask with the system's file dialog.
    Choose,
    /// A file dropped on the window, or opened again from the recent ones.
    File(PathBuf),
}

impl StocktakeView {
    /// Starts a new stocktake from a stock list, first asking before
    /// discarding one that has counts in it.
    pub(super) fn import_stock_list(
        &mut self,
        source: ImportSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let in_progress = self
            .open
            .as_ref()
            .map(|open| open.session.read(cx).stocktake())
            .filter(|stocktake| stocktake.counted_len() > 0)
            .map(|stocktake| (stocktake.counted_len(), stocktake.len()));
        let Some((counted, total)) = in_progress else {
            self.read_stock_list(source, window, cx);
            return;
        };

        let description: SharedString = format!(
            "{counted} av {total} varer er talt. En ny vareliste starter en ny varetelling, og tellingen som pågår forkastes."
        )
        .into();
        // Choosing a file needs more input; a given file doesn't.
        let ok_text = match source {
            ImportSource::Choose => "Forkast og importer…",
            ImportSource::File(_) => "Forkast og importer",
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            let source = source.clone();
            dialog
                .alert_frame(cx)
                .confirm()
                .title("Forkaste varetellingen som pågår?")
                .description(description.clone())
                .ok_text(ok_text)
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("Avbryt")
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.read_stock_list(source.clone(), window, cx)
                    })
                    .ok();
                    true
                })
        });
    }

    fn read_stock_list(
        &mut self,
        source: ImportSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = match source {
            ImportSource::File(path) => {
                self.read_file(path, window, cx);
                return;
            }
            ImportSource::Choose => cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Importer".into()),
            }),
        };
        cx.spawn_in(window, async move |this, cx| {
            // Cancelled, or the dialog couldn't open: nothing to import.
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| this.read_file(path, window, cx))
                .ok();
        })
        .detach();
    }

    /// Reads the stock list off the UI thread, then starts the stocktake.
    /// Replaces any read still under way, so the last file chosen wins.
    fn read_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let stock_list = cx.background_spawn({
            let path = path.clone();
            async move { stock_list::read(&path) }
        });
        self.import_task = Some(cx.spawn_in(window, async move |this, cx| {
            let stock_list = stock_list.await;
            this.update_in(cx, |this, window, cx| {
                this.finish_import(path, stock_list, window, cx)
            })
            .ok();
        }));
    }

    /// Starts a new stocktake from what was read, once any product the
    /// stock list lists at several locations has a pick location; or says
    /// why it couldn't be read.
    fn finish_import(
        &mut self,
        path: PathBuf,
        stock_list: Result<StockList, ImportError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match stock_list {
            Ok(stock_list) if stock_list.duplicates().is_empty() => {
                self.start_stocktake(path, stock_list.into_products(), window, cx);
            }
            Ok(stock_list) => self.resolve_duplicates(path, stock_list, window, cx),
            Err(error) => {
                if matches!(error, ImportError::NotFound) {
                    self.unavailable.insert(path);
                    cx.notify();
                }
                let description: SharedString = import_error_message(&error).into();
                window.open_alert_dialog(cx, move |dialog, _, cx| {
                    dialog
                        .alert_frame(cx)
                        .title("Varelisten kunne ikke importeres")
                        .description(description.clone())
                        .ok_text("OK")
                });
            }
        }
    }

    /// Starts a new stocktake over `products`, saved at once, and remembers
    /// the file among the recent ones.
    fn start_stocktake(
        &mut self,
        path: PathBuf,
        products: Vec<Product>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stocktake = Stocktake::new(products);
        let session = cx.new(|_| Session::new(stocktake, self.store_path.clone()));
        self.open_stocktake(session.clone(), window, cx);
        // After opening, so a failed save is reported like any other.
        session.update(cx, |session, cx| session.save(cx));
        self.unavailable.remove(&path);
        self.recent.add(path);
        self.save_recent();
    }

    /// Asks which of its locations is the pick location for every product
    /// the stock list lists at several, then starts the stocktake. The app
    /// suggests nothing: the counters know the storage. Cancelling imports
    /// nothing.
    fn resolve_duplicates(
        &mut self,
        path: PathBuf,
        stock_list: StockList,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let duplicates = stock_list.duplicates().len();
        let title: SharedString = match duplicates {
            1 => "1 vare står flere ganger i varelisten".into(),
            n => format!("{n} varer står flere ganger i varelisten").into(),
        };
        // Taken out when the import goes ahead, so the dialog can't import
        // twice.
        let stock_list = Rc::new(RefCell::new(Some(stock_list)));
        // Which line is picked for each duplicate, in the stock list's order.
        let picks: Rc<RefCell<Vec<Option<usize>>>> = Rc::new(RefCell::new(vec![None; duplicates]));
        let view = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, window, cx| {
            let list = stock_list.borrow();
            let Some(list) = list.as_ref() else {
                return dialog;
            };
            let picked = picks.borrow().clone();
            let groups = list.duplicates().iter().enumerate().map(|(ix, duplicate)| {
                let picks = picks.clone();
                render_duplicate(
                    list.product(duplicate),
                    duplicate,
                    picked[ix],
                    move |line| {
                        picks.borrow_mut()[ix] = Some(line);
                    },
                )
            });
            let import = {
                let view = view.clone();
                let stock_list = stock_list.clone();
                let picks = picks.clone();
                let path = path.clone();
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    let Some(mut list) = stock_list.borrow_mut().take() else {
                        return;
                    };
                    for (duplicate, line) in picks.borrow().iter().enumerate() {
                        if let Some(line) = line {
                            list.pick_location(duplicate, *line);
                        }
                    }
                    window.close_dialog(cx);
                    let path = path.clone();
                    view.update(cx, |this, cx| {
                        this.start_stocktake(path, list.into_products(), window, cx)
                    })
                    .ok();
                }
            };

            dialog
                .title(title.clone())
                .close_button(false)
                .child(
                    v_flex()
                        .mt_6()
                        .gap_6()
                        .child(Text::new(
                            "Lagersystemet har disse varene på flere lokasjoner. Velg hvilken som \
                             er plukklokasjonen for hver; antallene legges sammen. Valget kan \
                             gjøres om fra tellingen av varen.",
                        ))
                        .children(groups),
                )
                .footer(
                    DialogFooter::new()
                        .gap_3()
                        .child(
                            div()
                                .flex_none()
                                .child(DialogClose::new().trigger(|button| {
                                    Button::from(button)
                                        .ghost()
                                        .label("Avbryt")
                                        .with_focus_ring(FocusRing::Solid)
                                })),
                        )
                        .child(
                            Button::new("import-with-duplicates")
                                .primary()
                                .label("Importer")
                                .disabled(picked.iter().any(Option::is_none))
                                .on_click(import),
                        ),
                )
                .dialog_frame(cx)
                // Wide enough for the explanation to wrap in two lines and
                // for long product names to fit on one.
                .w(Spacing(168.).to_pixels(window.rem_size()))
        });
    }

    pub(super) fn on_drop_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stock_list = paths
            .paths()
            .iter()
            .find(|path| stock_list::is_supported(path));
        match stock_list {
            Some(path) => self.import_stock_list(ImportSource::File(path.clone()), window, cx),
            None => window.push_notification(
                Notification::warning("Slipp varelisten her. Den må være en Excel-fil (.xlsx)."),
                cx,
            ),
        }
    }

    /// Imports a stock list from the recent ones. One that has gone missing
    /// since it was last looked for is marked unavailable instead, without
    /// first asking to discard the stocktake in progress for it.
    pub(super) fn open_recent(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path.is_file() {
            self.import_stock_list(ImportSource::File(path), window, cx);
            return;
        }
        let title: SharedString = format!("Fant ikke {}", file_name(&path)).into();
        self.unavailable.insert(path);
        cx.notify();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            dialog
                .alert_frame(cx)
                .title(title.clone())
                .description("Filen er flyttet, slettet eller på en disk som ikke er koblet til.")
                .ok_text("OK")
        });
    }

    /// Looks for the recent stock lists off the UI thread, since a drive
    /// that isn't connected can take seconds to answer, and marks the ones
    /// not found as unavailable.
    pub(super) fn check_recent(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self.recent.iter().map(Path::to_path_buf).collect();
        let unavailable = cx.background_spawn(async move {
            paths
                .into_iter()
                .filter(|path| !path.is_file())
                .collect::<HashSet<_>>()
        });
        self.recent_check = Some(cx.spawn(async move |this, cx| {
            let unavailable = unavailable.await;
            this.update(cx, |this, cx| {
                this.unavailable = unavailable;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Losing the recent stock lists costs a trip to the file dialog, not
    /// any counts, so a failed save isn't worth interrupting the counter.
    fn save_recent(&self) {
        recent::save(&self.recent_path, &self.recent).ok();
    }

    /// Writes the counted stock list to a file the counter picks, first
    /// warning if anything is uncounted.
    pub(super) fn export_stocktake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let uncounted = open.session.read(cx).stocktake().uncounted_len();
        if uncounted == 0 {
            self.choose_export_path(window, cx);
            return;
        }

        let title: SharedString = match uncounted {
            1 => "1 vare er ikke talt".into(),
            n => format!("{n} varer er ikke talt").into(),
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog
                .alert_frame(cx)
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

    /// Asks where to save, then writes a snapshot of the stocktake as it is
    /// now; counts made while the dialog is open aren't in the file.
    fn choose_export_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        let stocktake = open.session.read(cx).stocktake().clone();
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
                    let description: SharedString =
                        format!("Filen kunne ikke skrives: {error}").into();
                    window.open_alert_dialog(cx, move |dialog, _, cx| {
                        dialog
                            .alert_frame(cx)
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
}

/// One product the stock list lists at several locations: its name, with
/// what tells it apart after it, then a radio per location with what the
/// business system says is there. `on_pick` gets the index of the line
/// picked.
fn render_duplicate(
    product: &Product,
    duplicate: &stock_list::Duplicate,
    picked: Option<usize>,
    on_pick: impl Fn(usize) + 'static,
) -> impl IntoElement {
    let item_number = product.item_number();
    let detail = if product.description().is_empty() {
        item_number.to_string()
    } else {
        format!("{item_number} · {}", product.description())
    };
    let lines = duplicate.lines().iter().map(|line| {
        let location = match line.location() {
            "" => "Uten lokasjon".to_string(),
            location => location.to_string(),
        };
        Radio::new(format!("duplicate:{item_number}:{}", line.location()))
            .label(format!("{location} · {} stk", line.system_quantity()))
    });
    // The name with its detail after it on one line, the locations side by
    // side below, so a duplicate stays short.
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .items_baseline()
                .gap_3()
                .child(Label::new(product.name().to_string()))
                .child(Text::new(detail)),
        )
        .child(
            RadioGroup::horizontal(format!("duplicate:{item_number}"))
                .selected_index(picked)
                .children(lines)
                .on_click(move |line, window, _| {
                    on_pick(*line);
                    window.refresh();
                }),
        )
}

/// Why the import failed, in the counter's words.
fn import_error_message(error: &ImportError) -> String {
    match error {
        ImportError::NotFound => {
            "Filen finnes ikke lenger. Den kan være flyttet eller slettet.".into()
        }
        ImportError::UnsupportedFormat => {
            "Filen er ikke en Excel-fil. Velg varelisten fra lagersystemet.".into()
        }
        ImportError::Unreadable(reason) => format!("Filen kunne ikke leses: {reason}"),
        ImportError::MissingColumns(columns) => format!(
            "Filen ser ikke ut som en vareliste. Den mangler kolonnene {}.",
            columns.join(", ")
        ),
        ImportError::InvalidQuantity { row, value } => format!(
            "Rad {row} har «{value}» i {}, som ikke er et helt antall.",
            stock_list::column::SYSTEM_QUANTITY
        ),
        ImportError::Empty => "Varelisten inneholder ingen varer.".into(),
    }
}
