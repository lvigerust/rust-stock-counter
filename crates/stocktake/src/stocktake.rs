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
    /// Where the counter moved the pick location to, `None` while it's the
    /// one the stock list gives.
    #[serde(default)]
    moved_location: Option<String>,
    /// What was counted at the pick location. Saved under its name from
    /// before counts were kept per location, so an older save resumes with
    /// its counts at the pick location.
    #[serde(rename = "counted_quantity")]
    pick_count: Option<i64>,
    /// What was counted at each overflow location, in location order. None
    /// holds zero: a count of zero removes the location.
    #[serde(default)]
    overflow_counts: Vec<OverflowCount>,
}

/// How far a product has been counted.
///
/// Ordered by the work left, so sorting by it puts uncounted products first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CountState {
    Uncounted,
    /// Counted at overflow locations, but not yet at its pick location.
    PartlyCounted,
    Counted,
}

/// Units of a product counted at one of its overflow locations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct OverflowCount {
    location: String,
    quantity: i64,
}

/// A location as the counter typed it, trimmed and upper-cased so one shelf
/// isn't recorded twice.
pub fn normalize_location(location: &str) -> String {
    location.trim().to_uppercase()
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
            moved_location: None,
            pick_count: None,
            overflow_counts: Vec::new(),
        }
    }

    pub fn item_number(&self) -> &str {
        &self.item_number
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pick location, empty when the product has none: where the
    /// counter moved it, or else the one the stock list gives.
    pub fn location(&self) -> &str {
        self.moved_location.as_deref().unwrap_or(&self.location)
    }

    /// The pick location the stock list gives, whether or not it was moved.
    pub fn listed_location(&self) -> &str {
        &self.location
    }

    /// Whether `location` is the pick location. An empty location is too,
    /// so a product without one can still be counted at it.
    pub fn is_pick_location(&self, location: &str) -> bool {
        let location = normalize_location(location);
        location.is_empty() || location == normalize_location(self.location())
    }

    /// Whether the pick location can be moved: only while nothing has been
    /// counted there, so a counted pick location is never lost.
    pub fn can_move_pick_location(&self) -> bool {
        self.pick_count.is_none()
    }

    /// What has been counted at `location`, `None` while nothing has.
    pub fn count_at(&self, location: &str) -> Option<i64> {
        if self.is_pick_location(location) {
            return self.pick_count;
        }
        let location = normalize_location(location);
        self.overflow_counts
            .iter()
            .find(|count| count.location == location)
            .map(|count| count.quantity)
    }

    /// Every location something has been counted at, with what was counted
    /// there: the pick location first, once it has been counted, then the
    /// overflow locations.
    pub fn counts(&self) -> impl Iterator<Item = (&str, i64)> {
        let pick = self.pick_count.map(|quantity| (self.location(), quantity));
        pick.into_iter().chain(self.overflow_counts())
    }

    /// The overflow locations counted at, with what was counted there.
    pub fn overflow_counts(&self) -> impl Iterator<Item = (&str, i64)> {
        self.overflow_counts
            .iter()
            .map(|count| (count.location.as_str(), count.quantity))
    }

    pub fn overflow_len(&self) -> usize {
        self.overflow_counts.len()
    }

    /// The letters the location starts with, such as `C` for `C4-7`. Empty
    /// when the location is, or when it starts with something else.
    pub fn aisle(&self) -> &str {
        let location = self.location();
        let end = location
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(location.len());
        &location[..end]
    }

    /// The EAN on the packaging, empty when the product has none.
    pub fn barcode(&self) -> &str {
        &self.barcode
    }

    pub fn system_quantity(&self) -> i64 {
        self.system_quantity
    }

    /// What was counted at all its locations together, `None` until the
    /// product is counted.
    pub fn counted_quantity(&self) -> Option<i64> {
        self.pick_count.map(|pick_count| {
            pick_count
                + self
                    .overflow_counts
                    .iter()
                    .map(|count| count.quantity)
                    .sum::<i64>()
        })
    }

    /// Whether the product is counted: its pick location has been, if only
    /// as zero. Units found at overflow locations alone don't make it
    /// counted, so they can't hide a pick location nobody checked.
    pub fn is_counted(&self) -> bool {
        self.pick_count.is_some()
    }

    pub fn count_state(&self) -> CountState {
        if self.is_counted() {
            CountState::Counted
        } else if self.overflow_counts.is_empty() {
            CountState::Uncounted
        } else {
            CountState::PartlyCounted
        }
    }

    /// Counted minus system quantity, `None` until the product is counted.
    pub fn difference(&self) -> Option<i64> {
        self.counted_quantity()
            .map(|counted| counted - self.system_quantity)
    }

    /// Makes `location` the pick location, if it can be moved. Anything
    /// counted there as an overflow location becomes the pick location's
    /// count. Returns whether it was moved.
    fn move_pick_location(&mut self, location: &str) -> bool {
        let location = normalize_location(location);
        if !self.can_move_pick_location() || location.is_empty() {
            return false;
        }
        if let Ok(ix) = self
            .overflow_counts
            .binary_search_by(|count| compare_locations(&count.location, &location))
        {
            self.pick_count = Some(self.overflow_counts.remove(ix).quantity);
        }
        self.moved_location = (location != normalize_location(&self.location)).then_some(location);
        true
    }

    /// Records what was counted at `location`, replacing any earlier count
    /// there. Zero at an overflow location removes it, which is how a
    /// mistyped one is corrected.
    fn set_count(&mut self, location: &str, quantity: i64) {
        if self.is_pick_location(location) {
            self.pick_count = Some(quantity);
            return;
        }
        let location = normalize_location(location);
        let found = self
            .overflow_counts
            .binary_search_by(|count| compare_locations(&count.location, &location));
        match (found, quantity) {
            (Ok(ix), 0) => {
                self.overflow_counts.remove(ix);
            }
            (Ok(ix), _) => self.overflow_counts[ix].quantity = quantity,
            (Err(_), 0) => {}
            (Err(ix), _) => self
                .overflow_counts
                .insert(ix, OverflowCount { location, quantity }),
        }
    }

    fn matches_exactly(&self, query: &str) -> bool {
        (!self.barcode.is_empty() && self.barcode.eq_ignore_ascii_case(query))
            || self.item_number.eq_ignore_ascii_case(query)
    }

    fn contains(&self, needle: &str) -> bool {
        [&self.item_number, &self.name, &self.location, &self.barcode]
            .into_iter()
            .chain(&self.moved_location)
            .chain(self.overflow_counts.iter().map(|count| &count.location))
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
    /// false. Partly counted products go with the uncounted ones, since
    /// their pick location is still to count.
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

    /// Records what was counted of a product at `location`, replacing any
    /// earlier count there. Zero at an overflow location removes it.
    pub fn set_count(&mut self, id: ProductId, location: &str, quantity: i64) {
        self.products[id.0].set_count(location, quantity);
    }

    /// Makes `location` the product's pick location, as long as nothing has
    /// been counted at the current one. Anything counted at `location` as an
    /// overflow location becomes the pick location's count. Returns whether
    /// it was moved.
    pub fn move_pick_location(&mut self, id: ProductId, location: &str) -> bool {
        self.products[id.0].move_pick_location(location)
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
        stocktake.set_count(ProductId(0), "", 33);
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
        stocktake.set_count(ProductId(3), "", 0);
        assert_eq!(stocktake.counted_len(), 1);
        assert_eq!(stocktake.product(ProductId(3)).difference(), Some(0));
        stocktake.set_count(ProductId(0), "", 30);
        assert_eq!(stocktake.product(ProductId(0)).difference(), Some(-3));
        assert_eq!(stocktake.uncounted_len(), 2);
    }

    #[test]
    fn counts_add_up_across_locations() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "C4-7", 40);
        stocktake.set_count(id, " d2-1 ", 6);
        stocktake.set_count(id, "A1", 2);
        let product = stocktake.product(id);
        assert_eq!(product.counted_quantity(), Some(48));
        assert_eq!(product.difference(), Some(-1));
        // The pick location first, then the overflow locations in order.
        assert_eq!(
            product.counts().collect::<Vec<_>>(),
            [("C4-7", 40), ("A1", 2), ("D2-1", 6)]
        );
        assert_eq!(product.count_at("c4-7"), Some(40));
        assert_eq!(product.count_at("D2-1"), Some(6));
        assert_eq!(product.count_at("D2-2"), None);
    }

    #[test]
    fn a_count_replaces_the_one_at_its_location() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "D2-1", 6);
        stocktake.set_count(id, "D2-1", 4);
        stocktake.set_count(id, "", 40);
        stocktake.set_count(id, "C4-7", 41);
        assert_eq!(stocktake.product(id).counted_quantity(), Some(45));
    }

    #[test]
    fn zero_at_an_overflow_location_removes_it() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "D21", 4);
        stocktake.set_count(id, "D21", 0);
        assert!(!stocktake.product(id).is_counted());
        assert_eq!(stocktake.product(id).overflow_len(), 0);
        // Zero at the pick location is a count like any other.
        stocktake.set_count(id, "C4-7", 0);
        assert_eq!(stocktake.product(id).counted_quantity(), Some(0));
    }

    #[test]
    fn counted_only_at_an_overflow_location_is_partly_counted() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "D2-1", 5);
        let product = stocktake.product(id);
        assert_eq!(product.count_state(), CountState::PartlyCounted);
        assert!(!product.is_counted());
        assert_eq!(product.counted_quantity(), None);
        assert_eq!(product.counts().collect::<Vec<_>>(), [("D2-1", 5)]);
        assert_eq!(stocktake.counted_len(), 0);

        // Zero at the pick location counts it.
        stocktake.set_count(id, "C4-7", 0);
        let product = stocktake.product(id);
        assert_eq!(product.count_state(), CountState::Counted);
        assert_eq!(product.counted_quantity(), Some(5));
        assert_eq!(
            product.counts().collect::<Vec<_>>(),
            [("C4-7", 0), ("D2-1", 5)]
        );
    }

    #[test]
    fn a_product_without_a_pick_location_counts_there_when_left_empty() {
        let mut stocktake = stocktake();
        let id = ProductId(2);
        stocktake.set_count(id, "", 16);
        stocktake.set_count(id, "B1", 1);
        assert_eq!(
            stocktake.product(id).counts().collect::<Vec<_>>(),
            [("", 16), ("B1", 1)]
        );
    }

    #[test]
    fn search_finds_overflow_locations_but_aisles_go_by_pick_location() {
        let mut stocktake = stocktake();
        stocktake.set_count(ProductId(1), "E9", 3);
        assert_eq!(stocktake.search("e9"), [ProductId(1)]);
        let mut filter = Filter::default();
        filter.set_aisle_shown("C", false);
        assert_eq!(stocktake.search_filtered("e9", &filter), []);
        assert_eq!(stocktake.aisles(), [("C", 3), ("", 1)]);
    }

    #[test]
    fn moving_the_pick_location() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "D2-1", 6);
        assert!(stocktake.move_pick_location(id, " d2-1 "));
        let product = stocktake.product(id);
        assert_eq!(product.location(), "D2-1");
        assert_eq!(product.listed_location(), "C4-7");
        assert_eq!(product.aisle(), "D");
        // What was counted there as an overflow location is the pick count now.
        assert_eq!(product.count_at("D2-1"), Some(6));
        assert_eq!(product.overflow_len(), 0);
        assert_eq!(product.count_at("C4-7"), None);
        assert_eq!(stocktake.search("D2-1"), [id]);

        // Once counted, it stays.
        assert!(!stocktake.move_pick_location(id, "E1"));
        assert_eq!(stocktake.product(id).location(), "D2-1");
    }

    #[test]
    fn moving_the_pick_location_back() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        assert!(stocktake.move_pick_location(id, "D2-1"));
        stocktake.set_count(id, "C4-7", 3);
        assert!(stocktake.move_pick_location(id, "c4-7"));
        let product = stocktake.product(id);
        assert_eq!(product.location(), "C4-7");
        assert_eq!(product.count_at("C4-7"), Some(3));
        assert_eq!(product.count_at("D2-1"), None);
    }
}
