//! UI integration tests: the real view in a headless window, driven by
//! keys and clicks the way a counter at the scanner drives it.

use std::path::PathBuf;

use gpui_kit::component::Root;
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

    // The count dialog offers the system quantity, selected, so Enter
    // confirms it and typing replaces it.
    assert!(counter.is_focused("count"));
    assert_eq!(counter.value("count").as_deref(), Some(BURANO.1));
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

    // Enter alone confirms what the dialog offers, here the earlier count.
    counter.input(BURANO.0);
    counter.press("enter");
    assert_eq!(counter.value("count").as_deref(), Some("47"));
    counter.press("enter");
    assert_eq!(counter.counted(BURANO.0), Some(47));
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
    assert_eq!(counter.value("count").as_deref(), Some(VENETO.1));
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
