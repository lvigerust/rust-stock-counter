//! The screen before any stock list is imported.

use std::{path::PathBuf, rc::Rc, time::Duration};

use gpui_kit::Div;
use gpui_kit::component::{alert::Alert, kbd::Kbd};
use ui::{RowButton, Spacing, prelude::*};

use crate::{
    CONTEXT, ImportStockList,
    path_display::{file_name, folder},
};

/// How far apart the welcome's parts arrive, so they read top to bottom.
const STAGGER: Duration = Duration::from_millis(70);

/// Opens one of the recent stock lists.
type OpenRecent = Rc<dyn Fn(&PathBuf, &mut Window, &mut App)>;

/// The logo over short lists of ways to start, like Zed's welcome:
/// open a stock list, drop one on the window, or reopen a recent one.
///
/// Importing is the owner's job; this only asks for it: through
/// [`ImportStockList`], the same action as the menu shortcut, or the
/// owner's handler for a recent stock list.
#[derive(IntoElement)]
pub(crate) struct Welcome {
    resume_error: Option<SharedString>,
    recent: Vec<PathBuf>,
    on_open_recent: Option<OpenRecent>,
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
        let recent = self
            .on_open_recent
            .filter(|_| !self.recent.is_empty())
            .map(|on_open| render_recent(self.recent, on_open, cx));
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
                        .w(Spacing(112.))
                        .gap_8()
                        .child(
                            Appear::new("welcome-title")
                                .flex()
                                .justify_center()
                                .child(ui::Logo::new(Spacing(14.))),
                        )
                        .child(
                            Appear::new("welcome-start")
                                .delay(STAGGER)
                                .child(render_start(window, cx)),
                        )
                        .when_some(recent, |this, recent| {
                            this.child(
                                Appear::new("welcome-recent")
                                    .delay(STAGGER * 2)
                                    .child(recent),
                            )
                        }),
                ),
            )
    }
}

/// "Kom igang": opening a stock list, and a reminder that dropping one on
/// the window works too. The whole window accepts the drop.
fn render_start(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let shortcut = Kbd::binding_for_action(&ImportStockList, Some(CONTEXT), window)
        .map(|kbd| kbd.appearance(false));
    let open = RowButton::new("import-first", |_, window, cx| {
        window.dispatch_action(Box::new(ImportStockList), cx)
    })
    .map(|row| row_content(row, IconName::FolderOpen, "Åpne fil", cx))
    .children(shortcut.map(|kbd| div().text_color(muted).child(kbd)));

    section("KOM IGANG", cx).child(open).child(
        row_content(h_flex(), IconName::FileDown, "Eller dra filen hit", cx).text_color(muted),
    )
}

/// "Nylig åpnet": the stock lists imported before, each with the folder
/// it's in. Opening one imports it again.
fn render_recent(paths: Vec<PathBuf>, on_open: OpenRecent, cx: &App) -> Div {
    let muted = cx.theme().muted_foreground;
    section("NYLIG ÅPNET", cx).children(paths.into_iter().map(|path| {
        let label = file_name(&path);
        let folder = folder(&path);
        // Keyed by the file, so focus stays with it as the list reorders.
        let id = SharedString::from(format!("open-recent:{}", path.display()));
        let on_open = on_open.clone();
        RowButton::new(id, move |_, window, cx| on_open(&path, window, cx))
            .map(|row| row_content(row, IconName::FileSpreadsheet, label, cx))
            .child(div().flex_none().text_color(muted).child(folder))
    }))
}

/// A muted, uppercase label with a rule running to the trailing edge, over
/// the rows the caller adds.
fn section(label: &'static str, cx: &App) -> Div {
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

/// An icon and a label, the shape of every row in the welcome, whether it's
/// a button or not.
fn row_content<E: Styled + ParentElement>(
    row: E,
    icon: IconName,
    label: impl Into<SharedString>,
    cx: &App,
) -> E {
    row.gap_3()
        .px_2()
        .py_1p5()
        .child(
            Icon::new(icon)
                .small()
                .text_color(cx.theme().muted_foreground),
        )
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
}
