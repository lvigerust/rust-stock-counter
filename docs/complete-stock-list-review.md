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
6. **Resolve duplicate item numbers in a dialog at import.** (Q4) After the file is read, for each item number on more than one line, a dialog lists the lines and the person importing picks one location; the app doesn't suggest one, and the import doesn't finish until every duplicate is resolved. The chosen location becomes the product's pick location. Open before the data model is written: whether the product's system quantity is the chosen line's or the sum of both lines (`Como Fronter 60`: 0 or 16).
7. **Add a sidebar filter that hides products with zero system quantity.** (Q5, Q14) A checkbox next to the status filter, off by default. It also hides most `A1` lines.
8. **Read `ProduktDesc2` (column E) on import.** (Q7) By header name, as the product's description, with a test. Showing it waits for the rest of Q7.

## Tasks that need answers first

These wait for the counters' answers. The questions, in Norwegian and ready to hand out, are in [questions-for-counters.md](questions-for-counters.md); the numbers below match its headings. They cover both what the complete list showed and the counters' seven requirements from their first trial of the app.

1. **Q1 to Q3, counting from a phone or tablet.** Devices and scanning, what happens when the network drops, and what happens when several people count at once. These decide the cloud design: offline support and how conflicting counts are merged.
   - _Answered in part._ Counting is done mostly on an iPad, and the wifi reaches the whole storage.
   - _Decision:_ no offline queue in v1; the app needs the network. When two counters count the same product at the same location, the counts are added, the same as when one counter counts a location twice (the spec's "Enter adds"). Counts at different locations are kept apart already.
   - _Still open:_ how the barcode is scanned on the iPad (camera or a wireless scanner), how many count at once, who sees the whole stocktake, and whether login is needed. The cloud design's ADR waits for these.
2. **Q4, the same item number on two lines.** The counters want a duplicate check at import and the ability to delete a line. This decides whether the app suggests which line to delete, whether deleting can be undone, and who may delete.
   - _Answered._ The import checks for duplicates. For each duplicate product, the person importing picks the location explicitly, in a dialog.
   - _Decision:_ task 6 above. The app doesn't suggest a line. Nobody deletes lines later: the dialog at import is the only place, so there's no undo; importing the file again asks again.
   - _Still open:_ the product's system quantity after the choice (see task 6), and whether the export shows which location was chosen.
3. **Q5 and Q6, what `A1` and `N/A` mean.** Decide whether they are treated as "no location" for aisles and walk order, and whether products at `N/A` get a pick location while counting.
   - _Q5 answered in part._ `A1` is thought to be the system's default location for products without a place; not confirmed. The counters want a filter that hides products with zero stock.
   - _Decision:_ task 7 above. `A1` stays an ordinary location in aisle `A` until it's confirmed as a placeholder; the zero-stock filter hides most of its lines either way.
   - _Q6 still open._
4. **Q7, the extra column.** "Kolonne 3" is `ProduktDesc2` (column E), which holds text like «Porselen servant i Brun Matt». Confirmed. Decides where it is shown and whether it is searchable.
   - _Decision:_ task 8 above reads the column.
   - _Still open:_ where it's shown (table, count dialog, or both) and whether search matches it.
5. **Q8 and Q9, changing the pick location and marking it.** Decide where the red "NY PLUKKLOKASJON" mark is shown (table, count dialog, export), and what the follow-up list for the business system should contain.
6. **Q10, the date a product was "picked".** Unclear whether it means the date counted, the date moved, or a pick for an order. Decides the data model.
7. **Q11, "ferdig talt".** Decides how it differs from confirming, whether it can be undone, and whether a lead reviews it.
8. **Q12, how the storage is walked.** Decides the aisle filter's labels ("Reol Tilbehør" or just "Tilbehør") and whether `D` and `DL` are one group.
9. **Q13, products without a barcode.** Decides whether to add anything for finding unscannable products, such as a filter. Typing is harder on a phone.
10. **Q14, products with zero units.** Decides whether to add a "hide zero" filter, or a faster way to confirm them.
    - _Answered in part, through Q5:_ the filter is task 7 above. Whether counters walk to an empty shelf to check it is still open.
11. **Q15, how products are searched for.** Confirms task 1 above and shows how much leniency search needs.

Ask Q1 to Q4 first; the rest of the work depends most on them.

## Follow-up task: resolve the questions

**Priority: high.** Most of the work in the counters' seven requirements waits on this.

**Goal:** get an answer to every question in [questions-for-counters.md](questions-for-counters.md), and turn the answers into decisions the code can follow.

**Answered so far:** Q2, Q4 and Q7 in full; Q1, Q3 and Q5 in part. The decisions are under the question numbers above.

### Order of the questions

Ask in this order. A priority is higher when more work is blocked by the answer, or when a wrong guess is expensive to undo.

| Priority | Questions | Why this order |
| --- | --- | --- |
| **P1** | Q1 to Q3 (counting from a phone or tablet) | Decides the cloud design: offline support, how conflicting counts are merged, who can see the count. A wrong guess changes the architecture. |
| **P1** | Q4 (the same item number on two lines) | Requirement 2. Decides the import step, whether the app suggests which line to delete, and whether deleting can be undone. It changes the data model. |
| **P2** | Q10 (the date a product was "picked") | Requirement 5. The meaning is unclear and decides the data model. |
| **P2** | Q11 ("ferdig talt") | Requirement 6. Adds a state to the counting model, so `CONTEXT.md` must settle it before any code. |
| **P2** | Q8 and Q9 (changing the pick location and marking it) | Requirements 4 and 7. The behavior mostly exists; the open part is where the red mark shows and what the follow-up list contains. |
| **P2** | Q7 (the extra column) | Requirement 3. One quick confirmation that "kolonne 3" is `ProduktDesc2` (column E). Ask by message, don't wait for a session. |
| **P3** | Q5 and Q6 (what `A1` and `N/A` mean) | Decide how placeholders are treated in aisles and walk order. The filter is wrong today for 612 products. |
| **P3** | Q12 (how the storage is walked) | Decides the aisle filter's labels and grouping. |
| **P4** | Q13 to Q15 (no barcode, zero stock, search) | Improvements to finding and confirming products. Nothing else depends on them. |

### Steps

1. Present the P1 questions first, then the P2 ones. Q7 goes out right away, on its own.
2. Write each answer under its `Svar:` line in `questions-for-counters.md`, as the counters gave it.
3. For each answer, write a decision in this document under the question's number: what the app will do, in the English terms of `CONTEXT.md`. Where the counters disagree or are unsure, say so rather than picking one.
4. Record the lasting decisions:
   - New or changed terms (a "finished counting" state, a deleted product line, the date the requirements call "plukket") go in `CONTEXT.md`.
   - Choices that were hard to make, such as the cloud design and how conflicting counts are merged, go in an ADR.
   - Behavior changes go in `docs/spec.md`.
5. Turn the decisions into tasks and move each one to the "Tasks that can be done now" list when nothing blocks it.

**Done when** every question has an answer and a decision, `CONTEXT.md` and `docs/spec.md` match them, and the resulting tasks are listed. P1 and P2 are the milestone that unblocks implementation; P3 and P4 can follow.
