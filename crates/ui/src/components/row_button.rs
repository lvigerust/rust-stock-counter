use std::rc::Rc;

use gpui_kit::{ClickEvent, Hsla, MouseButton, Role, StyleRefinement, TestSupportExt as _};

use crate::prelude::*;

/// What a press on a [`RowButton`] does.
type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A row of content that is itself a button, such as an item in a list of
/// files: it lights up on hover, takes Tab like any control, Enter or Space
/// press it, and focus shows as [`StyledFocus::subtle_focus_ring`].
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let row = h_flex()
            .id(self.id.clone())
            .min_w_0()
            .children(self.children)
            .refine_style(&self.style);
        if self.disabled {
            return row.opacity(0.5).into_any_element();
        }

        // A tracked handle decides for itself whether Tab reaches it: the
        // element's own `tab_index` doesn't apply to it. Keyed by the row's
        // id, so the Tab order holds from frame to frame.
        let focus_handle = window
            .use_keyed_state(self.id, cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let theme = cx.theme();
        let (hover_bg, hover_fg) = self
            .hover_colors
            .unwrap_or((theme.accent, theme.accent_foreground));
        let on_click = self.on_click;
        row.role(Role::Button)
            // Lets UI tests find the row and see its focus. Does nothing
            // outside gpui-kit's `test-support` feature.
            .test_support()
            .track_focus(&focus_handle)
            .rounded(theme.radius)
            .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
            .subtle_focus_ring()
            // A click shouldn't leave the focus ring behind.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .into_any_element()
    }
}
