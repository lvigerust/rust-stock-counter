# Architecture

How the code is organised, and why. The layout borrows from the [Zed editor](https://github.com/zed-industries/zed/tree/main/crates), which is built on the same UI framework (GPUI), and follows the GPUI Kit coding guides in `.claude/skills/gpui-kit`. Some of this is more structure than a 3 000-line app strictly needs; the point is that it keeps working as the app grows, and each rule is cheap to follow from the start.

## Crates

```text
crates/
├── varetelling/     the application shell (the binary)
├── stocktake_ui/    the counting feature: views, table, dialogs
├── ui/              the app's design layer on top of gpui-kit
└── stocktake/       the domain model, with no UI at all
```

Dependencies only point downward:

```text
varetelling ──► stocktake_ui ──► ui ──► gpui-kit
                     │
                     └─────────► stocktake
```

| Crate          | Owns                                                                 | Must not                                   |
| -------------- | -------------------------------------------------------------------- | ------------------------------------------ |
| `stocktake`    | Products, counting rules, import, export, saving                     | Depend on GPUI or know how anything looks  |
| `ui`           | Motion, typography, small presentational components                  | Know what a stocktake is                   |
| `stocktake_ui` | The workflow: what happens on scan, Enter, Escape; screens and dialogs | Reach into the shell                       |
| `varetelling`  | Theme, window options, calling `init`                                | Contain feature logic                      |

Why split at all?

- **The domain crate builds and tests in seconds**, because it doesn't compile GPUI. `cargo test -p stocktake` is the fast loop for counting rules.
- **The boundaries are enforced by the compiler.** `stocktake` can't accidentally depend on a view, because it has no UI dependency to reach one with. In one crate, that rule would only be a convention.
- **`ui` is where consistency lives.** A `Delta` looks the same wherever a difference is shown. When the design changes, it changes in one place.

This mirrors Zed: `project` (model) sits under `project_panel` (view), `git` under `git_ui`, and every feature draws from the shared `ui` crate.

A crate isn't created for every screen, though. `stocktake_ui` holds the whole counting feature because the pieces change together. Split a new crate when a capability has its own state, a stable public seam, and more than one real user.

### Crate conventions

- **The crate root is named after the crate** (`[lib] path = "src/ui.rs"`), as in Zed, so an editor tab says `ui.rs` rather than one of four `lib.rs`.
- **Dependencies are declared once**, in the workspace `Cargo.toml`. Crates opt in with `gpui-kit.workspace = true`, so versions can't drift between crates.
- **Each crate states its public seam in its root file.** `stocktake_ui` exports `init` and `StocktakeView`; everything else is private.

## The `ui` crate

Modelled on Zed's `crates/ui`:

```text
ui/src/
├── ui.rs              the crate root: re-exports everything flat
├── prelude.rs         `use ui::prelude::*;` covers most view files
├── styles.rs          motion, typography
│   └── styles/
└── components.rs      one module per component
    └── components/
```

- **One file per component, re-exported flat.** Callers write `ui::WindowBar`, not `ui::components::window_bar::WindowBar`, so moving a file never breaks an import.
- **A prelude, kept short.** It holds what almost every view file needs. Anything rarer is imported by path, so a reader can see where it comes from.
- **Styles are code, not constants.** `StyledTypography::tabular_nums()` is an extension trait on any `Styled` element, the same shape as Zed's `StyledTypography`. Colors come from `cx.theme()` and are never hard-coded.
- **Components are `RenderOnce`**: plain values built fresh every frame, with builder methods. They hold no state between frames, so there's nothing to keep in sync.

| Item            | What it's for                                                           |
| --------------- | ----------------------------------------------------------------------- |
| `Appear`        | Fades and rises a region in the first time it renders                   |
| `flash()`       | A 0→1→0 strength, timed from a start the caller stores with the data    |
| `Delta`         | A signed change (`+2`, `−3`), tinted when non-zero                      |

### Layout

The counting screen is flat and runs edge to edge, like a pane in an editor (the layout takes after [tty7](https://github.com/l0ng-ai/tty7)): horizontal bands separated by hairlines, with no cards and no gaps between them. Progress sits in the status bar, not in headline figures, so the table gets the height.

The window opens full screen. The product column takes whatever width the fixed columns leave, and is refitted when the window changes size.

### Motion

All motion is built on gpui-base's motion primitives (`Presence`, `Timing`, `Keyframes`), not raw `with_animation`. There are two reasons:

1. They read durations and easing curves from the theme's motion tokens, so every animation in the app shares one timing.
2. They honour the system's reduced-motion setting: with it on, things appear in their final state.

Motion is only used where it explains a change: a region arriving (`Appear`), or the row you just counted (`flash`). Because the flash disappears under reduced motion, it's never the only sign that something changed. The table's count and status update too.

The flash's start time is stored with the product in the table delegate, not in element state. GPUI drops element state when an element leaves the screen, so a flash keyed to the row would replay each time the row scrolled back into view.

## The feature crate

```text
stocktake_ui/src/
├── stocktake_ui.rs        the seam: actions, key bindings, `init`
├── stocktake_view.rs      state and the counting workflow
│   └── stocktake_view/
│       └── files.rs       importing and exporting
├── product_table.rs       the table delegate: how rows render
├── count_dialog.rs        counting one product
├── welcome.rs             the screen before any stock list is imported
├── count_status.rs        a product's counted/uncounted marker
└── quantity.rs            parsing and entering a quantity
```

- **One type across several files.** `StocktakeView` is split into child modules by concern, as Zed does with `Editor`. Child modules can see their parent's private fields, so nothing has to be made public just to split a file.
- **Domain-aware components stay in the feature.** `CountStatus` says "Telt" / "Ikke telt", which are stocktake words, so it lives here and not in `ui`.
- **`Entity` for retained state, `RenderOnce` for everything else.** The view, the inputs and the table keep state between frames, so they are entities. `Welcome` and `CountStatus` are rebuilt every frame from values.
- **Commands are actions.** Import, export and cancel are GPUI actions bound in `init`. The menu item, the welcome's "Åpne fil", the key binding and the shortcut shown beside it all go through the same action, so a shortcut can't disagree with its button.
- **Identity comes from the domain.** Table rows are keyed by `ProductId::line()`, not by row position, so a row's animation follows its product through filtering.

## Tests

| Layer        | Where                                   | Proves                                             |
| ------------ | --------------------------------------- | -------------------------------------------------- |
| Domain       | `stocktake/src/*.rs`                    | Search order, lookup, import, export, saving, recent stock lists |
| UI workflow  | `stocktake_ui/src/stocktake_view.rs`    | Hiding and showing the sidebar, by click, shortcut and keyboard focus, in a headless window |

Run everything with `cargo test --workspace`. A plain `cargo test` only runs the default member (the shell). The import test reads the sample export from the untracked `data/` directory.
