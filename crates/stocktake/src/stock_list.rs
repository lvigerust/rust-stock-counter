//! Reads the stock list exported from MultiCase.
//!
//! The export is used as-is: the first sheet, headers in the first row, and
//! columns found by header name rather than position, so MultiCase can add,
//! drop or reorder other columns without breaking the import.

use std::{error::Error, fmt, path::Path};

use calamine::{Data, Reader as _, open_workbook_auto};

use crate::Product;

/// The headers the import reads, as MultiCase names them.
pub mod column {
    pub const ITEM_NUMBER: &str = "VareNR";
    pub const NAME: &str = "ProduktDesc1";
    pub const BARCODE: &str = "PrdEAN";
    pub const LOCATION: &str = "Lokasjon";
    pub const SYSTEM_QUANTITY: &str = "FysiskPaaLager";
}

/// The file extensions [`read`] accepts: Excel's formats, which MultiCase
/// exports to.
pub const EXTENSIONS: [&str; 3] = ["xlsx", "xlsm", "xls"];

/// Whether `path` has an extension [`read`] accepts. Says nothing about the
/// file's contents.
pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

/// Why a stock list couldn't be imported.
///
/// `Display` is for logs and developers. The interface words each case for
/// the counter itself.
#[derive(Debug)]
pub enum ImportError {
    /// Nothing exists at the path, such as a file moved since it was chosen.
    NotFound,
    /// The file isn't one of the [`EXTENSIONS`].
    UnsupportedFormat,
    /// The file exists but isn't a workbook that can be read, such as a
    /// damaged or password-protected one.
    Unreadable(calamine::Error),
    /// The first row lacks some of the headers the import needs, listed in
    /// the order the spreadsheet would show them.
    MissingColumns(Vec<&'static str>),
    /// A system quantity that isn't a whole number. `row` is the
    /// spreadsheet's own 1-based row number, as the counter sees it in Excel.
    InvalidQuantity { row: usize, value: String },
    /// The first sheet has no products: no rows, only headers, or only rows
    /// without an item number.
    Empty,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "the file doesn't exist"),
            Self::UnsupportedFormat => {
                write!(f, "the file isn't a workbook ({})", EXTENSIONS.join(", "))
            }
            Self::Unreadable(error) => write!(f, "the workbook can't be read: {error}"),
            Self::MissingColumns(columns) => {
                write!(f, "missing columns: {}", columns.join(", "))
            }
            Self::InvalidQuantity { row, value } => write!(
                f,
                "row {row}: {:?} in {} isn't a whole number",
                value,
                column::SYSTEM_QUANTITY
            ),
            Self::Empty => write!(f, "the stock list has no products"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable(error) => Some(error),
            _ => None,
        }
    }
}

/// Reads the products from a MultiCase stock list export.
///
/// Rows without an item number are skipped, since exports can end in blank
/// lines. Every other row must have a whole-number system quantity; an empty
/// one means nothing in stock.
pub fn read(path: &Path) -> Result<Vec<Product>, ImportError> {
    if !path.exists() {
        return Err(ImportError::NotFound);
    }
    if !is_supported(path) {
        return Err(ImportError::UnsupportedFormat);
    }
    let mut workbook = open_workbook_auto(path).map_err(ImportError::Unreadable)?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or(ImportError::Empty)?
        .map_err(ImportError::Unreadable)?;

    let mut rows = range.rows();
    let header = rows.next().ok_or(ImportError::Empty)?;
    let columns = Columns::find(header)?;

    let mut products = Vec::new();
    // Spreadsheet rows are 1-based and the header is row 1. A range starts at
    // its first non-empty row, which for an export is the header.
    let first_row = range.start().map_or(0, |(row, _)| row as usize) + 2;
    for (row_number, row) in (first_row..).zip(rows) {
        if let Some(product) = columns.product(row, row_number)? {
            products.push(product);
        }
    }

    if products.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(products)
}

/// Where each header the import reads sits in the header row.
struct Columns {
    item_number: usize,
    name: usize,
    barcode: usize,
    location: usize,
    system_quantity: usize,
}

impl Columns {
    /// Finds every header, case-insensitively, or names the ones missing.
    fn find(header: &[Data]) -> Result<Self, ImportError> {
        let position = |name: &str| {
            header
                .iter()
                .position(|cell| text(cell).eq_ignore_ascii_case(name))
        };
        let found = [
            column::ITEM_NUMBER,
            column::NAME,
            column::BARCODE,
            column::LOCATION,
            column::SYSTEM_QUANTITY,
        ]
        .map(|name| (name, position(name)));

        if let [
            (_, Some(item_number)),
            (_, Some(name)),
            (_, Some(barcode)),
            (_, Some(location)),
            (_, Some(system_quantity)),
        ] = found
        {
            return Ok(Self {
                item_number,
                name,
                barcode,
                location,
                system_quantity,
            });
        }
        let missing = found
            .into_iter()
            .filter(|(_, position)| position.is_none())
            .map(|(name, _)| name)
            .collect();
        Err(ImportError::MissingColumns(missing))
    }

    /// The product on one row, `None` for a row without an item number.
    fn product(&self, row: &[Data], row_number: usize) -> Result<Option<Product>, ImportError> {
        let cell = |ix: usize| row.get(ix).unwrap_or(&Data::Empty);
        let item_number = text(cell(self.item_number));
        if item_number.is_empty() {
            return Ok(None);
        }
        let quantity_cell = cell(self.system_quantity);
        let system_quantity =
            quantity(quantity_cell).ok_or_else(|| ImportError::InvalidQuantity {
                row: row_number,
                value: text(quantity_cell),
            })?;
        Ok(Some(Product::new(
            item_number,
            text(cell(self.name)),
            text(cell(self.location)),
            text(cell(self.barcode)),
            system_quantity,
        )))
    }
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

/// A cell as a whole number of units. An empty cell means nothing in stock.
/// Negative quantities are kept: MultiCase can show more units out than in.
fn quantity(cell: &Data) -> Option<i64> {
    match cell {
        Data::Empty => Some(0),
        Data::Int(value) => Some(*value),
        Data::Float(value) if value.fract() == 0.0 => Some(*value as i64),
        Data::String(value) if value.trim().is_empty() => Some(0),
        Data::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rust_xlsxwriter::Workbook;

    use super::*;

    /// A temporary file name unique to this test process and `name`.
    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("stocktake-{}-{name}", std::process::id()))
    }

    /// One cell of a fixture row.
    enum Cell {
        Text(&'static str),
        Number(f64),
        Blank,
    }
    use Cell::*;

    /// Writes a workbook whose first sheet holds `rows`, and returns its path.
    fn workbook(name: &str, rows: &[&[Cell]]) -> PathBuf {
        let path = temp_path(&format!("{name}.xlsx"));
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        for (row_ix, row) in (0u32..).zip(rows) {
            for (col_ix, cell) in (0u16..).zip(row.iter()) {
                match cell {
                    Text(text) => sheet.write_string(row_ix, col_ix, *text).map(drop),
                    Number(number) => sheet.write_number(row_ix, col_ix, *number).map(drop),
                    Blank => Ok(()),
                }
                .unwrap();
            }
        }
        workbook.save(&path).unwrap();
        path
    }

    /// Reads a fixture workbook and deletes it.
    fn read_fixture(name: &str, rows: &[&[Cell]]) -> Result<Vec<Product>, ImportError> {
        let path = workbook(name, rows);
        let result = read(&path);
        std::fs::remove_file(&path).ok();
        result
    }

    /// The headers in the order MultiCase exports them, with columns the
    /// import ignores in between.
    const HEADER: &[Cell] = &[
        Text("Firma"),
        Text("Lager"),
        Text("VareNR"),
        Text("ProduktDesc1"),
        Text("ProduktDesc2"),
        Text("PrdEAN"),
        Text("Utgått"),
        Text("Lokasjon"),
        Text("FysiskPaaLager"),
    ];

    #[test]
    fn reads_columns_by_header_whatever_their_order() {
        let products = read_fixture(
            "order",
            &[
                &[
                    Text("fysiskpaalager"),
                    Text("Lokasjon"),
                    Text("PRDEAN"),
                    Text("ProduktDesc1"),
                    Text("VareNR"),
                ],
                &[
                    Number(33.),
                    Text("C4-7"),
                    Number(7043811520667.),
                    Text(" Burano 120 Hvit "),
                    Number(152066.),
                ],
            ],
        )
        .unwrap();
        assert_eq!(
            products,
            [Product::new(
                "152066",
                "Burano 120 Hvit",
                "C4-7",
                "7043811520667",
                33
            )]
        );
    }

    #[test]
    fn numbers_stored_as_text_read_the_same() {
        let products = read_fixture(
            "text-numbers",
            &[
                HEADER,
                &[
                    Text("1"),
                    Text("1"),
                    Text("152066"),
                    Text("Burano"),
                    Blank,
                    Text("7043811520667"),
                    Text("Nei"),
                    Text("C4-7"),
                    Text(" 12 "),
                ],
            ],
        )
        .unwrap();
        assert_eq!(products[0].item_number(), "152066");
        assert_eq!(products[0].barcode(), "7043811520667");
        assert_eq!(products[0].system_quantity(), 12);
    }

    #[test]
    fn skips_rows_without_an_item_number_and_reads_empty_cells_as_empty() {
        let products = read_fixture(
            "blank-rows",
            &[
                HEADER,
                &[
                    Blank,
                    Blank,
                    Text("1"),
                    Text("Uten lokasjon"),
                    Blank,
                    Blank,
                    Blank,
                    Blank,
                    Blank,
                ],
                &[Blank, Blank, Blank, Text("Ingen varenummer")],
                &[
                    Blank,
                    Blank,
                    Text("2"),
                    Text("Negativ"),
                    Blank,
                    Blank,
                    Blank,
                    Text("A1"),
                    Number(-2.),
                ],
            ],
        )
        .unwrap();
        assert_eq!(products.len(), 2);
        assert_eq!(products[0].location(), "");
        assert_eq!(products[0].barcode(), "");
        // An empty quantity means nothing in stock.
        assert_eq!(products[0].system_quantity(), 0);
        assert_eq!(products[1].system_quantity(), -2);
    }

    #[test]
    fn names_every_missing_column() {
        let error = read_fixture(
            "missing",
            &[&[Text("VareNR"), Text("Lokasjon"), Text("Antall")]],
        )
        .unwrap_err();
        assert!(
            matches!(
                &error,
                ImportError::MissingColumns(columns)
                    if columns == &["ProduktDesc1", "PrdEAN", "FysiskPaaLager"]
            ),
            "{error:?}"
        );
    }

    #[test]
    fn rejects_a_quantity_that_isnt_whole_with_its_spreadsheet_row() {
        let row = |quantity| -> [Cell; 9] {
            [
                Blank,
                Blank,
                Text("1"),
                Text("Vare"),
                Blank,
                Blank,
                Blank,
                Blank,
                quantity,
            ]
        };
        for (quantity, value) in [(Number(2.5), "2.5"), (Text("mange"), "mange")] {
            let error =
                read_fixture("quantity", &[HEADER, &row(Number(1.)), &row(quantity)]).unwrap_err();
            assert!(
                matches!(&error, ImportError::InvalidQuantity { row: 3, value: v } if v == value),
                "{error:?}"
            );
        }
    }

    #[test]
    fn rejects_a_stock_list_without_products() {
        assert!(matches!(
            read_fixture("headers-only", &[HEADER]),
            Err(ImportError::Empty)
        ));
        assert!(matches!(
            read_fixture("no-rows", &[]),
            Err(ImportError::Empty)
        ));
    }

    #[test]
    fn rejects_missing_unsupported_and_damaged_files() {
        let missing = temp_path("does-not-exist.xlsx");
        assert!(matches!(read(&missing), Err(ImportError::NotFound)));

        let csv = temp_path("stock-list.csv");
        std::fs::write(&csv, "VareNR;ProduktDesc1\n").unwrap();
        assert!(matches!(read(&csv), Err(ImportError::UnsupportedFormat)));
        std::fs::remove_file(&csv).ok();

        let damaged = temp_path("damaged.xlsx");
        std::fs::write(&damaged, "not a workbook").unwrap();
        assert!(matches!(read(&damaged), Err(ImportError::Unreadable(_))));
        std::fs::remove_file(&damaged).ok();
    }

    #[test]
    fn knows_the_supported_extensions() {
        assert!(is_supported(Path::new("Vareliste.xlsx")));
        assert!(is_supported(Path::new("Vareliste.XLS")));
        assert!(!is_supported(Path::new("Vareliste.csv")));
        assert!(!is_supported(Path::new("Vareliste")));
    }

    /// The real MultiCase export lives in the workspace's untracked `data`
    /// directory, so this only runs where it's present:
    /// `cargo test -p stocktake -- --ignored`.
    #[test]
    #[ignore = "needs the untracked sample export in data/"]
    fn reads_the_sample_export() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/Vareliste - varetelling.xlsx");
        let products = read(&path).expect("sample stock list imports");
        assert_eq!(products.len(), 43);

        // Found by item number, since exports don't promise a row order.
        let burano = products
            .iter()
            .find(|product| product.item_number() == "152066")
            .expect("sample contains Burano 120 Hvit");
        assert_eq!(burano.name(), "Burano 120 Hvit");
        assert_eq!(burano.location(), "C4-7");
        assert_eq!(burano.barcode(), "7043811520667");
        assert_eq!(burano.system_quantity(), 33);
        assert!(!burano.is_counted());
    }
}
