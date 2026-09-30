use gpui_kit::StyleRefinement;

use crate::{Surface, prelude::*};

/// A content region on the page: a plain surface with a hairline border.
///
/// The card owns its surface, not its padding, so a table can run edge to
/// edge while a form pads itself. Give it a [`CardHeader`] for a title row
/// with the same insets everywhere.
///
/// ```ignore
/// Card::new()
///     .header(CardHeader::new("Varer").description("43 i varelisten"))
///     .child(div().p_5().child(content))
/// ```
#[derive(IntoElement)]
pub struct Card {
    style: StyleRefinement,
    header: Option<CardHeader>,
    children: Vec<AnyElement>,
}

impl Card {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            header: None,
            children: Vec::new(),
        }
    }

    pub fn header(mut self, header: CardHeader) -> Self {
        self.header = Some(header);
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .bg(Surface::Card.bg(cx))
            .border_1()
            .border_color(cx.theme().border)
            .rounded_xl()
            // Children such as a table's header row follow the rounded corners.
            .overflow_hidden()
            .refine_style(&self.style)
            .children(self.header)
            .children(self.children)
    }
}

/// A card's title row: a title, an optional muted description beneath it,
/// and an optional action at the trailing edge.
#[derive(IntoElement)]
pub struct CardHeader {
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
}

impl CardHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            action: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for CardHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_4()
            .px_5()
            .pt_5()
            .pb_4()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().font_semibold().child(self.title))
                    .when_some(self.description, |this, description| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(description),
                        )
                    }),
            )
            .children(self.action)
    }
}
