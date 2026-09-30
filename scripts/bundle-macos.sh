#!/usr/bin/env bash
# Builds the release executable and wraps it in `Scala Bad.app`, under
# target/release/bundle. The bundle's Info.plist is the one build.rs embeds
# in the executable, with the version taken from the crate.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

cargo build --release --locked -p varetelling

app="target/release/bundle/Scala Bad.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cp target/release/varetelling "$app/Contents/MacOS/varetelling"
cp crates/varetelling/Info.plist "$app/Contents/Info.plist"

pkgid="$(cargo pkgid -p varetelling)"
version="${pkgid##*[#@]}"
/usr/libexec/PlistBuddy \
    -c "Set :CFBundleShortVersionString $version" \
    -c "Set :CFBundleVersion $version" \
    "$app/Contents/Info.plist"

# An ad-hoc signature, so Apple silicon Macs will launch the bundle. It
# isn't notarized, so another Mac asks before opening it the first time.
codesign --force --sign - "$app"

echo "Built $app"
