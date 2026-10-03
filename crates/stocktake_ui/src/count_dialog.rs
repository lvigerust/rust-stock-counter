//! Counting one product: the dialog a scan, a search or a click on a row
//! opens. It takes the location and the quantity counted there.
//!
//! The dialog only collects them. What saving or cancelling does is the
//! caller's, passed in as callbacks, so this module knows nothing about the
//! view that opens it.

use std::{cell::Cell, rc::Rc};

use gpui_kit::ClickEvent;
use gpui_kit::component::{
    FocusableExt as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    description_list::{DescriptionItem, DescriptionList},
    dialog::{DialogClose, DialogFooter},
    input::{Input, InputState},
};
use stocktake::Product;
use ui::{Spacing, prelude::*};

/// The system quantity's label, in the dialog and as the table's column
/// header.
pub(crate) const SYSTEM_QUANTITY: &str = "I lagersystemet";

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

/// A single-line input for the location a count was made at, filled in with
/// the product's pick location.
pub(crate) fn location_input(
    product: &Product,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> InputState {
    InputState::new(window, cx)
        .placeholder("Lokasjon")
        .default_value(product.location().to_string())
}

/// What a save does with a count already made at the same location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Save {
    /// The new quantity takes the earlier one's place.
    Replace,
    /// The new quantity is added to the earlier one.
    Add,
}

/// What the dialog's buttons and keys do.
type Callback = Rc<dyn Fn(&mut Window, &mut App)>;
type SaveCallback = Rc<dyn Fn(Save, bool, &mut Window, &mut App)>;

/// Opens the dialog for `product`, with `location` (a [`location_input`])
/// and `input` (a [`quantity_input`], filled in by the caller) as its
/// fields.
///
/// `on_save` runs for Enter and the save buttons. It decides what the
/// quantity field's text means (a quantity, or perhaps a scan that landed in
/// the wrong field) and closes the dialog itself once it's done with it; the
/// buttons are only enabled while the field holds a quantity. While the
/// location already has a count, Enter adds to it, and a second button
/// replaces it instead. It's also told whether the location should become
/// the pick location: while the field holds another location and the pick
/// location can still be moved, a checkbox asks. `on_cancel` runs when
/// Escape or Avbryt close the dialog.
pub(crate) fn open(
    product: &Product,
    location: Entity<InputState>,
    input: Entity<InputState>,
    on_save: impl Fn(Save, bool, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let product = product.clone();
    let title: SharedString = product.name().to_string().into();
    let item_number = or_dash(product.item_number());
    let system_quantity: SharedString = product.system_quantity().to_string().into();
    // Only once the pick location itself has been counted.
    let pick_count = product.count_at(product.location());
    let on_save: SaveCallback = Rc::new(on_save);
    let on_cancel: Callback = Rc::new(on_cancel);
    // The checkbox's state, kept while it's hidden so it comes back as the
    // counter left it.
    let moves_pick_location = Rc::new(Cell::new(false));

    window.open_dialog(cx, move |dialog, window, cx| {
        // Built again every frame, so the buttons follow the typing.
        let valid = parse_quantity(&input.read(cx).value()).is_some();
        let typed_location = location.read(cx).value();
        let earlier = product.count_at(&typed_location);
        // What's left for the pick location once the buffer is counted.
        let expected = product
            .expected_at_pick_location()
            .filter(|_| product.is_pick_location(&typed_location));
        let offers_move =
            !product.is_pick_location(&typed_location) && product.can_move_pick_location();
        let moves = offers_move && moves_pick_location.get();
        let ring = cx.theme().ring;
        let surface = cx.theme().background;
        let muted = cx.theme().muted_foreground;
        let on_ok = {
            let on_save = on_save.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                on_save(Save::Add, moves, window, cx);
                // `on_save` closes the dialog when it's done with it.
                false
            }
        };
        let on_click = |save: Save| {
            let on_save = on_save.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                on_save(save, moves, window, cx)
            }
        };
        let save_button = |id: &'static str, label: &'static str| {
            Button::new(id)
                .rounded_lg()
                .accessibility_label(label)
                .child(div().text_sm().font_medium().child(label))
                .focus_ring(false)
                .solid_focus_ring(ring, surface)
                .disabled(!valid)
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
                            .item(SYSTEM_QUANTITY, system_quantity.clone(), 1),
                    )
                    .when_some(pick_count, |this, pick_count| {
                        this.child(location_counts(
                            "Lager",
                            [("Plukklokasjon", pick_count)].into_iter(),
                        ))
                    })
                    .when(product.overflow_len() > 0, |this| {
                        this.child(location_counts("Buffer", product.overflow_counts()))
                    })
                    .child(
                        v_flex()
                            .gap_3()
                            .child(div().text_sm().font_medium().child("Lokasjon"))
                            .child(Input::new(&location).id("location"))
                            .when(offers_move, |this| {
                                let moves_pick_location = moves_pick_location.clone();
                                this.child(
                                    Checkbox::new("move-pick-location")
                                        .label("Erstatt plukklokasjon")
                                        .small()
                                        .mt_1p5()
                                        .checked(moves)
                                        .on_click(move |checked, window, _| {
                                            moves_pick_location.set(*checked);
                                            window.refresh();
                                        }),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .mt_1p5()
                            .gap_3()
                            .child(div().text_sm().font_medium().child("Talt antall"))
                            .child(Input::new(&input).id("count"))
                            .when_some(earlier, |this, earlier| {
                                this.child(div().text_sm().text_color(muted).child(format!(
                                    "Allerede talt {earlier} her. Enter legger til."
                                )))
                            })
                            .when_some(expected, |this, expected| {
                                this.child(div().text_sm().text_color(muted).child(format!(
                                    "Forventet {expected} her, med det som er talt i buffer."
                                )))
                            }),
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
                    .map(|footer| match earlier {
                        None => footer.child(
                            save_button("save-count", "Lagre")
                                .primary()
                                .on_click(on_click(Save::Replace)),
                        ),
                        Some(_) => footer
                            .child(
                                save_button("replace-count", "Erstatt")
                                    .outline()
                                    .on_click(on_click(Save::Replace)),
                            )
                            .child(
                                save_button("add-count", "Legg til")
                                    .primary()
                                    .on_click(on_click(Save::Add)),
                            ),
                    }),
            )
            .p_8()
            .rounded_2xl()
            // Never wider than the window, less a margin; the dialog sees to
            // that itself.
            .w(Spacing(128.).to_pixels(window.rem_size()))
    });
}

/// A titled list of locations, each beside what was counted there.
fn location_counts<'a>(
    title: &'static str,
    counts: impl Iterator<Item = (&'a str, i64)>,
) -> impl IntoElement {
    v_flex()
        .gap_3()
        .child(div().text_sm().font_medium().child(title))
        .child(
            DescriptionList::new()
                .bordered(false)
                .columns(1)
                .children(counts.map(|(location, quantity)| DescriptionItem::Item {
                    label: or_dash(location).into(),
                    value: SharedString::from(quantity.to_string()).into(),
                    span: 1,
                })),
        )
}

/// A dash rather than an empty value, for a product the list leaves blank.
fn or_dash(text: &str) -> SharedString {
    match text {
        "" => "–".into(),
        text => text.to_string().into(),
    }
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
