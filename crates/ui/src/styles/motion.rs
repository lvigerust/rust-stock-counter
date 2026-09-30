//! Motion: the few ways things move in this app.
//!
//! Everything here is built on gpui-base's motion primitives rather than raw
//! `with_animation`, because those read the theme's motion tokens and honour
//! the system's reduced-motion setting: with it on, things simply appear in
//! their final state.
//!
//! Motion explains a change; it isn't decoration. Use [`Appear`] when a region
//! comes into existence and [`flash`] to point at something that just changed
//! somewhere the eye might not be looking.

use std::time::Duration;

use gpui_kit::base::motion::{
    Easing, Keyframe, Keyframes, Presence, PresencePhase, Timing, Transition, animate_keyframes,
};

use gpui_kit::StyleRefinement;

use crate::prelude::*;

/// How long a [`flash`] takes to fade out. Long enough to catch the eye after
/// looking back from the shelf, short enough to be gone before the next scan.
const FLASH_DURATION: Duration = Duration::from_millis(1400);

/// Fades its child in while it rises a short distance into place, the first
/// time it renders.
///
/// The `id` owns the animation: the same id keeps its finished state across
/// renders, and a new id plays the entrance again.
///
/// ```ignore
/// Appear::new("welcome").child(title)
/// Appear::new("welcome-actions").delay(Duration::from_millis(80)).child(button)
/// ```
#[derive(IntoElement)]
pub struct Appear {
    id: ElementId,
    style: StyleRefinement,
    delay: Duration,
    children: Vec<AnyElement>,
}

impl Appear {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            delay: Duration::ZERO,
            children: Vec::new(),
        }
    }

    /// Waits before starting, so a group of regions can arrive in reading
    /// order instead of all at once.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// Styles apply to the moving box, so `Appear::new(id).size_full()` can wrap
/// a whole region.
impl Styled for Appear {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Appear {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Appear {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens().clone();
        let sample = Presence::new(self.id.clone(), true)
            .transition(
                Transition::new(motion.duration_slow)
                    .delay(self.delay)
                    .easing(motion.easing_enter),
            )
            .sample(window, cx);
        let progress = match sample.phase {
            PresencePhase::Entering | PresencePhase::Exiting => sample.progress,
            PresencePhase::Present => 1.0,
            PresencePhase::Absent => 0.0,
        };
        let rise = motion.distance_medium.to_pixels(window.rem_size()) * (1.0 - progress);

        div()
            .id(self.id)
            .refine_style(&self.style)
            .relative()
            .top(rise)
            .opacity(progress)
            .children(self.children)
    }
}

/// How strongly to highlight something that just changed: rises quickly to 1,
/// then fades back to 0. Multiply a highlight color's opacity by it.
///
/// Include a generation in the `id` to flash the same thing again:
/// `("counted", generation)`. Under reduced motion it is always 0, so the
/// highlight must not be the only sign of the change.
pub fn flash(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> f32 {
    let keyframes = Keyframes::try_new([
        Keyframe::new(0.0, 0.0_f32),
        Keyframe::new(0.12, 1.0).ease(Easing::EaseOut),
        Keyframe::new(1.0, 0.0),
    ])
    .expect("flash keyframes are ordered and cover 0..=1");
    let id: ElementId = id.into();
    animate_keyframes(id, &keyframes, Timing::new(FLASH_DURATION), window, cx).value
}
