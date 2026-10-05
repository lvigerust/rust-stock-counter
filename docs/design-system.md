# Design system

The decisions the `ui` crate encodes, so a new screen looks like the ones before it. The rule of the crate: **a view names a role and composes parts; it never spells out a size, a weight, a radius or a focus ring itself.** Where no part fits, add one to `crates/ui`, styled after [Catalyst](https://catalyst.tailwindui.com)'s, and it's the same everywhere from then on. Where Catalyst and the gpui-kit Design Guides (`.agents/skills/gpui-kit-design-guides`) disagree, the guides win.

**Contents:** [Type](#type) · [Spacing](#spacing) · [Corners](#corners) · [Color](#color) · [Buttons](#buttons) · [Focus](#focus) · [Fields and dialogs](#fields-and-dialogs) · [Blocks](#blocks) · [Motion](#motion) · [Adding a part](#adding-a-part)

## Type

Four roles, each a `ui` component. The app reads in `text_sm` (14px at the default rem); nothing in it is larger except gpui-kit's dialog titles.

| Role | Part | Size, weight, color | Catalyst | Used for |
| --- | --- | --- | --- | --- |
| Heading | `Heading` | sm, semibold, foreground | `Subheading` | An empty state's first line |
| Label | `Label` | sm, medium, foreground | `Label` | What a control or a group of values is; `Field` draws it |
| Text | `Text` | sm, regular, muted | `Text` | Help under a field, why a list is empty, counts beside labels |
| Section heading | `SectionHeading` | xs, medium, muted | `SidebarHeading`, `DropdownHeading` | Over a group of rows: sidebar sections, menu groups, the welcome's lists, a tooltip's list |

A role takes `Styled`, so a caller can still truncate it or size it down for a dense row (`Text::new(n).text_xs()`). Numbers that line up down a column get `tabular_nums()` (`ui::StyledTypography`). Copy is sentence case, Catalyst's and the guides' default; the welcome's section headings dropped their uppercase for it.

## Spacing

Lengths are on Tailwind's quarter-rem scale, through GPUI's helpers (`p_4()`, `gap_2()`) or `ui::Spacing(n)` where a helper is missing. Raw `px(...)` is for geometry that must match something physical, and says so in a comment: the traffic lights, the sidebar's resizable width, the table's column widths.

| Relationship | Step | Where |
| --- | --- | --- |
| Icon beside its label | 2 (0.5rem) | Buttons, rows, menu items |
| Label to control, control to help | 3 (0.75rem) | `Field` |
| Padding of a pane, bar or sidebar section | 4 (1rem) | `WindowBar`, `SidebarHeader`/`Body`/`Footer` |
| Groups in one surface | 6 (1.5rem) | Fields in the count dialog |
| Sections | 8 (2rem) | `SidebarBody`'s sections, the welcome's lists |
| Padding of a dialog | 8 (2rem); 6 for an alert | `StyledDialog` |

## Corners

One base radius, set in the shell's theme (`crates/varetelling/src/theme.rs`, Catalyst's `rounded-lg`), and the theme's tiers from it. No radius is written at a call site.

| Tier | Value | Where |
| --- | --- | --- |
| `sm` | ½ × | Badges, checkboxes |
| `md` | 1 × | Buttons, inputs, menu items, rows |
| `xl` | 2 × | Dialogs and alerts (Catalyst's `rounded-2xl`) |

## Color

Every color is a theme token read from `cx.theme()`; the hex values live in `theme.rs` alone. Roles, not palette positions: `foreground`/`muted_foreground` for text, `border` for rules, `accent` for hover fills (the same translucent fill on every surface), `ring` for focus, `success`/`warning`/`danger` only for what they mean. Status is never color alone: `CountStatus` pairs each color with its own icon shape, and `Delta` carries the sign.

## Buttons

`ui::Button` is gpui-kit's with the app's type on its label, and the variant says what the action is:

- **primary** for the one commit in a decision area, the action Enter takes (Lagre, Legg til);
- **default** (filled) for an ordinary visible action (Eksporter telling);
- **outline** for an action that needs a boundary with less weight (Erstatt beside Legg til, the columns menu in the bar);
- **ghost** for quiet actions in bars and footers (hiding the sidebar, Avbryt);
- a **`RowButton`** when a row of content is itself the button (recent files, sidebar items).

A button's icon is muted on ghost and outline buttons and comes up on hover. Disabled buttons fade to half, in their own colors. Every action is a GPUI action, so its menu item, shortcut, button and tooltip can't disagree.

## Focus

Keyboard focus is a ring, never gpui-kit's recolored border (`theme.focus_ring = false`). The button picks the ring from its variant; a caller only says otherwise where a row of buttons should match.

| Ring | Look | On |
| --- | --- | --- |
| `FocusRing::Solid` | The theme's `ring` color, 2px, a 2px gap outside the control | Controls with a fill or border: primary, default, outline buttons, inputs |
| `FocusRing::Subtle` | A faint ring hugging the control (after tty7) | Controls with no boundary at rest: ghost buttons, rows, checkbox rows |

The solid ring's gap is painted in the color of what the button sits on (`Button::surface`), the window's background unless the button is in the sidebar. Avbryt in the count dialog is a ghost button that takes the solid ring (`with_focus_ring`), so focus looks the same on both footer buttons.

## Fields and dialogs

A control in a dialog sits in a `Field`: `Label` above, one or more `Text` lines of help below, Catalyst's spacing between. A dialog takes `dialog_frame` and an alert `alert_frame` (`ui::StyledDialog`), which set the padding and the `xl` corners, so the count dialog and the alerts are one family. Dialog copy follows the guides: the title names the decision, the body adds only what changes it, the confirm button names the result (`Forkast og importer…`) and `Avbryt` leaves.

## Blocks

Composed from named parts, as Catalyst's are, so a call site reads like the layout it builds:

- `Sidebar` › `WindowBar`, `SidebarHeader`, `SidebarBody` (scrolls) › `SidebarSection` › `SidebarHeading`, `SidebarItem`; `SidebarSpacer`; `SidebarFooter`.
- `Dropdown(DropdownButton, DropdownMenu › heading, DropdownItem, divider)`: gpui-kit's menu behavior with Catalyst's rows.
- `WindowBar` › `WindowBarItem`: the band that holds the window's own controls on macOS and Windows.
- `Badge`: one neutral badge for a count worth scanning for, such as `+2` overflow locations.
- `SwitchField`: a setting that is on or off, after Catalyst's `SwitchField`. Its `Label` and `Text` lead and the switch trails; pressing the text flips it too.
- `Field`, `Heading`, `Label`, `Text`, `SectionHeading`, `Delta`, `Logo`.

Domain-worded parts (`CountStatus`) live in the feature crate and compose these.

## Motion

Two animations, both in `ui::motion` on gpui-base's primitives: `Appear` for a region arriving the first time, `flash` for pointing at a row that just changed. Nothing runs at rest, and under reduced motion neither plays; see [architecture.md](architecture.md#motion).

## Adding a part

1. Find Catalyst's block (`~/Developer/Tailwind Plus/Catalyst/src/components`) and translate its decisions: parts, spacing, weights. Don't copy its source.
2. Build it on the gpui-kit component that owns the behavior (menu, dialog, tag, checkbox); own only the presentation.
3. Make it a `RenderOnce` with a builder: `new(...)`, fluent setters, `Styled` and `ParentElement` where it holds children. Use the type roles for its text and the theme's tiers for its corners.
4. Re-export it flat from `crates/ui/src/components.rs`, document it in its module, and add a row here.
