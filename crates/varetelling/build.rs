//! Embeds every font file in `assets/fonts` into the binary, so the
//! application ships with them, and on macOS embeds `Info.plist`, which
//! names the application in the menu bar.

use std::{env, fs, path::Path};

fn main() {
    embed_info_plist();
    embed_fonts();
}

/// Without an app bundle, macOS names the application in the menu bar after
/// the executable. A property list in the binary's `__info_plist` section
/// is read as the bundle's, so its `CFBundleName` is shown instead.
fn embed_info_plist() {
    let plist = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("Info.plist");
    println!("cargo:rerun-if-changed={}", plist.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!(
            "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}

fn embed_fonts() {
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
