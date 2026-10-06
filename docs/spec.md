# Stocktake app: specification

A desktop app for the year-end stocktake at Scala Bad: going through the storage and verifying that the number of units of each product matches what the business system says. Terminology follows [CONTEXT.md](../CONTEXT.md).

## Context

Today the stocktake is done in a spreadsheet on a laptop with a USB barcode scanner. Scanning a barcode jumps to the product's row, where the system quantity is pre-filled; the counter confirms it or overwrites it. This app replaces that spreadsheet.

- **Platform:** GPUI Kit desktop app, one laptop, one USB barcode scanner (acts as a keyboard).
- **Interface language:** Norwegian. Code and docs use the English terms in `CONTEXT.md`.

## Import

- A clean, minimal but good-looking button to import the stock list.
- The input is the business system's `.xlsx` export as-is (see `data/stock-list.xlsx`); no manual preparation.
- Columns are read by header name, not position.
- Per the storage owner: "Når det gjelder hva som er interessant så er det kolonne C, D, H og N" (the interesting columns are C, D, H and N). These are the columns shown to the counter:

  | Excel column | Header | Meaning |
  | --- | --- | --- |
  | C | `VareNR` | Item number |
  | D | `ProduktDesc1` | Name |
  | H | `Lokasjon` | Location (may be empty) |
  | N | `FysiskPaaLager` | System quantity |

- Also read, but not shown:

  | Excel column | Header | Meaning |
  | --- | --- | --- |
  | F | `PrdEAN` | Barcode (may be empty); needed so a scan can find the product |
  | E | `ProduktDesc2` | Description, e.g. «Porselen servant i Brun Matt» (may be empty); where it's shown is undecided |

- All other columns are ignored.
- An item number on more than one line (66 products in `data/complete-stock-list.xlsx`, always at different locations) is resolved before the import finishes: a dialog lists the lines for each such product, and the person importing picks one location, which becomes the product's pick location. The app doesn't suggest one. There's no undo; importing the file again asks again.

- Discontinued (`Utgått`) products are included and counted like any other product. Their status isn't shown.
- Importing while a stocktake is in progress shows a warning that the current stocktake will be discarded (e.g. "31/43 counted"). Confirming starts a fresh stocktake. Only one stocktake exists at a time.

## Counting screen

- The whole stock list as a table: location, item number, name, system quantity (columns H, C, D, N), then counted quantity, difference, and whether the product is counted or uncounted.
- The location column shows the pick location. A product counted at overflow locations shows how many after it, e.g. `C4-7 +2`, and its counted quantity is the total across all its locations.
- Two columns are hidden until they're switched on in the columns menu. **Bufferlokasjon** lists each overflow location, and is headed **Bufferlokasjoner** once a product has more than one; sorted, it goes by the first overflow location. **Buffer** is what was counted at the overflow locations together. Either way, products without an overflow location sort last.
- The aisle filter goes by pick location only.
- A checkbox in the sidebar, off by default, hides products whose system quantity is zero.
- After a count is saved, the table scrolls to the product and its row briefly highlights, so the counter sees where the count landed.
- When every product is counted, a summary says how many products have a difference, next to an export button.
- A stock list can also be imported by dropping the `.xlsx` file on the window.
- Before a stock list is imported, the welcome lists the last five imported, newest first, each with its folder. Opening one imports it again, with the same warning if a stocktake is in progress. A file that has been moved or deleted is removed from the list.
- The window opens full screen. Columns can be sorted; uncounted products stay at the bottom of the counted-quantity and difference columns whichever way they're sorted.
- One search field above the table, plus a progress indicator (e.g. 31/43 counted).

### Differences

- The mode menu atop the sidebar (or Cmd/Ctrl-2) switches the main pane to **Differanse**: a table of the counted products whose counted quantity differs from the system quantity, in walking order. Uncounted and partly counted products have no difference yet and aren't listed.
- It shows the stock list's columns without Status, and sorts the same way: by difference, the largest surplus or shortfall comes first.
- Picking a product opens its count dialog, so a difference can be counted again. One that now matches leaves the list.
- With nothing listed, it says so, and how many products are still uncounted.

### Finding a product

- The scanner and the keyboard type into the same search field.
- A barcode scan, or typing an item number or name, selects the matching row.
- Searching also matches the product's pick location and any overflow locations it has been counted at.
- Products without a barcode are found by typing.
- If nothing matches, an error dialog is shown. Unlisted products are not recorded.

### Counting a product

- When a row is selected, focus moves to its counted-quantity cell, pre-filled with the system quantity.
- Enter confirms the value, or the counter types a different number first to overwrite it.
- The count dialog has an editable location field, pre-filled with the pick location. Leaving it as is counts at the pick location, so the normal scan-and-Enter flow is unchanged.
- To register units at an overflow location, the counter types that location into the field before saving. Any text is accepted; it's trimmed and upper-cased (`c4-7` becomes `C4-7`) so one shelf isn't recorded twice.
- If the product has no pick location, the field starts empty, and an empty location counts as the pick location.
- When the field holds a location other than the pick location (compared after trimming and upper-casing), a checkbox **Erstatt plukklokasjon** appears below it, unchecked. Unchecked, the count is saved at an overflow location. Checked, the typed location becomes the product's pick location and the count is saved there; if units were already counted at that location as an overflow location, they become the pick location's count. Changing the field back to the pick location hides the checkbox.
- The pick location can be replaced at any time. What was counted there moves with it, and is added to anything already counted at the new location as an overflow location, so the counted quantity stays the same. With the box checked, the dialog offers **Erstatt** and **Legg til** against that count.
- A moved pick location is used everywhere in the app: the table's location column, sorting, search and the aisle filter. Typing the stock list's location with the box checked moves it back.
- Focus then returns to the search field for the next scan.
- A product is **uncounted** until its pick location has been confirmed or overwritten. A product counted as zero is counted, so an empty pick shelf is recorded by counting zero there.
- A product counted only at overflow locations is **partly counted** ("Delvis talt"): it has its own status in the table and its own checkbox in the sidebar's status filter, but it has no counted quantity or difference yet, and it's grouped with the uncounted products in the progress indicator and the export warning. Counting its pick location makes it counted.

### Counting a product again

- Counts are kept per location. When the location in the field already has a count, the dialog shows it and lets the counter either **replace** it or **add** to it; Enter adds. A location without a count yet just takes the new count.
- The dialog shows the system quantity ("I lagersystemet"). Below it, what was counted is listed by location: the pick location under "Lager" once it has been counted, and the overflow locations under "Buffer" when there are any.
- Replacing an overflow location's count with zero removes that location, which is how a mistyped location is corrected.

## Persistence

- Every confirm or overwrite is saved immediately. No save button.
- Reopening the app resumes the stocktake in progress.
- A stocktake saved before counts were kept per location resumes with each counted quantity at the product's pick location.

## Export

- Exports an `.xlsx` of the stock list with two added columns: **Counted quantity** and **Difference** (counted minus system quantity). The counted quantity is the total across all of a product's locations.
- A second sheet lists every product counted at one or more overflow locations, one row per location with the pick location first: item number, name, location, whether it's the pick location or an overflow location, and the quantity counted there. A partly counted product's pick location is marked uncounted. Products counted only at their pick location are left out. This is what's used to update `Lokasjon` in the business system, or to move the goods.
- If any products are uncounted, the app warns before exporting, and uncounted products are marked in the file.
- Adjustments are entered into the business system by hand from the exported file.

## Out of scope for v1

- Blind counting (hiding the system quantity until a count is entered). Considered UX work for later.
- Recording products that aren't on the stock list.
- Several laptops counting the same stocktake, whether merged or synced.
- Keeping past stocktakes.
- Importing results directly into the business system.

## Undecided

- **Stock list size.** Assumed to be about 50 products, like the sample, but not confirmed. A list of thousands, or one covering several warehouses, could change search and table design.
- **Stock movement during the stocktake.** It's unknown whether goods are received or shipped while counting. v1 assumes the storage is frozen and compares against one export taken right before counting. If stock moves, system quantities go stale mid-count and differences would need reconciling.
- **Unlisted products.** v1 only shows an error. Whether they should be recorded (barcode + quantity, listed separately in the export) is open.
- **Blind vs. pre-filled counting.** v1 pre-fills the system quantity, which risks counters confirming without really checking. Revisit once the basics work.
- **Barcode column.** The storage owner listed C, D, H and N as the interesting columns, which doesn't include the barcode (F). The spec still reads F so scanning works. Confirm this is fine, or whether they scan something else, such as the item number.
- **Export columns.** Whether the exported `.xlsx` should keep every original column, or only C, D, H and N plus Counted quantity and Difference.
- **Scanning into an open count cell.** If a counter scans the next product instead of pressing Enter first, the barcode is typed into the counted-quantity cell (or the "count again" field) and the scanner's Enter submits it. The app now guards the case where the text is exactly the barcode of a listed product: nothing is saved, the scanned product is counted next, and a hint says the previous count wasn't saved. A barcode that isn't on the list is still accepted as a quantity. Not yet checked with the real scanner; a maximum quantity could close that gap.
- **Location field in the count dialog.** The editable, pre-filled field is a first take on registering overflow locations; the interaction may change once it's tried on the floor.
- **Suggesting locations.** The location field could suggest the locations already in the stock list as the counter types. gpui-kit's combobox only picks from a list, and the field has to take any text, so this waits for a suitable component.
- **Enter adds at a counted location.** When the location already has a count, Enter adds to it rather than replacing it. Whether recounting a shelf (replace) is the more common case on the floor is open.
- **Resetting a pick location.** A counted pick location can't be moved. A reset that clears its count, so it can be counted again or moved, is planned.
- **Exporting moved pick locations.** The export doesn't yet say which products' pick location was moved. The plan: keep the stock list's `Lokasjon` and mark the move from old to new, on the Lokasjoner sheet and as a new-location column on the main sheet.
- **Showing overflow locations in the table.** The `C4-7 +2` hint is a stopgap. Check whether gpui-kit has a component that suits showing a product's locations better.
- **Importing results.** Whether the business system can import stocktake results directly, which would make a matching export format worthwhile.
- **System quantity of a resolved duplicate.** After the import dialog picks a location, whether the product's system quantity is that line's or the sum of both lines (`Como Fronter 60 - Lys Macchiato`: 0 at `A1`, 16 at `E2-8`). Also whether the export shows which location was chosen.
- **Platform.** The counters count mostly on iPads over wifi that reaches the whole storage, and want counts from several counters at one location added together. This contradicts the one-laptop platform above and the "several laptops" item under out of scope; it's settled in an ADR once the rest of Q1 to Q3 in [questions-for-counters.md](questions-for-counters.md) is answered.
