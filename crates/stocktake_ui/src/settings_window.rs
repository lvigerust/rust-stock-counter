//! The settings window, opened beside the counting window so a setting can
//! be tried while the count it affects stays in view (after tty7's).
//!
//! The window owns no settings: it draws [`SettingsState`], which the
//! counting window holds too. Nothing is left unsaved, so every way of
//! closing it is the same one.

use gpui_kit::component::{WindowExt as _, notification::Notification};
use gpui_kit::{Bounds, FocusHandle, Focusable, Subscription, WindowOptions, px, size};
use ui::{Heading, SectionHeading, SwitchField, WindowBar, prelude::*};

use crate::{
    CloseSettings, OpenSettings, SETTINGS_CONTEXT, StocktakeView,
    settings::{SettingsEvent, SettingsState},
};

/// The window's size when it opens, and the least it shrinks to.
const SIZE: (f32, f32) = (600., 320.);

pub(crate) struct SettingsWindow {
    state: Entity<SettingsState>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl SettingsWindow {
    /// Centered on screen, with the same title bar as the counting window.
    /// Window geometry is a platform boundary, so physical pixels are
    /// intentional here.
    pub(crate) fn options(cx: &App) -> WindowOptions {
        let (width, height) = SIZE;
        WindowOptions {
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(width), px(height)),
                cx,
            ))),
            window_min_size: Some(size(px(width), px(height))),
            titlebar: Some(WindowBar::titlebar_options("Innstillinger")),
            app_owns_titlebar_drag: true,
            ..Default::default()
        }
    }

    /// `owner` is the counting window's view: with it gone, so is this.
    pub(crate) fn new(
        state: Entity<SettingsState>,
        owner: &Entity<StocktakeView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let subscriptions = vec![
            // The page shows the state, and the counting window may change it.
            cx.observe(&state, |_, _, cx| cx.notify()),
            // This window is a root of its own, so it shows its own toasts.
            cx.subscribe_in(&state, window, |_, _, event, window, cx| match event {
                SettingsEvent::SaveFailed(reason) => window.push_notification(
                    Notification::error(format!(
                        "Innstillingen ble ikke lagret: {reason}. Den går tapt hvis appen lukkes."
                    ))
                    .autohide(false),
                    cx,
                ),
            }),
            cx.observe_release_in(owner, window, |_, _, window, _| window.remove_window()),
        ];
        Self {
            state,
            focus_handle,
            _subscriptions: subscriptions,
        }
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let open_count_on_paste = state.read(cx).open_count_on_paste();
        v_flex()
            .size_full()
            .key_context(SETTINGS_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(|_: &CloseSettings, window, _| window.remove_window())
            // Already open, so it comes to the front.
            .on_action(|_: &OpenSettings, window, _| window.activate_window())
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(WindowBar::new().child(Heading::new("Innstillinger")))
            .child(
                v_flex()
                    .px_4()
                    .pb_4()
                    .gap_6()
                    .child(SectionHeading::new("Telling"))
                    .child(
                        SwitchField::new("open-count-on-paste", open_count_on_paste)
                            .label("Åpne telling ved innliming")
                            .description(
                                "Limer du inn en hel strekkode eller et helt varenummer i søket, \
                             åpnes tellingen for varen med en gang, som ved skanning.",
                            )
                            .on_change(move |open, _, cx| {
                                state.update(cx, |state, cx| {
                                    state.set_open_count_on_paste(*open, cx)
                                })
                            }),
                    ),
            )
    }
}
