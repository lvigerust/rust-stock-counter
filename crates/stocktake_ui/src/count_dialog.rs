//! Counting one product: the dialog a scan, a search or a click on a row
//! opens. It shows what the system and any earlier count say, and takes the
//! counted quantity, which replaces the earlier count.

use gpui_kit::WeakEntity;
use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    input::{Input, InputState},
};
use stocktake::Product;
use ui::prelude::*;

use crate::{quantity::parse_quantity, stocktake_view::StocktakeView};

/// Opens the dialog for `product`. `input` is the retained quantity field,
/// owned by the view so it survives re-renders, and already filled in.
pub(crate) fn open(
    view: WeakEntity<StocktakeView>,
    input: Entity<InputState>,
    product: &Product,
    window: &mut Window,
    cx: &mut App,
) {
    let title: SharedString = product.name().to_string().into();
    let details: SharedString = [product.item_number(), product.location()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
        .into();
    let system = product.system_quantity();
    let counted = product.counted_quantity();

    window.open_dialog(cx, move |dialog, window, cx| {
        // Built again every frame, so Lagre follows the typing.
        let valid = parse_quantity(&input.read(cx).value()).is_some();
        // Enter and Lagre. `save_count` closes the dialog itself once
        // there's a valid quantity, so the dialog must not close on its own.
        let on_ok = {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.save_count(window, cx)).ok();
                false
            }
        };
        let on_save = {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.save_count(window, cx)).ok();
            }
        };
        // Escape, × and Avbryt.
        let on_cancel = {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.dismiss_count(window, cx))
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
                            .child(details.clone()),
                    )
                    .child(render_quantities(system, counted, cx))
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(div().text_sm().font_medium().child("Telt antall"))
                            .child(Input::new(&input).id("count")),
                    ),
            )
            .footer(
                DialogFooter::new()
                    // `DialogClose` fills its container; this keeps Avbryt
                    // as wide as its label, like the button beside it.
                    .child(
                        div()
                            .flex_none()
                            .child(DialogClose::new().trigger(|button| button.label("Avbryt"))),
                    )
                    .child(
                        Button::new("save-count")
                            .primary()
                            .label("Lagre")
                            .disabled(!valid)
                            .on_click(on_save),
                    ),
            )
    });
}

/// The system quantity, and the earlier count if there is one.
fn render_quantities(system: i64, counted: Option<i64>, cx: &App) -> impl IntoElement {
    let line = |label: &'static str, value: String| {
        h_flex()
            .justify_between()
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(div().font_medium().child(value))
    };

    v_flex()
        .gap_1p5()
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        .text_sm()
        .tabular_nums()
        .child(line("På lager", system.to_string()))
        .child(line(
            "Telt",
            counted.map_or_else(|| "–".to_string(), |counted| counted.to_string()),
        ))
}
