//! Writes the counted stock list for entering adjustments into MultiCase.

use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, XlsxError};

use crate::Stocktake;
use crate::stock_list::column;

/// Written in place of a counted quantity for uncounted products.
pub const UNCOUNTED_MARK: &str = "Ikke telt";

/// Writes the stock list in location order with the counted quantity and the
/// difference added. Uncounted products are marked and highlighted, and
/// their difference is left empty.
pub fn write(stocktake: &Stocktake, path: &Path) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Varetelling")?;

    let header = Format::new()
        .set_bold()
        .set_border_bottom(FormatBorder::Thin);
    let uncounted = Format::new().set_background_color(Color::RGB(0xFFF2CC));

    // The stock list's own headers, so the file reads like the export it
    // came from.
    let columns = [
        (column::LOCATION, 12.0),
        (column::ITEM_NUMBER, 12.0),
        (column::NAME, 36.0),
        (column::BARCODE, 16.0),
        (column::SYSTEM_QUANTITY, 16.0),
        ("Telt antall", 12.0),
        ("Differanse", 12.0),
    ];
    for (col, (name, width)) in (0u16..).zip(columns) {
        sheet.write_string_with_format(0, col, name, &header)?;
        sheet.set_column_width(col, width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    for (row, id) in (1u32..).zip(stocktake.search("")) {
        let product = stocktake.product(id);
        sheet.write_string(row, 0, product.location())?;
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

    workbook.save(path)
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
        stocktake.set_counted_quantity(counted, 7);

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
    }
}
