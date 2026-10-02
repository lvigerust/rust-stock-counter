//! A button that opens a menu below it, and the parts it's composed from,
//! after Catalyst's:
//!
//! ```ignore
//! Dropdown::new(
//!     "mode-menu",
//!     DropdownButton::new().child(label).child(chevron),
//!     DropdownMenu::new()
//!         .item(DropdownItem::new("Varetelling").checked(true).action(Box::new(ShowCounting)))
//!         .divider()
//!         .item(DropdownItem::new("Differanse").action(Box::new(ShowDifferences))),
//! )
//! ```
//!
//! A button standing on its own in a bar, rather than heading a column of
//! rows, is outlined, and can open its menu under its trailing edge:
//!
//! ```ignore
//! Dropdown::new(
//!     "columns",
//!     DropdownButton::new().outline().small().child(icon).child("Kolonner"),
//!     DropdownMenu::new().anchor(Anchor::TopRight).heading("Vis kolonner"),
//! )
//! ```
//!
//! The menu itself is gpui-kit's `PopupMenu`, which brings the arrow keys,
//! Escape and focus going back to the button. The parts here only
//! describe what goes in it, so the menu's parts are data rather than
//! elements: gpui-kit builds the menu afresh each time it opens.
//!
//! Each item is a row of our own inside gpui-kit's, padded like Catalyst's.
//! gpui-kit's own rows have a fixed height that can't take that padding.
//!
//! gpui-kit keeps an open menu as it was built. When the items change while
//! it's open, say a shortcut moves the check, [`Dropdown`] builds it again.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::FocusableExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Colorize as _, Size};
use gpui_kit::{
    AbsoluteLength, Action, Anchor, ClickEvent, FocusHandle, Hsla, Pixels, StyleRefinement,
    TestSupportExt as _, Toggled, WeakEntity,
};

use crate::Spacing;
use crate::prelude::*;

/// What a press on a [`DropdownItem`] does, besides its action.
type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A [`DropdownButton`] and the [`DropdownMenu`] it opens, under it.
#[derive(IntoElement)]
pub struct Dropdown {
    id: ElementId,
    button: DropdownButton,
    menu: DropdownMenu,
}

impl Dropdown {
    /// `id` keeps the button's focus and whether the menu is open from
    /// frame to frame, so it must be unique in the window.
    pub fn new(id: impl Into<ElementId>, button: DropdownButton, menu: DropdownMenu) -> Self {
        Self {
            id: id.into(),
            button,
            menu,
        }
    }
}

/// What a [`Dropdown`] keeps from frame to frame.
struct DropdownState {
    /// Where the menu's actions go, and where their shortcuts are looked up:
    /// a handle on the wrapper, so in the key context of whatever holds the
    /// dropdown. It's drawn before the menu ever opens, as GPUI needs to
    /// find its key context. It doesn't take focus: the button covers it,
    /// and a click focuses the button first.
    context: FocusHandle,
    /// The menu while it's open. gpui-kit owns it and drops it on closing.
    open_menu: WeakEntity<PopupMenu>,
    /// Whether the menu is open, to fill the button as it is on hover.
    open: bool,
    /// What the open menu was built from, to tell when it's out of date.
    built_from: Vec<EntryLook>,
}

impl RenderOnce for Dropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state_id = ElementId::NamedChild(Arc::new(self.id.clone()), "state".into());
        let state = window.use_keyed_state(state_id, cx, |_, cx| DropdownState {
            context: cx.focus_handle(),
            open_menu: WeakEntity::new_invalid(),
            open: false,
            built_from: Vec::new(),
        });
        let (context, open) = {
            let state = state.read(cx);
            (state.context.clone(), state.open)
        };
        let DropdownMenu {
            entries,
            min_width,
            anchor,
        } = self.menu;
        let min_width = min_width.map(|width| width.to_pixels(window.rem_size()));
        let entries: Rc<[MenuEntry]> = entries.into();
        let looks: Vec<EntryLook> = entries.iter().map(MenuEntry::look).collect();

        // Opened, and showing something else than it's asked to now: built
        // again after this frame, as nothing may change while it's drawn.
        let stale_menu = {
            let state = state.read(cx);
            state
                .open_menu
                .upgrade()
                .filter(|_| state.built_from != looks)
        };
        if let Some(menu) = stale_menu {
            let (state, entries, context) = (state.clone(), entries.clone(), context.clone());
            let looks = looks.clone();
            window.defer(cx, move |window, cx| {
                menu.update(cx, |menu, cx| {
                    menu.rebuild(window, cx, |menu, _, _| {
                        build_menu(menu, &entries, min_width, &context)
                    })
                });
                state.update(cx, |state, _| state.built_from = looks);
            });
        }

        let state = state.downgrade();
        let button = self.button.into_button(self.id, open, cx);
        div()
            .track_focus(&context)
            // A press on the open button closes the menu as it goes down, and
            // the button, no longer open but still held, would take
            // gpui-kit's darker pressed fill until it's let go. Keeping the
            // press from counting as one keeps the hover fill instead; the
            // menu still closes, as gpui-kit doesn't check.
            .when(open, |this| {
                this.capture_any_mouse_down(|_, window, _| window.prevent_default())
            })
            .child(
                button
                    .dropdown_menu_with_anchor(anchor, {
                        let state = state.clone();
                        move |menu, _, cx| {
                            let open_menu = cx.weak_entity();
                            state
                                .update(cx, |state, _| {
                                    state.open_menu = open_menu;
                                    state.built_from = looks.clone();
                                })
                                .ok();
                            build_menu(menu, &entries, min_width, &context)
                        }
                    })
                    .on_open_change(move |open, _, cx| {
                        state
                            .update(cx, |state, cx| {
                                state.open = *open;
                                cx.notify();
                            })
                            .ok();
                    }),
            )
    }
}

/// Fills gpui-kit's menu with `entries`, its actions going to `context`.
fn build_menu(
    menu: PopupMenu,
    entries: &[MenuEntry],
    min_width: Option<Pixels>,
    context: &FocusHandle,
) -> PopupMenu {
    let menu = menu
        .action_context(context.clone())
        .when_some(min_width, |menu, width| menu.min_w(width));
    // Labels line up: with an icon or a check on any item, every item
    // keeps the space for one.
    let leading = entries.iter().any(|entry| match entry {
        MenuEntry::Item(item) => item.icon.is_some() || item.checked,
        MenuEntry::Heading(_) | MenuEntry::Divider => false,
    });
    entries.iter().fold(menu, |menu, entry| match entry {
        MenuEntry::Item(item) => menu.item(item.to_popup_item(leading, context)),
        MenuEntry::Heading(label) => menu.item(heading_popup_item(label.clone())),
        MenuEntry::Divider => menu.separator(),
    })
}

/// The entry gpui-kit's menu draws for a heading: muted, smaller text,
/// inset like the items under it. gpui-kit's own heading is inset less, so
/// it wouldn't line up with them.
fn heading_popup_item(label: SharedString) -> PopupMenuItem {
    PopupMenuItem::element(move |_, cx| {
        div()
            .px_1()
            .pt_2()
            .pb_1()
            .text_xs()
            .font_medium()
            .text_color(cx.theme().muted_foreground)
            .child(label.clone())
    })
    // Not something to pick: the arrow keys pass over it.
    .disabled(true)
}

/// What opens a [`Dropdown`]: a ghost button, its children side by side.
/// The caller sets its width and padding, so it can line up with the rows
/// around it.
pub struct DropdownButton {
    style: StyleRefinement,
    children: Vec<AnyElement>,
    outline: bool,
    size: Size,
    accessibility_label: Option<SharedString>,
}

impl DropdownButton {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
            outline: false,
            size: Size::default(),
            accessibility_label: None,
        }
    }

    /// Outlined, for a button standing on its own, such as in a bar. It
    /// keeps gpui-kit's height and padding for its size, and its focus
    /// look, which recolors the outline.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// What assistive tech names the button, when its children aren't
    /// just its label.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// gpui-kit fills a button whose menu is open with another color than
    /// a hovered one, so moving onto the menu would change it. This is the
    /// hover fill, which gpui-kit keeps to itself, for the open button too.
    fn hover_fill(&self, cx: &App) -> Hsla {
        let theme = cx.theme();
        if self.outline {
            theme.input.mix_oklab(theme.transparent, 0.5)
        } else if theme.is_dark() {
            theme.accent.opacity(0.5)
        } else {
            theme.accent
        }
    }

    fn into_button(self, id: ElementId, open: bool, cx: &App) -> Button {
        let hover_fill = self.hover_fill(cx);
        Button::new(id)
            .with_size(self.size)
            .map(|button| {
                if self.outline {
                    button.outline()
                } else {
                    // gpui-kit would recolor a border for focus; this draws
                    // a faint ring around the button instead.
                    button
                        .ghost()
                        .h_auto()
                        .focus_ring(false)
                        .subtle_focus_ring()
                }
            })
            .when_some(self.accessibility_label, |button, label| {
                button.accessibility_label(label)
            })
            .refine_style(&self.style)
            .when(open, |button| button.bg(hover_fill))
            // gpui-kit centers a button's content; this row runs its full
            // width, so the caller can push a child to the trailing edge.
            .child(h_flex().w_full().min_w_0().gap_2().children(self.children))
    }
}

impl Default for DropdownButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Sizable for DropdownButton {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for DropdownButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for DropdownButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// What a [`Dropdown`] opens: its items, in order, with headings over and
/// dividers between groups of them.
pub struct DropdownMenu {
    entries: Vec<MenuEntry>,
    min_width: Option<AbsoluteLength>,
    anchor: Anchor,
}

enum MenuEntry {
    Item(Box<DropdownItem>),
    Heading(SharedString),
    Divider,
}

/// What a [`MenuEntry`] shows, to compare one frame's menu with another's.
/// Its icon, action and click handler are left out: they're the same for
/// as long as the item is the same item.
#[derive(Clone, PartialEq)]
enum EntryLook {
    Item {
        label: SharedString,
        description: Option<SharedString>,
        checked: bool,
        disabled: bool,
    },
    Heading(SharedString),
    Divider,
}

impl MenuEntry {
    fn look(&self) -> EntryLook {
        match self {
            MenuEntry::Item(item) => EntryLook::Item {
                label: item.label.clone(),
                description: item.description.clone(),
                checked: item.checked,
                disabled: item.disabled,
            },
            MenuEntry::Heading(label) => EntryLook::Heading(label.clone()),
            MenuEntry::Divider => EntryLook::Divider,
        }
    }
}

impl Default for DropdownMenu {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            min_width: None,
            anchor: Anchor::TopLeft,
        }
    }
}

impl DropdownMenu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Which of the menu's corners sits under the button's matching one:
    /// `TopLeft`, the default, lines it up with the button's leading edge,
    /// and `TopRight` with its trailing edge, for a button at the end of a
    /// bar.
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// A muted label over the items after it, naming what they choose.
    pub fn heading(mut self, label: impl Into<SharedString>) -> Self {
        self.entries.push(MenuEntry::Heading(label.into()));
        self
    }

    pub fn item(mut self, item: DropdownItem) -> Self {
        self.entries.push(MenuEntry::Item(Box::new(item)));
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = DropdownItem>) -> Self {
        self.entries.extend(
            items
                .into_iter()
                .map(|item| MenuEntry::Item(Box::new(item))),
        );
        self
    }

    /// The narrowest the menu gets, however short its items. Without it,
    /// the menu is as wide as its widest item.
    pub fn min_w(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// A thin rule between the items before it and those after.
    pub fn divider(mut self) -> Self {
        self.entries.push(MenuEntry::Divider);
        self
    }
}

/// One choice in a [`DropdownMenu`]: a label, and optionally an icon before
/// it and a muted line under it. Picking it dispatches its action, calls its
/// click handler, or both.
pub struct DropdownItem {
    label: SharedString,
    description: Option<SharedString>,
    icon: Option<Icon>,
    checked: bool,
    disabled: bool,
    action: Option<Box<dyn Action>>,
    on_click: Option<ClickHandler>,
}

impl DropdownItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
            icon: None,
            checked: false,
            disabled: false,
            action: None,
            on_click: None,
        }
    }

    /// A second, muted line under the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// An icon before the label. The menu lines up every item's label,
    /// with an icon or without.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Marks the item with a check, such as the option currently chosen.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// The action picking the item dispatches. The menu shows its shortcut.
    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        self.action = Some(action);
        self
    }

    /// What picking the item does, for one that has no action.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// The entry gpui-kit's menu draws for this item: a row of our own,
    /// which keeps the menu's action and keyboard handling. With `leading`,
    /// it starts with a slot for its icon or check.
    ///
    /// gpui-kit would draw the icon, check and shortcut centered on the
    /// whole row, between label and description, and the shortcut only in
    /// its own rows. This row draws them itself, on the label's line, the
    /// shortcut found in `context`'s key context the way gpui-kit finds it.
    fn to_popup_item(&self, leading: bool, context: &FocusHandle) -> PopupMenuItem {
        let label = self.label.clone();
        let description = self.description.clone();
        let action = self.action.as_ref().map(|action| action.boxed_clone());
        let context = context.clone();
        let checked = self.checked;
        // The slot shows the icon, or the check where there's none; an item
        // with both shows its check at the trailing edge instead.
        let slot_icon = self
            .icon
            .clone()
            .or_else(|| checked.then(|| Icon::new(IconName::Check)));
        let trailing_check = checked && self.icon.is_some();
        PopupMenuItem::element(move |window, cx| {
            let muted = cx.theme().muted_foreground;
            let shortcut = action
                .as_ref()
                .and_then(|action| Kbd::binding_for_action_in(action.as_ref(), &context, window))
                .map(|kbd| kbd.appearance(false));
            // gpui-kit names its own rows to assistive tech, but not one
            // like this: it carries its label and check itself. Only one
            // menu is open at a time, so the label keeps the id unique.
            h_flex()
                .id(SharedString::from(format!("menu-item:{label}")))
                .aria_label(label.clone())
                .aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .test_support()
                .flex_1()
                .min_w_0()
                .items_start()
                .gap_3()
                // Catalyst's padding: gpui-kit pads its row 8px at the sides,
                // so this adds 4px to each. The row grows to fit; gpui-kit
                // only sets its least height.
                .px_1()
                .py_1p5()
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_start()
                        .gap_2()
                        .when(leading, |this| {
                            this.child(
                                // As wide as the icon whether it holds one
                                // or not, so an empty slot still takes
                                // its space.
                                label_line()
                                    .flex_none()
                                    .w_4()
                                    .text_color(muted)
                                    .children(slot_icon.clone().map(|icon| icon.small())),
                            )
                        })
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(label_line().child(label.clone()))
                                .when_some(description.clone(), |this, description| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .line_height(Spacing(5.))
                                            .text_color(muted)
                                            .child(description),
                                    )
                                }),
                        ),
                )
                .when(trailing_check, |this| {
                    this.child(label_line().child(Icon::new(IconName::Check).small()))
                })
                .children(shortcut.map(|kbd| label_line().flex_none().text_color(muted).child(kbd)))
        })
        .disabled(self.disabled)
        .when_some(self.action.as_ref(), |item, action| {
            item.action(action.boxed_clone())
        })
        .when_some(self.on_click.clone(), |item, on_click| {
            item.on_click(move |event, window, cx| on_click(event, window, cx))
        })
    }
}

/// As tall as an item's label, its content centered on it: what sits beside
/// the label lines up with it, however many lines come under it.
fn label_line() -> gpui_kit::Div {
    h_flex().h(Spacing(6.)).line_height(Spacing(6.))
}

/// A disabled item is dimmed and can't be picked.
impl Disableable for DropdownItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
