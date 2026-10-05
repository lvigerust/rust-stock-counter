# Review against the complete stock list

The app was checked against `data/complete-stock-list.xlsx` by running the real importer over it and reading the UI code against the result. The app itself was not launched. Terminology follows [CONTEXT.md](../CONTEXT.md).

## What the list contains

- 3,673 rows, 155,899 units. 3,025 products have stock, 648 have zero.
- The import succeeds without errors. Searching takes about 45 ms in a debug build.
- **Duplicate item numbers:** 66 products appear on two lines (132 rows), always at different locations with different quantities. For example `Como Fronter 60 - Lys Macchiato` is `A1` with 0 units and `E2-8` with 16. 62 barcodes are shared the same way.
- **`N/A` as a location:** 612 products. `Product::aisle` reads it as aisle `N`, so the aisle filter offers "Reol N (612)".
- **No location:** 327 products.
- **`A1`:** 174 products, far more than any real shelf, and most of them have zero units. Probably a placeholder; unconfirmed.
- **Locations that aren't `C4-7`:** `DL-1` to `DL-14` (340 products, aisle `DL`), `Tilbehør 1-1` to `Tilbehør 18-6` (297, aisle `Tilbehør`) and `Pakkedisk` (22). The aisle filter has 12 entries, some of which read oddly ("Reol Tilbehør").
- **No barcode:** 953 products, 679 of them with stock. A further 40 have non-EAN barcodes like `SCAD1122`.
- **Zero units but a location:** 321 products.
- **Names with double spaces:** 3, such as `Maranello 90 - Gull  Børstet`, which a search for "Gull Børstet" doesn't find.

## Tasks that can be done now

These don't depend on how the counters work.

1. **Collapse repeated spaces in product names on import.** Trim and collapse in `stock_list::text`, with a test.
2. **Give an ambiguous scan visible feedback.** `Lookup::Ambiguous` does nothing today (`counting.rs:32`). Show a short message such as "2 varer har denne strekkoden. Velg riktig i tabellen." This holds whichever way the duplicates question (Q1) is answered.
3. **Make search cheaper.** Cache a lowercase search key per product instead of lowercasing four or more fields per product on every keystroke. No visible change.
4. **Add a regression test that imports `data/complete-stock-list.xlsx`.** Check that it imports and has the expected row count, so the importer is held to a real-world file.
5. **Record these facts in `docs/spec.md` or `CONTEXT.md`.** Facts only, no decisions: duplicate item numbers, the `N/A` and `A1` locations, and the non-`C4-7` location formats.

## Tasks that need answers first

These wait for the counters' answers. The questions, in Norwegian and ready to hand out, are in [questions-for-counters.md](questions-for-counters.md); the numbers below match its headings. They cover both what the complete list showed and the counters' seven requirements from their first trial of the app.

1. **Q1 to Q3, counting from a phone or tablet.** Devices and scanning, what happens when the network drops, and what happens when several people count at once. These decide the cloud design: offline support and how conflicting counts are merged.
2. **Q4, the same item number on two lines.** The counters want a duplicate check at import and the ability to delete a line. This decides whether the app suggests which line to delete, whether deleting can be undone, and who may delete.
3. **Q5 and Q6, what `A1` and `N/A` mean.** Decide whether they are treated as "no location" for aisles and walk order, and whether products at `N/A` get a pick location while counting.
4. **Q7, the extra column.** "Kolonne 3" is read as `ProduktDesc2` (column E), which holds text like «Porselen servant i Brun Matt». Unconfirmed. Decides where it is shown and whether it is searchable.
5. **Q8 and Q9, changing the pick location and marking it.** Decide where the red "NY PLUKKLOKASJON" mark is shown (table, count dialog, export), and what the follow-up list for the business system should contain.
6. **Q10, the date a product was "picked".** Unclear whether it means the date counted, the date moved, or a pick for an order. Decides the data model.
7. **Q11, "ferdig talt".** Decides how it differs from confirming, whether it can be undone, and whether a lead reviews it.
8. **Q12, how the storage is walked.** Decides the aisle filter's labels ("Reol Tilbehør" or just "Tilbehør") and whether `D` and `DL` are one group.
9. **Q13, products without a barcode.** Decides whether to add anything for finding unscannable products, such as a filter. Typing is harder on a phone.
10. **Q14, products with zero units.** Decides whether to add a "hide zero" filter, or a faster way to confirm them.
11. **Q15, how products are searched for.** Confirms task 1 above and shows how much leniency search needs.

Ask Q1 to Q4 first; the rest of the work depends most on them.
