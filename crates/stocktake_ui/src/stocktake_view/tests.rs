//! UI integration tests: the real view in a headless window, driven by
//! keys and clicks the way a counter at the scanner drives it.

use std::path::PathBuf;

use gpui_kit::component::Root;
use gpui_kit::component::table::TableDelegate as _;
use gpui_kit::test::{ElementSnapshot, TestWindowExt as _};
use gpui_kit::{AnyWindowHandle, TestAppContext, px, size};
use stocktake::{Product, Stocktake};

use super::*;

/// The first product's barcode and system quantity.
const BURANO: (&str, &str) = ("7043811520629", "49");
/// The second product's barcode and system quantity.
const VENETO: (&str, &str) = ("7043811507668", "3");

fn stocktake() -> Stocktake {
    Stocktake::new(vec![
        Product::new("152062", "Burano 120 Sort", "C4-7", BURANO.0, 49),
        Product::new("150766", "Veneto 90", "D3-1", VENETO.0, 3),
    ])
}

/// A store path of its own for each test, in a directory that starts empty.
fn store_path(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stocktake-ui-{test}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    dir.join("varetelling.json")
}

/// Drives the window like a counter at the keyboard. Each step is its own
/// update, so subscriptions and deferred focus run before the next step.
struct Counter<'a> {
    cx: &'a mut TestAppContext,
    window: AnyWindowHandle,
    view: Entity<StocktakeView>,
}

impl<'a> Counter<'a> {
    /// Opens the window on whatever is saved at `store_path`.
    fn open(cx: &'a mut TestAppContext, store_path: PathBuf) -> Self {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let mut view = None;
        let window = cx.open_window(size(px(1040.), px(720.)), |window, cx| {
            let stocktake_view =
                cx.new(|cx| StocktakeView::with_store_path(store_path, window, cx));
            view = Some(stocktake_view.clone());
            Root::new(stocktake_view, window, cx)
        });
        cx.run_until_parked();
        Self {
            cx,
            window: window.into(),
            view: view.unwrap(),
        }
    }

    /// Opens the window on a stocktake saved before, as when the app is
    /// reopened mid-count.
    fn resume(cx: &'a mut TestAppContext, test: &str) -> Self {
        let path = store_path(test);
        store::save(&path, &stocktake()).unwrap();
        Self::open(cx, path)
    }

    fn press(&mut self, key: &str) {
        self.step(|window, cx| window.press(key, cx));
    }

    /// Types at the focus, the way the scanner does, without Enter.
    fn input(&mut self, text: &str) {
        self.step(|window, cx| window.input(text, cx));
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

    /// The counted quantity of the product with `barcode`, as the session
    /// holds it.
    fn counted(&mut self, barcode: &str) -> Option<i64> {
        self.cx.read(|cx| {
            let session = self.view.read(cx).open.as_ref().unwrap().session.read(cx);
            let stocktake = session.stocktake();
            let id = stocktake.product_with_barcode(barcode).unwrap();
            stocktake.product(id).counted_quantity()
        })
    }
}

#[gpui_kit::test]
fn a_scan_counts_the_product_and_saves_it(cx: &mut TestAppContext) {
    let path = store_path("scan");
    store::save(&path, &stocktake()).unwrap();
    let mut counter = Counter::open(cx, path.clone());

    // Reopening resumes the stocktake, ready for a scan.
    assert!(counter.is_focused("search"));
    counter.input(BURANO.0);
    counter.press("enter");

    // The count dialog opens with an empty quantity, ready for typing.
    assert!(counter.is_focused("count"));
    assert_eq!(counter.value("count").as_deref(), Some(""));
    counter.input("47");
    counter.press("enter");

    assert!(counter.find("count").is_none());
    assert_eq!(counter.counted(BURANO.0), Some(47));
    // Saved at once, and back in an empty search for the next scan.
    let saved = store::load(&path).unwrap().unwrap();
    let id = saved.product_with_barcode(BURANO.0).unwrap();
    assert_eq!(saved.product(id).counted_quantity(), Some(47));
    assert!(counter.is_focused("search"));
    assert_eq!(counter.value("search").as_deref(), Some(""));

    // Counting it again starts empty too, and Enter alone saves nothing.
    counter.input(BURANO.0);
    counter.press("enter");
    assert_eq!(counter.value("count").as_deref(), Some(""));
    counter.press("enter");
    assert!(counter.is_focused("count"));
    assert_eq!(counter.counted(BURANO.0), Some(47));
}

#[gpui_kit::test]
fn counts_at_overflow_locations_add_up(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "overflow");
    /// Counts Burano at `location` (the pick location if `None`), saved
    /// with Enter or the button `save`.
    fn count(counter: &mut Counter, location: Option<&str>, quantity: &str, save: &'static str) {
        counter.input(BURANO.0);
        counter.press("enter");
        assert!(counter.is_focused("count"));
        if let Some(location) = location {
            counter.press("shift-tab");
            assert!(counter.is_focused("location"));
            counter.press("secondary-a");
            counter.input(location);
            counter.press("tab");
        }
        counter.input(quantity);
        // The dialog slides in, so its buttons are reached from the keyboard
        // rather than clicked where they were a frame ago.
        if save == "enter" {
            counter.press("enter");
        } else {
            // Past Avbryt to Erstatt.
            counter.press("tab");
            counter.press("tab");
            assert!(counter.is_focused("replace-count"));
            if save == "add-count" {
                counter.press("tab");
            }
            assert!(counter.is_focused(save));
            counter.press("space");
        }
        assert!(counter.find("count").is_none());
    }

    // The location starts as the pick location, so a plain count lands there.
    counter.input(BURANO.0);
    counter.press("enter");
    assert_eq!(counter.value("location").as_deref(), Some("C4-7"));
    counter.press("escape");
    count(&mut counter, None, "40", "enter");
    assert_eq!(counter.counted(BURANO.0), Some(40));

    // Units found at an overflow location add to the total.
    count(&mut counter, Some("d2-1"), "6", "enter");
    assert_eq!(counter.counted(BURANO.0), Some(46));

    // At a location already counted, Enter adds to that location's count…
    count(&mut counter, Some("D2-1"), "2", "enter");
    assert_eq!(counter.counted(BURANO.0), Some(48));
    // …and Erstatt replaces it: zero removes the overflow location.
    count(&mut counter, Some("D2-1"), "0", "replace-count");
    assert_eq!(counter.counted(BURANO.0), Some(40));
    count(&mut counter, None, "39", "replace-count");
    assert_eq!(counter.counted(BURANO.0), Some(39));
    count(&mut counter, None, "1", "add-count");
    assert_eq!(counter.counted(BURANO.0), Some(40));
}

#[gpui_kit::test]
fn a_count_can_move_the_pick_location(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "move-pick");
    let location = |counter: &mut Counter| {
        counter.cx.read(|cx| {
            let session = counter
                .view
                .read(cx)
                .open
                .as_ref()
                .unwrap()
                .session
                .read(cx);
            let stocktake = session.stocktake();
            let id = stocktake.product_with_barcode(VENETO.0).unwrap();
            stocktake.product(id).location().to_string()
        })
    };
    counter.input(VENETO.0);
    counter.press("enter");
    counter.press("shift-tab");
    assert!(counter.find("move-pick-location").is_none());

    // Another location offers to replace the pick location; the pick
    // location again, typed loosely, doesn't.
    counter.press("secondary-a");
    counter.input("e5");
    assert!(counter.find("move-pick-location").is_some());
    counter.press("secondary-a");
    counter.input(" d3-1");
    assert!(counter.find("move-pick-location").is_none());

    counter.press("secondary-a");
    counter.input("E5");
    counter.press("tab");
    assert!(counter.is_focused("move-pick-location"));
    counter.press("space");
    counter.press("tab");
    counter.input("3");
    counter.press("enter");
    assert!(counter.find("count").is_none());
    assert_eq!(location(&mut counter), "E5");
    assert_eq!(counter.counted(VENETO.0), Some(3));

    // Now the pick location is counted, so it can't be replaced.
    counter.input(VENETO.0);
    counter.press("enter");
    assert_eq!(counter.value("location").as_deref(), Some("E5"));
    counter.press("shift-tab");
    counter.press("secondary-a");
    counter.input("F1");
    assert!(counter.find("move-pick-location").is_none());
}

#[gpui_kit::test]
fn a_scan_into_the_count_dialog_counts_the_scanned_product_next(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "rescan");
    counter.input(BURANO.0);
    counter.press("enter");
    assert!(counter.is_focused("count"));

    // The counter scans the next product without confirming this one.
    counter.input(VENETO.0);
    counter.press("enter");

    // Nothing is saved for the first, and the second is up for counting.
    assert_eq!(counter.counted(BURANO.0), None);
    assert!(counter.is_focused("count"));
    assert_eq!(counter.value("count").as_deref(), Some(""));
    counter.input(VENETO.1);
    counter.press("enter");
    assert_eq!(counter.counted(VENETO.0), Some(3));
}

#[gpui_kit::test]
fn cancelling_a_count_saves_nothing(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "cancel");
    counter.input(VENETO.0);
    counter.press("enter");
    counter.input("12");
    counter.press("escape");

    assert!(counter.find("count").is_none());
    assert_eq!(counter.counted(VENETO.0), None);
    assert!(counter.is_focused("search"));
    assert_eq!(counter.value("search").as_deref(), Some(""));
}

#[gpui_kit::test]
fn importing_a_stock_list_starts_a_saved_stocktake(cx: &mut TestAppContext) {
    let store = store_path("import");
    let dir = store.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();

    // A stock list shaped like the business system's export, and a workbook that isn't one.
    let stock_list = dir.join("Vareliste.xlsx");
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    let header = [
        "VareNR",
        "ProduktDesc1",
        "PrdEAN",
        "Lokasjon",
        "FysiskPaaLager",
    ];
    for (col, name) in (0u16..).zip(header) {
        sheet.write_string(0, col, name).unwrap();
    }
    sheet.write_string(1, 0, "152062").unwrap();
    sheet.write_string(1, 1, "Burano 120 Sort").unwrap();
    sheet.write_string(1, 2, BURANO.0).unwrap();
    sheet.write_string(1, 3, "C4-7").unwrap();
    sheet.write_number(1, 4, 49.).unwrap();
    workbook.save(&stock_list).unwrap();
    let other = dir.join("Annet.xlsx");
    let mut workbook = rust_xlsxwriter::Workbook::new();
    workbook.add_worksheet().write_string(0, 0, "Navn").unwrap();
    workbook.save(&other).unwrap();

    let mut counter = Counter::open(cx, store.clone());
    let view = counter.view.clone();
    let import = |counter: &mut Counter, path: &PathBuf| {
        let path = path.clone();
        counter.step(|window, cx| {
            view.update(cx, |view, cx| {
                view.import_stock_list(ImportSource::File(path), window, cx)
            })
        });
    };

    // A file without the export's columns is turned away.
    import(&mut counter, &other);
    assert!(counter.find("search").is_none());
    assert_eq!(store::load(&store).unwrap(), None);

    // The export opens for counting, already saved and remembered.
    counter.press("escape");
    import(&mut counter, &stock_list);
    assert!(counter.is_focused("search"));
    assert_eq!(counter.counted(BURANO.0), None);
    assert_eq!(store::load(&store).unwrap().unwrap().len(), 1);
    let recent = recent::load(&recent::path_beside(&store)).unwrap();
    assert_eq!(recent.iter().next(), Some(stock_list.as_path()));
}

#[gpui_kit::test]
fn tab_reaches_the_welcome_buttons(cx: &mut TestAppContext) {
    let path = store_path("welcome");
    let mut recent = RecentStockLists::default();
    recent.add("/Users/lager/Vareliste.xlsx".into());
    recent::save(&recent::path_beside(&path), &recent).unwrap();
    let mut counter = Counter::open(cx, path);

    counter.press("tab");
    assert!(counter.is_focused("import-first"));
    counter.press("tab");
    assert!(counter.is_focused("open-recent:/Users/lager/Vareliste.xlsx"));
}

#[gpui_kit::test]
fn hides_and_shows_the_sidebar(cx: &mut TestAppContext) {
    let mut counter = Counter::open(cx, store_path("sidebar"));

    // Without a stocktake there's no sidebar, and no way to show it.
    assert!(counter.find("hide-sidebar").is_none());
    counter.click("show-sidebar");
    assert!(counter.find("hide-sidebar").is_none());
    counter.press("secondary-b");
    assert!(counter.find("hide-sidebar").is_none());

    // With one, the sidebar comes, and the window keeps focus.
    let view = counter.view.clone();
    counter.step(|window, cx| {
        view.update(cx, |view, cx| {
            let session = cx.new(|_| Session::new(stocktake(), view.store_path.clone()));
            view.open_stocktake(session, window, cx);
            view.focus_handle.focus(window, cx);
        })
    });

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
    counter.press("secondary-b");
    assert!(!shown(&mut counter));
    counter.press("secondary-b");
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

#[gpui_kit::test]
fn tab_skips_lagre_while_it_is_disabled(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "tab-lagre");
    counter.input(BURANO.0);
    counter.press("enter");

    // With no quantity, Lagre is disabled, and a full round of Tab never
    // lands on it: the two fields and Avbryt are the only stops.
    for _ in 0..3 {
        counter.press("tab");
        assert!(!counter.is_focused("save-count"));
    }
    assert!(counter.is_focused("count"));
    // Nor does a round of Shift-Tab.
    for _ in 0..3 {
        counter.press("shift-tab");
        assert!(!counter.is_focused("save-count"));
    }
    assert!(counter.is_focused("count"));

    // A quantity enables it, and Tab reaches it again.
    counter.input("12");
    counter.press("tab");
    counter.press("tab");
    assert!(counter.is_focused("save-count"));
}

#[gpui_kit::test]
fn the_mode_menu_switches_what_the_window_shows(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "mode-menu");
    let mode = |counter: &mut Counter| counter.cx.read(|cx| counter.view.read(cx).mode);
    assert_eq!(mode(&mut counter), Mode::Counting);

    // Opened with a click and picked from the keyboard, the second item
    // shows the differences: its action reaches the view from the menu.
    counter.click("mode-menu");
    counter.press("down");
    counter.press("down");
    counter.press("enter");
    assert_eq!(mode(&mut counter), Mode::Differences);
    assert!(counter.find("search").is_none());

    // The shortcut goes back.
    counter.press("secondary-1");
    assert_eq!(mode(&mut counter), Mode::Counting);
    assert!(counter.find("search").is_some());

    // With the menu open, a shortcut moves its check, and it stays open.
    let checked = |counter: &mut Counter, id| counter.find(id).and_then(|item| item.checked());
    counter.click("mode-menu");
    assert_eq!(checked(&mut counter, "menu-item:Varetelling"), Some(true));
    assert_eq!(checked(&mut counter, "menu-item:Differanse"), Some(false));
    counter.press("secondary-2");
    assert_eq!(mode(&mut counter), Mode::Differences);
    assert_eq!(checked(&mut counter, "menu-item:Varetelling"), Some(false));
    assert_eq!(checked(&mut counter, "menu-item:Differanse"), Some(true));
}

#[gpui_kit::test]
fn the_columns_menu_hides_and_shows_columns(cx: &mut TestAppContext) {
    let mut counter = Counter::resume(cx, "columns");
    let checked = |counter: &mut Counter, id| counter.find(id).and_then(|item| item.checked());
    let columns_count = |counter: &mut Counter| {
        counter.cx.read(|cx| {
            let table = &counter.view.read(cx).open.as_ref().unwrap().table;
            table.read(cx).delegate().columns_count(cx)
        })
    };
    assert!(counter.find("menu-item:Lokasjon").is_none());

    // Every column is listed, and shown.
    counter.click("columns");
    for id in [
        "menu-item:Lokasjon",
        "menu-item:Varenummer",
        "menu-item:Produkt",
        "menu-item:På lager",
        "menu-item:Telt",
        "menu-item:Differanse",
        "menu-item:Status",
    ] {
        assert_eq!(checked(&mut counter, id), Some(true), "{id}");
    }
    assert_eq!(columns_count(&mut counter), 7);

    // Picking one hides it, and the menu shows it unchecked next time.
    counter.click("menu-item:Varenummer");
    assert_eq!(columns_count(&mut counter), 6);
    counter.click("columns");
    assert_eq!(checked(&mut counter, "menu-item:Varenummer"), Some(false));

    // The product name always shows.
    counter.click("menu-item:Produkt");
    assert_eq!(columns_count(&mut counter), 6);

    // Picking it again shows it.
    counter.click("columns");
    counter.click("menu-item:Varenummer");
    assert_eq!(columns_count(&mut counter), 7);
}
