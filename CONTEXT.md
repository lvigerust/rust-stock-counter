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

**Barcode**:
The EAN printed on a product's packaging; scanning it identifies the product. Not every product has one.
_Avoid_: EAN, QR code

**Location**:
The shelf position code where a product is stored, such as `C4-7`.
_Avoid_: Lokasjon, bin, slot

**Aisle**:
The letters a location starts with, such as `C` in `C4-7`. The table can be filtered to some aisles. A product whose location is empty, or doesn't start with a letter, has no aisle.
_Avoid_: Reol, zone, rack

**System quantity**:
The number of units the business system says is in the storage for a product (FysiskPaaLager).
_Avoid_: Expected quantity, on hand, stock level

**Counted quantity**:
The number of units a counter has verified are physically in the storage for a product.
_Avoid_: Actual, physical quantity

**Difference**:
Counted quantity minus system quantity for a product; positive means more units on the shelf than the business system says.
_Avoid_: Variance, discrepancy, avvik

**Confirm**:
To accept the system quantity as the counted quantity for a product after checking the shelf.
_Avoid_: Approve, verify

**Uncounted**:
A product in the stocktake that nobody has confirmed or overwritten yet. A product counted as zero is counted, not uncounted.
_Avoid_: Pending, open, missing
