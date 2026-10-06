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
//! | [`settings`]   | The counter's settings, kept between launches             |
//!
//! Every file operation here is synchronous and returns its own error type;
//! the UI decides what runs in the background and how errors are worded.

pub mod export;
pub mod recent;
pub mod settings;
pub mod stock_list;
pub mod store;

use std::cmp::Ordering;
use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identifies a product by its position in the stocktake's product list.
///
/// The list never changes during a stocktake, so the position is stable.
/// Item numbers are not used because the business system can list one item
/// number on several lines; the import merges them, but a stocktake built
/// by hand needn't.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductId(usize);

impl ProductId {
    /// The product's position in the stocktake. Stable for the whole
    /// stocktake, so it can key UI state that must follow the product
    /// through filtering.
    pub fn line(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Product {
    item_number: String,
    name: String,
    /// Colour and style, which tell apart products with the same name.
    /// Empty when the stock list gives none.
    #[serde(default)]
    description: String,
    /// The pick location the stock list gives, or the one chosen at import
    /// when the stock list gives several.
    location: String,
    /// Every location the stock list gives, in its order, when it gives
    /// more than one; empty otherwise.
    #[serde(default)]
    listed_locations: Vec<String>,
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
    pick_count: Option<Count>,
    /// What was counted at each overflow location, in location order. None
    /// holds zero: a count of zero removes the location.
    #[serde(default)]
    overflow_counts: Vec<OverflowCount>,
    /// Whether a counter has stopped looking for more units. Only means
    /// anything once the product is counted.
    #[serde(default)]
    finished: bool,
    /// The fields that never change, lower-cased once so a search doesn't
    /// lower-case every product on every keystroke. Not saved; [`Stocktake`]
    /// rebuilds it when loaded.
    #[serde(skip)]
    search_key: String,
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
    /// Counted, and a counter has stopped looking for more units.
    Finished,
}

/// Units counted at one location, and when.
///
/// Saves from before counts were stamped hold a bare quantity, which reads
/// as a count made at an unknown time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedCount")]
struct Count {
    quantity: i64,
    counted_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SavedCount {
    Quantity(i64),
    Count {
        quantity: i64,
        #[serde(default)]
        counted_at: Option<DateTime<Utc>>,
    },
}

impl From<SavedCount> for Count {
    fn from(saved: SavedCount) -> Self {
        match saved {
            SavedCount::Quantity(quantity) => Self {
                quantity,
                counted_at: None,
            },
            SavedCount::Count {
                quantity,
                counted_at,
            } => Self {
                quantity,
                counted_at,
            },
        }
    }
}

/// Units of a product counted at one of its overflow locations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct OverflowCount {
    location: String,
    quantity: i64,
    #[serde(default)]
    counted_at: Option<DateTime<Utc>>,
}

/// Two counts together, `None` only while neither has been made. The
/// later time is kept, since that's when the sum was last right.
fn add_counts(a: Option<Count>, b: Option<Count>) -> Option<Count> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Count {
            quantity: a.quantity + b.quantity,
            counted_at: a.counted_at.max(b.counted_at),
        }),
        (a, b) => a.or(b),
    }
}

/// Whether two locations name the same shelf: the same after trimming,
/// ignoring letter case and leading zeros in numbers, so `c4-7` and `C4-7`
/// aren't recorded as two.
pub fn same_location(a: &str, b: &str) -> bool {
    compare_locations(a.trim(), b.trim()) == Ordering::Equal
}

impl Product {
    pub fn new(
        item_number: impl Into<String>,
        name: impl Into<String>,
        location: impl Into<String>,
        barcode: impl Into<String>,
        system_quantity: i64,
    ) -> Self {
        let mut product = Self {
            item_number: item_number.into(),
            name: name.into(),
            description: String::new(),
            location: location.into(),
            listed_locations: Vec::new(),
            barcode: barcode.into(),
            system_quantity,
            moved_location: None,
            pick_count: None,
            overflow_counts: Vec::new(),
            finished: false,
            search_key: String::new(),
        };
        product.refresh_search_key();
        product
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self.refresh_search_key();
        self
    }

    /// Gives the product the locations the stock list listed it at, when
    /// there were several. The pick location stays as it was.
    pub fn with_listed_locations(mut self, locations: Vec<String>) -> Self {
        self.listed_locations = locations;
        self
    }

    fn refresh_search_key(&mut self) {
        self.search_key = [
            &self.item_number,
            &self.name,
            &self.description,
            &self.barcode,
        ]
        .map(|field| field.to_lowercase())
        .join("\n");
    }

    pub fn item_number(&self) -> &str {
        &self.item_number
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Colour and style, empty when the stock list gives none.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The pick location, empty when the product has none: where the
    /// counter moved it, or else the one the stock list gives.
    pub fn location(&self) -> &str {
        self.moved_location.as_deref().unwrap_or(&self.location)
    }

    /// The pick location the stock list gives, whether or not it was moved.
    /// For a product the stock list listed several times, the one chosen at
    /// import.
    pub fn listed_location(&self) -> &str {
        &self.location
    }

    /// The locations the stock list listed the product at that aren't its
    /// listed pick location. Empty unless the stock list listed it several
    /// times.
    pub fn other_listed_locations(&self) -> impl Iterator<Item = &str> {
        self.listed_locations
            .iter()
            .map(String::as_str)
            .filter(|listed| !same_location(listed, &self.location))
    }

    /// Every location the product is known at, in no particular order and
    /// possibly repeated: the listed ones, where it was moved, and where it
    /// overflowed.
    fn locations(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.location.as_str())
            .chain(self.listed_locations.iter().map(String::as_str))
            .chain(self.moved_location.as_deref())
            .chain(
                self.overflow_counts
                    .iter()
                    .map(|count| count.location.as_str()),
            )
    }

    /// Whether `location` is the pick location. An empty location is too,
    /// so a product without one can still be counted at it.
    pub fn is_pick_location(&self, location: &str) -> bool {
        location.trim().is_empty() || same_location(location, self.location())
    }

    /// What has been counted at `location`, `None` while nothing has.
    pub fn count_at(&self, location: &str) -> Option<i64> {
        self.count_record_at(location).map(|count| count.quantity)
    }

    /// When `location` was last counted, `None` while it hasn't been, or
    /// when the count is from a save made before counts were stamped.
    pub fn counted_at(&self, location: &str) -> Option<DateTime<Utc>> {
        self.count_record_at(location)
            .and_then(|count| count.counted_at)
    }

    /// When the product was last counted at any of its locations.
    pub fn latest_counted_at(&self) -> Option<DateTime<Utc>> {
        self.pick_count
            .and_then(|count| count.counted_at)
            .into_iter()
            .chain(
                self.overflow_counts
                    .iter()
                    .filter_map(|count| count.counted_at),
            )
            .max()
    }

    fn count_record_at(&self, location: &str) -> Option<Count> {
        if self.is_pick_location(location) {
            return self.pick_count;
        }
        self.overflow_count_at(location)
    }

    /// What would be counted at `location` once the pick location moved
    /// there: the pick location's count, which moves with it, together with
    /// anything counted there as an overflow location.
    pub fn count_at_moved(&self, location: &str) -> Option<i64> {
        if self.is_pick_location(location) {
            return self.pick_count.map(|count| count.quantity);
        }
        add_counts(self.pick_count, self.overflow_count_at(location)).map(|count| count.quantity)
    }

    fn overflow_count_at(&self, location: &str) -> Option<Count> {
        self.find_overflow(location).ok().map(|ix| {
            let count = &self.overflow_counts[ix];
            Count {
                quantity: count.quantity,
                counted_at: count.counted_at,
            }
        })
    }

    /// Where `location` is among the overflow counts, or where it would go.
    fn find_overflow(&self, location: &str) -> Result<usize, usize> {
        let location = location.trim();
        self.overflow_counts
            .binary_search_by(|count| compare_locations(&count.location, location))
    }

    /// Every location something has been counted at, with what was counted
    /// there: the pick location first, once it has been counted, then the
    /// overflow locations.
    pub fn counts(&self) -> impl Iterator<Item = (&str, i64)> {
        let pick = self
            .pick_count
            .map(|count| (self.location(), count.quantity));
        pick.into_iter().chain(self.overflow_counts())
    }

    /// The overflow locations counted at, with what was counted there.
    pub fn overflow_counts(&self) -> impl Iterator<Item = (&str, i64)> {
        self.overflow_counts
            .iter()
            .map(|count| (count.location.as_str(), count.quantity))
    }

    /// What was counted at the overflow locations together, `None` when
    /// nothing was.
    pub fn overflow_quantity(&self) -> Option<i64> {
        (!self.overflow_counts.is_empty())
            .then(|| self.overflow_counts().map(|(_, quantity)| quantity).sum())
    }

    /// What the system quantity leaves for the pick location once the
    /// overflow locations are counted, never below zero. `None` unless the
    /// product is partly counted: with nothing counted elsewhere it's just
    /// the system quantity, and once the pick location is counted there's
    /// nothing left to expect.
    pub fn expected_at_pick_location(&self) -> Option<i64> {
        if self.is_counted() {
            return None;
        }
        self.overflow_quantity()
            .map(|overflow| (self.system_quantity - overflow).max(0))
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
            pick_count.quantity
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

    /// Whether a counter has marked the product finished: counted, and not
    /// worth looking for more units of.
    pub fn is_finished(&self) -> bool {
        self.finished && self.is_counted()
    }

    pub fn count_state(&self) -> CountState {
        if self.is_finished() {
            CountState::Finished
        } else if self.is_counted() {
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

    /// Makes `location`, already resolved by the stocktake, the pick
    /// location, unless it's empty. The pick location's count moves with it,
    /// and anything counted there as an overflow location is added to it, so
    /// the counted quantity stays the same. Returns whether it was moved.
    fn move_pick_location(&mut self, location: &str) -> bool {
        if location.is_empty() {
            return false;
        }
        if let Ok(ix) = self.find_overflow(location) {
            let overflow = self.overflow_counts.remove(ix);
            self.pick_count = add_counts(
                self.pick_count,
                Some(Count {
                    quantity: overflow.quantity,
                    counted_at: overflow.counted_at,
                }),
            );
        }
        self.moved_location =
            (!same_location(location, &self.location)).then(|| location.to_string());
        true
    }

    /// Makes one of the locations the stock list listed the product at its
    /// listed pick location, undoing the choice made at import. Counts move
    /// as with any move. Returns whether `location` was one of them.
    pub(crate) fn pick_listed_location(&mut self, location: &str) -> bool {
        let Some(listed) = self
            .listed_locations
            .iter()
            .find(|listed| same_location(listed, location))
            .cloned()
        else {
            return false;
        };
        self.move_pick_location(&listed);
        self.location = listed;
        self.moved_location = None;
        true
    }

    /// Records what was counted at `location`, already resolved by the
    /// stocktake, replacing any earlier count there. Zero at an overflow
    /// location removes it, which is how a mistyped one is corrected.
    fn set_count(&mut self, location: &str, quantity: i64, counted_at: DateTime<Utc>) {
        let count = Count {
            quantity,
            counted_at: Some(counted_at),
        };
        if self.is_pick_location(location) {
            self.pick_count = Some(count);
            return;
        }
        match (self.find_overflow(location), quantity) {
            (Ok(ix), 0) => {
                self.overflow_counts.remove(ix);
            }
            (Ok(ix), _) => {
                self.overflow_counts[ix].quantity = quantity;
                self.overflow_counts[ix].counted_at = count.counted_at;
            }
            (Err(_), 0) => {}
            (Err(ix), _) => self.overflow_counts.insert(
                ix,
                OverflowCount {
                    location: location.to_string(),
                    quantity,
                    counted_at: count.counted_at,
                },
            ),
        }
    }

    fn matches_exactly(&self, query: &str) -> bool {
        (!self.barcode.is_empty() && self.barcode.eq_ignore_ascii_case(query))
            || self.item_number.eq_ignore_ascii_case(query)
    }

    /// Whether the lower-cased `needle` is in any field a counter searches.
    fn contains(&self, needle: &str) -> bool {
        self.search_key.contains(needle)
            || self
                .locations()
                .any(|location| location.to_lowercase().contains(needle))
    }
}

/// Which products the table shows, besides those the search leaves out. The
/// default shows every product.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Stored as the aisles left out, so an aisle shows until the counter
    /// hides it.
    hidden_aisles: BTreeSet<String>,
    /// Likewise the count states left out.
    hidden_states: BTreeSet<CountState>,
    /// Whether products with a system quantity of zero are left out. The
    /// counters leave those for last.
    hides_zero_stock: bool,
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

    pub fn shows_state(&self, state: CountState) -> bool {
        !self.hidden_states.contains(&state)
    }

    pub fn set_state_shown(&mut self, state: CountState, shown: bool) {
        if shown {
            self.hidden_states.remove(&state);
        } else {
            self.hidden_states.insert(state);
        }
    }

    pub fn shows_zero_stock(&self) -> bool {
        !self.hides_zero_stock
    }

    pub fn set_zero_stock_shown(&mut self, shown: bool) {
        self.hides_zero_stock = !shown;
    }

    fn shows(&self, product: &Product) -> bool {
        self.shows_aisle(product.aisle())
            && self.shows_state(product.count_state())
            && (self.shows_zero_stock() || product.system_quantity != 0)
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
#[serde(from = "SavedStocktake")]
pub struct Stocktake {
    products: Vec<Product>,
}

/// A stocktake as saved. Loading goes through [`Stocktake::new`] so the
/// products' search keys, which aren't saved, are rebuilt.
#[derive(Deserialize)]
struct SavedStocktake {
    products: Vec<Product>,
}

impl From<SavedStocktake> for Stocktake {
    fn from(saved: SavedStocktake) -> Self {
        Self::new(saved.products)
    }
}

impl Stocktake {
    pub fn new(mut products: Vec<Product>) -> Self {
        for product in &mut products {
            product.refresh_search_key();
        }
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

    /// Products not counted yet, partly counted ones included.
    pub fn uncounted_len(&self) -> usize {
        self.len() - self.counted_len()
    }

    /// How many products are in `state`.
    pub fn state_len(&self, state: CountState) -> usize {
        self.products
            .iter()
            .filter(|p| p.count_state() == state)
            .count()
    }

    /// How many products the business system says there are none of.
    pub fn zero_stock_len(&self) -> usize {
        self.products
            .iter()
            .filter(|p| p.system_quantity == 0)
            .count()
    }

    /// Records what was counted of a product at `location`, replacing any
    /// earlier count there, stamped with now. Zero at an overflow location
    /// removes it. The location is spelled as [`Self::resolve_location`]
    /// says.
    pub fn set_count(&mut self, id: ProductId, location: &str, quantity: i64) {
        let location = self.resolve_location(location);
        self.products[id.0].set_count(&location, quantity, Utc::now());
    }

    /// Makes `location` the product's pick location, unless it's empty. The
    /// pick location's count moves with it, and anything counted at
    /// `location` as an overflow location is added to it. Returns whether it
    /// was moved.
    pub fn move_pick_location(&mut self, id: ProductId, location: &str) -> bool {
        let location = self.resolve_location(location);
        self.products[id.0].move_pick_location(&location)
    }

    /// Makes one of the locations the stock list listed the product at its
    /// pick location, undoing the choice made at import. Returns whether
    /// `location` was one of them.
    pub fn pick_listed_location(&mut self, id: ProductId, location: &str) -> bool {
        self.products[id.0].pick_listed_location(location)
    }

    /// Marks a product finished, or unmarks it. Means nothing until the
    /// product is counted.
    pub fn set_finished(&mut self, id: ProductId, finished: bool) {
        self.products[id.0].finished = finished;
    }

    /// A location as the counter typed it, spelled the way it should be
    /// recorded: trimmed, and as it's already spelled in the stocktake if a
    /// location there is the [same](same_location), so `tilbehør 1-1`
    /// becomes `Tilbehør 1-1`; otherwise upper-cased, so `c4-7` becomes
    /// `C4-7`.
    pub fn resolve_location(&self, typed: &str) -> String {
        let typed = typed.trim();
        if typed.is_empty() {
            return String::new();
        }
        self.products
            .iter()
            .flat_map(Product::locations)
            .find(|known| same_location(known, typed))
            .map_or_else(|| typed.to_uppercase(), str::to_string)
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

    /// The counted products whose counted quantity differs from the system
    /// quantity, in the order [`Self::search`] gives. Uncounted ones have no
    /// difference yet, so they're left out.
    pub fn differences(&self) -> Vec<ProductId> {
        self.search("")
            .into_iter()
            .filter(|id| self.product(*id).difference().is_some_and(|d| d != 0))
            .collect()
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
        let exact = self.exact_matches(query);
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

    /// The one product whose barcode or item number is exactly `text`.
    /// `None` when none is, or when several are: the business system can
    /// list one item number on several lines, and a guess between them
    /// would count the wrong one.
    pub fn product_matching_exactly(&self, text: &str) -> Option<ProductId> {
        match self.exact_matches(text.trim()).as_slice() {
            [id] => Some(*id),
            _ => None,
        }
    }

    fn exact_matches(&self, text: &str) -> Vec<ProductId> {
        self.products()
            .filter(|(_, product)| product.matches_exactly(text))
            .map(|(id, _)| id)
            .collect()
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
    fn search_matches_the_description() {
        let stocktake = Stocktake::new(vec![
            Product::new("1", "Como Fronter 120 - Grå Driftwood", "A1", "", 1)
                .with_description("Como Standard - Ramtre"),
            Product::new("2", "Como Fronter 120 - Grå Driftwood", "A2", "", 1)
                .with_description("Como Standard - Slett"),
        ]);
        assert_eq!(stocktake.search("ramtre"), [ProductId(0)]);
        assert_eq!(stocktake.search("como standard").len(), 2);
        assert_eq!(
            stocktake.product(ProductId(1)).description(),
            "Como Standard - Slett"
        );
    }

    #[test]
    fn differences_are_counted_products_that_differ() {
        let mut stocktake = stocktake();
        // Counted as expected, so no difference.
        stocktake.set_count(ProductId(0), "C4-10", 33);
        // Counted short.
        stocktake.set_count(ProductId(1), "C4-7", 40);
        // Units at an overflow location alone don't make it counted.
        stocktake.set_count(ProductId(2), "D2-1", 3);
        assert_eq!(stocktake.differences(), [ProductId(1)]);

        // Counted at zero where the system says zero isn't a difference;
        // counted over is, and comes first in walking order.
        stocktake.set_count(ProductId(3), "C3-1", 0);
        assert_eq!(stocktake.differences(), [ProductId(1)]);
        stocktake.set_count(ProductId(3), "C3-1", 2);
        assert_eq!(stocktake.differences(), [ProductId(3), ProductId(1)]);
    }

    #[test]
    fn aisle_is_the_leading_letters() {
        let aisle = |location| Product::new("1", "", location, "", 0).aisle().to_string();
        assert_eq!(aisle("C4-7"), "C");
        assert_eq!(aisle("AB12"), "AB");
        assert_eq!(aisle("DL-3"), "DL");
        assert_eq!(aisle("Tilbehør 1-1"), "Tilbehør");
        assert_eq!(aisle("Pakkedisk"), "Pakkedisk");
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
    fn filter_hides_products_by_count_state() {
        let mut stocktake = stocktake();
        stocktake.set_count(ProductId(0), "", 33);
        stocktake.set_count(ProductId(1), "D2-1", 2);
        assert_eq!(stocktake.state_len(CountState::Counted), 1);
        assert_eq!(stocktake.state_len(CountState::PartlyCounted), 1);
        assert_eq!(stocktake.state_len(CountState::Uncounted), 2);
        assert_eq!(stocktake.uncounted_len(), 3);

        let mut filter = Filter::default();
        filter.set_state_shown(CountState::Counted, false);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 3);
        filter.set_state_shown(CountState::Uncounted, false);
        assert_eq!(stocktake.search_filtered("", &filter), [ProductId(1)]);
        filter.set_state_shown(CountState::Counted, true);
        filter.set_state_shown(CountState::PartlyCounted, false);
        assert_eq!(stocktake.search_filtered("", &filter), [ProductId(0)]);
        filter.set_aisle_shown("C", false);
        assert_eq!(stocktake.search_filtered("", &filter), []);
    }

    #[test]
    fn filter_hides_zero_stock() {
        let stocktake = stocktake();
        assert_eq!(stocktake.zero_stock_len(), 1);
        let mut filter = Filter::default();
        assert!(filter.shows_zero_stock());
        filter.set_zero_stock_shown(false);
        assert_eq!(
            stocktake.search_filtered("", &filter),
            [ProductId(1), ProductId(0), ProductId(2)]
        );
        // A counted zero-stock product is hidden too; the filter goes by the
        // system quantity.
        filter.set_zero_stock_shown(true);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 4);
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
    fn product_matching_exactly_needs_a_whole_barcode_or_item_number() {
        let stocktake = stocktake();
        assert_eq!(
            stocktake.product_matching_exactly(" 7043811520629\n"),
            Some(ProductId(1))
        );
        assert_eq!(
            stocktake.product_matching_exactly("152062"),
            Some(ProductId(1))
        );
        // Partial numbers, names and nothing at all aren't whole identifiers.
        assert_eq!(stocktake.product_matching_exactly("15206"), None);
        assert_eq!(stocktake.product_matching_exactly("veneto 90"), None);
        assert_eq!(stocktake.product_matching_exactly(""), None);
    }

    #[test]
    fn product_matching_exactly_skips_an_item_number_listed_twice() {
        let stocktake = Stocktake::new(vec![
            Product::new("1", "Vare, parti A", "A1", "", 1),
            Product::new("1", "Vare, parti B", "A2", "", 2),
        ]);
        assert_eq!(stocktake.product_matching_exactly("1"), None);
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
        assert_eq!(product.overflow_quantity(), Some(8));
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
        assert_eq!(stocktake.product(id).overflow_quantity(), None);
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
        assert_eq!(product.overflow_quantity(), Some(5));
        assert_eq!(product.counts().collect::<Vec<_>>(), [("D2-1", 5)]);
        assert_eq!(product.expected_at_pick_location(), Some(44));
        assert_eq!(stocktake.counted_len(), 0);

        // Zero at the pick location counts it.
        stocktake.set_count(id, "C4-7", 0);
        let product = stocktake.product(id);
        assert_eq!(product.count_state(), CountState::Counted);
        assert_eq!(product.expected_at_pick_location(), None);
        assert_eq!(product.counted_quantity(), Some(5));
        assert_eq!(
            product.counts().collect::<Vec<_>>(),
            [("C4-7", 0), ("D2-1", 5)]
        );
    }

    #[test]
    fn nothing_is_expected_at_the_pick_location_beyond_the_system_quantity() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        assert_eq!(stocktake.product(id).expected_at_pick_location(), None);
        stocktake.set_count(id, "D2-1", 60);
        assert_eq!(stocktake.product(id).expected_at_pick_location(), Some(0));
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
    fn a_typed_location_takes_the_spelling_already_in_use() {
        let mut stocktake = Stocktake::new(vec![
            Product::new("1", "Vare", "Tilbehør 1-1", "", 1),
            Product::new("2", "Vare", "C4-7", "", 1),
        ]);
        // A known location keeps its spelling, a new one is upper-cased.
        assert_eq!(stocktake.resolve_location(" tilbehør 1-1 "), "Tilbehør 1-1");
        assert_eq!(stocktake.resolve_location("c4-7"), "C4-7");
        assert_eq!(
            stocktake.resolve_location("golv ytre lager"),
            "GOLV YTRE LAGER"
        );
        assert_eq!(stocktake.resolve_location("  "), "");

        // Once recorded, a new location's spelling is in use too.
        stocktake.set_count(ProductId(1), "golv ytre lager", 2);
        assert_eq!(
            stocktake.resolve_location("Golv Ytre Lager"),
            "GOLV YTRE LAGER"
        );
        assert_eq!(
            stocktake.product(ProductId(1)).count_at("golv ytre lager"),
            Some(2)
        );

        // Counting at the known spelling's shelf in another case records it once.
        stocktake.set_count(ProductId(1), "TILBEHØR 1-1", 3);
        assert_eq!(
            stocktake
                .product(ProductId(1))
                .overflow_counts()
                .collect::<Vec<_>>(),
            [("GOLV YTRE LAGER", 2), ("Tilbehør 1-1", 3)]
        );
        assert!(stocktake.move_pick_location(ProductId(1), "tilbehør 1-1"));
        assert_eq!(stocktake.product(ProductId(1)).location(), "Tilbehør 1-1");
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

        // Once counted, its count moves with it.
        assert!(stocktake.move_pick_location(id, "E1"));
        let product = stocktake.product(id);
        assert_eq!(product.location(), "E1");
        assert_eq!(product.count_at("E1"), Some(6));
        assert_eq!(product.count_at("D2-1"), None);
        assert!(!stocktake.move_pick_location(id, " "));
    }

    #[test]
    fn a_moved_pick_count_is_added_to_one_at_the_new_location() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        stocktake.set_count(id, "C4-7", 5);
        stocktake.set_count(id, "D2-1", 2);
        assert_eq!(stocktake.product(id).count_at_moved("d2-1"), Some(7));
        assert_eq!(stocktake.product(id).count_at_moved("E1"), Some(5));
        assert!(stocktake.move_pick_location(id, "D2-1"));
        let product = stocktake.product(id);
        assert_eq!(product.count_at("D2-1"), Some(7));
        assert_eq!(product.overflow_len(), 0);
        assert_eq!(product.counted_quantity(), Some(7));
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

    #[test]
    fn picking_another_listed_location_undoes_the_import_choice() {
        let mut stocktake = Stocktake::new(vec![
            Product::new("1", "Como Fronter 60", "A1", "", 16)
                .with_listed_locations(vec!["A1".into(), "E2-8".into()]),
        ]);
        let id = ProductId(0);
        let product = stocktake.product(id);
        assert_eq!(
            product.other_listed_locations().collect::<Vec<_>>(),
            ["E2-8"]
        );
        assert_eq!(stocktake.search("e2-8"), [id]);

        // Counts move with the pick location, and the new one is the listed
        // one, not a move.
        stocktake.set_count(id, "A1", 0);
        stocktake.set_count(id, "E2-8", 16);
        assert!(stocktake.pick_listed_location(id, "e2-8"));
        let product = stocktake.product(id);
        assert_eq!(product.location(), "E2-8");
        assert_eq!(product.listed_location(), "E2-8");
        assert_eq!(product.other_listed_locations().collect::<Vec<_>>(), ["A1"]);
        assert_eq!(product.count_at("E2-8"), Some(16));
        assert_eq!(product.counted_quantity(), Some(16));
        assert_eq!(product.overflow_len(), 0);

        // A location the stock list didn't list isn't one of them.
        assert!(!stocktake.pick_listed_location(id, "B1"));
        // Picking a listed location after a move clears the move.
        assert!(stocktake.move_pick_location(id, "B1"));
        assert!(stocktake.pick_listed_location(id, "A1"));
        assert_eq!(stocktake.product(id).location(), "A1");
        assert_eq!(stocktake.product(id).listed_location(), "A1");
    }

    #[test]
    fn finished_is_a_state_of_counted_products() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        // Unmarked and uncounted, the mark means nothing.
        stocktake.set_finished(id, true);
        assert!(!stocktake.product(id).is_finished());
        assert_eq!(stocktake.product(id).count_state(), CountState::Uncounted);

        stocktake.set_count(id, "C4-7", 48);
        assert_eq!(stocktake.product(id).count_state(), CountState::Finished);
        assert!(stocktake.product(id).is_counted());
        assert_eq!(stocktake.counted_len(), 1);
        assert_eq!(stocktake.state_len(CountState::Finished), 1);
        assert_eq!(stocktake.state_len(CountState::Counted), 0);

        // Counting again leaves the mark alone; unmarking removes it.
        stocktake.set_count(id, "D2-1", 1);
        assert_eq!(stocktake.product(id).count_state(), CountState::Finished);
        stocktake.set_finished(id, false);
        assert_eq!(stocktake.product(id).count_state(), CountState::Counted);

        let mut filter = Filter::default();
        filter.set_state_shown(CountState::Counted, false);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 3);
        stocktake.set_finished(id, true);
        assert_eq!(stocktake.search_filtered("", &filter).len(), 4);
    }

    #[test]
    fn counts_are_stamped_when_made() {
        let mut stocktake = stocktake();
        let id = ProductId(1);
        let product = stocktake.product(id);
        assert_eq!(product.latest_counted_at(), None);
        assert_eq!(product.counted_at("C4-7"), None);

        let before = Utc::now();
        stocktake.set_count(id, "D2-1", 2);
        let overflow_at = stocktake.product(id).counted_at("D2-1").unwrap();
        assert!(overflow_at >= before);
        stocktake.set_count(id, "C4-7", 40);
        let product = stocktake.product(id);
        let pick_at = product.counted_at("C4-7").unwrap();
        assert!(pick_at >= overflow_at);
        assert_eq!(product.latest_counted_at(), Some(pick_at));

        // A moved count keeps the later of the two times.
        assert!(stocktake.move_pick_location(id, "D2-1"));
        assert_eq!(stocktake.product(id).counted_at("D2-1"), Some(pick_at));
    }

    #[test]
    fn a_loaded_stocktake_searches_like_a_new_one() {
        let saved = r#"{"products":[{"item_number":"1","name":"Burano","description":"Hvit",
            "location":"A1","barcode":"","system_quantity":5}]}"#;
        let stocktake: Stocktake = serde_json::from_str(saved).unwrap();
        assert_eq!(stocktake.search("hvit"), [ProductId(0)]);
        assert_eq!(stocktake.search("burano"), [ProductId(0)]);
    }
}
