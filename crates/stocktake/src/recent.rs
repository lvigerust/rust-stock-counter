//! The stock lists imported before, so one can be opened again without
//! looking for it.

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::store;

/// How many stock lists are remembered. A stocktake is yearly, so a few
/// cover every list that's still worth opening.
pub const LIMIT: usize = 5;

/// Stock list files, the most recently imported first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecentStockLists(Vec<PathBuf>);

impl RecentStockLists {
    pub fn iter(&self) -> impl Iterator<Item = &Path> {
        self.0.iter().map(PathBuf::as_path)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Puts the stock list first. One imported before moves up instead of
    /// being listed twice, and the oldest goes past [`LIMIT`].
    pub fn add(&mut self, path: PathBuf) {
        self.remove(&path);
        self.0.insert(0, path);
        self.0.truncate(LIMIT);
    }

    pub fn remove(&mut self, path: &Path) {
        self.0.retain(|recent| recent != path);
    }
}

/// Where the recent stock lists are kept: beside the stocktake in progress.
pub fn path_beside(store_path: &Path) -> PathBuf {
    store_path.with_file_name("recent.json")
}

/// Loads the recent stock lists, empty when none are saved.
pub fn load(path: &Path) -> io::Result<RecentStockLists> {
    Ok(store::read_json(path)?.unwrap_or_default())
}

/// Saves the recent stock lists. A crash mid-write leaves the previous save
/// intact.
pub fn save(path: &Path, recent: &RecentStockLists) -> io::Result<()> {
    store::write_json(path, recent)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn paths(recent: &RecentStockLists) -> Vec<&str> {
        recent.iter().map(|path| path.to_str().unwrap()).collect()
    }

    #[test]
    fn newest_first_without_repeats() {
        let mut recent = RecentStockLists::default();
        recent.add("a.xlsx".into());
        recent.add("b.xlsx".into());
        recent.add("a.xlsx".into());
        assert_eq!(paths(&recent), ["a.xlsx", "b.xlsx"]);

        recent.remove(Path::new("a.xlsx"));
        assert_eq!(paths(&recent), ["b.xlsx"]);
    }

    #[test]
    fn keeps_only_the_newest() {
        let mut recent = RecentStockLists::default();
        for ix in 0..LIMIT + 2 {
            recent.add(format!("{ix}.xlsx").into());
        }
        assert_eq!(recent.iter().count(), LIMIT);
        let newest = PathBuf::from(format!("{}.xlsx", LIMIT + 1));
        assert_eq!(recent.iter().next(), Some(newest.as_path()));
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("stocktake-recent-{}", std::process::id()));
        let path = path_beside(&dir.join("varetelling.json"));
        assert!(load(&path).unwrap().is_empty());

        let mut recent = RecentStockLists::default();
        recent.add("/Users/lager/Vareliste.xlsx".into());
        save(&path, &recent).unwrap();

        assert_eq!(load(&path).unwrap(), recent);
        fs::remove_dir_all(dir).ok();
    }
}
