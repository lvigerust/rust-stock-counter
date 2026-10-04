//! What the window shows of the stocktake: the stock list for counting, or
//! the products that came out different. Picked from the menu atop the
//! sidebar, the menu bar or a shortcut; the sidebar's body and the main pane
//! both follow it.

use gpui_kit::Action;
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
    pub(super) const ALL: [Mode; 2] = [Mode::Counting, Mode::Differences];

    /// The mode's name, in the menu and on the button that opens it.
    pub(super) fn label(self) -> &'static str {
        match self {
            Mode::Counting => "Varetelling",
            Mode::Differences => "Differanse",
        }
    }

    /// A line under the mode's name in the menu, on what it shows.
    pub(super) fn description(self) -> &'static str {
        match self {
            Mode::Counting => "Tell varene på lageret.",
            Mode::Differences => "Varer der talt antall ikke stemmer.",
        }
    }

    /// The action that switches to the mode, so the menu shows its shortcut.
    pub(super) fn action(self) -> Box<dyn Action> {
        match self {
            Mode::Counting => Box::new(ShowCounting),
            Mode::Differences => Box::new(ShowDifferences),
        }
    }
}

impl StocktakeView {
    pub(super) fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        // TODO(human)
        let _ = window;
        cx.notify();
    }
}
