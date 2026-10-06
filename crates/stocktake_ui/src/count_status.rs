use stocktake::CountState;
use ui::prelude::*;

/// How far a product has been counted, as an icon and a word.
///
/// Lives here rather than in `ui` because "counted" is a stocktake word. The
/// icon shape differs as well as its color, so the state reads without color.
///
/// Uncounted and partly counted are the states that need work, so they keep
/// full-strength text; counted and finished step back to muted text once
/// their check marks have said so.
#[derive(IntoElement)]
pub(crate) struct CountStatus {
    state: CountState,
}

impl CountStatus {
    pub fn new(state: CountState) -> Self {
        Self { state }
    }

    /// The status in words, as shown and as copied from the table.
    pub fn label(state: CountState) -> &'static str {
        match state {
            CountState::Uncounted => "Ikke talt",
            CountState::PartlyCounted => "Delvis talt",
            CountState::Counted => "Talt",
            CountState::Finished => "Ferdig talt",
        }
    }
}

impl RenderOnce for CountStatus {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (icon, color) = match self.state {
            CountState::Uncounted => (IconName::CircleDashed, cx.theme().muted_foreground),
            CountState::PartlyCounted => (IconName::CircleDotDashed, cx.theme().warning),
            CountState::Counted => (IconName::CircleCheck, cx.theme().success),
            CountState::Finished => (IconName::CheckCheck, cx.theme().success),
        };
        let done = matches!(self.state, CountState::Counted | CountState::Finished);
        h_flex()
            .gap_2p5()
            .when(done, |this| this.text_color(cx.theme().muted_foreground))
            .child(Icon::new(icon).small().text_color(color))
            .child(Self::label(self.state))
    }
}
