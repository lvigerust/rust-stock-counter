# Reference: tty7's settings window

Notes on how [tty7](https://github.com/l0ng-ai/tty7) builds its settings, kept as a reference for building something similar here later. Read from tty7's `main` branch on 2026-10-01.

Most of this was read directly in tty7's source and is quoted or described from it. Two parts couldn't be read because the files are too long for the tools used: the function that opens the window, and the code that draws the navigation column. Those are marked _inferred_ below.

**Contents:** [What it is](#what-it-is) · [The pieces](#the-pieces) · [Lifecycle](#lifecycle) · [The page](#the-page) · [Lessons tty7 learned](#lessons-tty7-learned) · [Adapting it to this app](#adapting-it-to-this-app)

## What it is

tty7's settings are **not a modal dialog. They open in a second window** beside the workspace. The module's doc comment gives the reason:

> The settings page used to cover the workspace it was opened from, which hid the terminal a setting was being tried out on. It now opens beside it.

So changing a setting (a font, a color theme) shows its effect live in the main window while settings stay open.

tty7 builds on its own fork of gpui-component 0.5 and a pinned Zed GPUI, not gpui-kit. The structure carries over; some APIs differ (see [Adapting it to this app](#adapting-it-to-this-app)).

## The pieces

| Piece | File | Role |
| --- | --- | --- |
| `window_options()` | `src/ui/settings_window.rs` | How the second window looks and sizes |
| `SettingsWindow` | `src/ui/settings_window.rs` | A thin view: renders the app's settings page, handles close |
| `SettingsState` | `src/ui/settings.rs` | All settings UI state: section, search, unsaved changes, forms |
| `Tty7App::settings`, `Tty7App::settings_window` | `src/ui/app.rs` | Where that state and the window handle live |
| Page renderers | `src/ui/settings/pages.rs`, `shell.rs`, `hosts.rs`, … | One renderer per section |
| Control kit | `src/ui/settings/kit.rs` | The page's own switches, buttons, fields, steppers |

### The window

```rust
const DEFAULT_SIZE: (f32, f32) = (960., 700.);
const MIN_SIZE: (f32, f32) = (720., 480.);

pub(crate) fn window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(DEFAULT_SIZE.0), px(DEFAULT_SIZE.1)),
            cx,
        ))),
        app_id: Some("tty7".to_owned()),
        titlebar: Some(TitlebarOptions {
            // Not drawn (the title bar is the window's own), but it is the
            // name the Window menu, Mission Control and VoiceOver give it.
            title: Some(crate::ui::i18n::t(crate::ui::i18n::L10nKey::SettingsWindowTitle).into()),
            traffic_light_position: Some(crate::ui::theme::traffic_light_position()),
            ..TitleBar::title_bar_options()
        }),
        window_decorations: Some(WindowDecorations::Client),
        window_background: crate::ui::theme::background_appearance(cx),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        ..Default::default()
    }
}
```

It's an ordinary window: centered and resizable, drawing its own title bar with the traffic lights placed like the main window's. Its title is never drawn, but macOS uses it in the Window menu, Mission Control and VoiceOver.

### The view

`SettingsWindow` owns no settings state. It holds a weak handle to the app and two subscriptions:

```rust
pub(crate) struct SettingsWindow {
    pub(crate) app: WeakEntity<Tty7App>,
    _observe: Subscription,
    _release: Subscription,
}

pub(crate) fn new(app: &Entity<Tty7App>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    // Every settings change notifies the app, not this view; without this
    // the page would change underneath and never repaint here.
    let observe = cx.observe(app, |_, _, cx| cx.notify());
    // The workspace that owns the page is gone, so the page is too.
    let release = cx.observe_release_in(app, window, |_, _, window, _| window.remove_window());
    // The close button asks the same question Escape does: a half-edited
    // form or theme draft gets its prompt, and the window only goes once
    // it is answered.
    let weak = app.downgrade();
    window.on_window_should_close(cx, move |window, cx| match weak.upgrade() {
        Some(app) => {
            app.update(cx, |this, cx| this.close_settings_checked(window, cx));
            false
        }
        None => true,
    });
    Self {
        app: app.downgrade(),
        _observe: observe,
        _release: release,
    }
}
```

Its `render` asks the app to draw the page into this window:

```rust
impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The workspace window sets this in its own render; a second window
        // has to, or the page lays out against the default 16px rem.
        window.set_rem_size(px(cx.global::<Config>().ui_font_size));
        crate::ui::settings::kit::note_scale(window);
        let page = self.app.upgrade().map(|app| {
            let page = app.update(cx, |this, cx| {
                // The state can be gone for a frame between closing and the
                // window being removed.
                this.has_settings()
                    .then(|| this.render_settings(window, cx).into_any_element())
            });
            if page.is_none() {
                // Closed without passing through `close_settings` — nothing
                // is left to draw, so the window goes too.
                app.update(cx, |this, cx| this.forget_settings_window(cx));
                window.remove_window();
            }
            page
        });
        gpui::div()
            .size_full()
            .bg(crate::ui::theme::overlay_background(cx))
            .text_color(cx.theme().foreground)
            // ⌘W closes this window the way it closes a tab in a workspace.
            .on_action(cx.listener(|this, _: &CloseActiveTab, window, cx| this.close(window, cx)))
            // ⌘, lands here while this window has focus; it is already open.
            .on_action(cx.listener(|_, _: &OpenSettings, window, _| window.activate_window()))
            .when_some(page.flatten(), |root, page| root.child(page))
            // This window is a `Root` of its own, and a `Root` only shows the
            // toasts it is asked to draw: without this, everything the page
            // reports — an ssh_config import, a passphrase it could not store
            // — was pushed into a layer nobody rendered.
            .children(gpui_component::Root::render_notification_layer(window, cx))
    }
}
```

### Where the state lives

On the app entity, not on the window:

```rust
settings: Option<SettingsState>,
/// The window settings are drawn in, and the workspace window that opened
/// it (where focus goes back to on close). `None` while settings is shut —
/// and always in tests, which keep drawing settings over the workspace so
/// they can drive it through the one test window they have.
pub(crate) settings_window: Option<(gpui::AnyWindowHandle, gpui::AnyWindowHandle)>,
```

`settings` being `Some` means "settings are open". The window is only a place to draw them. That's why the view checks `has_settings()` each frame and removes itself once the state is gone.

## Lifecycle

```text
⌘, (OpenSettings)
  └─► app: settings already open? ──yes──► activate that window          (inferred)
                                   └─no──► settings = Some(SettingsState::…)
                                           open_window(window_options, SettingsWindow::new)
                                           settings_window = Some((settings, workspace))

while open
  a setting changes ──► app notifies ──► SettingsWindow observes it ──► re-renders the page
  ⌘, in the settings window ──► activate_window (it's already open)

closing: ⌘W, Escape, or the window's close button
  └─► close_settings_checked: unsaved form or theme draft? ──► ask first
      └─► settings = None ──► next frame: has_settings() is false ──► remove_window
          focus returns to the workspace window kept in settings_window   (inferred)

the workspace (app entity) is released ──► observe_release_in ──► remove_window
```

The open step is _inferred_ from the fields, the `OpenSettings` action and `window_options()`. The function that calls `open_window` wasn't readable.

## The page

`SettingsState` (`src/ui/settings.rs`) holds everything the page needs:

```rust
pub(crate) struct SettingsState {
    pub(crate) focus_handle: gpui::FocusHandle,
    pub(crate) section: SettingsSection,
    pub(crate) search: Entity<InputState>,
    pub(crate) shortcut_search: Entity<InputState>,
    pub(crate) modified_only: bool,
    pub(crate) save_error: Option<String>,
    pub(crate) saved_config: Config,
    pub(crate) search_active: bool,
    // … scroll state, search results, SSH forms, theme editing,
    // mobile pairing, agent hooks
}

pub(crate) enum SettingsSection {
    General, Appearance, Terminal, KeyboardMouse, Ssh, Mobile, Agents,
    Keybindings, About,
}
```

- **Two columns.** A navigation list of sections on the left, 220 px wide and narrowing to 176 px before its labels hide (`NAV_W`, `NAV_W_MIN`). The selected section's content is on the right, with paragraphs capped at 620 px (`READING_COLUMN`) and at least 420 px of content width (`CONTENT_W`). Each `SettingsSection` has a title and an icon. How the list itself is drawn is _inferred_.
- **One renderer per section**: `render_settings_general`, `render_settings_appearance`, `render_settings_terminal`, `render_settings_input`, `render_settings_about` in `pages.rs`, with larger sections in their own files (`hosts.rs`, `shortcuts.rs`, `theme_picker.rs`, …). Each builds rows with helpers such as `settings_row`, `settings_switch` and `settings_choice`.
- **Search and "modified only"** filter the rows across sections. `navigate_settings(section, setting)` jumps to a section, clears the search, turns off "modified only" and reveals the given setting. Other parts of the app can use it to deep-link into settings.
- **Unsaved changes.** `saved_config` is the last saved configuration, so the page can tell what changed. Forms (SSH profiles, theme drafts) prompt before closing.

### The control kit

`src/ui/settings/kit.rs` is a small set of controls used only on the settings page. Its doc comment:

> Settings is drawn to one design, and that design is quieter than the rest of the app: ink at a handful of fixed strengths instead of the theme's named roles, hairline rings instead of borders, and controls that sit on the page rather than on cards.

It provides `switch`, `button` (secondary, primary, link, danger), `keycap`, `search_field`, `text_field`, `section_head`, `select_trigger` with `menu_panel`, `stepper` and `slider`, plus `Tk`, the page's color strengths. A group heading, for example:

```rust
pub(crate) fn section_head(title: &str, desc: Option<String>, tk: &Tk) -> Div {
    v_flex()
        .gap(px(3.))
        .pb(px(8.))
        .child(
            div()
                .text_size(fs(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk.heading)
                .child(title.to_string()),
        )
        .when_some(desc.filter(|d| !d.is_empty()), |col, desc| {
            col.child(
                div()
                    .text_size(fs(12.))
                    .line_height(px(17.))
                    .text_color(tk.k6)
                    .child(desc),
            )
        })
}
```

## Lessons tty7 learned

These come from comments in tty7's code, and each applies to any second window:

1. **Each window has its own rem size.** The second window must set it in its own `render`, or it lays out against the default 16 px.
2. **Each window is its own `Root`.** Toasts pushed while settings had focus went into a layer that window never drew. In tty7's gpui-component fork the window has to render the notification layer itself.
3. **The close button must ask the same question as Escape.** `on_window_should_close` returns `false` and routes to the same unsaved-changes check, so clicking × can't lose a half-edited form.
4. **State can disappear a frame before the window does.** The view handles `has_settings() == false` by removing the window instead of panicking or drawing nothing.
5. **The open command must work from both windows.** In the settings window, ⌘, just brings it to the front.
6. **The window dies with its owner.** `observe_release_in` on the app removes the settings window if the workspace goes away.
7. **Tests can't easily drive two windows.** In tests tty7 draws settings over the workspace in its single test window. That's possible because the window is only a drawing surface for state that lives elsewhere.

## Adapting it to this app

How the same design would map onto this codebase (gpui-kit 0.7; see [architecture.md](architecture.md)). This section is a plan, not working code.

**First decide: window or dialog.** tty7 chose a window so settings don't hide what they affect. This app's window is maximized and used for one task. If a setting doesn't need its effect seen live behind it, a gpui-kit dialog or sheet is simpler and avoids most of the lessons above. Use a window when that live preview matters.

If it's a window:

| tty7 | Here |
| --- | --- |
| `cx.open_window(window_options(cx), …)` | `gpui_kit::open_window(options, cx, \|window, cx\| …)`. It wraps the view in gpui-kit's `Root`, which in 0.7 mounts the dialog, sheet and notification layers in every window itself (`gpui-component-0.7.0/src/root.rs`). Lesson 2's workaround isn't needed |
| `TitlebarOptions { traffic_light_position, .. }` | `ui::WindowBar::titlebar_options(title)` with `app_owns_titlebar_drag: true`, as in `crates/varetelling/src/main.rs`, and a `ui::WindowBar` at the top of the view |
| Theme per window | Call `theme::follow_system_appearance(window, cx)` for the new window too (`crates/varetelling/src/theme.rs`) |
| `settings: Option<SettingsState>` on the app | A settings model entity owned by `StocktakeView` or the shell, like `Session` (`crates/stocktake_ui/src/session.rs`). Values the domain needs belong in `crates/stocktake`, with a JSON store like `recent.rs` |
| `settings_window: Option<(AnyWindowHandle, AnyWindowHandle)>` | The same: keep the settings window's handle to activate it instead of opening a second one, and the main window's handle to return focus |
| `OpenSettings` on ⌘, | An action in `stocktake_ui.rs` bound to `secondary-,` in `init`. The menu item goes in the app menu in `crates/varetelling/src/menus.rs`, where macOS expects Settings… |
| `observe`, `observe_release_in`, `on_window_should_close`, `activate_window`, `remove_window`, `set_rem_size` | All exist in the GPUI this app uses (`gpui-pre` 0.3.7) |
| `kit.rs` controls | Prefer gpui-kit's own `Switch`, `Select`, `Input`, `Button` and `Checkbox`, and add a shared look to `crates/ui` only where they fall short |
| Tests draw settings inline | Keep the settings state outside the window view, so a test in `stocktake_view/tests.rs` can render the page in the one test window |

Whichever surface you choose, keep tty7's two structural choices: **settings state lives outside the view that draws it**, and **every way of closing goes through one checked close**.
