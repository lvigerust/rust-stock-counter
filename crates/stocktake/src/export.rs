//! Writes the counted stock list for entering adjustments into the business system.

use std::path::Path;

use chrono::{DateTime, Datelike as _, Local, Utc};
use rust_xlsxwriter::{Color, ExcelDateTime, Format, FormatBorder, Workbook, Worksheet, XlsxError};

use crate::stock_list::column;
use crate::{Product, Stocktake};

/// Written in place of a counted quantity for uncounted products.
pub const UNCOUNTED_MARK: &str = "Ikke talt";

/// Marks a location on the locations sheet as the product's pick location.
pub const PICK_LOCATION_MARK: &str = "Plukklokasjon";

/// Marks a location on the locations sheet as one of the product's overflow
/// locations.
pub const OVERFLOW_LOCATION_MARK: &str = "Overflyt";

/// Heads the column with the day a product was last counted.
pub const DATE_COUNTED: &str = "Dato talt";

/// Heads the column with a product's pick location when a counter moved it.
pub const NEW_PICK_LOCATION: &str = "Ny plukklokasjon";

/// The formats the sheets share.
struct Formats {
    header: Format,
    /// Highlights what's missing for an uncounted product.
    uncounted: Format,
    /// A day, as the counters write dates.
    date: Format,
    /// A moved pick location, red so it stands out on paper too.
    moved: Format,
}

impl Formats {
    fn new() -> Self {
        Self {
            header: Format::new()
                .set_bold()
                .set_border_bottom(FormatBorder::Thin),
            uncounted: Format::new().set_background_color(Color::RGB(0xFFF2CC)),
            date: Format::new().set_num_format("dd.mm.yyyy"),
            moved: Format::new().set_font_color(Color::Red),
        }
    }
}

/// Writes the stock list in location order with the counted quantity, the
/// difference, the day it was counted and any new pick location added.
/// Uncounted products are marked and highlighted, and their difference is
/// left empty.
///
/// If any product was counted at overflow locations, a second sheet lists
/// where its units were found, for updating its location in the business
/// system.
pub fn write(stocktake: &Stocktake, path: &Path) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let formats = Formats::new();
    write_counts(&mut workbook, stocktake, &formats)?;
    if stocktake
        .products()
        .any(|(_, product)| product.overflow_len() > 0)
    {
        write_locations(&mut workbook, stocktake, &formats)?;
    }
    workbook.save(path)
}

fn write_header(
    sheet: &mut Worksheet,
    columns: &[(&str, f64)],
    formats: &Formats,
) -> Result<(), XlsxError> {
    for (col, (name, width)) in (0u16..).zip(columns) {
        sheet.write_string_with_format(0, col, *name, &formats.header)?;
        sheet.set_column_width(col, *width)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    Ok(())
}

/// Writes the day of `counted_at` where the counters are, or nothing when
/// the count is from before counts were stamped.
fn write_date(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    counted_at: Option<DateTime<Utc>>,
    formats: &Formats,
) -> Result<(), XlsxError> {
    let Some(counted_at) = counted_at else {
        return Ok(());
    };
    let date = counted_at.with_timezone(&Local).date_naive();
    let date = ExcelDateTime::from_ymd(
        u16::try_from(date.year()).unwrap_or_default(),
        date.month() as u8,
        date.day() as u8,
    )?;
    sheet.write_datetime_with_format(row, col, &date, &formats.date)?;
    Ok(())
}

/// Whether a counter moved the product's pick location away from the one
/// the stock list gives. A product moved back isn't moved.
fn is_moved(product: &Product) -> bool {
    product.location() != product.listed_location()
}

fn write_counts(
    workbook: &mut Workbook,
    stocktake: &Stocktake,
    formats: &Formats,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Varetelling")?;

    // The stock list's own headers, so the file reads like the export it
    // came from.
    write_header(
        sheet,
        &[
            (column::LOCATION, 12.0),
            (column::ITEM_NUMBER, 12.0),
            (column::NAME, 36.0),
            (column::DESCRIPTION, 30.0),
            (column::BARCODE, 16.0),
            (column::SYSTEM_QUANTITY, 16.0),
            ("Talt antall", 12.0),
            ("Differanse", 12.0),
            (DATE_COUNTED, 12.0),
            (NEW_PICK_LOCATION, 18.0),
        ],
        formats,
    )?;

    for (row, id) in (1u32..).zip(stocktake.search("")) {
        let product = stocktake.product(id);
        // The stock list's own; a move goes in the last column.
        sheet.write_string(row, 0, product.listed_location())?;
        sheet.write_string(row, 1, product.item_number())?;
        sheet.write_string(row, 2, product.name())?;
        sheet.write_string(row, 3, product.description())?;
        sheet.write_string(row, 4, product.barcode())?;
        sheet.write_number(row, 5, product.system_quantity() as f64)?;
        match (product.counted_quantity(), product.difference()) {
            (Some(counted), Some(difference)) => {
                sheet.write_number(row, 6, counted as f64)?;
                sheet.write_number(row, 7, difference as f64)?;
            }
            _ => {
                sheet.write_string_with_format(row, 6, UNCOUNTED_MARK, &formats.uncounted)?;
                sheet.write_blank(row, 7, &formats.uncounted)?;
            }
        }
        write_date(sheet, row, 8, product.latest_counted_at(), formats)?;
        if is_moved(product) {
            sheet.write_string_with_format(row, 9, product.location(), &formats.moved)?;
        }
    }
    Ok(())
}

/// One row for each location a product split across locations was counted
/// at, the pick location first. A pick location not yet counted is marked
/// as uncounted rather than left out, so it isn't read as empty. A moved
/// pick location's row is red.
fn write_locations(
    workbook: &mut Workbook,
    stocktake: &Stocktake,
    formats: &Formats,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Lokasjoner")?;
    write_header(
        sheet,
        &[
            (column::ITEM_NUMBER, 12.0),
            (column::NAME, 36.0),
            (column::LOCATION, 12.0),
            ("Lokasjonstype", 16.0),
            ("Talt antall", 12.0),
            (DATE_COUNTED, 12.0),
        ],
        formats,
    )?;
    let plain = Format::new();

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
            let format = if mark == PICK_LOCATION_MARK && is_moved(product) {
                &formats.moved
            } else {
                &plain
            };
            sheet.write_string_with_format(row, 0, product.item_number(), format)?;
            sheet.write_string_with_format(row, 1, product.name(), format)?;
            sheet.write_string_with_format(row, 2, location, format)?;
            sheet.write_string_with_format(row, 3, mark, format)?;
            match quantity {
                Some(quantity) => {
                    sheet.write_number_with_format(row, 4, quantity as f64, format)?
                }
                None => sheet.write_string_with_format(row, 4, UNCOUNTED_MARK, format)?,
            };
            write_date(sheet, row, 5, product.counted_at(location), formats)?;
            row += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use calamine::{Data, Reader as _, open_workbook_auto};
    use chrono::NaiveDate;

    use super::*;
    use crate::{Product, Stocktake};

    /// Today where the counters are, as Excel stores a date: days since
    /// 1899-12-30.
    fn today_as_excel_days() -> f64 {
        let epoch = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
        (Local::now().date_naive() - epoch).num_days() as f64
    }

    fn excel_days(cell: Option<&Data>) -> Option<f64> {
        match cell {
            Some(Data::DateTime(date)) => Some(date.as_f64()),
            Some(Data::Float(days)) => Some(*days),
            _ => None,
        }
    }

    #[test]
    fn exports_counted_and_uncounted_products() {
        let mut stocktake = Stocktake::new(vec![
            Product::new("1", "Counted", "A1", "", 5).with_description("Hvit"),
            Product::new("2", "Uncounted", "A2", "", 3),
            Product::new("3", "Moved", "A3", "", 1),
        ]);
        let ids = stocktake.search("");
        stocktake.set_count(ids[0], "", 7);
        stocktake.set_count(ids[2], "B1", 1);
        assert!(stocktake.move_pick_location(ids[2], "B1"));

        let path =
            std::env::temp_dir().join(format!("stocktake-export-{}.xlsx", std::process::id()));
        write(&stocktake, &path).unwrap();

        let mut workbook = open_workbook_auto(&path).unwrap();
        let range = workbook.worksheet_range_at(0).unwrap().unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(range.get((1, 3)), Some(&Data::String("Hvit".to_string())));
        assert_eq!(range.get((1, 6)), Some(&Data::Float(7.0)));
        assert_eq!(range.get((1, 7)), Some(&Data::Float(2.0)));
        assert_eq!(excel_days(range.get((1, 8))), Some(today_as_excel_days()));
        // Not moved, so the last column is empty.
        assert!(range.get((1, 9)).is_none_or(|cell| *cell == Data::Empty));

        assert_eq!(
            range.get((2, 6)),
            Some(&Data::String(UNCOUNTED_MARK.to_string()))
        );
        assert!(range.get((2, 8)).is_none_or(|cell| *cell == Data::Empty));

        // The stock list's location stays; the move is its own column.
        assert_eq!(range.get((3, 0)), Some(&Data::String("A3".to_string())));
        assert_eq!(range.get((3, 9)), Some(&Data::String("B1".to_string())));
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

        let mut workbook = open_workbook_auto(&path).unwrap();
        let counts = workbook.worksheet_range_at(0).unwrap().unwrap();
        let locations = workbook.worksheet_range("Lokasjoner").unwrap();
        std::fs::remove_file(&path).ok();

        // The main sheet has the totals.
        assert_eq!(counts.get((1, 6)), Some(&Data::Float(5.0)));
        let rows: Vec<Vec<String>> = locations
            .rows()
            .skip(1)
            .map(|row| row.iter().take(5).map(|cell| cell.to_string()).collect())
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
        // Each counted location has its day; the uncounted pick location
        // has none.
        assert_eq!(
            excel_days(locations.get((1, 5))),
            Some(today_as_excel_days())
        );
        assert_eq!(
            excel_days(locations.get((2, 5))),
            Some(today_as_excel_days())
        );
        assert!(
            locations
                .get((3, 5))
                .is_none_or(|cell| *cell == Data::Empty)
        );
        assert_eq!(
            excel_days(locations.get((4, 5))),
            Some(today_as_excel_days())
        );
    }
}
