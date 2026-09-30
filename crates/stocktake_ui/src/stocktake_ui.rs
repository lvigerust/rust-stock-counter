//! The counting window: importing a stock list, counting it and exporting
//! the result.
//!
//! This crate is one feature, kept together: the view that owns the workflow,
//! the table and dialogs it opens, and the small components that only make
//! sense with stocktake words in them. Its public seam is [`init`] and
//! [`StocktakeView`]; the shell needs nothing else.
//!
//! | Module           | Owns                                                   |
//! | ---------------- | ------------------------------------------------------ |
//! | `stocktake_view` | The workflow: session, focus, commands, the layout     |
//! | `product_table`  | How the stock list renders as rows                     |
//! | `recount_dialog` | Counting an already-counted product again              |
//! | `welcome`        | The screen before any stock list is imported           |
//! | `count_status`   | A product's counted/uncounted marker                   |
//! | `quantity`       | Parsing and entering a quantity                        |

mod count_status;
mod product_table;
mod quantity;
mod recount_dialog;
mod stocktake_view;
mod welcome;

use gpui_kit::{App, KeyBinding, actions};

pub use stocktake_view::StocktakeView;

actions!(
    stocktake,
    [
        /// Replaces the stock list, starting a new stocktake.
        ImportStockList,
        /// Writes the counted stock list to an Excel file.
        ExportStocktake,
        /// Abandons the count being entered without saving it.
        CancelCount,
        /// Moves focus to the search field, ready for a scan.
        FocusSearch,
        FocusNext,
        FocusPrevious
    ]
);

/// Key context of the whole window.
const CONTEXT: &str = "Stocktake";

/// Key context around the counted-quantity input, so Escape can cancel the
/// count after the input itself has ignored it.
const COUNT_CELL_CONTEXT: &str = "CountCell";

/// Registers the feature's key bindings. Call once at startup, after
/// `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        // `secondary` is Cmd on macOS and Ctrl elsewhere.
        KeyBinding::new("secondary-o", ImportStockList, Some(CONTEXT)),
        KeyBinding::new("secondary-e", ExportStocktake, Some(CONTEXT)),
        KeyBinding::new("escape", CancelCount, Some(COUNT_CELL_CONTEXT)),
        KeyBinding::new("secondary-f", FocusSearch, Some(CONTEXT)),
        // The table binds Tab to moving between columns, which traps focus in
        // it. These replace that, and also take precedence over the window's
        // own Tab handling so focus can skip the table.
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some("DataTable")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("DataTable")),
    ]);
}
