use std::rc::Rc;

use gpui_kit::component::FocusableExt as _;
use gpui_kit::component::button::{ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::{ClickEvent, Hsla, StyleRefinement, phi};

use crate::Button;
use crate::prelude::*;

/// What a press on a [`RowButton`] does.
type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A row of content that is itself a button, such as an item in a list of
/// files: a [`Button`] that lights up on hover, takes Tab like any control,
/// Enter or Space press it, and focus shows as
/// [`StyledFocus::subtle_focus_ring`].
///
/// Lays its children out side by side; the caller sets the padding and gap,
/// so a pressable row can line up with plain ones around it.
///
/// ```ignore
/// RowButton::new(("open-recent", ix), |_, window, cx| open(window, cx))
///     .gap_3()
///     .px_2()
///     .child(Icon::new(IconName::FileSpreadsheet))
///     .child(file_name)
/// ```
#[derive(IntoElement)]
pub struct RowButton {
    id: ElementId,
    on_click: ClickHandler,
    hover_colors: Option<(Hsla, Hsla)>,
    disabled: bool,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl RowButton {
    /// `id` keeps the row's focus from frame to frame, so it must be unique
    /// in the window and stable for what the row stands for.
    pub fn new(
        id: impl Into<ElementId>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            on_click: Rc::new(on_click),
            hover_colors: None,
            disabled: false,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }

    /// The fill and text color on hover. Defaults to the theme's accent;
    /// rows on another surface, such as the sidebar, pass its accent.
    pub fn hover_colors(mut self, background: Hsla, foreground: Hsla) -> Self {
        self.hover_colors = Some((background, foreground));
        self
    }
}

/// A disabled row is dimmed, and ignores clicks and Tab.
impl Disableable for RowButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for RowButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for RowButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for RowButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (hover_bg, hover_fg) = self
            .hover_colors
            .unwrap_or((theme.accent, theme.accent_foreground));
        let on_click = self.on_click;
        Button::new(self.id)
            .custom(
                ButtonCustomVariant::new(cx)
                    .hover(hover_bg)
                    .active(hover_bg)
                    .foreground(hover_fg),
            )
            // A row of content, not a command: as tall as its padding and
            // lines make it, its children from the leading edge, in the
            // weight and line height of the plain rows around it.
            .min_w_0()
            .h_auto()
            .p_0()
            .justify_start()
            .gap_0()
            .font_normal()
            .line_height(phi())
            // gpui-kit would recolor a border for focus; this draws a faint
            // ring around the row instead.
            .focus_ring(false)
            .subtle_focus_ring()
            .disabled(self.disabled)
            // The button fades a disabled row, but can't read a custom
            // variant's colors to keep them; these are the row's at rest.
            .when(self.disabled, |row| {
                row.bg(theme.transparent).text_color(hover_fg)
            })
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .children(self.children)
            .refine_style(&self.style)
    }
}
