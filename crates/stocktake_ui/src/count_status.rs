use ui::prelude::*;

/// Whether a product has been counted, as an icon and a word.
///
/// Lives here rather than in `ui` because "counted" is a stocktake word. The
/// icon shape differs as well as its color, so the state reads without color.
///
/// Uncounted is the state that needs work, so it keeps full-strength text;
/// counted steps back to muted text once its check mark has said so.
#[derive(IntoElement)]
pub(crate) struct CountStatus {
    counted: bool,
}

impl CountStatus {
    pub fn new(counted: bool) -> Self {
        Self { counted }
    }

    /// The status in words, as shown and as copied from the table.
    pub fn label(counted: bool) -> &'static str {
        if counted { "Telt" } else { "Ikke telt" }
    }
}

impl RenderOnce for CountStatus {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (icon, color) = if self.counted {
            (IconName::CircleCheck, cx.theme().success)
        } else {
            (IconName::CircleDashed, cx.theme().muted_foreground)
        };
        h_flex()
            .gap_2p5()
            .when(self.counted, |this| {
                this.text_color(cx.theme().muted_foreground)
            })
            .child(Icon::new(icon).small().text_color(color))
            .child(Self::label(self.counted))
    }
}
