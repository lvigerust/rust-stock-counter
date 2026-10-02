//! Keeps the stocktake in progress on disk, so closing the app loses nothing.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::Stocktake;

/// Where the stocktake in progress lives. Only one exists at a time.
pub fn default_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Varetelling")
        .join("varetelling.json")
}

/// Loads the saved stocktake, `None` when there isn't one.
pub fn load(path: &Path) -> io::Result<Option<Stocktake>> {
    read_json(path)
}

/// Saves the stocktake. A crash mid-write leaves the previous save intact.
pub fn save(path: &Path, stocktake: &Stocktake) -> io::Result<()> {
    write_json(path, stocktake)
}

/// Reads a value saved with [`write_json`], `None` when there isn't one.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Writes a value as JSON to a temporary file, then renames it over the old
/// one, so a crash mid-write leaves the previous save intact.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(value).map_err(io::Error::other)?,
    )?;
    fs::rename(&temporary, path)
}

/// Deletes the saved stocktake, so the next start has none to resume.
pub fn clear(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Product;

    #[test]
    fn saves_and_resumes() {
        let dir = std::env::temp_dir().join(format!("stocktake-store-{}", std::process::id()));
        let path = dir.join("varetelling.json");
        assert_eq!(load(&path).unwrap(), None);

        let mut stocktake = Stocktake::new(vec![Product::new("1", "Vare", "A1", "", 5)]);
        let id = stocktake.search("")[0];
        stocktake.set_count(id, "", 0);
        save(&path, &stocktake).unwrap();

        assert_eq!(load(&path).unwrap(), Some(stocktake));

        clear(&path).unwrap();
        assert_eq!(load(&path).unwrap(), None);
        // Clearing when there's nothing saved isn't an error.
        clear(&path).unwrap();
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_damaged_save_is_an_error_not_a_missing_one() {
        let dir = std::env::temp_dir().join(format!("stocktake-damaged-{}", std::process::id()));
        let path = dir.join("varetelling.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{\"products\": [").unwrap();

        assert!(load(&path).is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_save_from_before_locations_resumes_at_the_pick_location() {
        let saved = r#"{"products":[{"item_number":"1","name":"Vare","location":"A1",
            "barcode":"","system_quantity":5,"counted_quantity":4}]}"#;
        let stocktake: Stocktake = serde_json::from_str(saved).unwrap();
        let (_, product) = stocktake.products().next().unwrap();
        assert_eq!(product.count_at("A1"), Some(4));
        assert_eq!(product.counted_quantity(), Some(4));
    }
}
