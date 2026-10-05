# Agent instructions

## Grilling skill

When using the grilling skill, ask questions with the AskUserQuestion tool instead of listing many questions in a single message. Ask one question (or a small, related batch) at a time and wait for the answer before moving on.

## Language

Code is English; the interface is Norwegian. Name types, variants, functions, actions and comments with the English terms in `CONTEXT.md` (`Mode::Differences`, not `Mode::Differanse`). Norwegian appears only in strings the user sees.

## Component reference

`ui` components follow Catalyst's blocks and styling: a component is composed from named parts (`Sidebar`, `SidebarHeader`, `SidebarItem`, …), as Catalyst's are. Its source is at `~/Developer/Tailwind Plus/Catalyst/src/components`. It's licensed, so translate its patterns; don't copy its source into the repo. Where Catalyst and the gpui-kit Design Guides disagree, the guides win: Catalyst is a web kit, and its breakpoints, forced-colors rules and blue focus fill don't carry over to a desktop app.

`docs/design-system.md` records the decisions the `ui` crate encodes. In a view, name a type role (`Heading`, `Label`, `Text`, `SectionHeading`) and compose parts (`Field`, `Button`, `Badge`, …) rather than writing sizes, weights, radii or focus rings at the call site; add a part to `ui` when none fits.

## Building and testing

Never launch the application. Don't run `cargo run`, `cargo build`, `scripts/bundle-macos.sh` (it also installs into `/Applications`), or open the built app. The user builds and runs it themselves; when you're done, say so.

Do check the code you write, with commands that compile and test it without opening the app:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (a plain `cargo test` only runs the shell crate). The UI tests use headless windows, so nothing appears on screen.

Report which of these passed, and show the output of any that failed.
