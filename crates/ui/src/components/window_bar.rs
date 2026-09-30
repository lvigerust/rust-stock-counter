use gpui_kit::{MouseButton, Pixels, StyleRefinement, TitlebarOptions, point, px};

use crate::prelude::*;

/// The bar's horizontal padding, the same 1rem the sidebar's sections use.
const PADDING: Pixels = px(16.);

/// Where the window's traffic lights sit, measured from the top-left corner.
/// The inset is the bar's padding plus the 6px an icon button's glyph sits
/// inside it (a 14px icon centered in 24px, and the glyph's own margin), so
/// the lights line up with what the bar's buttons draw rather than their
/// bounds. The vertical offset centers the 14px buttons in the bar's row of
/// controls.
const TRAFFIC_LIGHTS: (Pixels, Pixels) = (px(22.), px(21.));

/// How far the bar's content starts from the leading edge, so it clears the
/// traffic lights with the same gap it keeps between its own groups.
const TRAFFIC_LIGHTS_CLEARANCE: Pixels = px(94.);

/// The band at the top of the window that the traffic lights sit in, so the
/// app's own controls share a row with them instead of sitting under a
/// separate system title bar. A window split into panes gives each pane
/// its own bar; only the leading one holds the traffic lights.
///
/// Pressing and dragging its background moves the window, and double-clicking
/// it does what the counter chose in System Settings (zoom or minimize). A
/// control inside it must stop left mouse-down from reaching the bar, or
/// selecting text or pressing a button would move the window instead.
///
/// The window must be opened with [`WindowBar::titlebar_options`] and
/// `app_owns_titlebar_drag`, so the traffic lights line up with the bar and
/// the bar alone decides what drags.
#[derive(IntoElement)]
pub struct WindowBar {
    traffic_lights: bool,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl WindowBar {
    /// The bar's height: a row of small controls (1.5rem) with 1rem of
    /// padding above and below, the same padding the sidebar's sections use.
    pub const HEIGHT: Pixels = px(56.);

    pub fn new() -> Self {
        Self {
            traffic_lights: true,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// Whether the traffic lights sit in this bar, so its content starts
    /// after them. Only the bar at the window's leading edge holds them.
    /// Default is `true`.
    pub fn traffic_lights(mut self, traffic_lights: bool) -> Self {
        self.traffic_lights = traffic_lights;
        self
    }

    /// A transparent system title bar with its traffic lights moved into
    /// the bar.
    pub fn titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
        let (x, y) = TRAFFIC_LIGHTS;
        TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(x, y)),
        }
    }
}

impl Default for WindowBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for WindowBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for WindowBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Whether a press on the bar may still turn into a window move.
struct DragState {
    pressed: bool,
}

impl RenderOnce for WindowBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| DragState { pressed: false });
        // Full screen hides the traffic lights, so nothing needs clearing.
        let leading = if self.traffic_lights && cfg!(target_os = "macos") && !window.is_fullscreen()
        {
            TRAFFIC_LIGHTS_CLEARANCE
        } else {
            PADDING
        };

        h_flex()
            .id("window-bar")
            .flex_none()
            .h(Self::HEIGHT)
            .pl(leading)
            .pr(PADDING)
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| state.pressed = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| state.pressed = false),
            )
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| state.pressed = false))
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.pressed {
                    state.pressed = false;
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            .children(self.children)
            .refine_style(&self.style)
    }
}
