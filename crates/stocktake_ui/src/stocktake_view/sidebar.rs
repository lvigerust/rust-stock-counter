//! The sidebar: the app's name, the status and aisle filters, starting over,
//! and the button that hides it or shows it again.

use gpui_kit::component::{
    FocusableExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    collapsible::Collapsible,
    separator::Separator,
};
use gpui_kit::{Focusable as _, MouseButton, Pixels, px};
use ui::{
    Sidebar, SidebarBody, SidebarFooter, SidebarHeading, SidebarItem, SidebarSection, WindowBar,
    prelude::*,
};

use super::StocktakeView;
use crate::count_status::CountStatus;
use crate::{APP_NAME, CONTEXT, ToggleSidebar};

/// Wide enough for a product name beside an icon, and a bar that clears the
/// traffic lights.
const SIDEBAR_WIDTH: Pixels = px(256.);

impl StocktakeView {
    /// Whether the sidebar is on screen: only with a stocktake, and only
    /// while it isn't hidden.
    pub(super) fn sidebar_shown(&self) -> bool {
        self.open.is_some() && !self.sidebar_collapsed
    }

    /// How much of the window's width the sidebar takes.
    pub(super) fn sidebar_width(&self) -> Pixels {
        if self.sidebar_shown() {
            SIDEBAR_WIDTH
        } else {
            px(0.)
        }
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
    /// filters and the way to start over. Each section holds its content as
    /// [`SidebarItem`]s.
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = Self::render_sidebar_toggle(true, false, cx);
        let status_filter = self.render_status_filter(cx);
        let aisle_filter = self.render_aisle_filter(cx);
        Sidebar::new()
            .w(SIDEBAR_WIDTH)
            .child(WindowBar::new().justify_end().child(toggle))
            .child(Separator::horizontal().color(cx.theme().sidebar_border))
            .child(
                SidebarBody::new()
                    .children(status_filter)
                    .children(aisle_filter),
            )
            .child(
                SidebarFooter::new()
                    .child(SidebarSection::new().child(Self::render_discard_button(cx))),
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
                .label(CountStatus::label(counted))
                .checked(open.filter.shows_counted(counted))
                .on_click(cx.listener(move |this, shown: &bool, _, cx| {
                    this.set_counted_shown(counted, *shown, cx)
                }));
            render_filter_item(checkbox, len, cx)
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
                .label(label)
                .checked(open.filter.shows_aisle(&aisle))
                .on_click(cx.listener(move |this, shown: &bool, _, cx| {
                    this.set_aisle_shown(&aisle, *shown, cx)
                }));
            render_filter_item(checkbox, len, cx)
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
                    .subtle_focus_ring(cx)
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

/// One choice in a filter: its checkbox, then how many products it covers.
fn render_filter_item(checkbox: Checkbox, len: usize, cx: &App) -> SidebarItem {
    SidebarItem::new()
        .child(checkbox.small().flex_1().min_w_0())
        .child(
            div()
                .text_xs()
                .tabular_nums()
                .text_color(cx.theme().muted_foreground)
                .child(len.to_string()),
        )
}

/// The app's name, atop the sidebar.
#[expect(
    dead_code,
    reason = "taken out of the sidebar for now; restore it in a SidebarHeader"
)]
fn render_app_name() -> impl IntoElement {
    SidebarItem::new().child(
        div()
            .min_w_0()
            .truncate()
            .text_sm()
            .font_semibold()
            .child(APP_NAME),
    )
}
