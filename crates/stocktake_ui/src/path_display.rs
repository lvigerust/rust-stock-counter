//! Showing file paths to the counter.

use std::path::Path;

use gpui_kit::SharedString;

/// The file's name, or the whole path when it has none.
pub(crate) fn file_name(path: &Path) -> SharedString {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
        .into()
}

/// The folder a file is in, with the home folder shortened to `~`.
pub(crate) fn folder(path: &Path) -> SharedString {
    folder_under(path, dirs::home_dir().as_deref())
}

fn folder_under(path: &Path, home: Option<&Path>) -> SharedString {
    let Some(folder) = path.parent() else {
        return SharedString::default();
    };
    match home.and_then(|home| folder.strip_prefix(home).ok()) {
        Some(relative) if relative.as_os_str().is_empty() => "~".into(),
        Some(relative) => format!("~/{}", relative.display()).into(),
        None => folder.display().to_string().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_the_home_folder() {
        let home = Some(Path::new("/Users/lager"));
        let folder = |path: &str| folder_under(Path::new(path), home).to_string();
        assert_eq!(folder("/Users/lager/Vareliste.xlsx"), "~");
        assert_eq!(
            folder("/Users/lager/Downloads/Vareliste.xlsx"),
            "~/Downloads"
        );
        assert_eq!(folder("/Volumes/USB/Vareliste.xlsx"), "/Volumes/USB");
        assert_eq!(file_name(Path::new("/a/Vareliste.xlsx")), "Vareliste.xlsx");
    }
}
