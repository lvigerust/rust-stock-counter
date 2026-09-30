# Stocktake app: specification

A desktop app for the year-end stocktake at Scala Bad: going through the storage and verifying that the number of units of each product matches what MultiCase says. Terminology follows [CONTEXT.md](../CONTEXT.md).

## Context

Today the stocktake is done in a spreadsheet on a laptop with a USB barcode scanner. Scanning a barcode jumps to the product's row, where the system quantity is pre-filled; the counter confirms it or overwrites it. This app replaces that spreadsheet.

- **Platform:** GPUI Kit desktop app, one laptop, one USB barcode scanner (acts as a keyboard).
- **Interface language:** Norwegian. Code and docs use the English terms in `CONTEXT.md`.

## Import

- A clean, minimal but good-looking button to import the stock list.
- The input is the MultiCase `.xlsx` export as-is (see `data/Vareliste - varetelling.xlsx`); no manual preparation.
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

- All other columns are ignored.

- Discontinued (`Utgått`) products are included and counted like any other product. Their status isn't shown.
- Importing while a stocktake is in progress shows a warning that the current stocktake will be discarded (e.g. "31/43 counted"). Confirming starts a fresh stocktake. Only one stocktake exists at a time.

## Counting screen

- The whole stock list as a table: location, item number, name, system quantity (columns H, C, D, N), then counted quantity, difference, and whether the product is counted or uncounted.
- After a count is saved, the table scrolls to the product and its row briefly highlights, so the counter sees where the count landed.
- When every product is counted, a summary says how many products have a difference, next to an export button.
- A stock list can also be imported by dropping the `.xlsx` file on the window.
- Before a stock list is imported, the welcome lists the last five imported, newest first, each with its folder. Opening one imports it again, with the same warning if a stocktake is in progress. A file that has been moved or deleted is removed from the list.
- The window opens full screen. Columns can be sorted; uncounted products stay at the bottom of the counted-quantity and difference columns whichever way they're sorted.
- One search field above the table, plus a progress indicator (e.g. 31/43 counted).

### Finding a product

- The scanner and the keyboard type into the same search field.
- A barcode scan, or typing an item number or name, selects the matching row.
- Products without a barcode are found by typing.
- If nothing matches, an error dialog is shown. Unlisted products are not recorded.

### Counting a product

- When a row is selected, focus moves to its counted-quantity cell, pre-filled with the system quantity.
- Enter confirms the value, or the counter types a different number first to overwrite it.
- Focus then returns to the search field for the next scan.
- A product is **uncounted** until it has been confirmed or overwritten. A product counted as zero is counted.

### Counting a product again

- When an already-counted product is selected again (e.g. more units found at a second location), a dialog shows the current counted quantity and lets the counter either **replace** it or **add** to it.

## Persistence

- Every confirm or overwrite is saved immediately. No save button.
- Reopening the app resumes the stocktake in progress.

## Export

- Exports an `.xlsx` of the stock list with two added columns: **Counted quantity** and **Difference** (counted minus system quantity).
- If any products are uncounted, the app warns before exporting, and uncounted products are marked in the file.
- Adjustments are entered into MultiCase by hand from the exported file.

## Out of scope for v1

- Blind counting (hiding the system quantity until a count is entered). Considered UX work for later.
- Recording products that aren't on the stock list.
- Several laptops counting the same stocktake, whether merged or synced.
- Keeping past stocktakes.
- Importing results directly into MultiCase.

## Undecided

- **Stock list size.** Assumed to be about 50 products, like the sample, but not confirmed. A list of thousands, or one covering several warehouses, could change search and table design.
- **Stock movement during the stocktake.** It's unknown whether goods are received or shipped while counting. v1 assumes the storage is frozen and compares against one export taken right before counting. If stock moves, system quantities go stale mid-count and differences would need reconciling.
- **Unlisted products.** v1 only shows an error. Whether they should be recorded (barcode + quantity, listed separately in the export) is open.
- **Blind vs. pre-filled counting.** v1 pre-fills the system quantity, which risks counters confirming without really checking. Revisit once the basics work.
- **Barcode column.** The storage owner listed C, D, H and N as the interesting columns, which doesn't include the barcode (F). The spec still reads F so scanning works. Confirm this is fine, or whether they scan something else, such as the item number.
- **Export columns.** Whether the exported `.xlsx` should keep every original column, or only C, D, H and N plus Counted quantity and Difference.
- **Scanning into an open count cell.** If a counter scans the next product instead of pressing Enter first, the barcode is typed into the counted-quantity cell (or the "count again" field) and the scanner's Enter submits it. The app now guards the case where the text is exactly the barcode of a listed product: nothing is saved, the scanned product is counted next, and a hint says the previous count wasn't saved. A barcode that isn't on the list is still accepted as a quantity. Not yet checked with the real scanner; a maximum quantity could close that gap.
- **MultiCase import.** Whether MultiCase can import stocktake results directly, which would make a matching export format worthwhile.
