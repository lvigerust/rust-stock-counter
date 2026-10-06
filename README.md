# Scala Bad

A desktop app for the year-end stocktake at Scala Bad. A counter imports the stock list exported from the business system as an Excel file, scans each product's barcode, confirms or corrects how many units are on the shelf, and exports the counted list back to Excel. Every count is saved the moment it's made.

Written in Rust on [GPUI](https://www.gpui.rs/) through [gpui-kit](https://gpui-kit.com). The interface is Norwegian; the code and docs use the English terms in [CONTEXT.md](CONTEXT.md).

## Documents

| Document | What it holds |
| --- | --- |
| [CONTEXT.md](CONTEXT.md) | The domain vocabulary: the words the code uses, and the ones it avoids |
| [docs/spec.md](docs/spec.md) | The product: what the app does and what's still undecided |
| [docs/architecture.md](docs/architecture.md) | How the code is laid out: crates, state, actions, files, focus, tests |
| [docs/design-system.md](docs/design-system.md) | The visual language the `ui` crate encodes: type roles, spacing, corners, buttons, focus, dialogs |
| [docs/inspiration.md](docs/inspiration.md) | The projects the interface takes after |
| [AGENTS.md](AGENTS.md) | Instructions for coding agents working in this repository |

## Building and running

```sh
cargo run                    # the app (the default workspace member)
cargo test --workspace       # every crate's tests; a plain `cargo test` runs only the shell's
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The UI tests open headless windows, so nothing appears on screen. `scripts/bundle-macos.sh` builds the release executable, wraps it in `Scala Bad.app` and installs it in `/Applications`. The Windows executable is built by the `Windows build` GitHub Actions workflow, which can't be cross-compiled from macOS: after CI passes on a push to `main`, it publishes a release tagged `vMAJOR.MINOR.PATCH` (MAJOR.MINOR from the version in `Cargo.toml`, PATCH counting up), and it can also be run from the Actions tab.

## Layout

```text
crates/
├── stocktake/      the domain: products, counting rules, import, export, saving (no UI)
├── stocktake_ui/   the counting window: the feature's model, workflow, screens and dialogs
├── ui/             the design layer on top of gpui-kit: motion, typography, small components
└── varetelling/    the shell: main, window options, theme, menu bar
data/               the sample stock list the import tests read
docs/               the spec, the architecture and the inspiration notes
scripts/            bundling the macOS app
```
