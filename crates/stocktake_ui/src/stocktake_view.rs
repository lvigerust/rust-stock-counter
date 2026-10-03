//! The window's root view: importing a stock list, counting it, and
//! exporting the result.
//!
//! [`StocktakeView`] owns the workflow (which product is being counted,
//! where focus goes next) and the layout. The counts themselves live in a
//! [`Session`] entity, which saves them.
//!
//! One type, several files, split by concern the way Zed splits its editor.
//! Child modules see the view's private fields, so nothing is made public
//! just to split a file:
//!
//! | File          | Holds                                                     |
//! | ------------- | --------------------------------------------------------- |
//! | this file     | State, lifecycle, focus, actions, the main pane's layout |
//! | `counting.rs` | Scan or search → count dialog → saved count               |
//! | `files.rs`    | Importing stock lists, recent ones, exporting to Excel    |
//! | `mode.rs`     | What the window shows: counting, or the differences       |
//! | `sidebar.rs`  | The sidebar: filters, starting over, hiding it            |

mod counting;
mod files;
mod mode;
mod sidebar;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use gpui_kit::component::{
    Size, WindowExt as _,
    input::{InputEvent, InputGroup, InputGroupAddon, InputGroupInput, InputState},
    notification::Notification,
    progress::Progress,
    status_bar::StatusBar,
    table::{DataTable, TableEvent, TableState},
};
use gpui_kit::{
    Anchor, ClickEvent, DefiniteLength, DragMoveEvent, ExternalPaths, FocusHandle, Focusable,
    Pixels, Subscription, Task,
};
use stocktake::{
    Filter, ProductId,
    recent::{self, RecentStockLists},
    store,
};
use ui::{
    Dropdown, DropdownButton, DropdownItem, DropdownMenu, Spacing, WindowBar, WindowBarItem,
    prelude::*,
};

use crate::{
    CONTEXT, ExportStocktake, FocusNext, FocusPrevious, FocusSearch, ImportStockList, ShowCounting,
    ShowDifferences, ToggleSidebar,
    product_table::{ProductColumn, ProductTable, ROW_HEIGHT},
    session::{SaveState, Session, SessionEvent},
    welcome::Welcome,
};
use files::ImportSource;
use mode::Mode;
use sidebar::DraggedSidebar;

/// Padding around the content of the main pane.
const MAIN_PADDING: Spacing = Spacing(12.);

/// The search field's height, a size up from a medium input.
const SEARCH_HEIGHT: Spacing = Spacing(10.);

/// Space above and below the search field in the main pane's bar.
const SEARCH_PADDING: Spacing = Spacing(6.);

/// Space between the search field and the columns menu beside it, tighter
/// than the bar's own gap, so the two read as one group.
const SEARCH_GAP: Spacing = Spacing(3.);

pub struct StocktakeView {
    /// The window's own focus, so its commands and shortcuts work while no
    /// control inside it has focus.
    focus_handle: FocusHandle,
    /// Where the stocktake in progress is saved.
    store_path: PathBuf,
    /// Where [`Self::recent`] is saved.
    recent_path: PathBuf,
    /// The stock lists imported before, offered on the welcome.
    recent: RecentStockLists,
    /// The recent stock lists not found when last looked for. They stay
    /// remembered, so one on a drive that's connected again comes back.
    unavailable: HashSet<PathBuf>,
    /// The look for the recent stock lists under way, if any. A newer one
    /// replaces it.
    recent_check: Option<Task<()>>,
    /// The stocktake being counted, and everything on screen that goes with
    /// it. `None` shows the welcome.
    open: Option<OpenStocktake>,
    /// Why the saved stocktake couldn't be resumed, shown on the welcome.
    resume_error: Option<SharedString>,
    /// What the window shows of the stocktake, picked atop the sidebar.
    mode: Mode,
    /// Whether the counter hid the sidebar. Without a stocktake it's hidden
    /// anyway, having nothing to act on; see [`Self::sidebar_shown`].
    sidebar_collapsed: bool,
    /// How wide the counter made the sidebar, kept while it's hidden so it
    /// comes back the same. For as long as the window is open.
    sidebar_width: Pixels,
    /// Whether the counter closed the aisle filter in the sidebar, leaving
    /// only its heading.
    aisle_filter_collapsed: bool,
    /// The last stock list read in the background. A newer import replaces
    /// it, which cancels it if it's still reading, so only the last file
    /// chosen is opened.
    import_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// What exists only while a stocktake is open. Dropping it resets all of it
/// at once: the search, the filter and any count in progress start over with
/// the next stocktake.
struct OpenStocktake {
    session: Entity<Session>,
    table: Entity<TableState<ProductTable>>,
    /// Where every scan lands, and where products are looked up by number
    /// or name.
    search: Entity<InputState>,
    /// What the sidebar leaves out of the table, besides the search.
    filter: Filter,
    /// The product being counted, while its count dialog is open.
    count: Option<Count>,
    _subscriptions: Vec<Subscription>,
}

/// A count dialog that is open.
struct Count {
    product: ProductId,
    /// The dialog's location field.
    location: Entity<InputState>,
    /// The dialog's quantity field.
    input: Entity<InputState>,
    _input_events: [Subscription; 2],
}

impl OpenStocktake {
    fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_string()
    }

    /// Shows the products the search and the filter leave.
    fn refresh_rows(&self, cx: &mut App) {
        let rows = self
            .session
            .read(cx)
            .stocktake()
            .search_filtered(&self.query(cx), &self.filter);
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows, cx);
            // Row indices now point at different products.
            table.clear_selection(cx);
        });
    }
}

impl StocktakeView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_store_path(store::default_path(), window, cx)
    }

    /// Resumes the stocktake saved at `store_path`, if there is one. The
    /// recent stock lists are kept beside it.
    fn with_store_path(store_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let recent_path = recent::path_beside(&store_path);
        // Without them the welcome offers only the file dialog, which is
        // no reason to stop the counter.
        let recent = recent::load(&recent_path).unwrap_or_default();
        let subscriptions = vec![
            // The product column takes the width the others leave.
            cx.observe_window_bounds(window, |this, window, cx| this.fit_columns(window, cx)),
            // Coming back to the window may be from plugging in the drive a
            // recent stock list is on.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && this.open.is_none() {
                    this.check_recent(cx);
                }
            }),
        ];
        let mut this = Self {
            focus_handle,
            store_path,
            recent_path,
            recent,
            unavailable: HashSet::new(),
            recent_check: None,
            open: None,
            resume_error: None,
            mode: Mode::default(),
            sidebar_collapsed: false,
            sidebar_width: sidebar::DEFAULT_SIDEBAR_WIDTH,
            aisle_filter_collapsed: false,
            import_task: None,
            _subscriptions: subscriptions,
        };
        match store::load(&this.store_path) {
            Ok(Some(stocktake)) => {
                let session = cx.new(|_| Session::new(stocktake, this.store_path.clone()));
                this.open_stocktake(session, window, cx);
            }
            Ok(None) => this.check_recent(cx),
            Err(error) => {
                this.resume_error =
                    Some(format!("Den lagrede varetellingen kunne ikke åpnes: {error}").into());
                this.check_recent(cx);
            }
        }
        this
    }

    /// Shows `session` for counting, in place of the welcome or the
    /// stocktake before it, with focus in the search ready for a scan.
    fn open_stocktake(
        &mut self,
        session: Entity<Session>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Skann strekkode, eller søk på varenummer eller navn")
        });
        let delegate = ProductTable::new(session.clone(), cx);
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .col_selectable(false)
                .col_movable(false)
        });
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                InputEvent::Change => {
                    if let Some(open) = &this.open {
                        open.refresh_rows(cx);
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => this.find_product(window, cx),
                _ => {}
            }),
            cx.subscribe_in(&table, window, |this, _, event, window, cx| {
                if let TableEvent::SelectRow(row_ix) = event {
                    this.select_row(*row_ix, window, cx);
                }
            }),
            // Counts and the save state show in the table and status bar.
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe_in(&session, window, |_, _, event, window, cx| match event {
                SessionEvent::SaveFailed(reason) => window.push_notification(
                    Notification::error(format!(
                        "Tellingen ble ikke lagret: {reason}. Den går tapt hvis appen lukkes."
                    ))
                    .autohide(false),
                    cx,
                ),
            }),
        ];
        search.update(cx, |search, cx| search.focus(window, cx));
        self.open = Some(OpenStocktake {
            session,
            table,
            search,
            filter: Filter::default(),
            count: None,
            _subscriptions: subscriptions,
        });
        self.resume_error = None;
        self.fit_columns(window, cx);
        cx.notify();
    }

    /// Forgets the stocktake in progress, on screen and on disk, back to how
    /// the app first opens. For testing: nothing asks before the counts go.
    fn discard_stocktake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        if let Err(error) = open.session.read(cx).discard() {
            window.push_notification(
                Notification::error(format!("Tellingen kunne ikke slettes: {error}")),
                cx,
            );
            return;
        }
        // Focus in the search or the sidebar would go with them, leaving
        // nothing to take the shortcuts.
        self.focus_handle.focus(window, cx);
        self.open = None;
        self.check_recent(cx);
        cx.notify();
    }

    /// Gives the product column whatever width the other columns leave, so
    /// a full-screen window shows long names instead of empty space.
    fn fit_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        // The table spans the main pane.
        let width = window.viewport_size().width - self.shown_sidebar_width();
        open.table.update(cx, |table, cx| {
            table.delegate_mut().set_width(width);
            table.refresh(cx);
            cx.notify();
        });
    }

    /// Shows `column` if it's hidden, or hides it, as picked from the columns
    /// menu.
    fn toggle_column(&mut self, column: ProductColumn, cx: &mut Context<Self>) {
        let Some(open) = &self.open else {
            return;
        };
        open.table.update(cx, |table, cx| {
            let shown = table.delegate().is_shown(column);
            table.delegate_mut().set_shown(column, !shown, cx);
            // Column indices now point at different columns, and hiding the
            // sorted one reorders the rows.
            table.clear_selection(cx);
            table.refresh(cx);
            cx.notify();
        });
        cx.notify();
    }

    /// Tab and Shift-Tab, skipping the table. Rows are reached by scanning,
    /// searching or clicking, and the table shows no focus ring, so a Tab
    /// stop on it would look like lost focus.
    fn move_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let table_focus = self.open.as_ref().map(|open| open.table.focus_handle(cx));
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
    /// The pane beside the sidebar: its bar, holding the search, then the
    /// stock list once one is imported, or the welcome before, then the
    /// status bar, which only comes with the stock list.
    fn render_main(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open.as_ref();
        let sidebar_shown = self.sidebar_shown();
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                // With the sidebar hidden, the traffic lights move into this
                // bar, followed by the way back, disabled without a
                // stocktake; its glyph sits inside the button, so the gap
                // after it is shorter by that inset. With the sidebar shown,
                // the search is inset by the pane's padding, in line with
                // the table's content below. The bar always runs to the
                // window's trailing edge, so it holds the Windows caption
                // buttons.
                WindowBar::new()
                    .leading_edge(!sidebar_shown)
                    .h(self.bar_height())
                    .gap(WindowBar::GAP - WindowBar::ICON_INSET)
                    .when(sidebar_shown, |this| this.px(MAIN_PADDING))
                    .when(!sidebar_shown, |this| {
                        this.child(Self::render_sidebar_toggle(false, open.is_none(), cx))
                    })
                    .when_some(
                        open.filter(|_| self.mode == Mode::Counting),
                        |this, open| {
                            this.child(
                                WindowBarItem::new()
                                    .h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(SEARCH_GAP)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(Self::render_search(&open.search, cx)),
                                    )
                                    .child(Self::render_columns_menu(open, cx)),
                            )
                        },
                    ),
            )
            .child(
                // The table runs to the pane's edges, so it shows as many
                // rows as fit and its scrollbar sits against the window. Its
                // outer columns inset their content by the pane's padding.
                v_flex().flex_1().min_h_0().map(|this| match open {
                    Some(_) if self.mode == Mode::Differences => {
                        this.child(mode::render_differences(cx))
                    }
                    // The rows set their own height; the text stays small.
                    Some(open) => this.child(
                        div().flex_1().min_h_0().text_sm().child(
                            DataTable::new(&open.table)
                                .with_size(Size::Size(ROW_HEIGHT))
                                // No frame: it would end at the window's
                                // edge. The rows' own lines separate them.
                                .bordered(false),
                        ),
                    ),
                    // Padded at the bottom as deep as the bar is at the top,
                    // so the welcome centers on the whole pane.
                    None => this.child(
                        div().size_full().pb(self.bar_height()).child(
                            Welcome::new()
                                .resume_error(self.resume_error.clone())
                                .unavailable(self.unavailable.iter().cloned())
                                .recent(
                                    self.recent.iter().map(Path::to_path_buf),
                                    cx.listener(|this, path: &PathBuf, window, cx| {
                                        this.open_recent(path.clone(), window, cx)
                                    }),
                                ),
                        ),
                    ),
                }),
            )
            .children(open.map(|open| Self::render_status(open.session.read(cx), cx)))
    }

    /// The height of the main pane's bar. With the sidebar shown, taller than
    /// the sidebar's bar: the search gets [`SEARCH_PADDING`] above and below.
    /// Hidden, the bar keeps its height, so the search stays level with the
    /// traffic lights.
    fn bar_height(&self) -> DefiniteLength {
        if self.sidebar_shown() {
            (SEARCH_HEIGHT + SEARCH_PADDING * 2.).into()
        } else {
            WindowBar::HEIGHT.into()
        }
    }

    /// A filled field, quieter than the table below it.
    fn render_search(search: &Entity<InputState>, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        // The border sits on a wrapper: the group recolors its own border
        // on focus, after any style set here, so it draws none.
        div()
            .h(SEARCH_HEIGHT)
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .child(
                InputGroup::new("search-group")
                    .size_full()
                    .border_0()
                    .bg(theme.muted)
                    // The addon sizes and mutes the icon itself.
                    .addon(
                        InputGroupAddon::new("search-icon")
                            .child(Icon::new(IconName::ScanBarcode))
                            .pl_3(),
                    )
                    // 12px of inline padding on both sides, a little roomier
                    // than the defaults. The input takes the leading side (8px
                    // beside the icon by default). It resets its trailing side
                    // to 10px whenever the clear button shows, so the group
                    // adds the other 2px there.
                    .pr_4()
                    .input(
                        InputGroupInput::new(search)
                            .id("search")
                            .cleanable(true)
                            .pl_2(),
                    ),
            )
    }

    /// Beside the search, the same height: which of the table's columns to
    /// show. Clicking a column shows or hides it, and the menu stays open for
    /// the next; the product name always shows, so its item is checked but
    /// can't be picked.
    fn render_columns_menu(open: &OpenStocktake, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let button = DropdownButton::new()
            .outline()
            .small()
            .accessibility_label("Kolonner")
            .h(SEARCH_HEIGHT)
            .child(Icon::new(IconName::Columns3Cog).small().text_color(muted))
            .child("Kolonner")
            .child(Icon::new(IconName::ChevronDown).small().text_color(muted));
        // Under the button, lined up with its trailing edge, since it sits
        // against the end of the bar.
        let menu = DropdownMenu::new()
            .anchor(Anchor::TopRight)
            .heading("Vis kolonner")
            .min_w(Spacing(64.))
            .items(ProductColumn::ALL.map(|column| {
                let table = open.table.read(cx).delegate();
                DropdownItem::new(table.column_name(column, cx))
                    .checked(table.is_shown(column))
                    .disabled(!column.is_hideable())
                    .stays_open(true)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_column(column, cx)
                    }))
            }));
        Dropdown::new("columns", button, menu)
    }

    /// Along the bottom of the main pane, while there's a stock list: how
    /// much of it is counted at the leading edge, and whether every count is
    /// on disk at the trailing edge.
    fn render_status(session: &Session, cx: &App) -> impl IntoElement + use<> {
        let stocktake = session.stocktake();
        let (counted, total) = (stocktake.counted_len(), stocktake.len());
        // Rounded down, so 100 % means every product is counted.
        let percent = counted * 100 / total.max(1);
        let theme = cx.theme();
        let (icon, color, label) = match session.save_state() {
            SaveState::Saved => (IconName::Check, theme.muted_foreground, "Lagret"),
            SaveState::Failed => (IconName::TriangleAlert, theme.danger, "Ikke lagret"),
        };
        StatusBar::new()
            .flex_none()
            .bg(theme.background)
            .h_8()
            .pl(MAIN_PADDING)
            .pr(Spacing(6.))
            .left(
                h_flex()
                    .gap_4()
                    .child(
                        div()
                            .tabular_nums()
                            .child(format!("{counted} / {total} varer talt")),
                    )
                    .child(
                        h_flex()
                            .gap_2p5()
                            .child(
                                div().w_56().child(
                                    Progress::new("progress")
                                        .xsmall()
                                        .color(theme.muted_foreground)
                                        .value(percent as f32)
                                        .accessibility_label(format!(
                                            "{counted} av {total} varer talt"
                                        )),
                                ),
                            )
                            .child(div().tabular_nums().child(format!("{percent} %"))),
                    ),
            )
            .right(
                h_flex()
                    .gap_2()
                    .text_color(color)
                    .child(Icon::new(icon).xsmall())
                    .child(label),
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
        let is_open = self.open.is_some();
        div()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ImportStockList, window, cx| {
                this.import_stock_list(ImportSource::Choose, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FocusNext, window, cx| this.move_focus(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPrevious, window, cx| {
                    this.move_focus(false, window, cx)
                }),
            )
            // Without a stocktake there's nothing to export, search or show
            // in a sidebar, so these shortcuts and menu items are disabled.
            .when(is_open, |this| {
                this.on_action(cx.listener(|this, _: &ExportStocktake, window, cx| {
                    this.export_stocktake(window, cx)
                }))
                .on_action(
                    cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
                )
                .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                    this.toggle_sidebar(window, cx)
                }))
                .on_action(cx.listener(|this, _: &ShowCounting, window, cx| {
                    this.set_mode(Mode::Counting, window, cx)
                }))
                .on_action(cx.listener(
                    |this, _: &ShowDifferences, window, cx| {
                        this.set_mode(Mode::Differences, window, cx)
                    },
                ))
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.on_drop_files(paths, window, cx)
            }))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedSidebar>, window, cx| {
                    this.drag_sidebar_edge(event, window, cx)
                }),
            )
            .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().drop_target))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .size_full()
                    .when(self.sidebar_shown(), |this| {
                        this.child(self.render_sidebar(cx))
                    })
                    .child(self.render_main(cx)),
            )
    }
}

#[cfg(test)]
mod tests;
