//! What the window shows of the stocktake: the stock list for counting, or
//! the products that came out different. Picked from the menu atop the
//! sidebar, the menu bar or a shortcut; the sidebar's body and the main pane
//! both follow it.

use gpui_kit::{Action, Focusable as _};
use ui::prelude::*;

use super::StocktakeView;
use crate::{ShowCounting, ShowDifferences};

/// What the window shows of the stocktake.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    /// The stock list, with the search and filters for counting it.
    #[default]
    Counting,
    /// The products whose counted quantity differs from the system quantity.
    Differences,
}

impl Mode {
    /// Every mode, in the order the menu lists them.
    pub(super) const ALL: [Self; 2] = [Self::Counting, Self::Differences];

    /// The mode's name, in the menu and on the button that opens it.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Counting => "Varetelling",
            Self::Differences => "Differanse",
        }
    }

    /// A line under the mode's name in the menu, on what it shows.
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Counting => "Tell varene på lageret.",
            Self::Differences => "Varer der talt antall ikke stemmer.",
        }
    }

    /// The action that switches to the mode, so the menu shows its shortcut.
    pub(super) fn action(self) -> Box<dyn Action> {
        match self {
            Self::Counting => Box::new(ShowCounting),
            Self::Differences => Box::new(ShowDifferences),
        }
    }
}

impl StocktakeView {
    /// Shows `mode`. The search and the table of the mode left go away with
    /// it, and focus on them would go too, leaving nothing to take the
    /// shortcuts; it returns to the window instead, as it does when the
    /// sidebar hides. Focus anywhere else stays where it is.
    pub(super) fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        let leaves_with_mode = self.open.as_ref().is_some_and(|open| {
            let hidden = match mode {
                Mode::Counting => vec![open.differences.focus_handle(cx)],
                Mode::Differences => {
                    vec![open.search.focus_handle(cx), open.table.focus_handle(cx)]
                }
            };
            hidden
                .iter()
                .any(|handle| handle.contains_focused(window, cx))
        });
        if leaves_with_mode {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }
}
