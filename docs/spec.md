# Stocktake app: specification

An app for the year-end stocktake at Scala Bad: going through the storage and verifying that the number of units of each product matches what the business system says. Terminology follows [CONTEXT.md](../CONTEXT.md).

## Context

Until now the stocktake has been done on paper, and before that in a spreadsheet on a laptop with a USB barcode scanner. Scanning a barcode jumps to the product's row, where the system quantity is pre-filled; the counter confirms it or overwrites it. This app replaces that.

- **Platform:** a native iPad app, with the one stocktake shared through CloudKit so up to four counters count at once; see [ADR 0001](adr/0001-native-ipad-app.md). It was designed and built first as a GPUI Kit desktop app for one laptop and a USB scanner, which this document describes screen for screen. The desktop app is where each rule is worked out and tried, since it builds and tests in seconds; the port starts once the open questions are settled there.
- **Scanning:** the device's camera, or a wireless scanner, which acts as a keyboard and types into the search field like the USB scanner did.
- **Interface language:** Norwegian. Code and docs use the English terms in `CONTEXT.md`.

## Import

- A clean, minimal but good-looking button to import the stock list.
- The input is the business system's `.xlsx` export as-is (see `data/stock-list.xlsx`); no manual preparation.
- Columns are read by header name, not position.
- Per the storage owner: "Når det gjelder hva som er interessant så er det kolonne C, D, H og N" (the interesting columns are C, D, H and N). The counters later asked for column E too, which tells apart products with the same name. These are the columns shown to the counter:

  | Excel column | Header | Meaning |
  | --- | --- | --- |
  | C | `VareNR` | Item number |
  | D | `ProduktDesc1` | Name |
  | E | `ProduktDesc2` | Description: colour and style, e.g. «Como Standard - Ramtre» (may be empty) |
  | H | `Lokasjon` | Location (may be empty; `N/A` means empty) |
  | N | `FysiskPaaLager` | System quantity |

- Also read, but not shown:

  | Excel column | Header | Meaning |
  | --- | --- | --- |
  | F | `PrdEAN` | Barcode (may be empty); needed so a scan can find the product |

- All other columns are ignored.
- `N/A` in `Lokasjon` is the business system's way of saying the product has no location. It's read as an empty location, so the product has no aisle and sorts last, and the counter gives it a location when it's found.
- An item number on more than one line (66 products in `data/complete-stock-list.xlsx`, always at different locations) becomes one product whose system quantity is the lines' sum. Before the import finishes, a dialog lists the lines for each such product, and the person importing picks which location is its pick location; the app doesn't suggest one. The other lines' locations stay on the product, and any counter can later make one of them the pick location from the count dialog, which undoes the choice. Importing the file again asks again.

- Discontinued (`Utgått`) products are included and counted like any other product. Their status isn't shown.
- Importing while a stocktake is in progress shows a warning that the current stocktake will be discarded (e.g. "31/43 counted"). Confirming starts a fresh stocktake. Only one stocktake exists at a time.

## Counting screen

- The whole stock list as a table: location, item number, name with the description on a muted line under it, system quantity (columns H, C, D and E, N), then counted quantity, difference, and whether the product is counted, uncounted or finished.
- The location column shows the pick location. A product counted at overflow locations shows how many after it, e.g. `C4-7 +2`, and its counted quantity is the total across all its locations.
- Two columns are hidden until they're switched on in the columns menu. **Bufferlokasjon** lists each overflow location, and is headed **Bufferlokasjoner** once a product has more than one; sorted, it goes by the first overflow location. **Buffer** is what was counted at the overflow locations together. Either way, products without an overflow location sort last.
- The aisle filter goes by pick location only. An aisle that's a letter is labelled «Reol A»; the others are labelled as the location starts (`DL`, `Tilbehør`, `Pakkedisk`), and `D` and `DL` are separate aisles.
- A checkbox in the sidebar, off by default, hides products whose system quantity is zero. The counters leave those for last: once the rest is counted, the filter is switched off and what still stands at zero is confirmed.
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
- A barcode scan, or typing an item number, name or description, selects the matching row. The counters mostly type part of the name, such as «Burano»; the search ignores letter case.
- Searching also matches the product's pick location and any overflow locations it has been counted at, so typing «tilbehør» lists that aisle.
- Products without a barcode are found by typing.
- If nothing matches, an error dialog is shown. Unlisted products are not recorded. If several products match, Enter opens nothing: a notice says how many matched, and the table already lists them to pick from.

### Counting a product

- When a row is selected, focus moves to its counted-quantity cell, pre-filled with the system quantity.
- Enter confirms the value, or the counter types a different number first to overwrite it.
- The count dialog has an editable location field, pre-filled with the pick location. Leaving it as is counts at the pick location, so the normal scan-and-Enter flow is unchanged.
- To register units at an overflow location, the counter types that location into the field before saving. Any text is accepted, so new locations such as «golv ytre lager» need nothing set up first. It's trimmed, and compared with the stocktake's locations ignoring case so one shelf isn't recorded twice: a match takes the spelling already in use (`tilbehør 1-1` becomes `Tilbehør 1-1`), and anything else is upper-cased (`c4-7` becomes `C4-7`).
- The dialog shows the product's description under its name, since products with the same name differ only there.
- If the product has no pick location, the field starts empty, and an empty location counts as the pick location.
- When the field holds a location other than the pick location (compared after trimming and upper-casing), a checkbox **Erstatt plukklokasjon** appears below it, unchecked. Unchecked, the count is saved at an overflow location. Checked, the typed location becomes the product's pick location and the count is saved there; if units were already counted at that location as an overflow location, they become the pick location's count. Changing the field back to the pick location hides the checkbox.
- The pick location can be replaced at any time. What was counted there moves with it, and is added to anything already counted at the new location as an overflow location, so the counted quantity stays the same. With the box checked, the dialog offers **Erstatt** and **Legg til** against that count.
- A moved pick location is used everywhere in the app: the table's location column, sorting, search and the aisle filter. Typing the stock list's location with the box checked moves it back.
- Focus then returns to the search field for the next scan.
- A product is **uncounted** until its pick location has been confirmed or overwritten. A product counted as zero is counted, so an empty pick shelf is recorded by counting zero there.
- A product counted only at overflow locations is **partly counted** ("Delvis talt"): it has its own status in the table and its own checkbox in the sidebar's status filter, but it has no counted quantity or difference yet, and it's grouped with the uncounted products in the progress indicator and the export warning. Counting its pick location makes it counted.
- A product that came from several stock-list lines shows the locations that weren't picked at import, and the counter can make one of them the pick location instead. What was counted at the old pick location moves with it, as with any move.

### Finishing a product

- A counted product can be marked **finished** ("Ferdig talt") with a button in the count dialog. It means the counter has stopped looking for more units, usually because the difference is within what they believe can be right. The app never marks a product finished on its own.
- Finished is a status of its own in the table and the status filter, so the products that are counted but not finished are one checkbox away.
- A finished product stays editable. Counting it again leaves the mark alone; the same button removes it.

### Counting a product again

- Counts are kept per location. When the location in the field already has a count, the dialog shows it and lets the counter either **replace** it or **add** to it; Enter adds. A location without a count yet just takes the new count.
- The dialog shows the system quantity ("I lagersystemet"). Below it, what was counted is listed by location: the pick location under "Lager" once it has been counted, and the overflow locations under "Buffer" when there are any.
- Replacing an overflow location's count with zero removes that location, which is how a mistyped location is corrected.

## Persistence

- Every confirm or overwrite is saved immediately, stamped with the time it was saved. No save button.
- Reopening the app resumes the stocktake in progress.
- A stocktake saved before counts were kept per location resumes with each counted quantity at the product's pick location.

## Export

- Exports an `.xlsx` of the stock list with added columns: **Counted quantity**, **Difference** (counted minus system quantity), **Date counted** (the product's latest count, as a date) and **New pick location**. The counted quantity is the total across all of a product's locations. `Lokasjon` keeps what the stock list said; a product whose pick location was moved has the new one in the last column, in red, so it stands out on paper too. A product moved back to its stock-list location isn't marked.
- A second sheet lists every product counted at one or more overflow locations, one row per location with the pick location first: item number, name, location, whether it's the pick location or an overflow location, the quantity counted there and the date it was counted. A moved pick location's row is in red. A partly counted product's pick location is marked uncounted. Products counted only at their pick location are left out. This is what's used to update `Lokasjon` in the business system, or to move the goods.
- If any products are uncounted, the app warns before exporting, and uncounted products are marked in the file.
- Adjustments are entered into the business system by hand from the exported file.

## Out of scope for v1

- Blind counting (hiding the system quantity until a count is entered). Considered UX work for later.
- Recording products that aren't on the stock list.
- Keeping past stocktakes.
- Showing who counted what. The iCloud account is the only identity, and nobody asked for it.
- Importing results directly into the business system.

## Undecided

- **Stock movement during the stocktake.** It's unknown whether goods are received or shipped while counting. v1 assumes the storage is frozen and compares against one export taken right before counting. If stock moves, system quantities go stale mid-count and differences would need reconciling.
- **Unlisted products.** v1 only shows an error. Whether they should be recorded (barcode + quantity, listed separately in the export) is open.
- **Blind vs. pre-filled counting.** v1 pre-fills the system quantity, which risks counters confirming without really checking. Revisit once the basics work.
- **Export columns.** Whether the exported `.xlsx` should keep every original column, or only C, D, H and N plus Counted quantity and Difference.
- **Scanning into an open count cell.** If a counter scans the next product instead of pressing Enter first, the barcode is typed into the counted-quantity cell (or the "count again" field) and the scanner's Enter submits it. The app now guards the case where the text is exactly the barcode of a listed product: nothing is saved, the scanned product is counted next, and a hint says the previous count wasn't saved. A barcode that isn't on the list is still accepted as a quantity. Not yet checked with the real scanner; a maximum quantity could close that gap.
- **Location field in the count dialog.** The editable, pre-filled field is a first take on registering overflow locations; the interaction may change once it's tried on the floor.
- **Suggesting locations.** The location field could suggest the locations already in the stock list as the counter types. gpui-kit's combobox only picks from a list, and the field has to take any text, so this waits for a suitable component.
- **Enter adds at a counted location.** When the location already has a count, Enter adds to it rather than replacing it. Whether recounting a shelf (replace) is the more common case on the floor is open.
- **Resetting a pick location.** A counted pick location can't be moved. A reset that clears its count, so it can be counted again or moved, is planned.
- **Showing overflow locations in the table.** The `C4-7 +2` hint is a stopgap. Check whether gpui-kit has a component that suits showing a product's locations better.
- **Importing results.** Whether the business system can import stocktake results directly, which would make a matching export format worthwhile.
- **Exporting a merged duplicate.** A product merged from several stock-list lines is one row in the export. How the line that wasn't picked is reported, so it can be cleared in the business system, is open.
- **Reviewing finished products.** Whether a lead goes through the finished products and approves them. Nothing is built for it until asked.
- **Several counters at one location.** Counts from two counters at the same location are added together (Q3). What the count dialog shows while another counter's count is arriving, and whether "replace" can replace a count someone else just made, is worked out in the iPad app.
