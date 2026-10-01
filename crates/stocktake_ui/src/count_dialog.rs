//! Counting one product: the dialog a scan, a search or a click on a row
//! opens. It shows what the system and any earlier count say, and takes the
//! counted quantity.
//!
//! The dialog only collects the quantity. What saving or cancelling does is
//! the caller's, passed in as callbacks, so this module knows nothing about
//! the view that opens it.

use std::rc::Rc;

use gpui_kit::ClickEvent;
use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    input::{Input, InputState},
};
use stocktake::Product;
use ui::prelude::*;

/// A single-line input that accepts whole, non-negative numbers.
pub(crate) fn quantity_input(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    InputState::new(window, cx)
        .placeholder("Antall")
        .validate(|text, _| text.chars().all(|c| c.is_ascii_digit()))
}

/// The quantity typed into a [`quantity_input`], `None` while it's empty or
/// too large to be a quantity.
pub(crate) fn parse_quantity(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

/// What the dialog's buttons and keys do.
type Callback = Rc<dyn Fn(&mut Window, &mut App)>;

/// Opens the dialog for `product`, with `input` (a [`quantity_input`], filled
/// in by the caller) as its quantity field.
///
/// `on_save` runs for Enter and Lagre. It decides what the field's text
/// means (a quantity, or perhaps a scan that landed in the wrong field) and
/// closes the dialog itself once it's done with it; Lagre is only enabled
/// while the field holds a quantity. `on_cancel` runs when Escape, × or
/// Avbryt close the dialog.
pub(crate) fn open(
    product: &Product,
    input: Entity<InputState>,
    on_save: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
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
    let on_save: Callback = Rc::new(on_save);
    let on_cancel: Callback = Rc::new(on_cancel);

    window.open_dialog(cx, move |dialog, window, cx| {
        // Built again every frame, so Lagre follows the typing.
        let valid = parse_quantity(&input.read(cx).value()).is_some();
        let on_ok = {
            let on_save = on_save.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                on_save(window, cx);
                // `on_save` closes the dialog when it's done with it.
                false
            }
        };
        let on_click_save = {
            let on_save = on_save.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| on_save(window, cx)
        };
        let on_cancel = {
            let on_cancel = on_cancel.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                on_cancel(window, cx);
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
                            .on_click(on_click_save),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whole_quantities_only() {
        assert_eq!(parse_quantity("12"), Some(12));
        assert_eq!(parse_quantity(" 0 "), Some(0));
        assert_eq!(parse_quantity(""), None);
        // A scanned barcode can be longer than any quantity.
        assert_eq!(parse_quantity("99999999999999999999"), None);
    }
}
