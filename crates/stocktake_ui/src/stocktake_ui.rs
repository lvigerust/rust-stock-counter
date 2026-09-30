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
//! | `count_dialog`   | Counting one product                                   |
//! | `welcome`        | The screen before any stock list is imported           |
//! | `count_status`   | A product's counted/uncounted marker                   |
//! | `quantity`       | Parsing and entering a quantity                        |

mod count_dialog;
mod count_status;
mod product_table;
mod quantity;
mod stocktake_view;
// Not rendered while the screens are rebuilt; kept to draw from.
#[allow(dead_code)]
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
        /// Moves focus to the search field, ready for a scan.
        FocusSearch,
        FocusNext,
        FocusPrevious,
        /// Hides the sidebar, or shows it again.
        ToggleSidebar
    ]
);

/// The application's name, shown atop the sidebar and in the menu bar. It
/// must match `CFBundleName` in the shell's `Info.plist`.
pub const APP_NAME: &str = "Scala Bad";

/// Key context of the whole window.
const CONTEXT: &str = "Stocktake";

/// Registers the feature's key bindings. Call once at startup, after
/// `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        // `secondary` is Cmd on macOS and Ctrl elsewhere.
        KeyBinding::new("secondary-o", ImportStockList, Some(CONTEXT)),
        KeyBinding::new("secondary-e", ExportStocktake, Some(CONTEXT)),
        KeyBinding::new("secondary-f", FocusSearch, Some(CONTEXT)),
        // The standard macOS shortcut for showing and hiding a sidebar.
        KeyBinding::new("ctrl-cmd-s", ToggleSidebar, Some(CONTEXT)),
        // The table binds Tab to moving between columns, which traps focus in
        // it. These replace that, and also take precedence over the window's
        // own Tab handling so focus can skip the table.
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some("DataTable")),
        KeyBinding::new("shift-tab", FocusPrevious, Some("DataTable")),
    ]);
}
