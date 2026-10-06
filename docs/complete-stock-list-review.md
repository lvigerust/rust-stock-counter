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
6. **Merge duplicate item numbers into one product, and pick its pick location in a dialog at import.** (Q4) After the file is read, the lines of each item number on more than one line become one product whose system quantity is their sum (`Como Fronter 60`: 0 at `A1` plus 16 at `E2-8` is 16). A dialog lists the lines and the person importing picks which location is the pick location; the app doesn't suggest one, and the import doesn't finish until every duplicate is resolved. The other lines' locations stay on the product as its stock-list locations, and the count dialog lets any counter switch the pick location to one of them, which is the undo the counters asked for. Nothing is deleted, so nothing is lost by picking wrong.
7. **Add a sidebar filter that hides products with zero system quantity.** (Q5, Q14) A checkbox next to the status filter, off by default. It also hides most `A1` lines.
8. **Read `ProduktDesc2` (column E) on import.** (Q7) By header name, as the product's description, with a test.
9. **Show the description.** (Q7) A column after the name in the table, on by default, and a line under the name in the count dialog. Search matches it, which the search key from task 3 makes cheap.
10. **Read `N/A` in `Lokasjon` as no location.** (Q6) On import, with a test. The aisle filter then stops offering "Reol N (612)", and the 612 products sort last, with the other unlocated products, where the count dialog's location field already starts empty for them.
11. **Label the aisle filter by what the location starts with.** (Q12) "Reol A" for a letter; `DL`, `Tilbehør` and `Pakkedisk` as they're written, without "Reol". Typing "tilbehør" in the search already matches locations.
12. **Match typed locations case-insensitively.** (Q6, Q12) Locations such as `Tilbehør 1-1` and the new ones the counters will create («golv ytre lager») aren't upper-case codes. A location typed in the count dialog that matches one already in the stocktake, ignoring case, takes that spelling; otherwise it's trimmed and upper-cased as today.
13. **Record when each count was made.** (Q10) The app stamps every count at a location with the time it was saved. The export shows the date counted: the product's latest count on the main sheet, each location's on the Lokasjoner sheet. It isn't shown in the app.
14. **Mark moved pick locations in red in the export.** (Q8, Q9) The main sheet keeps the stock list's `Lokasjon` and adds a `Ny plukklokasjon` column, in red, for products whose pick location was moved. The Lokasjoner sheet shows the move too, in red. This is the list the business system is updated from.
15. **Add the finished state.** (Q11) A **Ferdig talt** button in the count dialog, enabled once the product is counted, marks it finished: the counter has stopped looking for more units because the difference is close enough. A finished product gets its own status in the table and the status filter, so counted-but-not-finished products are one checkbox away. It stays editable: counting again leaves the mark alone, and the same button unmarks it. The term goes in `CONTEXT.md` first.

## Tasks that need answers first

These wait for the counters' answers. The questions, in Norwegian and ready to hand out, are in [questions-for-counters.md](questions-for-counters.md); the numbers below match its headings. They cover both what the complete list showed and the counters' seven requirements from their first trial of the app.

1. **Q1 to Q3, counting from a phone or tablet.** Devices and scanning, what happens when the network drops, and what happens when several people count at once. These decide the cloud design: offline support and how conflicting counts are merged.
   - _Answered._ They've counted on paper until now, and want to count on iPads or phones because a laptop's battery doesn't last. The barcode is scanned with the device's camera if possible; wireless scanners can be bought. At most four people count at once. The wifi is stable over the whole storage. Anyone with the link can see the stocktake; no login.
   - _Decision:_ no offline queue in v1; the app needs the network. When two counters count the same product at the same location, the counts are added, the same as when one counter counts a location twice (the spec's "Enter adds"). Counts at different locations are kept apart already. No login and no counter identity in v1: a link is the access, and nobody asked to see who counted what. A wireless scanner acts as a keyboard, so the search field keeps working for it; camera scanning is new.
   - _Platform decided:_ a native iPad app on CloudKit, with the rules ported from `crates/stocktake`; the desktop app stays as the design being ported. [ADR 0001](adr/0001-native-ipad-app.md). The tasks above still describe behaviour, so they hold for the port; which of them land in the desktop app first is a separate choice.
2. **Q4, the same item number on two lines.** The counters want a duplicate check at import and the ability to delete a line. This decides whether the app suggests which line to delete, whether deleting can be undone, and who may delete.
   - _Answered._ The import checks for duplicates. For each duplicate product, the person importing picks the location explicitly, in a dialog; they know the storage, and it's mostly the line with 0 that goes. Deleting must be undoable, by anyone.
   - _Decision:_ task 6 above, reworked from the first round. The lines are merged, not deleted: the system quantity is their sum, so the difference is right whichever line is picked, and the choice only sets the pick location. Undo is switching the pick location to the other line from the count dialog, which any counter can do. The app doesn't suggest a line.
   - _Still open:_ how the export reports the line that wasn't picked, so it can be cleared in the business system.
3. **Q5 and Q6, what `A1` and `N/A` mean.** Decide whether they are treated as "no location" for aisles and walk order, and whether products at `N/A` get a pick location while counting.
   - _Q5 answered._ `A1` is the default location new products get when they're received, and a physical location too. Some are never moved to another location in the system because the counters know where they are. The counters want a filter that hides products with zero stock.
   - _Decision:_ `A1` stays an ordinary location in aisle `A`. The zero-stock filter is task 7 above.
   - _Q6 answered._ `N/A` means the product has no location at all. Those products are counted in the same round and get a location during the stocktake; new locations such as «golv ytre lager» will be created as they go.
   - _Decision:_ tasks 10 and 12 above. `N/A` is read as no location. The existing flow sets the pick location: the count dialog's location field starts empty for an unlocated product, and typing a location with **Erstatt plukklokasjon** checked moves it there. Any text is a location, so new ones need nothing extra.
4. **Q7, the extra column.** "Kolonne 3" is `ProduktDesc2` (column E), which holds text like «Porselen servant i Brun Matt». Confirmed. Decides where it is shown and whether it is searchable.
   - _Answered._ Column E holds colour and style and is important for counting: three products are named «Como Fronter 120 - Grå Driftwood» and only column E tells them apart. The counters will use iPads and maybe a PC, so the screen isn't as small as the question assumed.
   - _Decision:_ tasks 8 and 9 above. The column is read, shown in the table and the count dialog, and searched.
5. **Q8 and Q9, changing the pick location and marking it.** Decide where the red "NY PLUKKLOKASJON" mark is shown (table, count dialog, export), and what the follow-up list for the business system should contain.
   - _Answered in part._ Products are moved to new locations all the time without the pick location being changed in MultiCase. A moved pick location must show in red in the exported Excel sheet, for printing too.
   - _Decision:_ task 14 above. The app's move flow stands as it is; the export carries the mark, with the stock list's location kept next to the new one, since that's what the business system is updated from. Nothing extra in the app: the table already shows the moved pick location, and nobody asked for the mark there.
   - _Still open:_ whether they move the goods before or after typing the new location (the app doesn't care), who updates the business system afterwards, and whether a product moved back to its stock-list location should still be marked. Until told otherwise, it isn't: there's nothing to update.
6. **Q10, the date a product was "picked".** Unclear whether it means the date counted, the date moved, or a pick for an order. Decides the data model.
   - _Answered._ It's the date the product was counted, and it only needs to show in the export and on paper.
   - _Decision:_ task 13 above. The app stamps every count automatically; a date without a time is what's shown. Keeping the time of each count costs nothing and answers "which count was last".
7. **Q11, "ferdig talt".** Decides how it differs from confirming, whether it can be undone, and whether a lead reviews it.
   - _Answered in part._ When a product is counted and the difference is within what they believe can be right, they stop counting and looking for that item number. The quantity must still be editable afterwards.
   - _Decision:_ task 15 above. Finished is a state of its own, after counted: counted says the pick location has a number, finished says nobody is looking for more. It's set by hand, never by the app, and never locks anything.
   - _Still open:_ whether a lead reviews finished products. Nothing is built for that until asked.
8. **Q12, how the storage is walked.** Decides the aisle filter's labels ("Reol Tilbehør" or just "Tilbehør") and whether `D` and `DL` are one group.
   - _Answered._ There's no fixed order. They'd search for «tilbehør», or for a product group such as «Polito».
   - _Decision:_ task 11 above. `D` and `DL` stay separate groups. A product group is the first word of the name in practice («Polito», «Burano», «Como»), and the search already matches names, so nothing is added for it.
9. **Q13, products without a barcode.** Decides whether to add anything for finding unscannable products, such as a filter. Typing is harder on a phone.
   - _Answered._ They go by the shelf.
   - _Decision:_ nothing new. The aisle filter, the walking order and location search are how a shelf is found.
10. **Q14, products with zero units.** Decides whether to add a "hide zero" filter, or a faster way to confirm them.
    - _Answered._ Zero-stock products are dealt with when the counters consider the stocktake done and those products still stand at 0.
    - _Decision:_ the filter, task 7 above, hides them while counting. At the end it's switched off and what's left is confirmed. A later idea, not a task: confirm every remaining zero-stock product as zero in one go.
11. **Q15, how products are searched for.** Confirms task 1 above and shows how much leniency search needs.
    - _Answered in part._ They type part of the name, such as «Burano». Whether they mix spaces or letter case wasn't answered.
    - _Decision:_ task 1 above. Search is already case-insensitive, so no further leniency is added.

## Follow-up task: resolve the questions

**Priority: high.** Most of the work in the counters' seven requirements waits on this.

**Goal:** get an answer to every question in [questions-for-counters.md](questions-for-counters.md), and turn the answers into decisions the code can follow.

**Answered so far:** every question, after two rounds; Q8, Q9, Q11 and Q15 in part. The decisions are under the question numbers above, and the tasks they produced are 6 to 15 in the list that can be done now. The platform is decided in [ADR 0001](adr/0001-native-ipad-app.md). What's left are the open points under Q4, Q8, Q9 and Q11, none of which blocks a task.

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
