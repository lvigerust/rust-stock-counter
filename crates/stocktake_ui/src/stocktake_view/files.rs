//! Getting stock lists in and results out: importing (chosen, dropped or
//! opened again from the recent ones), and exporting to Excel.
//!
//! Files are read and written on the background executor, so a large or
//! slow file never stalls the window; the results come back to the view.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use gpui_kit::component::{WindowExt as _, button::ButtonVariant, notification::Notification};
use gpui_kit::{ExternalPaths, PathPromptOptions};
use stocktake::{
    Product, Stocktake, export, recent,
    stock_list::{self, ImportError},
};
use ui::{StyledDialog as _, prelude::*};

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
        let products = cx.background_spawn({
            let path = path.clone();
            async move { stock_list::read(&path) }
        });
        self.import_task = Some(cx.spawn_in(window, async move |this, cx| {
            let products = products.await;
            this.update_in(cx, |this, window, cx| {
                this.finish_import(path, products, window, cx)
            })
            .ok();
        }));
    }

    /// Starts a new stocktake from what was read, saved at once, and
    /// remembers the file among the recent ones; or says why it couldn't.
    fn finish_import(
        &mut self,
        path: PathBuf,
        products: Result<Vec<Product>, ImportError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match products {
            Ok(products) => {
                let stocktake = Stocktake::new(products);
                let session = cx.new(|_| Session::new(stocktake, self.store_path.clone()));
                self.open_stocktake(session.clone(), window, cx);
                // After opening, so a failed save is reported like any other.
                session.update(cx, |session, cx| session.save(cx));
                self.unavailable.remove(&path);
                self.recent.add(path);
                self.save_recent();
            }
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
