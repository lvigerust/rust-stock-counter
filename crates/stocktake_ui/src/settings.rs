//! The counter's settings as the windows see them: one entity both the
//! counting window and the settings window read, so a change shows in each
//! at once. The values and their file belong to [`stocktake::settings`].

use std::path::PathBuf;

use gpui_kit::EventEmitter;
use stocktake::settings::{self, Settings};
use ui::prelude::*;

/// The settings, saved the moment one changes, as a count is.
pub(crate) struct SettingsState {
    settings: Settings,
    /// Where [`Self::settings`] is saved.
    path: PathBuf,
}

pub(crate) enum SettingsEvent {
    /// A change is in effect but couldn't be written, so it's lost at the
    /// next launch. Carries the reason.
    SaveFailed(SharedString),
}

impl EventEmitter<SettingsEvent> for SettingsState {}

impl SettingsState {
    /// The settings saved at `path`. Without them, or if the file is
    /// damaged, the defaults apply: a setting isn't worth stopping the
    /// counter for.
    pub(crate) fn load(path: PathBuf) -> Self {
        Self {
            settings: settings::load(&path).unwrap_or_default(),
            path,
        }
    }

    pub(crate) fn open_count_on_paste(&self) -> bool {
        self.settings.open_count_on_paste()
    }

    pub(crate) fn set_open_count_on_paste(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.settings.open_count_on_paste() == open {
            return;
        }
        self.settings.set_open_count_on_paste(open);
        cx.notify();
        if let Err(error) = settings::save(&self.path, &self.settings) {
            cx.emit(SettingsEvent::SaveFailed(error.to_string().into()));
        }
    }
}
