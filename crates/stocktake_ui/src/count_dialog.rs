//! Counting one product: the dialog a scan, a search or a click on a row
//! opens. It takes the counted quantity.
//!
//! The dialog only collects the quantity. What saving or cancelling does is
//! the caller's, passed in as callbacks, so this module knows nothing about
//! the view that opens it.

use std::rc::Rc;

use gpui_kit::ClickEvent;
use gpui_kit::component::{
    FocusableExt as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    description_list::DescriptionList,
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
/// while the field holds a quantity. `on_cancel` runs when Escape or Avbryt
/// close the dialog.
pub(crate) fn open(
    product: &Product,
    input: Entity<InputState>,
    on_save: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let title: SharedString = product.name().to_string().into();
    // A dash rather than an empty value, for a product the list leaves blank.
    let or_dash = |text: &str| -> SharedString {
        match text {
            "" => "–".into(),
            text => text.to_string().into(),
        }
    };
    let location = or_dash(product.location());
    let item_number = or_dash(product.item_number());
    let system_quantity: SharedString = product.system_quantity().to_string().into();
    let on_save: Callback = Rc::new(on_save);
    let on_cancel: Callback = Rc::new(on_cancel);

    window.open_dialog(cx, move |dialog, window, cx| {
        // Built again every frame, so Lagre follows the typing.
        let valid = parse_quantity(&input.read(cx).value()).is_some();
        let ring = cx.theme().ring;
        let surface = cx.theme().background;
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
            // Avbryt and Escape already cancel. The × would be one more Tab
            // stop, and gpui-kit gives it no way to show focus here.
            .close_button(false)
            // Just over a fifth of the way down the window, worked out each
            // frame so it follows a resize.
            .margin_top(window.viewport_size().height / 4.5)
            .on_ok(on_ok)
            .on_cancel(on_cancel)
            .child(
                v_flex()
                    .mt_6()
                    .gap_6()
                    .child(
                        DescriptionList::new()
                            .bordered(false)
                            .columns(1)
                            .item("Varenummer", item_number.clone(), 1)
                            .item("Lokasjon", location.clone(), 1)
                            .item("På lager", system_quantity.clone(), 1),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .child(div().text_sm().font_medium().child("Telt antall"))
                            .child(Input::new(&input).id("count")),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .gap_3()
                    // `DialogClose` fills its container; this keeps Avbryt
                    // as wide as its label, like the button beside it.
                    // Neither button has a border for gpui-kit to recolor
                    // on focus, so each draws the blue ring itself.
                    .child(
                        div()
                            .flex_none()
                            .child(DialogClose::new().trigger(|button| {
                                button
                                    .ghost()
                                    .rounded_lg()
                                    .accessibility_label("Avbryt")
                                    .child(div().text_sm().font_medium().child("Avbryt"))
                                    .focus_ring(false)
                                    .solid_focus_ring(ring, surface)
                            })),
                    )
                    .child(
                        Button::new("save-count")
                            .primary()
                            .rounded_lg()
                            .accessibility_label("Lagre")
                            .child(div().text_sm().font_medium().child("Lagre"))
                            .focus_ring(false)
                            .solid_focus_ring(ring, surface)
                            .disabled(!valid)
                            .on_click(on_click_save),
                    ),
            )
            .p_8()
            .rounded_2xl()
            // Never wider than the window, less a margin; the dialog sees to
            // that itself.
            .w(window.rem_size() * 32.)
    });
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
