//! The counter's settings, kept beside the stocktake so they survive a
//! restart and a new stocktake alike.

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::store;

/// What the counter can change. A setting missing from the saved file, such
/// as one added after it was written, takes its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    open_count_on_paste: bool,
}

impl Settings {
    /// Whether pasting a whole barcode or item number into the search opens
    /// the count dialog at once, as a scan does. On by default: the dialog
    /// only opens for an exact match.
    pub fn open_count_on_paste(&self) -> bool {
        self.open_count_on_paste
    }

    pub fn set_open_count_on_paste(&mut self, open: bool) {
        self.open_count_on_paste = open;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            open_count_on_paste: true,
        }
    }
}

/// Where the settings are kept: beside the stocktake in progress.
pub fn path_beside(store_path: &Path) -> PathBuf {
    store_path.with_file_name("settings.json")
}

/// Loads the settings, the defaults when none are saved.
pub fn load(path: &Path) -> io::Result<Settings> {
    Ok(store::read_json(path)?.unwrap_or_default())
}

/// Saves the settings. A crash mid-write leaves the previous save intact.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    store::write_json(path, settings)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn opens_the_count_on_paste_until_turned_off() {
        assert!(Settings::default().open_count_on_paste());
    }

    #[test]
    fn a_file_without_the_setting_takes_the_default() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("stocktake-settings-{}", std::process::id()));
        let path = path_beside(&dir.join("varetelling.json"));
        assert_eq!(load(&path).unwrap(), Settings::default());

        let mut settings = Settings::default();
        settings.set_open_count_on_paste(false);
        save(&path, &settings).unwrap();

        assert_eq!(load(&path).unwrap(), settings);
        fs::remove_dir_all(dir).ok();
    }
}
