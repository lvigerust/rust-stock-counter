//! The screen before any stock list is imported.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use gpui_kit::component::{alert::Alert, kbd::Kbd};
use gpui_kit::{Div, FocusHandle, MouseButton, Role, Stateful, rems};
use ui::prelude::*;

use crate::{APP_NAME, CONTEXT, ImportStockList};

/// How far apart the welcome's parts arrive, so they read top to bottom.
const STAGGER: Duration = Duration::from_millis(70);

/// The app's name over short lists of ways to start, like Zed's welcome:
/// open a stock list, drop one on the window, or reopen a recent one.
///
/// Importing is the owner's job; this only asks for it: through
/// [`ImportStockList`], the same action as the menu shortcut, or the
/// owner's handler for a recent stock list.
#[derive(IntoElement)]
pub(crate) struct Welcome {
    resume_error: Option<SharedString>,
    recent: Vec<PathBuf>,
    on_open_recent: Option<Rc<dyn Fn(&PathBuf, &mut Window, &mut App)>>,
}

impl Welcome {
    pub fn new() -> Self {
        Self {
            resume_error: None,
            recent: Vec::new(),
            on_open_recent: None,
        }
    }

    /// The stock lists imported before, most recent first, and what opens
    /// one. Without any, the section isn't shown.
    pub fn recent(
        mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        on_open: impl Fn(&PathBuf, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.recent = paths.into_iter().collect();
        self.on_open_recent = Some(Rc::new(on_open));
        self
    }

    /// Why the saved stocktake couldn't be resumed, shown above the welcome.
    pub fn resume_error(mut self, error: Option<SharedString>) -> Self {
        self.resume_error = error;
        self
    }
}

impl RenderOnce for Welcome {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .when_some(self.resume_error, |this, error| {
                this.child(Alert::error("resume-error", error))
            })
            .child(
                v_flex().flex_1().items_center().justify_center().child(
                    v_flex()
                        .w(rems(28.))
                        .gap_8()
                        .child(
                            Appear::new("welcome-title")
                                .px_2()
                                .text_2xl()
                                .font_semibold()
                                .child(APP_NAME),
                        )
                        .child(
                            Appear::new("welcome-start")
                                .delay(STAGGER)
                                .child(Self::render_start(window, cx)),
                        )
                        .when(!self.recent.is_empty(), |this| {
                            this.child(Appear::new("welcome-recent").delay(STAGGER * 2).child(
                                Self::render_recent(self.recent, self.on_open_recent, window, cx),
                            ))
                        }),
                ),
            )
    }
}

impl Welcome {
    /// "Kom igang": opening a stock list, and a reminder that dropping one on
    /// the window works too. The whole window accepts the drop.
    fn render_start(window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let shortcut = Kbd::binding_for_action(&ImportStockList, Some(CONTEXT), window)
            .map(|kbd| kbd.appearance(false));
        let open = Self::row(IconName::FolderOpen, "Åpne fil", cx)
            .children(shortcut.map(|kbd| div().text_color(muted).child(kbd)));

        Self::section("KOM IGANG", cx)
            .child(
                Self::button(open, "import-first", window, cx).on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(ImportStockList), cx)
                }),
            )
            .child(Self::row(IconName::FileDown, "Eller dra filen hit", cx).text_color(muted))
    }

    /// "Nylig åpnet": the stock lists imported before, each with the folder
    /// it's in. Opening one imports it again.
    fn render_recent(
        paths: Vec<PathBuf>,
        on_open: Option<Rc<dyn Fn(&PathBuf, &mut Window, &mut App)>>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let section = Self::section("NYLIG ÅPNET", cx);
        section.children(paths.into_iter().enumerate().map(|(ix, path)| {
            let row = Self::row(IconName::FileSpreadsheet, file_name(&path), cx)
                .child(div().flex_none().text_color(muted).child(folder(&path)));
            let on_open = on_open.clone();
            Self::button(row, ("open-recent", ix), window, cx).on_click(move |_, window, cx| {
                if let Some(on_open) = &on_open {
                    on_open(&path, window, cx);
                }
            })
        }))
    }

    /// A muted, uppercase label with a rule running to the trailing edge,
    /// over the rows the caller adds.
    fn section(label: &'static str, cx: &App) -> gpui_kit::Div {
        let theme = cx.theme();
        v_flex().gap_1().child(
            h_flex()
                .gap_3()
                .px_2()
                .pb_2()
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .font_medium()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(div().flex_1().h_px().bg(theme.border)),
        )
    }

    /// An icon and a label, the shape of every item in the list.
    fn row(icon: IconName, label: impl Into<SharedString>, cx: &App) -> Div {
        h_flex()
            .gap_3()
            .px_2()
            .py_1p5()
            .child(
                Icon::new(icon)
                    .small()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(div().flex_1().min_w_0().truncate().child(label.into()))
    }

    /// Makes a row a button: it lights up on hover, takes Tab like any
    /// control, and Enter or Space press it. `id` keeps its focus from frame
    /// to frame, so it must be unique in the window.
    fn button(
        row: Div,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        let id = id.into();
        let theme = cx.theme();
        let radius = theme.radius;
        let (hover_bg, hover_fg) = (theme.accent, theme.accent_foreground);
        let focus_handle = Self::focus_handle(id.clone(), window, cx);
        row.id(id)
            .role(Role::Button)
            .track_focus(&focus_handle)
            .tab_index(0)
            .rounded(radius)
            .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
            .subtle_focus_ring(cx)
            // A click shouldn't leave the focus ring behind.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
    }

    /// Keeps a row's focus from frame to frame, so Tab can land on it.
    fn focus_handle(id: ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
        window
            .use_keyed_state(id, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
    }
}

fn file_name(path: &Path) -> SharedString {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
        .into()
}

/// The folder a file is in, with the home folder shortened to `~`.
fn folder(path: &Path) -> SharedString {
    let Some(folder) = path.parent() else {
        return SharedString::default();
    };
    let home = dirs::home_dir();
    match home
        .as_deref()
        .and_then(|home| folder.strip_prefix(home).ok())
    {
        Some(relative) if relative.as_os_str().is_empty() => "~".into(),
        Some(relative) => format!("~/{}", relative.display()).into(),
        None => folder.display().to_string().into(),
    }
}
