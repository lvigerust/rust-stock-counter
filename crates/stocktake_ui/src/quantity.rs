//! Entering a quantity: the counted-quantity cell and the recount field
//! accept the same thing, so they are built the same way.

use gpui_kit::component::input::InputState;
use ui::prelude::*;

/// A single-line input that accepts whole, non-negative numbers.
pub(crate) fn quantity_input(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    InputState::new(window, cx).validate(|text, _| text.chars().all(|c| c.is_ascii_digit()))
}

pub(crate) fn parse_quantity(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}
