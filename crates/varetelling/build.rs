//! On macOS, embeds `Info.plist`, which names the application in the menu
//! bar.

use std::{env, path::Path};

fn main() {
    embed_info_plist();
}

/// Without an app bundle, macOS names the application in the menu bar after
/// the executable. A property list in the binary's `__info_plist` section
/// is read as the bundle's, so its `CFBundleName` is shown instead.
fn embed_info_plist() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let plist = Path::new(&manifest_dir).join("Info.plist");
    println!("cargo:rerun-if-changed={}", plist.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!(
            "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}
