//! The stocktake: the stock list being counted and what has been counted so far.
//!
//! This crate is the domain model. It has no UI dependency, so it builds and
//! tests in seconds and can't accidentally depend on how things look.
//!
//! | Module         | Owns                                                      |
//! | -------------- | --------------------------------------------------------- |
//! | this root      | [`Product`], [`Stocktake`], searching, lookup, ordering   |
//! | [`stock_list`] | Reading and validating the business system's export      |
//! | [`export`]     | Writing the counted stock list back out as `.xlsx`        |
//! | [`store`]      | Keeping the stocktake in progress on disk                 |
//! | [`recent`]     | Remembering which stock lists were imported               |
//!
//! Every file operation here is synchronous and returns its own error type;
//! the UI decides what runs in the background and how errors are worded.

pub mod export;
pub mod recent;
pub mod stock_list;
pub mod store;

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Identifies a product by its line in the stock list.
///
/// The stock list never changes during a stocktake, so the line is stable.
/// Item numbers are not used because the business system can list one item number on
/// several lines (one per batch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductId(usize);

impl ProductId {
    /// The product's line in the stock list. Stable for the whole stocktake,
    /// so it can key UI state that must follow the product through filtering.
    pub fn line(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Product {
    item_number: String,
    name: String,
    location: String,
    barcode: String,
    system_quantity: i64,
    counted_quantity: Option<i64>,
}

impl Product {
    pub fn new(
        item_number: impl Into<String>,
        name: impl Into<String>,
        location: impl Into<String>,
        barcode: impl Into<String>,
        system_quantity: i64,
    ) -> Self {
        Self {
            item_number: item_number.into(),
            name: name.into(),
            location: location.into(),
            barcode: barcode.into(),
            system_quantity,
            counted_quantity: None,
        }
    }

    pub fn item_number(&self) -> &str {
        &self.item_number
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The shelf position, empty when the product has none.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The letters the location starts with, such as `C` for `C4-7`. Empty
    /// when the location is, or when it starts with something else.
    pub fn aisle(&self) -> &str {
        let end = self
            .location
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(self.location.len());
        &self.location[..end]
    }

    /// The EAN on the packaging, empty when the product has none.
    pub fn barcode(&self) -> &str {
        &self.barcode
    }

    pub fn system_quantity(&self) -> i64 {
        self.system_quantity
    }

    /// `None` while the product is uncounted.
    pub fn counted_quantity(&self) -> Option<i64> {
        self.counted_quantity
    }

    pub fn is_counted(&self) -> bool {
        self.counted_quantity.is_some()
    }

    /// Counted minus system quantity, `None` while uncounted.
    pub fn difference(&self) -> Option<i64> {
        self.counted_quantity
            .map(|counted| counted - self.system_quantity)
    }

    fn matches_exactly(&self, query: &str) -> bool {
        (!self.barcode.is_empty() && self.barcode.eq_ignore_ascii_case(query))
            || self.item_number.eq_ignore_ascii_case(query)
    }

    fn contains(&self, needle: &str) -> bool {
        [&self.item_number, &self.name, &self.location, &self.barcode]
            .iter()
            .any(|field| field.to_lowercase().contains(needle))
    }
}

/// Which products the table shows, besides those the search leaves out. The
/// default shows every product.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Stored as the aisles left out, so an aisle shows until the counter
    /// hides it.
    hidden_aisles: BTreeSet<String>,
    hides_counted: bool,
    hides_uncounted: bool,
}

impl Filter {
    pub fn shows_aisle(&self, aisle: &str) -> bool {
        !self.hidden_aisles.contains(aisle)
    }

    pub fn set_aisle_shown(&mut self, aisle: &str, shown: bool) {
        if shown {
            self.hidden_aisles.remove(aisle);
        } else {
            self.hidden_aisles.insert(aisle.to_string());
        }
    }

    /// Whether counted products show, or uncounted ones if `counted` is
    /// false.
    pub fn shows_counted(&self, counted: bool) -> bool {
        if counted {
            !self.hides_counted
        } else {
            !self.hides_uncounted
        }
    }

    pub fn set_counted_shown(&mut self, counted: bool, shown: bool) {
        if counted {
            self.hides_counted = !shown;
        } else {
            self.hides_uncounted = !shown;
        }
    }

    fn shows(&self, product: &Product) -> bool {
        self.shows_aisle(product.aisle()) && self.shows_counted(product.is_counted())
    }
}

/// What a search for one product found.
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Exactly one product: count it.
    Found(ProductId),
    /// Several products match; the counter has to pick one.
    Ambiguous(usize),
    NotFound,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stocktake {
    products: Vec<Product>,
}

impl Stocktake {
    pub fn new(products: Vec<Product>) -> Self {
        Self { products }
    }

    pub fn product(&self, id: ProductId) -> &Product {
        &self.products[id.0]
    }

    /// Every product, in stock list order.
    pub fn products(&self) -> impl Iterator<Item = (ProductId, &Product)> {
        self.products
            .iter()
            .enumerate()
            .map(|(ix, product)| (ProductId(ix), product))
    }

    pub fn len(&self) -> usize {
        self.products.len()
    }

    /// Always false for an imported stock list, which import rejects when
    /// empty; here for completeness alongside [`Self::len`].
    pub fn is_empty(&self) -> bool {
        self.products.is_empty()
    }

    pub fn counted_len(&self) -> usize {
        self.products.iter().filter(|p| p.is_counted()).count()
    }

    pub fn uncounted_len(&self) -> usize {
        self.len() - self.counted_len()
    }

    pub fn set_counted_quantity(&mut self, id: ProductId, quantity: i64) {
        self.products[id.0].counted_quantity = Some(quantity);
    }

    /// Every aisle in the stock list and how many products it holds, in the
    /// order the storage is walked, with products without one last.
    pub fn aisles(&self) -> Vec<(&str, usize)> {
        let mut aisles: Vec<(&str, usize)> = Vec::new();
        for product in &self.products {
            match aisles
                .iter_mut()
                .find(|(aisle, _)| *aisle == product.aisle())
            {
                Some((_, len)) => *len += 1,
                None => aisles.push((product.aisle(), 1)),
            }
        }
        aisles.sort_by(|(a, _), (b, _)| compare_locations(a, b));
        aisles
    }

    /// Products matching the search text, ordered by location so the table
    /// follows the counter's walk through the storage. An empty search
    /// matches everything.
    pub fn search(&self, query: &str) -> Vec<ProductId> {
        self.search_filtered(query, &Filter::default())
    }

    /// Like [`Self::search`], keeping only the products the filter shows.
    pub fn search_filtered(&self, query: &str, filter: &Filter) -> Vec<ProductId> {
        let needle = query.trim().to_lowercase();
        let mut ids: Vec<_> = self
            .products()
            .filter(|(_, product)| filter.shows(product))
            .filter(|(_, product)| needle.is_empty() || product.contains(&needle))
            .map(|(id, _)| id)
            .collect();
        ids.sort_by(|a, b| {
            let (a, b) = (self.product(*a), self.product(*b));
            compare_locations(a.location(), b.location())
                .then_with(|| natural_cmp(a.item_number(), b.item_number()))
        });
        ids
    }

    /// The product whose barcode is exactly `text`, if any.
    pub fn product_with_barcode(&self, text: &str) -> Option<ProductId> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        self.products()
            .find(|(_, product)| product.barcode == text)
            .map(|(id, _)| id)
    }

    /// Finds the one product a scan or typed search refers to.
    ///
    /// A barcode or item number that matches exactly wins over partial
    /// matches, so scanning never lands on a product that merely contains the
    /// scanned digits.
    pub fn lookup(&self, query: &str) -> Lookup {
        let query = query.trim();
        if query.is_empty() {
            return Lookup::NotFound;
        }
        let exact: Vec<_> = self
            .products()
            .filter(|(_, product)| product.matches_exactly(query))
            .map(|(id, _)| id)
            .collect();
        let candidates = if exact.is_empty() {
            self.search(query)
        } else {
            exact
        };
        match candidates.as_slice() {
            [] => Lookup::NotFound,
            [id] => Lookup::Found(*id),
            many => Lookup::Ambiguous(many.len()),
        }
    }
}

/// Orders locations the way the storage is walked: naturally, so `C4-10`
/// comes after `C4-9`, with products that have no location last.
pub fn compare_locations(a: &str, b: &str) -> Ordering {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => natural_cmp(a, b),
    }
}

/// Compares runs of digits by value, so `C4-10` sorts after `C4-9`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let a_len = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
                let b_len = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
                let (a_num, b_num) = (
                    a[..a_len].trim_start_matches('0'),
                    b[..b_len].trim_start_matches('0'),
                );
                let ordering = a_num.len().cmp(&b_num.len()).then_with(|| a_num.cmp(b_num));
                if ordering != Ordering::Equal {
                    return ordering;
                }
                (a, b) = (&a[a_len..], &b[b_len..]);
            }
            (Some(x), Some(y)) => {
                let ordering = x.to_lowercase().cmp(y.to_lowercase());
                if ordering != Ordering::Equal {
                    return ordering;
                }
                (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stocktake() -> Stocktake {
        Stocktake::new(vec![
            Product::new("152066", "Burano 120 Hvit", "C4-10", "7043811520667", 33),
            Product::new("152062", "Burano 120 Sort", "C4-7", "7043811520629", 49),
            Product::new("150765", "Veneto 75", "", "", 16),
            Product::new("150766", "Veneto 90", "C3-1", "7043811507668", 0),
        ])
    }

    #[test]
    fn search_orders_by_location_with_unlocated_last() {
        let stocktake = stocktake();
        let items: Vec<_> = stocktake
            .search("")
            .into_iter()
            .map(|id| stocktake.product(id).item_number())
            .collect();
        assert_eq!(items, ["150766", "152062", "152066", "150765"]);
    }

    #[test]
    fn aisle_is_the_leading_letters() {
        let aisle = |location| Product::new("1", "", location, "", 0).aisle().to_string();
        assert_eq!(aisle("C4-7"), "C");
        assert_eq!(aisle("AB12"), "AB");
        assert_eq!(aisle("12-3"), "");
        assert_eq!(aisle(""), "");
    }

    #[test]
    fn aisles_count_products_with_unlocated_last() {
        assert_eq!(stocktake().aisles(), [("C", 3), ("", 1)]);
    }

    #[test]
    fn filter_hides_aisles() {
        let stocktake = stocktake();
        let mut filter = Filter::default();
        filter.set_aisle_shown("C", false);
        assert_eq!(stocktake.search_filtered("", &filter), [ProductId(2)]);
        assert_eq!(stocktake.search_filtered("burano", &filter), []);
        filter.set_aisle_shown("C", true);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 4);
    }

    #[test]
    fn filter_hides_counted_or_uncounted_products() {
        let mut stocktake = stocktake();
        stocktake.set_counted_quantity(ProductId(0), 33);
        let mut filter = Filter::default();
        filter.set_counted_shown(true, false);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 3);
        filter.set_counted_shown(true, true);
        filter.set_counted_shown(false, false);
        assert_eq!(stocktake.search_filtered("", &filter), [ProductId(0)]);
        filter.set_aisle_shown("C", false);
        assert_eq!(stocktake.search_filtered("", &filter), []);
    }

    #[test]
    fn lookup_prefers_exact_barcode_and_item_number() {
        let stocktake = stocktake();
        assert_eq!(
            stocktake.lookup("7043811520629"),
            Lookup::Found(ProductId(1))
        );
        assert_eq!(stocktake.lookup(" 150765 "), Lookup::Found(ProductId(2)));
    }

    #[test]
    fn product_with_barcode_needs_an_exact_barcode() {
        let stocktake = stocktake();
        assert_eq!(
            stocktake.product_with_barcode("7043811520629"),
            Some(ProductId(1))
        );
        // Item numbers, partial barcodes and products without one don't count.
        assert_eq!(stocktake.product_with_barcode("152062"), None);
        assert_eq!(stocktake.product_with_barcode("70438115206"), None);
        assert_eq!(stocktake.product_with_barcode(""), None);
    }

    #[test]
    fn lookup_by_name() {
        let stocktake = stocktake();
        assert_eq!(stocktake.lookup("veneto 90"), Lookup::Found(ProductId(3)));
        assert_eq!(stocktake.lookup("burano"), Lookup::Ambiguous(2));
        assert_eq!(stocktake.lookup("1234"), Lookup::NotFound);
        assert_eq!(stocktake.lookup("  "), Lookup::NotFound);
    }

    #[test]
    fn counting_zero_counts() {
        let mut stocktake = stocktake();
        assert_eq!(stocktake.counted_len(), 0);
        stocktake.set_counted_quantity(ProductId(3), 0);
        assert_eq!(stocktake.counted_len(), 1);
        assert_eq!(stocktake.product(ProductId(3)).difference(), Some(0));
        stocktake.set_counted_quantity(ProductId(0), 30);
        assert_eq!(stocktake.product(ProductId(0)).difference(), Some(-3));
        assert_eq!(stocktake.uncounted_len(), 2);
    }
}
