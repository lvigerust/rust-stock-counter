//! How focus shows on controls that have no border of their own.

use gpui_kit::{BoxShadow, Pixels, px, white};

use crate::prelude::*;

/// How far the ring reaches outside the control.
const FOCUS_RING_WIDTH: Pixels = px(1.5);

/// The ring is white, faded until it marks where focus is without drawing
/// the eye the way a solid border does.
const FOCUS_RING_OPACITY: f32 = 0.05;

pub trait StyledFocus: InteractiveElement + Sized {
    /// A faint ring just outside the control while it has keyboard focus, the
    /// way tty7 marks focus on its icon buttons.
    ///
    /// It's a shadow, so it takes no space and follows the control's corners.
    /// A gpui-kit component draws its own focus look as well; turn that off
    /// with `focus_ring(false)` so the two don't stack.
    fn subtle_focus_ring(self) -> Self {
        let color = white().alpha(FOCUS_RING_OPACITY);
        self.focus_visible(|style| {
            style.shadow(vec![
                BoxShadow::new(px(0.), px(0.), color).spread_radius(FOCUS_RING_WIDTH),
            ])
        })
    }
}

impl<E: InteractiveElement> StyledFocus for E {}
