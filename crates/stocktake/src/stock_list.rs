//! Reads the stock list exported from the business system.
//!
//! The export is used as-is: the first sheet, headers in the first row, and
//! columns found by header name rather than position, so the business system can add,
//! drop or reorder other columns without breaking the import.

use std::{error::Error, fmt, path::Path};

use calamine::{Data, Reader as _, open_workbook_auto};

use crate::{Product, same_location};

/// The headers the import reads, as the business system names them.
pub mod column {
    pub const ITEM_NUMBER: &str = "VareNR";
    pub const NAME: &str = "ProduktDesc1";
    /// Colour and style. Optional: an export without it imports with empty
    /// descriptions.
    pub const DESCRIPTION: &str = "ProduktDesc2";
    pub const BARCODE: &str = "PrdEAN";
    pub const LOCATION: &str = "Lokasjon";
    pub const SYSTEM_QUANTITY: &str = "FysiskPaaLager";
}

/// What the business system writes in [`column::LOCATION`] for a product
/// without a location. Read as an empty location.
pub const NO_LOCATION: &str = "N/A";

/// The file extensions [`read`] accepts: Excel's formats, which the business system
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

/// Reads the business system's stock list export.
///
/// Rows without an item number are skipped, since exports can end in blank
/// lines. Every other row must have a whole-number system quantity; an empty
/// one means nothing in stock. Lines with the same item number become one
/// product; see [`StockList::duplicates`].
pub fn read(path: &Path) -> Result<StockList, ImportError> {
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

    let mut lines: Vec<Lines> = Vec::new();
    // Spreadsheet rows are 1-based and the header is row 1. A range starts at
    // its first non-empty row, which for an export is the header.
    let first_row = range.start().map_or(0, |(row, _)| row as usize) + 2;
    for (row_number, row) in (first_row..).zip(rows) {
        let Some((item_number, line)) = columns.line(row, row_number)? else {
            continue;
        };
        match lines
            .iter_mut()
            .find(|lines| lines.item_number == item_number)
        {
            Some(lines) => lines.add(line),
            None => lines.push(Lines::new(item_number, line)),
        }
    }

    if lines.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(StockList::new(lines))
}

/// The stock list as read, before it becomes a stocktake.
///
/// An item number on more than one line is one product whose system
/// quantity is the lines' sum. When the lines are at different locations,
/// the one with the most stock is its pick location, or with no stock at
/// any, the first that isn't empty. When several tie for the most stock,
/// the product is a [duplicate](Self::duplicates): the person importing
/// picks which location is its pick location, with [`Self::pick_location`],
/// before the stocktake starts.
#[derive(Debug)]
pub struct StockList {
    products: Vec<Product>,
    duplicates: Vec<Duplicate>,
}

/// A product the stock list lists at several locations that tie for the
/// most stock, and its lines.
#[derive(Debug)]
pub struct Duplicate {
    /// Which of the stock list's products this is.
    product: usize,
    lines: Vec<Line>,
}

/// One location a duplicate is listed at, and what the business system
/// says is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    location: String,
    system_quantity: i64,
}

impl StockList {
    fn new(lines: Vec<Lines>) -> Self {
        let mut products = Vec::with_capacity(lines.len());
        let mut duplicates = Vec::new();
        for lines in lines {
            let locations = lines.distinct_locations();
            // Where the system has the most stock is where it is picked
            // from, so the other lines are merged into it. Only a tie for
            // the most stock is a choice to make; with no stock anywhere
            // there is nothing to choose by, so the first location that
            // isn't empty is taken. The choice can still be changed from
            // the count, as the product keeps every location.
            let most = locations.iter().map(|line| line.system_quantity).max();
            let mut at_most = locations
                .iter()
                .filter(|line| Some(line.system_quantity) == most);
            let pick = match (at_most.next(), at_most.next()) {
                (Some(line), None) => Some(line),
                _ if most == Some(0) => locations
                    .iter()
                    .find(|line| !line.location.is_empty())
                    .or_else(|| locations.first()),
                _ => None,
            };
            let mut product = Product::new(
                lines.item_number,
                lines.name,
                pick.unwrap_or(&locations[0]).location.clone(),
                lines.barcode,
                locations.iter().map(|line| line.system_quantity).sum(),
            )
            .with_description(lines.description);
            if locations.len() > 1 {
                product = product.with_listed_locations(
                    locations.iter().map(|line| line.location.clone()).collect(),
                );
                if pick.is_none() {
                    duplicates.push(Duplicate {
                        product: products.len(),
                        lines: locations,
                    });
                }
            }
            products.push(product);
        }
        Self {
            products,
            duplicates,
        }
    }

    pub fn len(&self) -> usize {
        self.products.len()
    }

    /// Never true: [`read`] rejects a stock list without products.
    pub fn is_empty(&self) -> bool {
        self.products.is_empty()
    }

    /// The products listed at several locations that tie for the most
    /// stock, in stock list order. A tie at none isn't one.
    pub fn duplicates(&self) -> &[Duplicate] {
        &self.duplicates
    }

    /// The product a duplicate is about.
    pub fn product(&self, duplicate: &Duplicate) -> &Product {
        &self.products[duplicate.product]
    }

    /// Makes a duplicate's `line` its pick location.
    pub fn pick_location(&mut self, duplicate: usize, line: usize) {
        let duplicate = &self.duplicates[duplicate];
        let location = &duplicate.lines[line].location;
        self.products[duplicate.product].pick_listed_location(location);
    }

    /// The products, each duplicate at the location picked for it, or its
    /// first line's until one is.
    pub fn into_products(self) -> Vec<Product> {
        self.products
    }
}

impl Duplicate {
    /// The locations the product is listed at, in stock list order.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}

impl Line {
    pub fn location(&self) -> &str {
        &self.location
    }

    pub fn system_quantity(&self) -> i64 {
        self.system_quantity
    }
}

/// Every line of one item number, as read.
struct Lines {
    item_number: String,
    name: String,
    description: String,
    barcode: String,
    lines: Vec<Line>,
}

/// One row of the stock list, besides its item number.
struct ReadLine {
    name: String,
    description: String,
    barcode: String,
    location: String,
    system_quantity: i64,
}

impl Lines {
    fn new(item_number: String, line: ReadLine) -> Self {
        Self {
            item_number,
            name: line.name,
            description: line.description,
            barcode: line.barcode,
            lines: vec![Line {
                location: line.location,
                system_quantity: line.system_quantity,
            }],
        }
    }

    /// Adds another line of the same item number. The first line's name and
    /// description stand; a barcode is taken from whichever line has one.
    fn add(&mut self, line: ReadLine) {
        if self.barcode.is_empty() {
            self.barcode = line.barcode;
        }
        self.lines.push(Line {
            location: line.location,
            system_quantity: line.system_quantity,
        });
    }

    /// The lines with the [same](same_location) location folded into one,
    /// in the order they were first listed.
    fn distinct_locations(&self) -> Vec<Line> {
        let mut distinct: Vec<Line> = Vec::new();
        for line in &self.lines {
            match distinct
                .iter_mut()
                .find(|known| same_location(&known.location, &line.location))
            {
                Some(known) => known.system_quantity += line.system_quantity,
                None => distinct.push(line.clone()),
            }
        }
        distinct
    }
}

/// Where each header the import reads sits in the header row.
struct Columns {
    item_number: usize,
    name: usize,
    description: Option<usize>,
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
                description: position(column::DESCRIPTION),
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

    /// The line on one row with its item number, `None` for a row without
    /// one.
    fn line(
        &self,
        row: &[Data],
        row_number: usize,
    ) -> Result<Option<(String, ReadLine)>, ImportError> {
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
        let line = ReadLine {
            name: text(cell(self.name)),
            description: self.description.map(cell).map(text).unwrap_or_default(),
            barcode: text(cell(self.barcode)),
            location: location(cell(self.location)),
            system_quantity,
        };
        Ok(Some((item_number, line)))
    }
}

/// A cell as text with its whitespace tidied: trimmed, and runs of spaces
/// collapsed, so `Gull  Børstet` is found by typing `Gull Børstet`. Whole
/// numbers lose their `.0`, so an item number or barcode stored as a number
/// reads the same as one stored as text.
fn text(cell: &Data) -> String {
    match cell {
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Int(value) => value.to_string(),
        Data::Empty => String::new(),
        other => other
            .to_string()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// A location cell as text, with the business system's [`NO_LOCATION`] read
/// as none.
fn location(cell: &Data) -> String {
    let location = text(cell);
    if location.eq_ignore_ascii_case(NO_LOCATION) {
        String::new()
    } else {
        location
    }
}

/// A cell as a whole number of units. An empty cell means nothing in stock.
/// Negative quantities are kept: the business system can show more units out than in.
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
    fn read_fixture_list(name: &str, rows: &[&[Cell]]) -> Result<StockList, ImportError> {
        let path = workbook(name, rows);
        let result = read(&path);
        std::fs::remove_file(&path).ok();
        result
    }

    /// Reads a fixture workbook's products and deletes it.
    fn read_fixture(name: &str, rows: &[&[Cell]]) -> Result<Vec<Product>, ImportError> {
        read_fixture_list(name, rows).map(StockList::into_products)
    }

    /// The headers in the order the business system exports them, with columns the
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
    fn reads_the_description_when_the_export_has_one() {
        let products = read_fixture(
            "description",
            &[
                HEADER,
                &[
                    Blank,
                    Blank,
                    Text("1"),
                    Text("Como Fronter 120 - Grå Driftwood"),
                    Text(" Como Standard -  Ramtre "),
                    Blank,
                    Blank,
                    Text("B2-1"),
                    Number(3.),
                ],
            ],
        )
        .unwrap();
        assert_eq!(products[0].description(), "Como Standard - Ramtre");
    }

    #[test]
    fn tidies_the_whitespace_in_text() {
        let products = read_fixture(
            "spaces",
            &[
                &[
                    Text("VareNR"),
                    Text("ProduktDesc1"),
                    Text("PrdEAN"),
                    Text("Lokasjon"),
                    Text("FysiskPaaLager"),
                ],
                &[
                    Text("1"),
                    Text("Maranello 90 - Gull  Børstet "),
                    Blank,
                    Text(" C4-7"),
                    Number(1.),
                ],
            ],
        )
        .unwrap();
        assert_eq!(products[0].name(), "Maranello 90 - Gull Børstet");
        assert_eq!(products[0].location(), "C4-7");
    }

    #[test]
    fn reads_the_business_systems_no_location_as_none() {
        let products = read_fixture(
            "no-location",
            &[
                &[
                    Text("VareNR"),
                    Text("ProduktDesc1"),
                    Text("PrdEAN"),
                    Text("Lokasjon"),
                    Text("FysiskPaaLager"),
                ],
                &[Text("1"), Text("Uplassert"), Blank, Text("N/A"), Number(4.)],
                &[
                    Text("2"),
                    Text("Uplassert"),
                    Blank,
                    Text(" n/a "),
                    Number(4.),
                ],
            ],
        )
        .unwrap();
        assert_eq!(products[0].location(), "");
        assert_eq!(products[0].aisle(), "");
        assert_eq!(products[1].location(), "");
    }

    #[test]
    fn merges_the_lines_into_the_location_with_the_most_stock() {
        let columns: &[Cell] = &[
            Text("VareNR"),
            Text("ProduktDesc1"),
            Text("PrdEAN"),
            Text("Lokasjon"),
            Text("FysiskPaaLager"),
        ];
        let list = read_fixture_list(
            "empty-lines",
            &[
                columns,
                &[Text("1"), Text("Som regel"), Blank, Text("A1"), Number(0.)],
                &[
                    Text("1"),
                    Text("Som regel"),
                    Blank,
                    Text("E2-8"),
                    Number(16.),
                ],
                &[
                    Text("2"),
                    Text("Begge tomme"),
                    Blank,
                    Text("N/A"),
                    Number(0.),
                ],
                &[
                    Text("2"),
                    Text("Begge tomme"),
                    Blank,
                    Text("B1"),
                    Number(0.),
                ],
                &[Text("3"), Text("Begge"), Blank, Text("A1"), Number(1.)],
                &[Text("3"), Text("Begge"), Blank, Text("B2-6"), Number(16.)],
            ],
        )
        .unwrap();
        // Products 1 and 3 have nothing to choose, and product 2 nothing to
        // choose by.
        assert!(list.duplicates().is_empty());

        let products = list.into_products();
        assert_eq!(products[0].location(), "E2-8");
        assert_eq!(products[0].system_quantity(), 16);
        assert_eq!(
            products[0].other_listed_locations().collect::<Vec<_>>(),
            ["A1"]
        );
        // With no stock anywhere, a location beats none.
        assert_eq!(products[1].location(), "B1");
        assert_eq!(products[1].system_quantity(), 0);
        assert_eq!(
            products[1].other_listed_locations().collect::<Vec<_>>(),
            [""]
        );
        assert_eq!(products[2].location(), "B2-6");
        assert_eq!(products[2].system_quantity(), 17);
        assert_eq!(
            products[2].other_listed_locations().collect::<Vec<_>>(),
            ["A1"]
        );
    }

    #[test]
    fn merges_lines_with_the_same_item_number() {
        let columns: &[Cell] = &[
            Text("VareNR"),
            Text("ProduktDesc1"),
            Text("PrdEAN"),
            Text("Lokasjon"),
            Text("FysiskPaaLager"),
        ];
        let mut list = read_fixture_list(
            "duplicates",
            &[
                columns,
                &[
                    Text("1"),
                    Text("Como Fronter 60"),
                    Blank,
                    Text("A1"),
                    Number(16.),
                ],
                &[Text("2"), Text("Alene"), Text("7"), Text("B1"), Number(2.)],
                &[
                    Text("1"),
                    Text("Como Fronter 60"),
                    Text("9"),
                    Text("E2-8"),
                    Number(16.),
                ],
                // The same location twice isn't a choice to make; it's one line.
                &[Text("2"), Text("Alene"), Blank, Text("b1"), Number(3.)],
            ],
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list.duplicates().len(), 1);

        let duplicate = &list.duplicates()[0];
        let product = list.product(duplicate);
        assert_eq!(product.item_number(), "1");
        assert_eq!(product.system_quantity(), 32);
        // The barcode comes from whichever line has one.
        assert_eq!(product.barcode(), "9");
        assert_eq!(product.location(), "A1");
        assert_eq!(
            duplicate.lines(),
            [
                Line {
                    location: "A1".into(),
                    system_quantity: 16
                },
                Line {
                    location: "E2-8".into(),
                    system_quantity: 16
                },
            ]
        );

        list.pick_location(0, 1);
        let products = list.into_products();
        assert_eq!(products[0].location(), "E2-8");
        assert_eq!(
            products[0].other_listed_locations().collect::<Vec<_>>(),
            ["A1"]
        );
        assert_eq!(products[1].item_number(), "2");
        assert_eq!(products[1].system_quantity(), 5);
        assert_eq!(products[1].location(), "B1");
        assert_eq!(products[1].other_listed_locations().count(), 0);
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

    /// A real export from the business system, kept in the workspace's
    /// `data` directory.
    #[test]
    fn reads_the_sample_export() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/stock-list.xlsx");
        let products = read(&path)
            .expect("sample stock list imports")
            .into_products();
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

    /// The complete export the counters work from, so the import is held to
    /// a real-world file: 3,673 rows, 66 item numbers on two lines, `N/A`
    /// locations, and names with doubled spaces.
    #[test]
    fn reads_the_complete_export() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/complete-stock-list.xlsx");
        let list = read(&path).expect("complete stock list imports");
        // None of the 66 ties for the most stock short of none, so none is
        // a choice.
        assert!(list.duplicates().is_empty());
        assert_eq!(list.len(), 3_673 - 66);

        let products = list.into_products();
        let find = |item_number: &str| {
            products
                .iter()
                .find(|product| product.item_number() == item_number)
                .unwrap_or_else(|| panic!("{item_number} is in the list"))
        };
        assert!(products.iter().all(|product| product.aisle() != "N"));
        assert!(products.iter().any(|product| product.location().is_empty()));
        assert_eq!(find("201594").name(), "Maranello 90 - Gull Børstet");
        assert_eq!(find("202559").description(), "Como Standard - Ramtre");
        // Listed at A1 with none and at E2-8 with 16: picked from E2-8.
        let como = find("202559");
        assert_eq!(como.location(), "E2-8");
        assert_eq!(como.system_quantity(), 16);
        // Listed at A1 with 1 and at B2-6 with 16: picked from B2-6.
        let massimo = find("204990");
        assert_eq!(massimo.location(), "B2-6");
        assert_eq!(massimo.system_quantity(), 17);
        // Listed at A1 and without a location, with none at either.
        assert_eq!(find("203526").location(), "A1");
    }
}
