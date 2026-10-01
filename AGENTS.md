# Agent instructions

## Grilling skill

When using the grilling skill, ask questions with the AskUserQuestion tool instead of listing many questions in a single message. Ask one question (or a small, related batch) at a time and wait for the answer before moving on.

## Building and testing

Never launch the application. Don't run `cargo run`, `cargo build`, `scripts/bundle-macos.sh` (it also installs into `/Applications`), or open the built app. The user builds and runs it themselves; when you're done, say so.

Do check the code you write, with commands that compile and test it without opening the app:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (a plain `cargo test` only runs the shell crate). The UI tests use headless windows, so nothing appears on screen.

Report which of these passed, and show the output of any that failed.
