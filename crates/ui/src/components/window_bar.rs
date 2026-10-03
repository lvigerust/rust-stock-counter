use gpui_kit::{
    Hsla, MouseButton, Pixels, StyleRefinement, TitlebarOptions, WindowControlArea, point, px,
};

use crate::prelude::*;

/// The bar's horizontal padding, the same 1rem the sidebar's sections use.
const PADDING: Pixels = px(16.);

/// macOS: where the window's traffic lights sit, measured from the top-left
/// corner. The inset is the bar's padding plus the 6px an icon button's glyph
/// sits inside it (a 14px icon centered in 24px, and the glyph's own margin),
/// so the lights line up with what the bar's buttons draw rather than their
/// bounds. The vertical offset centers the 14px buttons in the bar's row of
/// controls.
const TRAFFIC_LIGHTS: (Pixels, Pixels) = (px(22.), px(21.));

/// macOS: where the traffic lights end: three 14px buttons 9px apart, from
/// [`TRAFFIC_LIGHTS`], as AppKit lays them out on macOS 26 and later.
const TRAFFIC_LIGHTS_END: Pixels = px(82.);

/// Windows: the width of each caption button (minimize, maximize or restore,
/// close), as Windows 11 draws them. They run the bar's full height and sit
/// flush against the window's top-right corner.
const CAPTION_BUTTON_WIDTH: Pixels = px(46.);

/// The band at the top of the window that holds the window's own controls,
/// so the app's controls share a row with them instead of sitting under a
/// separate system title bar. A window split into panes gives each pane its
/// own bar; the bar at the window's leading or trailing edge holds the
/// controls that sit there.
///
/// On macOS, the traffic lights sit at the leading edge and the bar's content
/// starts after them. Pressing and dragging its background moves the window,
/// and double-clicking it does what the counter chose in System Settings
/// (zoom or minimize).
///
/// On Windows, the bar draws minimize, maximize or restore, and close at the
/// trailing edge, and its content ends before them. Its background is the
/// system's title bar, so Windows moves the window, maximizes it on
/// double-click, snaps it, and shows the Snap Layouts flyout over maximize.
///
/// A control inside the bar belongs in a [`WindowBarItem`], or selecting text
/// or pressing a button would move the window instead.
///
/// The window must be opened with [`WindowBar::titlebar_options`] and
/// `app_owns_titlebar_drag`, so the traffic lights line up with the bar and
/// the bar alone decides what drags.
#[derive(IntoElement)]
pub struct WindowBar {
    leading_edge: bool,
    trailing_edge: bool,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl WindowBar {
    /// The bar's height: a row of small controls (1.5rem) with 1rem of
    /// padding above and below, the same padding the sidebar's sections use.
    pub const HEIGHT: Pixels = px(56.);

    /// How far apart the bar's controls are drawn, the traffic lights
    /// included. Measured between what each control draws: a border, or an
    /// icon button's glyph, which sits [`Self::ICON_INSET`] inside the button.
    pub const GAP: Pixels = px(20.);

    /// How far an icon button's glyph sits inside its bounds: a 14px icon
    /// centered in 24px, and the glyph's own margin. A gap beside one is
    /// that much shorter, so the glyph sits [`Self::GAP`] from its neighbor.
    pub const ICON_INSET: Pixels = px(6.);

    pub fn new() -> Self {
        Self {
            leading_edge: true,
            trailing_edge: true,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// Whether this bar runs along the window's leading edge, so it holds
    /// the macOS traffic lights and its content starts after them.
    /// Default is `true`.
    pub fn leading_edge(mut self, leading_edge: bool) -> Self {
        self.leading_edge = leading_edge;
        self
    }

    /// Whether this bar runs along the window's trailing edge, so it holds
    /// the Windows caption buttons and its content ends before them.
    /// Default is `true`.
    pub fn trailing_edge(mut self, trailing_edge: bool) -> Self {
        self.trailing_edge = trailing_edge;
        self
    }

    /// A system title bar that the bar draws over: transparent, with the
    /// traffic lights moved into the bar on macOS. On Windows the system
    /// title bar is removed, caption buttons included; the bar draws them.
    pub fn titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
        let (x, y) = TRAFFIC_LIGHTS;
        TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: true,
            traffic_light_position: cfg!(target_os = "macos").then(|| point(x, y)),
        }
    }

    fn render_macos(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| DragState { pressed: false });
        // Full screen hides the traffic lights, so nothing needs clearing.
        // Otherwise the content starts so an icon button's glyph sits a gap
        // after them.
        let leading = if self.leading_edge && cfg!(target_os = "macos") && !window.is_fullscreen() {
            TRAFFIC_LIGHTS_END + Self::GAP - Self::ICON_INSET
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

    /// The bar's content, then the caption buttons beside it rather than in
    /// it: the system takes the first control area under the pointer, so a
    /// drag area around the buttons would swallow them. The bar's style
    /// applies to the content, so padding set by the caller still keeps the
    /// content clear of the buttons. Full screen has no caption buttons.
    fn render_windows(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let caption_buttons = self.trailing_edge && !window.is_fullscreen();
        let maximize = if window.is_maximized() {
            CaptionButton::Restore
        } else {
            CaptionButton::Maximize
        };

        h_flex()
            .flex_none()
            .items_stretch()
            .child(
                h_flex()
                    .id("window-bar")
                    .flex_1()
                    .min_w_0()
                    .h(Self::HEIGHT)
                    .px(PADDING)
                    .window_control_area(WindowControlArea::Drag)
                    .children(self.children)
                    .refine_style(&self.style),
            )
            .when(caption_buttons, |this| {
                this.child(CaptionButton::Minimize)
                    .child(maximize)
                    .child(CaptionButton::Close)
            })
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
        if cfg!(target_os = "windows") {
            self.render_windows(window, cx).into_any_element()
        } else {
            self.render_macos(window, cx).into_any_element()
        }
    }
}

/// A control inside a [`WindowBar`]: a press on it is the control's, not the
/// start of a window move. On Windows it also blocks the bar's drag area
/// behind it, which the system hit-tests before the app sees the press.
#[derive(IntoElement)]
pub struct WindowBarItem {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl WindowBarItem {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for WindowBarItem {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for WindowBarItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for WindowBarItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for WindowBarItem {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(cfg!(target_os = "windows"), |this| this.occlude())
            .children(self.children)
            .refine_style(&self.style)
    }
}

/// Windows: one of the window's caption buttons. The system handles the
/// click, through the control area the button marks, so it has no handler.
#[derive(IntoElement, Clone, Copy)]
enum CaptionButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl CaptionButton {
    fn id(self) -> &'static str {
        match self {
            Self::Minimize => "minimize-window",
            Self::Maximize => "maximize-window",
            Self::Restore => "restore-window",
            Self::Close => "close-window",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Maximize => IconName::WindowMaximize,
            Self::Restore => IconName::WindowRestore,
            Self::Close => IconName::WindowClose,
        }
    }

    fn area(self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Maximize | Self::Restore => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }

    /// The fill and glyph color under the pointer, then while pressed. Close
    /// turns red, as Windows draws it.
    fn hover_colors(self, cx: &App) -> (Hsla, Hsla, Hsla) {
        let theme = cx.theme();
        match self {
            Self::Close => (theme.danger, theme.danger_active, theme.danger_foreground),
            _ => (
                theme.secondary_hover,
                theme.secondary_active,
                theme.foreground,
            ),
        }
    }
}

impl RenderOnce for CaptionButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (hover, active, foreground) = self.hover_colors(cx);
        h_flex()
            .id(self.id())
            .flex_none()
            .w(CAPTION_BUTTON_WIDTH)
            .justify_center()
            .text_color(cx.theme().foreground)
            .hover(|style| style.bg(hover).text_color(foreground))
            .active(|style| style.bg(active).text_color(foreground))
            .window_control_area(self.area())
            .child(Icon::new(self.icon()).small())
    }
}
