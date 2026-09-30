use std::cmp::Ordering;

use crate::prelude::*;

/// A signed change between two quantities: `+2`, `−3` or `0`.
///
/// Zero is quiet. Any other value is set on a warning tint, because a change
/// is something to look at. The sign carries the direction, so meaning never
/// depends on color alone.
#[derive(IntoElement)]
pub struct Delta {
    value: i64,
}

impl Delta {
    pub fn new(value: i64) -> Self {
        Self { value }
    }
}

impl RenderOnce for Delta {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text = match self.value.cmp(&0) {
            Ordering::Greater => format!("+{}", self.value),
            // A real minus sign, as wide as the plus, rather than a hyphen.
            Ordering::Less => format!("\u{2212}{}", self.value.unsigned_abs()),
            Ordering::Equal => "0".to_string(),
        };
        div()
            .tabular_nums()
            .map(|this| {
                if self.value == 0 {
                    this.text_color(cx.theme().muted_foreground)
                } else {
                    this.px_1p5()
                        .rounded_sm()
                        .bg(cx.theme().warning.opacity(0.18))
                        .font_medium()
                }
            })
            .child(text)
    }
}
