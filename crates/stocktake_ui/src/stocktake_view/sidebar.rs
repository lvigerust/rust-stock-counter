//! The sidebar: the status and aisle filters, starting over, exporting, the
//! button that hides it or shows it again, and the edge that resizes it.

use gpui_kit::component::{
    FocusableExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    collapsible::Collapsible,
    separator::Separator,
};
use gpui_kit::{
    ClickEvent, CursorStyle, DragMoveEvent, Empty, Focusable as _, MouseButton, Pixels, px,
};
use ui::{
    Dropdown, DropdownButton, DropdownItem, DropdownMenu, Sidebar, SidebarBody, SidebarFooter,
    SidebarHeader, SidebarHeading, SidebarItem, SidebarSection, SidebarSpacer, Spacing, WindowBar,
    prelude::*,
};

use super::{Mode, StocktakeView};
use crate::count_status::CountStatus;
use crate::{CONTEXT, ToggleSidebar};

/// The sidebar's width when the window opens, and after a double-click on
/// its edge: wide enough for an aisle's label beside its count, and a bar
/// that clears the traffic lights.
pub(super) const DEFAULT_SIDEBAR_WIDTH: Pixels = px(256.);

/// The narrowest the counter can drag it: the traffic lights and the hide
/// button still fit in its bar.
const MIN_SIDEBAR_WIDTH: Pixels = px(200.);

/// The widest the counter can drag it, so the table keeps the room it needs.
const MAX_SIDEBAR_WIDTH: Pixels = px(400.);

/// How wide the band that grabs the sidebar's edge is. It sits inside the
/// sidebar against its trailing rule: the main pane is painted after the
/// sidebar, so any part of the band past the rule would sit under the pane
/// and never get the pointer.
const RESIZE_HANDLE_WIDTH: Pixels = px(6.);

/// What's dragged while the counter resizes the sidebar. It draws nothing:
/// the sidebar itself follows the pointer.
pub(super) struct DraggedSidebar;

impl Render for DraggedSidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl StocktakeView {
    /// Whether the sidebar is on screen: only with a stocktake, and only
    /// while it isn't hidden.
    pub(super) fn sidebar_shown(&self) -> bool {
        self.open.is_some() && !self.sidebar_collapsed
    }

    /// How much of the window's width the sidebar takes: nothing while
    /// it's hidden.
    pub(super) fn shown_sidebar_width(&self) -> Pixels {
        if self.sidebar_shown() {
            self.sidebar_width
        } else {
            px(0.)
        }
    }

    /// Sets the sidebar's width, kept between the narrowest and widest it
    /// can be, and refits the table to the room left.
    fn resize_sidebar(&mut self, width: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        let width = width.max(MIN_SIDEBAR_WIDTH).min(MAX_SIDEBAR_WIDTH);
        if width == self.sidebar_width {
            return;
        }
        self.sidebar_width = width;
        self.fit_columns(window, cx);
        cx.notify();
    }

    /// The pointer moved while dragging the sidebar's edge: the edge
    /// follows it. Registered on the whole window, since a quick drag
    /// leaves the edge's narrow band behind.
    pub(super) fn drag_sidebar_edge(
        &mut self,
        event: &DragMoveEvent<DraggedSidebar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Without this the cursor falls back to an arrow once the pointer
        // leaves the band.
        cx.set_active_drag_cursor_style(CursorStyle::ResizeColumn, window);
        self.resize_sidebar(event.event.position.x - event.bounds.left(), window, cx);
    }

    /// The button that was pressed goes away with its bar and its twin
    /// appears in the other one. Focus on it would go with it, leaving
    /// nothing to take Tab or the shortcuts, so it returns to the window
    /// instead; the next Tab reaches the twin, the first stop in either
    /// state. Focus in the search or the table stays where it is.
    pub(super) fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.fit_columns(window, cx);
        let stays = self.open.as_ref().is_some_and(|open| {
            open.search.focus_handle(cx).contains_focused(window, cx)
                || open.table.focus_handle(cx).contains_focused(window, cx)
        });
        if !stays {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    fn set_counted_shown(&mut self, counted: bool, shown: bool, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        open.filter.set_counted_shown(counted, shown);
        open.refresh_rows(cx);
        cx.notify();
    }

    fn set_aisle_shown(&mut self, aisle: &str, shown: bool, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else {
            return;
        };
        open.filter.set_aisle_shown(aisle, shown);
        open.refresh_rows(cx);
        cx.notify();
    }

    /// The pane along the leading edge: the top bar with the traffic lights
    /// and the button that hides the sidebar, set apart by a rule, then the
    /// filters with the way to start over below them, against the footer and
    /// its way to export. Each section holds its content as [`SidebarItem`]s.
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = Self::render_sidebar_toggle(true, false, cx);
        let status_filter = self.render_status_filter(cx);
        let aisle_filter = self.render_aisle_filter(cx);
        Sidebar::new()
            .relative()
            .w(self.sidebar_width)
            .child(WindowBar::new().justify_end().child(toggle))
            .child(Separator::horizontal().color(cx.theme().sidebar_border))
            .child(SidebarHeader::new().child(render_mode_menu(self.mode, cx)))
            .child(SidebarBody::new().map(|body| {
                match self.mode {
                    Mode::Counting => body
                        .children(status_filter)
                        .children(aisle_filter)
                        .child(SidebarSpacer::new())
                        .child(SidebarSection::new().child(Self::render_discard_button(cx))),
                    Mode::Differences => body,
                }
            }))
            .child(SidebarFooter::new().child(Self::render_export_button(cx)))
            .child(Self::render_resize_handle(cx))
    }

    /// A band over the sidebar's trailing rule that resizes it: dragging it
    /// moves the edge, and a double-click puts it back at its default width.
    /// Hovering it draws the rule in the sidebar items' hover color, to show
    /// it can be grabbed.
    fn render_resize_handle(cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let line = cx.theme().sidebar_accent;
        div()
            .id("sidebar-resize-handle")
            .group("sidebar-resize-handle")
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(RESIZE_HANDLE_WIDTH)
            .flex()
            .justify_end()
            // Over the sidebar's bar and its scrolling body, the band is the
            // one the pointer is on.
            .occlude()
            .cursor_col_resize()
            .on_drag(DraggedSidebar, |_, _, _, cx| cx.new(|_| DraggedSidebar))
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    this.resize_sidebar(DEFAULT_SIDEBAR_WIDTH, window, cx);
                }
            }))
            .child(
                div()
                    .w(px(2.))
                    .h_full()
                    .group_hover("sidebar-resize-handle", move |style| style.bg(line)),
            )
    }

    fn toggle_aisle_filter(&mut self, cx: &mut Context<Self>) {
        self.aisle_filter_collapsed = !self.aisle_filter_collapsed;
        cx.notify();
    }

    /// A checkbox each for uncounted and counted products, beside how many
    /// there are, so the table can be narrowed to what's left to count.
    /// Only while there's a stock list.
    fn render_status_filter(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let open = self.open.as_ref()?;
        let stocktake = open.session.read(cx).stocktake();
        let statuses = [
            (true, stocktake.counted_len()),
            (false, stocktake.uncounted_len()),
        ];
        let items = statuses.map(|(counted, len)| {
            let id = if counted {
                "status-counted"
            } else {
                "status-uncounted"
            };
            let checkbox = Checkbox::new(id)
                .checked(open.filter.shows_counted(counted))
                .on_click(cx.listener(move |this, shown: &bool, _, cx| {
                    this.set_counted_shown(counted, *shown, cx)
                }));
            render_filter_item(checkbox, CountStatus::label(counted).into(), len, cx)
        });
        Some(
            SidebarSection::new()
                .child(SidebarHeading::new("Status"))
                .children(items),
        )
    }

    /// A checkbox per aisle, beside how many products it holds, so the table
    /// can be narrowed to the part of the storage being counted. Only while
    /// there's a stock list, and one spanning more than one aisle. Its
    /// heading opens and closes it.
    fn render_aisle_filter(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let open = self.open.as_ref()?;
        let aisles: Vec<(SharedString, usize)> = open
            .session
            .read(cx)
            .stocktake()
            .aisles()
            .into_iter()
            .map(|(aisle, len)| (aisle.to_string().into(), len))
            .collect();
        if aisles.len() < 2 {
            return None;
        }
        let items = aisles.into_iter().map(|(aisle, len)| {
            let label = if aisle.is_empty() {
                "Uten reol".into()
            } else {
                SharedString::from(format!("Reol {aisle}"))
            };
            let checkbox = Checkbox::new(format!("aisle-{aisle}"))
                .checked(open.filter.shows_aisle(&aisle))
                .on_click(cx.listener(move |this, shown: &bool, _, cx| {
                    this.set_aisle_shown(&aisle, *shown, cx)
                }));
            render_filter_item(checkbox, label, len, cx)
        });
        let open = !self.aisle_filter_collapsed;
        let heading = SidebarHeading::new("Lokasjoner").on_toggle(
            "toggle-aisle-filter",
            open,
            cx.listener(|this, _, _, cx| this.toggle_aisle_filter(cx)),
        );
        Some(
            Collapsible::new()
                .motion_id("aisle-filter")
                .open(open)
                .gap_0p5()
                .child(heading)
                .content(SidebarSection::new().children(items)),
        )
    }

    /// Starts over without the stocktake in progress. For testing.
    fn render_discard_button(cx: &mut Context<Self>) -> impl IntoElement + use<> {
        SidebarItem::new()
            .on_click(
                "discard-stocktake",
                cx.listener(|this, _, window, cx| this.discard_stocktake(window, cx)),
            )
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::Trash).small())
            .child(div().min_w_0().truncate().child("Tøm varetelling"))
    }

    /// Writes the stocktake to a file, as the menu's Eksporter telling… does.
    fn render_export_button(cx: &mut Context<Self>) -> impl IntoElement + use<> {
        Button::new("export-stocktake")
            .w_full()
            .icon(IconName::Share)
            .label("Eksporter telling")
            .on_click(cx.listener(|this, _, window, cx| this.export_stocktake(window, cx)))
    }

    /// Hides the sidebar from its own bar, or shows it again from the main
    /// pane's bar once it's hidden. Disabled while there's no sidebar to show,
    /// its tooltip then saying what brings one; the shortcut is left out, as
    /// it does nothing either.
    pub(super) fn render_sidebar_toggle(
        expanded: bool,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (id, tooltip) = if expanded {
            ("hide-sidebar", "Skjul sidepanel")
        } else {
            ("show-sidebar", "Vis sidepanel")
        };
        // A press here is the button's, not the start of a window drag.
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(id)
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeft)
                    .disabled(disabled)
                    // Quieter than the content at rest; hover brings it up.
                    .text_color(cx.theme().muted_foreground)
                    // gpui-kit would recolor a border for focus; this draws
                    // a faint ring around the button instead.
                    .focus_ring(false)
                    .subtle_focus_ring()
                    .map(|button| {
                        if disabled {
                            button.tooltip("Åpne en fil for å vise sidepanelet")
                        } else {
                            button.tooltip_with_action(tooltip, &ToggleSidebar, Some(CONTEXT))
                        }
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_sidebar(window, cx))),
            )
    }
}

/// One choice in a filter: its checkbox and label, then how many products
/// it covers.
///
/// The checkbox is the whole row, the count inside it beside the label, so
/// its focus ring goes around the row and a click anywhere on it toggles
/// it. It takes the item's padding for that.
fn render_filter_item(
    checkbox: Checkbox,
    label: SharedString,
    len: usize,
    cx: &App,
) -> SidebarItem {
    let theme = cx.theme();
    // With the theme's ring off, gpui-kit marks focus by recoloring a
    // border the checkbox doesn't have, so it showed none; this draws a
    // faint ring around the row instead.
    let checkbox = checkbox
        .small()
        .accessibility_label(label.clone())
        .flex_1()
        .min_w_0()
        .p_2()
        .rounded(theme.radius)
        .focus_ring(false)
        .subtle_focus_ring()
        .child(
            h_flex()
                .gap_2()
                .child(div().flex_1().min_w_0().truncate().child(label))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .tabular_nums()
                        .text_color(theme.muted_foreground)
                        .child(len.to_string()),
                ),
        );
    SidebarItem::new().p_0().child(checkbox)
}

/// Atop the sidebar, the mode the window is in, as a button that opens a
/// menu of every mode below it, the current one checked. The name takes the
/// width the chevron leaves, and truncates past it.
fn render_mode_menu(current: Mode, cx: &App) -> impl IntoElement {
    let button = DropdownButton::new()
        .w_full()
        .p_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .font_semibold()
                .child(current.label()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .small()
                .text_color(cx.theme().muted_foreground),
        );
    // Catalyst's desktop width for the menu atop its sidebar (`lg:min-w-64`).
    let menu = DropdownMenu::new()
        .min_w(Spacing(64.))
        .items(Mode::ALL.map(|mode| {
            DropdownItem::new(mode.label())
                .description(mode.description())
                .checked(mode == current)
                .action(mode.action())
        }));
    Dropdown::new("mode-menu", button, menu)
}
