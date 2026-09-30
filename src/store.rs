//! Keeps the stocktake in progress on disk, so closing the app loses nothing.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::stocktake::Stocktake;

/// Where the stocktake in progress lives. Only one exists at a time.
pub fn default_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Varetelling")
        .join("varetelling.json")
}

/// Loads the saved stocktake, `None` when there isn't one.
pub fn load(path: &Path) -> io::Result<Option<Stocktake>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Saves by writing a temporary file and renaming it over the old one, so a
/// crash mid-write leaves the previous save intact.
pub fn save(path: &Path, stocktake: &Stocktake) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(stocktake).map_err(io::Error::other)?,
    )?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stocktake::Product;

    #[test]
    fn saves_and_resumes() {
        let dir = std::env::temp_dir().join(format!("stocktake-store-{}", std::process::id()));
        let path = dir.join("varetelling.json");
        assert_eq!(load(&path).unwrap(), None);

        let mut stocktake = Stocktake::new(vec![Product::new("1", "Vare", "A1", "", 5)]);
        let id = stocktake.search("")[0];
        stocktake.set_counted_quantity(id, 0);
        save(&path, &stocktake).unwrap();

        assert_eq!(load(&path).unwrap(), Some(stocktake));
        fs::remove_dir_all(dir).ok();
    }
}
