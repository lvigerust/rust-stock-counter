//! Counting an already-counted product again, for example when more units
//! turn up at a second location.
//!
//! The dialog shows what each choice would produce while the counter types,
//! so "add" and "replace" are never a guess.

use gpui_kit::WeakEntity;
use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    input::{Input, InputState},
};
use ui::prelude::*;

use crate::{quantity::parse_quantity, stocktake_view::StocktakeView};

/// How a second count of an already-counted product combines with the first.
#[derive(Clone, Copy)]
pub(crate) enum Recount {
    Replace,
    Add,
}

impl Recount {
    pub fn apply(self, counted: i64, found: i64) -> i64 {
        match self {
            Self::Replace => found,
            Self::Add => counted + found,
        }
    }
}

/// Opens the dialog for a product already counted to `counted`. `input` is
/// the retained quantity field, owned by the view so it survives re-renders.
pub(crate) fn open(
    view: WeakEntity<StocktakeView>,
    input: Entity<InputState>,
    name: &str,
    counted: i64,
    window: &mut Window,
    cx: &mut App,
) {
    let title: SharedString = format!("«{name}» er allerede telt").into();
    let description: SharedString =
        format!("Telt antall er {counted}. Skriv hvor mange du har funnet nå.").into();

    window.open_dialog(cx, move |dialog, window, cx| {
        // Built again every frame, so the outcomes follow the typing.
        let found = parse_quantity(&input.read(cx).value());
        let on_choice = |recount: Recount| {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.apply_recount(recount, window, cx))
                    .ok();
            }
        };

        // Enter confirms the dialog, which means the primary choice: Add.
        // `apply_recount` closes the dialog itself once there's a valid
        // quantity, so the dialog must not close on its own here.
        let on_ok = {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.apply_recount(Recount::Add, window, cx))
                    .ok();
                false
            }
        };
        // Escape, × and Avbryt.
        let on_cancel = {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.dismiss_recount(window, cx))
                    .ok();
                true
            }
        };

        dialog
            .title(title.clone())
            .w(window.rem_size() * 28.)
            .on_ok(on_ok)
            .on_cancel(on_cancel)
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(description.clone()),
                    )
                    .child(Input::new(&input).id("recount"))
                    .child(render_outcomes(counted, found, cx)),
            )
            .footer(
                DialogFooter::new()
                    // `DialogClose` fills its container; this keeps Avbryt
                    // as wide as its label, like the buttons beside it.
                    .child(
                        div()
                            .flex_none()
                            .child(DialogClose::new().trigger(|button| button.label("Avbryt"))),
                    )
                    .child(
                        Button::new("recount-replace")
                            .label("Erstatt")
                            .disabled(found.is_none())
                            .on_click(on_choice(Recount::Replace)),
                    )
                    .child(
                        Button::new("recount-add")
                            .primary()
                            .label("Legg til")
                            .disabled(found.is_none())
                            .on_click(on_choice(Recount::Add)),
                    ),
            )
    });
}

/// What Add and Replace would each leave as the counted quantity. The lines
/// stay put while empty, so the dialog doesn't jump on the first keystroke.
fn render_outcomes(counted: i64, found: Option<i64>, cx: &App) -> impl IntoElement {
    let value = |recount: Recount| {
        found
            .map(|found| recount.apply(counted, found).to_string())
            .unwrap_or_else(|| "–".to_string())
    };
    let found_text = found
        .map(|found| found.to_string())
        .unwrap_or_else(|| "…".to_string());
    let outcome = |label: &'static str, sum: String, result: String| {
        h_flex()
            .gap_3()
            .child(
                div()
                    .w_20()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(cx.theme().muted_foreground)
                    .child(sum),
            )
            .child(div().font_semibold().child(result))
    };

    v_flex()
        .gap_1p5()
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        .text_sm()
        .tabular_nums()
        .child(outcome(
            "Legg til",
            format!("{counted} + {found_text} ="),
            value(Recount::Add),
        ))
        .child(outcome(
            "Erstatt",
            format!("{counted} forkastes"),
            value(Recount::Replace),
        ))
}
