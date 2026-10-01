//! How focus shows on controls that have no border of their own.

use gpui_kit::{BoxShadow, Hsla, Pixels, px, white};

use crate::prelude::*;

/// How far the ring reaches outside the control.
const FOCUS_RING_WIDTH: Pixels = px(1.5);

/// The ring is white, faded until it marks where focus is without drawing
/// the eye the way a solid border does.
const FOCUS_RING_OPACITY: f32 = 0.05;

/// How wide the solid ring is.
const SOLID_FOCUS_RING_WIDTH: Pixels = px(2.);

/// The space between the control's edge and the solid ring.
const SOLID_FOCUS_RING_GAP: Pixels = px(2.);

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

    /// A ring of `color` around the control while it has keyboard focus,
    /// set a small gap away from its edge. Pass the theme's `ring` for the
    /// blue that marks focus everywhere else.
    ///
    /// GPUI fills a shadow's whole shape, not just what reaches past the
    /// control, so a control without a background of its own (a ghost
    /// button) would show the ring's color through it. A second shadow in
    /// `surface`, the color of what the control sits on, covers the inside
    /// again and leaves the gap.
    ///
    /// Like [`Self::subtle_focus_ring`], it needs gpui-kit's own focus look
    /// turned off with `focus_ring(false)`.
    fn solid_focus_ring(self, color: Hsla, surface: Hsla) -> Self {
        self.focus_visible(move |style| {
            // Painted in order, so the surface lands on top of the ring.
            style.shadow(vec![
                BoxShadow::new(px(0.), px(0.), color)
                    .spread_radius(SOLID_FOCUS_RING_GAP + SOLID_FOCUS_RING_WIDTH),
                BoxShadow::new(px(0.), px(0.), surface).spread_radius(SOLID_FOCUS_RING_GAP),
            ])
        })
    }
}

impl<E: InteractiveElement> StyledFocus for E {}
