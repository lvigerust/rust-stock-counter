# Stocktake

Year-end verification that the number of units of each product physically in the storage matches what the business system says. At Scala Bad, the business system is MultiCase; the interface calls it _lagersystemet_ where it needs to name it at all.

The terms below are the canonical language for code and discussion. The people doing the stocktake see a Norwegian interface; Norwegian words listed under _Avoid_ belong in the interface, not the code.

## Language

**Stocktake**:
The year-end event of going through the storage and verifying every product's quantity against the system.
_Avoid_: Inventory, count (as a noun for the whole event), varetelling

**Stock list**:
The list of products exported from the business system that a stocktake starts from, one line per product.
_Avoid_: Vareliste, spreadsheet, export

**Product**:
A kind of item held in the storage, identified by its item number.
_Avoid_: Vare, article, SKU

**Item number**:
The business system's identifier for a product (VareNR).
_Avoid_: Product ID, article number

**Description**:
A product's colour and style (ProduktDesc2), which tells apart products with the same name. Not every product has one.
_Avoid_: Beskrivelse, variant, column E

**Barcode**:
The EAN printed on a product's packaging; scanning it identifies the product. Not every product has one.
_Avoid_: EAN, QR code

**Location**:
A place in the storage where units of a product are kept, usually a shelf position code such as `C4-7`. A product can be kept at more than one location.
_Avoid_: Lokasjon, bin, slot

**Pick location**:
The location a product is picked from. It starts as the one the stock list gives, which may be empty, or as the one chosen at import when the stock list gives several; a counter can move it at any time during the stocktake. What was counted there moves with it.
_Avoid_: Lager, plukklokasjon, main location, primary location, home location

**Overflow location**:
Any location other than its pick location where units of a product are found during a stocktake.
_Avoid_: Buffer, extra location, secondary location, new location

**Aisle**:
The letters a location starts with, such as `C` in `C4-7`. The table can be filtered to some aisles by pick location. A product whose pick location is empty, or doesn't start with a letter, has no aisle.
_Avoid_: Reol, zone, rack

**System quantity**:
The number of units the business system says is in the storage for a product (FysiskPaaLager).
_Avoid_: Expected quantity, on hand, stock level

**Counted quantity**:
The number of units a counter has verified are physically in the storage for a product: the sum of what was counted at each of its locations.
_Avoid_: Actual, physical quantity

**Difference**:
Counted quantity minus system quantity for a product; positive means more units on the shelf than the business system says.
_Avoid_: Variance, discrepancy, avvik

**Confirm**:
To accept the system quantity as the counted quantity for a product after checking the shelf.
_Avoid_: Approve, verify

**Uncounted**:
A product whose pick location nobody has confirmed or overwritten yet. A product counted as zero is counted, not uncounted.
_Avoid_: Pending, open, missing

**Partly counted**:
An uncounted product with units counted at one or more overflow locations. It's counted once its pick location is, if only as zero.
_Avoid_: Delvis talt, in progress

**Finished**:
A counted product a counter has marked as no longer worth looking for more units of, usually because its difference is close enough to accept. It can still be counted again.
_Avoid_: Ferdig talt, done, closed, approved, locked
