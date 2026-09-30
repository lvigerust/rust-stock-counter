//! Reads the stock list exported from MultiCase.
//!
//! The export is used as-is: the first sheet, headers in the first row, and
//! columns found by header name rather than position.

use std::{fmt, path::Path};

use calamine::{Data, Reader as _, open_workbook_auto};

use crate::stocktake::Product;

const ITEM_NUMBER: &str = "VareNR";
const NAME: &str = "ProduktDesc1";
const BARCODE: &str = "PrdEAN";
const LOCATION: &str = "Lokasjon";
const SYSTEM_QUANTITY: &str = "FysiskPaaLager";

/// Why a stock list couldn't be imported. `Display` is written for the
/// counter, in Norwegian.
#[derive(Debug)]
pub enum ImportError {
    Unreadable(String),
    MissingColumns(Vec<&'static str>),
    InvalidQuantity { row: usize, value: String },
    Empty,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(reason) => write!(f, "Filen kunne ikke leses: {reason}"),
            Self::MissingColumns(columns) => write!(
                f,
                "Filen mangler kolonnene {}. Bruk vareliste-eksporten fra MultiCase.",
                columns.join(", ")
            ),
            Self::InvalidQuantity { row, value } => write!(
                f,
                "Rad {row} har «{value}» i {SYSTEM_QUANTITY}, som ikke er et helt antall."
            ),
            Self::Empty => write!(f, "Varelisten inneholder ingen varer."),
        }
    }
}

pub fn read(path: &Path) -> Result<Vec<Product>, ImportError> {
    let mut workbook =
        open_workbook_auto(path).map_err(|error| ImportError::Unreadable(error.to_string()))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or(ImportError::Empty)?
        .map_err(|error| ImportError::Unreadable(error.to_string()))?;

    let mut rows = range.rows();
    let header = rows.next().ok_or(ImportError::Empty)?;
    let column = |name: &str| {
        header
            .iter()
            .position(|cell| text(cell).eq_ignore_ascii_case(name))
    };
    let (item_number, name, barcode, location, system_quantity) = match (
        column(ITEM_NUMBER),
        column(NAME),
        column(BARCODE),
        column(LOCATION),
        column(SYSTEM_QUANTITY),
    ) {
        (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a, b, c, d, e),
        found => {
            let missing = [
                (found.0, ITEM_NUMBER),
                (found.1, NAME),
                (found.2, BARCODE),
                (found.3, LOCATION),
                (found.4, SYSTEM_QUANTITY),
            ]
            .into_iter()
            .filter(|(ix, _)| ix.is_none())
            .map(|(_, name)| name)
            .collect();
            return Err(ImportError::MissingColumns(missing));
        }
    };

    let mut products = Vec::new();
    // Spreadsheet rows are 1-based and the header is row 1.
    for (row_ix, row) in rows.enumerate().map(|(ix, row)| (ix + 2, row)) {
        let cell = |ix: usize| row.get(ix).map(text).unwrap_or_default();
        if cell(item_number).is_empty() {
            // Blank lines at the end of an export.
            continue;
        }
        let quantity_cell = row.get(system_quantity).unwrap_or(&Data::Empty);
        let quantity = quantity(quantity_cell).ok_or_else(|| ImportError::InvalidQuantity {
            row: row_ix,
            value: text(quantity_cell),
        })?;
        products.push(Product::new(
            cell(item_number),
            cell(name),
            cell(location),
            cell(barcode),
            quantity,
        ));
    }

    if products.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(products)
}

/// A cell as trimmed text. Whole numbers lose their `.0`, so an item number
/// or barcode stored as a number reads the same as one stored as text.
fn text(cell: &Data) -> String {
    match cell {
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Int(value) => value.to_string(),
        Data::Empty => String::new(),
        other => other.to_string().trim().to_string(),
    }
}

/// An empty cell means nothing in stock.
fn quantity(cell: &Data) -> Option<i64> {
    match cell {
        Data::Empty => Some(0),
        Data::Int(value) => Some(*value),
        Data::Float(value) if value.fract() == 0.0 => Some(*value as i64),
        Data::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sample_export() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/Vareliste - varetelling.xlsx");
        let products = read(&path).expect("sample stock list imports");
        assert_eq!(products.len(), 43);

        let first = &products[0];
        assert_eq!(first.item_number(), "152066");
        assert_eq!(first.name(), "Burano 120 Hvit");
        assert_eq!(first.location(), "C4-7");
        assert_eq!(first.barcode(), "7043811520667");
        assert_eq!(first.system_quantity(), 33);
        assert!(!first.is_counted());
    }
}
