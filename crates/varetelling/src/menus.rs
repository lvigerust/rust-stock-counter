//! The native menu bar: every command is reachable from it, with its
//! shortcut shown beside it.

use gpui_kit::{App, Menu, MenuItem};
use stocktake_ui::{APP_NAME, ExportStocktake, FocusSearch, ImportStockList, ToggleSidebar};

use crate::Quit;

/// Builds the menu bar. Call after every key binding is registered: the menu
/// bar reads shortcuts from the keymap when it's built.
pub fn init(cx: &mut App) {
    cx.set_menus([
        Menu::new(APP_NAME).items([MenuItem::action(format!("Avslutt {APP_NAME}"), Quit)]),
        Menu::new("Fil").items([
            MenuItem::action("Importer vareliste…", ImportStockList),
            MenuItem::action("Eksporter telling…", ExportStocktake),
            MenuItem::separator(),
            MenuItem::action("Søk", FocusSearch),
        ]),
        Menu::new("Vis").items([MenuItem::action("Vis/skjul sidepanel", ToggleSidebar)]),
    ]);
}
