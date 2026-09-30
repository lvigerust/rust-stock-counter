//! The stock list as a table, with the counted-quantity cell of the product
//! being counted turned into an input.

use std::{cmp::Ordering, time::Instant};

use gpui_kit::component::{
    input::{Input, InputState},
    table::{Column, ColumnSort, TableDelegate, TableState},
};
use gpui_kit::{Div, Pixels, Stateful, px};
use stocktake::{ProductId, Stocktake, compare_locations, natural_cmp};
use ui::{Delta, flash, prelude::*};

use crate::{COUNT_CELL_CONTEXT, count_status::CountStatus};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProductColumn {
    Location,
    ItemNumber,
    Name,
    SystemQuantity,
    CountedQuantity,
    Difference,
    Status,
}

impl ProductColumn {
    const ALL: [Self; 7] = [
        Self::Location,
        Self::ItemNumber,
        Self::Name,
        Self::SystemQuantity,
        Self::CountedQuantity,
        Self::Difference,
        Self::Status,
    ];

    /// The narrowest the product name gets; it takes whatever the window
    /// has beyond the other columns.
    const MIN_NAME_WIDTH: f32 = 320.;

    fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::SystemQuantity | Self::CountedQuantity | Self::Difference
        )
    }

    fn key_and_name(self) -> (&'static str, &'static str) {
        match self {
            Self::Location => ("location", "Lokasjon"),
            Self::ItemNumber => ("item-number", "Varenummer"),
            Self::Name => ("name", "Produkt"),
            Self::SystemQuantity => ("system-quantity", "På lager"),
            Self::CountedQuantity => ("counted-quantity", "Telt"),
            Self::Difference => ("difference", "Differanse"),
            Self::Status => ("status", "Status"),
        }
    }

    /// Fixed widths for everything but the name. Column widths are table
    /// geometry, which the table API takes in pixels.
    fn fixed_width(self) -> Option<f32> {
        match self {
            Self::Location => Some(120.),
            Self::ItemNumber => Some(140.),
            Self::Name => None,
            Self::SystemQuantity => Some(120.),
            Self::CountedQuantity => Some(120.),
            Self::Difference => Some(130.),
            Self::Status => Some(140.),
        }
    }
}

/// When the most recently counted product was counted. The flash is timed
/// from here rather than from the row element, which is recreated whenever
/// the row scrolls back into view.
#[derive(Clone, Copy)]
pub struct LastCounted {
    pub id: ProductId,
    pub at: Instant,
}

pub struct ProductTable {
    stocktake: Entity<Stocktake>,
    /// The products matching the search, in the storage's walking order.
    matches: Vec<ProductId>,
    /// `matches` in display order: sorted when the counter picked a column.
    rows: Vec<ProductId>,
    /// The column the counter sorted by, if any.
    sort: Option<(ProductColumn, ColumnSort)>,
    /// How wide the table is, so the name column can take what's left.
    width: Pixels,
    count_input: Entity<InputState>,
    /// The product whose counted quantity is being entered.
    counting: Option<ProductId>,
    last_counted: Option<LastCounted>,
}

impl ProductTable {
    pub fn new(stocktake: Entity<Stocktake>, count_input: Entity<InputState>, cx: &App) -> Self {
        let matches = stocktake.read(cx).search("");
        Self {
            stocktake,
            rows: matches.clone(),
            matches,
            sort: None,
            width: px(0.),
            count_input,
            counting: None,
            last_counted: None,
        }
    }

    pub fn set_rows(&mut self, matches: Vec<ProductId>, cx: &App) {
        self.matches = matches;
        self.apply_sort(cx);
    }

    /// Call `TableState::refresh` afterwards, so the table reads the new
    /// column widths.
    pub fn set_width(&mut self, width: Pixels) {
        self.width = width;
    }

    pub fn product_at(&self, row_ix: usize) -> Option<ProductId> {
        self.rows.get(row_ix).copied()
    }

    pub fn row_of(&self, id: ProductId) -> Option<usize> {
        self.rows.iter().position(|row| *row == id)
    }

    pub fn counting(&self) -> Option<ProductId> {
        self.counting
    }

    pub fn set_counting(&mut self, counting: Option<ProductId>) {
        self.counting = counting;
    }

    /// Flashes the row of the product that was just counted, so the counter
    /// sees where the count landed after looking back from the shelf.
    pub fn set_last_counted(&mut self, last_counted: LastCounted) {
        self.last_counted = Some(last_counted);
    }

    fn apply_sort(&mut self, cx: &App) {
        self.rows = self.matches.clone();
        let Some((column, direction)) = self.sort else {
            return;
        };
        let stocktake = self.stocktake.read(cx);
        // Stable, so equal values keep the walking order.
        self.rows.sort_by(|a, b| {
            let (a, b) = (stocktake.product(*a), stocktake.product(*b));
            let ordering = match column {
                ProductColumn::Location => compare_locations(a.location(), b.location()),
                ProductColumn::ItemNumber => natural_cmp(a.item_number(), b.item_number()),
                ProductColumn::Name => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
                ProductColumn::SystemQuantity => a.system_quantity().cmp(&b.system_quantity()),
                ProductColumn::CountedQuantity => {
                    return uncounted_last(a.counted_quantity(), b.counted_quantity(), direction);
                }
                ProductColumn::Difference => {
                    return uncounted_last(a.difference(), b.difference(), direction);
                }
                // Uncounted first when ascending: that's the work left.
                ProductColumn::Status => a.is_counted().cmp(&b.is_counted()),
            };
            directed(ordering, direction)
        });
    }

    fn render_counted_quantity(&self, id: ProductId, cx: &App) -> AnyElement {
        if self.counting == Some(id) {
            return div()
                .key_context(COUNT_CELL_CONTEXT)
                .w_full()
                .child(Input::new(&self.count_input).id("count").xsmall())
                .into_any_element();
        }
        match self.stocktake.read(cx).product(id).counted_quantity() {
            Some(quantity) => quantity.to_string().into_any_element(),
            None => div()
                .text_color(cx.theme().muted_foreground)
                .child("–")
                .into_any_element(),
        }
    }
}

fn directed(ordering: Ordering, direction: ColumnSort) -> Ordering {
    match direction {
        ColumnSort::Descending => ordering.reverse(),
        ColumnSort::Ascending | ColumnSort::Default => ordering,
    }
}

/// Uncounted products have no value to compare, so they stay at the bottom
/// whichever way the column is sorted.
fn uncounted_last(a: Option<i64>, b: Option<i64>, direction: ColumnSort) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => directed(a.cmp(&b), direction),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

impl TableDelegate for ProductTable {
    fn columns_count(&self, _: &App) -> usize {
        ProductColumn::ALL.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let column = ProductColumn::ALL[col_ix];
        let (key, name) = column.key_and_name();
        let fixed: f32 = ProductColumn::ALL
            .iter()
            .filter_map(|column| column.fixed_width())
            .sum();
        // Leave room for the vertical scrollbar.
        let name_width = (f32::from(self.width) - fixed - 16.).max(ProductColumn::MIN_NAME_WIDTH);
        let sort = match self.sort {
            Some((sorted, direction)) if sorted == column => direction,
            _ => ColumnSort::Default,
        };
        Column::new(key, name)
            .width(px(column.fixed_width().unwrap_or(name_width)))
            .min_width(px(64.))
            .movable(false)
            .sort(sort)
            .when(column.is_numeric(), |column| column.text_right())
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sort = match sort {
            ColumnSort::Default => None,
            direction => Some((ProductColumn::ALL[col_ix], direction)),
        };
        self.apply_sort(cx);
        // A selected row index now points at a different product.
        let table = cx.entity();
        cx.defer(move |cx| table.update(cx, |table, cx| table.clear_selection(cx)));
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let column = ProductColumn::ALL[col_ix];
        h_flex()
            .size_full()
            .when(column.is_numeric(), |this| this.justify_end())
            .child(column.key_and_name().1)
    }

    /// Rows are keyed by product, not position, so row state follows the
    /// product when a search reorders or filters the table.
    fn render_tr(
        &mut self,
        row_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let Some(id) = self.product_at(row_ix) else {
            return div().id(("row", row_ix));
        };
        let row = div().id(("product", id.line()));
        match self.last_counted {
            Some(last) if last.id == id => {
                let strength = flash(last.at, window, cx);
                row.when(strength > 0.0, |row| {
                    row.bg(cx.theme().success.opacity(0.22 * strength))
                })
            }
            _ => row,
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(id) = self.product_at(row_ix) else {
            return div().into_any_element();
        };
        let column = ProductColumn::ALL[col_ix];
        let product = self.stocktake.read(cx).product(id);
        let content = match column {
            ProductColumn::Location => product.location().to_string().into_any_element(),
            ProductColumn::ItemNumber => div()
                .text_color(cx.theme().muted_foreground)
                .child(product.item_number().to_string())
                .into_any_element(),
            // The row's focal point: what the counter reads off the shelf.
            ProductColumn::Name => div()
                .truncate()
                .font_medium()
                .child(product.name().to_string())
                .into_any_element(),
            ProductColumn::SystemQuantity => {
                product.system_quantity().to_string().into_any_element()
            }
            ProductColumn::CountedQuantity => self.render_counted_quantity(id, cx),
            ProductColumn::Difference => match product.difference() {
                Some(difference) => Delta::new(difference).into_any_element(),
                None => div().into_any_element(),
            },
            ProductColumn::Status => CountStatus::new(product.is_counted()).into_any_element(),
        };
        h_flex()
            .size_full()
            .when(column.is_numeric(), |this| {
                this.justify_end().tabular_nums()
            })
            .child(content)
            .into_any_element()
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::SearchX).large())
            .child(div().text_sm().child("Ingen varer passer søket"))
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, cx: &App) -> String {
        let Some(id) = self.product_at(row_ix) else {
            return String::new();
        };
        let product = self.stocktake.read(cx).product(id);
        match ProductColumn::ALL[col_ix] {
            ProductColumn::Location => product.location().to_string(),
            ProductColumn::ItemNumber => product.item_number().to_string(),
            ProductColumn::Name => product.name().to_string(),
            ProductColumn::SystemQuantity => product.system_quantity().to_string(),
            ProductColumn::CountedQuantity => product
                .counted_quantity()
                .map(|quantity| quantity.to_string())
                .unwrap_or_default(),
            ProductColumn::Difference => product
                .difference()
                .map(|difference| difference.to_string())
                .unwrap_or_default(),
            ProductColumn::Status => if product.is_counted() {
                "Telt"
            } else {
                "Ikke telt"
            }
            .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncounted_products_sort_last_both_ways() {
        let mut values = vec![None, Some(3), Some(-1), None, Some(0)];
        values.sort_by(|a, b| uncounted_last(*a, *b, ColumnSort::Ascending));
        assert_eq!(values, [Some(-1), Some(0), Some(3), None, None]);
        values.sort_by(|a, b| uncounted_last(*a, *b, ColumnSort::Descending));
        assert_eq!(values, [Some(3), Some(0), Some(-1), None, None]);
    }
}
