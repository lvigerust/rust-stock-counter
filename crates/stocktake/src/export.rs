//! Writes the counted stock list for entering adjustments into the business system.

use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, XlsxError};

use crate::Stocktake;
use crate::stock_list::column;

/// Written in place of a counted quantity for uncounted products.
pub const UNCOUNTED_MARK: &str = "Ikke talt";

/// Marks a location on the locations sheet as the product's pick location.
pub const PICK_LOCATION_MARK: &str = "Plukklokasjon";

/// Marks a location on the locations sheet as one of the product's overflow
/// locations.
pub const OVERFLOW_LOCATION_MARK: &str = "Overflyt";

/// Writes the stock list in location order with the counted quantity and the
/// difference added. Uncounted products are marked and highlighted, and
/// their difference is left empty.
///
/// If any product was counted at overflow locations, a second sheet lists
/// where its units were found, for updating its location in the business
/// system.
pub fn write(stocktake: &Stocktake, path: &Path) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_border_bottom(FormatBorder::Thin);
    write_counts(&mut workbook, stocktake, &header)?;
    if stocktake
        .products()
        .any(|(_, product)| product.overflow_len() > 0)
    {
        write_locations(&mut workbook, stocktake, &header)?;
    }
    workbook.save(path)
}

fn write_counts(
    workbook: &mut Workbook,
    stocktake: &Stocktake,
    header: &Format,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Varetelling")?;
    let uncounted = Format::new().set_background_color(Color::RGB(0xFFF2CC));

    // The stock list's own headers, so the file reads like the export it
    // came from.
    let columns = [
        (column::LOCATION, 12.0),
        (column::ITEM_NUMBER, 12.0),
        (column::NAME, 36.0),
        (column::BARCODE, 16.0),
        (column::SYSTEM_QUANTITY, 16.0),
        ("Talt antall", 12.0),
        ("Differanse", 12.0),
    ];
    for (col, (name, width)) in (0u16..).zip(columns) {
        sheet.write_string_with_format(0, col, name, header)?;
        sheet.set_column_width(col, width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    for (row, id) in (1u32..).zip(stocktake.search("")) {
        let product = stocktake.product(id);
        // The stock list's own, until the export reports moves.
        sheet.write_string(row, 0, product.listed_location())?;
        sheet.write_string(row, 1, product.item_number())?;
        sheet.write_string(row, 2, product.name())?;
        sheet.write_string(row, 3, product.barcode())?;
        sheet.write_number(row, 4, product.system_quantity() as f64)?;
        match (product.counted_quantity(), product.difference()) {
            (Some(counted), Some(difference)) => {
                sheet.write_number(row, 5, counted as f64)?;
                sheet.write_number(row, 6, difference as f64)?;
            }
            _ => {
                sheet.write_string_with_format(row, 5, UNCOUNTED_MARK, &uncounted)?;
                sheet.write_blank(row, 6, &uncounted)?;
            }
        }
    }
    Ok(())
}

/// One row for each location a product split across locations was counted
/// at, the pick location first. A pick location not yet counted is marked
/// as uncounted rather than left out, so it isn't read as empty.
fn write_locations(
    workbook: &mut Workbook,
    stocktake: &Stocktake,
    header: &Format,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Lokasjoner")?;

    let columns = [
        (column::ITEM_NUMBER, 12.0),
        (column::NAME, 36.0),
        (column::LOCATION, 12.0),
        ("Lokasjonstype", 16.0),
        ("Talt antall", 12.0),
    ];
    for (col, (name, width)) in (0u16..).zip(columns) {
        sheet.write_string_with_format(0, col, name, header)?;
        sheet.set_column_width(col, width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    let split = stocktake
        .search("")
        .into_iter()
        .map(|id| stocktake.product(id))
        .filter(|product| product.overflow_len() > 0);
    let mut row = 1u32;
    for product in split {
        let pick = (
            product.location(),
            PICK_LOCATION_MARK,
            product.count_at(product.location()),
        );
        let overflow = product
            .overflow_counts()
            .map(|(location, quantity)| (location, OVERFLOW_LOCATION_MARK, Some(quantity)));
        for (location, mark, quantity) in std::iter::once(pick).chain(overflow) {
            sheet.write_string(row, 0, product.item_number())?;
            sheet.write_string(row, 1, product.name())?;
            sheet.write_string(row, 2, location)?;
            sheet.write_string(row, 3, mark)?;
            match quantity {
                Some(quantity) => sheet.write_number(row, 4, quantity as f64)?,
                None => sheet.write_string(row, 4, UNCOUNTED_MARK)?,
            };
            row += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Product, Stocktake};

    #[test]
    fn exports_counted_and_uncounted_products() {
        let mut stocktake = Stocktake::new(vec![
            Product::new("1", "Counted", "A1", "", 5),
            Product::new("2", "Uncounted", "A2", "", 3),
        ]);
        let counted = stocktake.search("1")[0];
        stocktake.set_count(counted, "", 7);

        let path =
            std::env::temp_dir().join(format!("stocktake-export-{}.xlsx", std::process::id()));
        write(&stocktake, &path).unwrap();

        use calamine::{Data, Reader as _, open_workbook_auto};
        let mut workbook = open_workbook_auto(&path).unwrap();
        let range = workbook.worksheet_range_at(0).unwrap().unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(range.get((1, 5)), Some(&Data::Float(7.0)));
        assert_eq!(range.get((1, 6)), Some(&Data::Float(2.0)));
        assert_eq!(
            range.get((2, 5)),
            Some(&Data::String(UNCOUNTED_MARK.to_string()))
        );
        // Nothing was found off its pick location, so there's no second sheet.
        assert_eq!(workbook.sheet_names().len(), 1);
    }

    #[test]
    fn lists_the_locations_of_products_split_across_them() {
        let mut stocktake = Stocktake::new(vec![
            Product::new("1", "Split", "A1", "", 5),
            Product::new("2", "In place", "A2", "", 3),
            Product::new("3", "Moved", "A3", "", 4),
        ]);
        let ids = stocktake.search("");
        stocktake.set_count(ids[0], "A1", 3);
        stocktake.set_count(ids[0], "D2-1", 2);
        stocktake.set_count(ids[1], "A2", 3);
        stocktake.set_count(ids[2], "B1", 4);

        let path = std::env::temp_dir().join(format!(
            "stocktake-export-locations-{}.xlsx",
            std::process::id()
        ));
        write(&stocktake, &path).unwrap();

        use calamine::{Data, Reader as _, open_workbook_auto};
        let mut workbook = open_workbook_auto(&path).unwrap();
        let counts = workbook.worksheet_range_at(0).unwrap().unwrap();
        let locations = workbook.worksheet_range("Lokasjoner").unwrap();
        std::fs::remove_file(&path).ok();

        // The main sheet has the totals.
        assert_eq!(counts.get((1, 5)), Some(&Data::Float(5.0)));
        let rows: Vec<Vec<String>> = locations
            .rows()
            .skip(1)
            .map(|row| row.iter().map(|cell| cell.to_string()).collect())
            .collect();
        assert_eq!(
            rows,
            [
                ["1", "Split", "A1", PICK_LOCATION_MARK, "3"],
                ["1", "Split", "D2-1", OVERFLOW_LOCATION_MARK, "2"],
                ["3", "Moved", "A3", PICK_LOCATION_MARK, UNCOUNTED_MARK],
                ["3", "Moved", "B1", OVERFLOW_LOCATION_MARK, "4"],
            ]
        );
    }
}
