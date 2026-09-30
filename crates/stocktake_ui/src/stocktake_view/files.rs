//! Getting stock lists in and results out: importing (chosen or
//! dropped), and exporting to Excel.

use std::path::{Path, PathBuf};

use gpui_kit::component::{WindowExt as _, button::ButtonVariant, notification::Notification};
use gpui_kit::{ExternalPaths, PathPromptOptions, WeakEntity};
use stocktake::{Stocktake, export, stock_list};
use ui::prelude::*;

use super::StocktakeView;

/// Where a new stock list comes from.
#[derive(Clone)]
pub(super) enum ImportSource {
    /// Ask with the system's file dialog.
    Choose,
    /// A file dropped on the window.
    Dropped(PathBuf),
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
            .session
            .as_ref()
            .map(|session| session.stocktake.read(cx))
            .filter(|stocktake| stocktake.counted_len() > 0)
            .map(|stocktake| (stocktake.counted_len(), stocktake.len()));
        let Some((counted, total)) = in_progress else {
            self.read_stock_list(source, window, cx);
            return;
        };

        let description: SharedString = format!(
            "{counted} av {total} varer er telt. En ny vareliste starter en ny varetelling, og tellingen som pågår forkastes."
        )
        .into();
        // Choosing a file needs more input; a dropped file doesn't.
        let ok_text = match source {
            ImportSource::Choose => "Forkast og importer…",
            ImportSource::Dropped(_) => "Forkast og importer",
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            let source = source.clone();
            dialog
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
        let path = match source {
            ImportSource::Dropped(path) => {
                Self::spawn_read(cx.entity().downgrade(), path, window, cx);
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
            let Ok(Ok(Some(paths))) = path.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |_, window, cx| {
                Self::spawn_read(cx.entity().downgrade(), path, window, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Reads the stock list off the UI thread, then starts the stocktake.
    fn spawn_read(this: WeakEntity<Self>, path: PathBuf, window: &mut Window, cx: &mut App) {
        let products = cx.background_spawn(async move { stock_list::read(&path) });
        window
            .spawn(cx, async move |cx| {
                let products = products.await;
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

    pub(super) fn on_drop_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let spreadsheet = paths.paths().iter().find(|path| is_spreadsheet(path));
        match spreadsheet {
            Some(path) => self.import_stock_list(ImportSource::Dropped(path.clone()), window, cx),
            None => window.push_notification(
                Notification::warning("Slipp vareliste-eksporten fra MultiCase, en .xlsx-fil."),
                cx,
            ),
        }
    }

    pub(super) fn export_stocktake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
}

/// Whether a dropped file looks like a stock list export.
fn is_spreadsheet(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "xlsx" | "xlsm" | "xls"
            )
        })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
