//! The frame every dialog shares, after Catalyst's `Dialog` and `Alert`.

use crate::prelude::*;

pub trait StyledDialog: Styled + Sized {
    /// A dialog with work in it, such as fields: 2rem of padding, and the
    /// theme's `xl` corners, a tier up from the controls inside it.
    ///
    /// gpui-kit's dialog spaces its header, body and footer by the top
    /// padding, so the footer sits as far from the fields as Catalyst's
    /// `DialogActions` does.
    fn dialog_frame(self, cx: &App) -> Self {
        self.p_8().rounded(cx.theme().radius_tokens().xl)
    }

    /// An alert holding one short decision: the same corners, 1.5rem of
    /// padding, as Catalyst's `Alert` is tighter than its `Dialog`.
    fn alert_frame(self, cx: &App) -> Self {
        self.p_6().rounded(cx.theme().radius_tokens().xl)
    }
}

impl<E: Styled> StyledDialog for E {}
