//! Motion: the few ways things move in this app.
//!
//! Everything here is built on gpui-base's motion primitives rather than raw
//! `with_animation`, because those read the theme's motion tokens and honour
//! the system's reduced-motion setting: with it on, things simply appear in
//! their final state.
//!
//! Motion explains a change; it isn't decoration. Use [`flash`] to point at
//! something that just changed somewhere the eye might not be looking.

use std::time::{Duration, Instant};

use gpui_kit::base::motion::{Easing, Keyframe, Keyframes, Timing};

use crate::prelude::*;

/// How long a [`flash`] takes to fade out. Long enough to catch the eye after
/// looking back from the shelf, short enough to be gone before the next scan.
const FLASH_DURATION: Duration = Duration::from_millis(1400);

/// How strongly to highlight something that changed at `started_at`: rises
/// quickly to 1, then fades back to 0. Multiply a highlight color's opacity
/// by it.
///
/// The caller owns `started_at` (read it from `cx.background_executor().now()`)
/// and keeps it with the thing that changed, not with the element that shows
/// it. Element state is dropped when an element leaves the screen, so a flash
/// keyed to a table row would replay every time the row scrolled back in.
///
/// Under reduced motion it is always 0, so the highlight must not be the only
/// sign of the change.
pub fn flash(started_at: Instant, window: &mut Window, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return 0.0;
    }
    let elapsed = cx
        .background_executor()
        .now()
        .saturating_duration_since(started_at);
    let sample = Timing::new(FLASH_DURATION).sample(elapsed);
    if sample.finished {
        return 0.0;
    }
    window.request_animation_frame();
    let keyframes = Keyframes::try_new([
        Keyframe::new(0.0, 0.0_f32),
        Keyframe::new(0.12, 1.0).ease(Easing::EaseOut),
        Keyframe::new(1.0, 0.0),
    ])
    .expect("flash keyframes are ordered and cover 0..=1");
    keyframes.sample(sample.directed_progress)
}
