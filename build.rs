//! Embeds every font file in `assets/fonts` into the binary, so the
//! application ships with them.

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/fonts");
    // Cargo rescans a directory's contents, so adding or removing a font rebuilds.
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut paths: Vec<_> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf"))
        })
        .collect();
    paths.sort();

    let entries: String = paths
        .iter()
        .map(|path| format!("    include_bytes!({:?}),\n", path.display().to_string()))
        .collect();
    let source = format!("pub static FONTS: &[&[u8]] = &[\n{entries}];\n");
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("fonts.rs"),
        source,
    )
    .unwrap();
}
