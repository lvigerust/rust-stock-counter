//! The stock list as a table.

use std::{cmp::Ordering, time::Instant};

use gpui_kit::component::{
    table::{Column, ColumnSort, TableDelegate, TableState},
    tag::Tag,
    tooltip::Tooltip,
};
use gpui_kit::{Div, Edges, Pixels, Stateful, px};
use stocktake::{ProductId, compare_locations, natural_cmp};
use ui::{Delta, flash, prelude::*};

use crate::{count_status::CountStatus, session::Session};

/// How tall each row is: a 1.5rem line and [`ROW_PADDING`] above and below
/// it. Table geometry is in pixels.
pub const ROW_HEIGHT: Pixels = px(56.);

/// Padding around a cell's content, 1rem, so columns are 2rem apart.
const ROW_PADDING: Pixels = px(16.);

/// Padding on the row's outer edges instead: the table spans the whole pane,
/// so the first and last columns inset their content by the pane's 3rem, in
/// line with the search field above.
const EDGE_PADDING: Pixels = px(48.);

/// A cell's padding: [`ROW_PADDING`] around its content, [`EDGE_PADDING`] on
/// the table's outer edges, of `columns_count` shown columns.
fn cell_paddings(col_ix: usize, columns_count: usize) -> Edges<Pixels> {
    Edges {
        top: ROW_PADDING,
        bottom: ROW_PADDING,
        left: if col_ix == 0 {
            EDGE_PADDING
        } else {
            ROW_PADDING
        },
        right: if col_ix + 1 == columns_count {
            EDGE_PADDING
        } else {
            ROW_PADDING
        },
    }
}

/// One of the table's columns, which the counter can show or hide.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProductColumn {
    Location,
    ItemNumber,
    Name,
    SystemQuantity,
    CountedQuantity,
    Difference,
    Status,
}

impl ProductColumn {
    /// Every column, in the order the table shows them.
    pub const ALL: [Self; 7] = [
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

    /// The column's header, in the table and in the columns menu.
    pub fn name(self) -> &'static str {
        self.key_and_name().1
    }

    /// Whether the counter can hide the column. The product name stays: it's
    /// what the counter reads off the shelf, and it takes the width the other
    /// columns leave.
    pub fn is_hideable(self) -> bool {
        self != Self::Name
    }

    fn key_and_name(self) -> (&'static str, &'static str) {
        match self {
            Self::Location => ("location", "Lokasjon"),
            Self::ItemNumber => ("item-number", "Varenummer"),
            Self::Name => ("name", "Produkt"),
            Self::SystemQuantity => ("system-quantity", "På lager"),
            Self::CountedQuantity => ("counted-quantity", "Talt"),
            Self::Difference => ("difference", "Differanse"),
            Self::Status => ("status", "Status"),
        }
    }

    /// Fixed widths for everything but the name, padding included. Column
    /// widths are table geometry, which the table API takes in pixels.
    fn fixed_width(self) -> Option<f32> {
        match self {
            Self::Location => Some(160.),
            Self::ItemNumber => Some(156.),
            Self::Name => None,
            Self::SystemQuantity => Some(136.),
            Self::CountedQuantity => Some(136.),
            Self::Difference => Some(146.),
            Self::Status => Some(180.),
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

/// How the stock list renders as rows: which products show, in what order,
/// and how each cell looks. The data itself is read from the [`Session`].
pub struct ProductTable {
    session: Entity<Session>,
    /// The products matching the search, in the storage's walking order.
    matches: Vec<ProductId>,
    /// The columns shown, in the order of [`ProductColumn::ALL`].
    columns: Vec<ProductColumn>,
    /// `matches` in display order: sorted when the counter picked a column.
    rows: Vec<ProductId>,
    /// The column the counter sorted by, if any.
    sort: Option<(ProductColumn, ColumnSort)>,
    /// How wide the table is, so the name column can take what's left.
    width: Pixels,
    last_counted: Option<LastCounted>,
}

impl ProductTable {
    pub fn new(session: Entity<Session>, cx: &App) -> Self {
        let matches = session.read(cx).stocktake().search("");
        Self {
            session,
            rows: matches.clone(),
            matches,
            columns: ProductColumn::ALL.to_vec(),
            sort: None,
            width: px(0.),
            last_counted: None,
        }
    }

    pub fn is_shown(&self, column: ProductColumn) -> bool {
        self.columns.contains(&column)
    }

    /// Shows or hides `column`, unless it's one that always shows. Hiding
    /// the sorted column puts the rows back in walking order, as there'd be
    /// no arrow left to say how they're sorted. Call `TableState::refresh`
    /// afterwards, so the table reads the new columns.
    pub fn set_shown(&mut self, column: ProductColumn, shown: bool, cx: &App) {
        if !column.is_hideable() {
            return;
        }
        self.columns = ProductColumn::ALL
            .into_iter()
            .filter(|&other| {
                if other == column {
                    shown
                } else {
                    self.is_shown(other)
                }
            })
            .collect();
        if !shown && self.sort.is_some_and(|(sorted, _)| sorted == column) {
            self.sort = None;
            self.apply_sort(cx);
        }
    }

    /// Shows these products, sorted by the column the counter picked.
    pub fn set_rows(&mut self, matches: Vec<ProductId>, cx: &App) {
        self.matches = matches;
        self.apply_sort(cx);
    }

    /// Call `TableState::refresh` afterwards, so the table reads the new
    /// column widths.
    pub fn set_width(&mut self, width: Pixels) {
        self.width = width;
    }

    /// Which way `column` is sorted, if it's the sorted one.
    fn sort_of(&self, column: ProductColumn) -> ColumnSort {
        match self.sort {
            Some((sorted, direction)) if sorted == column => direction,
            _ => ColumnSort::Default,
        }
    }

    pub fn product_at(&self, row_ix: usize) -> Option<ProductId> {
        self.rows.get(row_ix).copied()
    }

    pub fn row_of(&self, id: ProductId) -> Option<usize> {
        self.rows.iter().position(|row| *row == id)
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
        let stocktake = self.session.read(cx).stocktake();
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
        match self
            .session
            .read(cx)
            .stocktake()
            .product(id)
            .counted_quantity()
        {
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
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let column = self.columns[col_ix];
        let (key, name) = column.key_and_name();
        let fixed: f32 = self
            .columns
            .iter()
            .filter_map(|column| column.fixed_width())
            .sum();
        // Leave room for the vertical scrollbar.
        let name_width = (f32::from(self.width) - fixed - 16.).max(ProductColumn::MIN_NAME_WIDTH);
        // No sort state for the table: it would draw its own arrow, pinned
        // to the cell's trailing edge. `render_th` draws it beside the label
        // instead and sorts on a click anywhere in the cell.
        Column::new(key, name)
            .width(px(column.fixed_width().unwrap_or(name_width)))
            .paddings(cell_paddings(col_ix, self.columns.len()))
            .min_width(px(64.))
            .movable(false)
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
            direction => Some((self.columns[col_ix], direction)),
        };
        self.apply_sort(cx);
        // A selected row index now points at a different product.
        let table = cx.entity();
        cx.defer(move |cx| table.update(cx, |table, cx| table.clear_selection(cx)));
    }

    /// The label at the leading edge and its sort arrow at the trailing
    /// edge, the same in every column. The whole cell sorts: this spans it
    /// by reaching out over the cell's padding and padding itself by the
    /// same amount.
    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let column = self.columns[col_ix];
        let paddings = cell_paddings(col_ix, self.columns.len());
        let sort = self.sort_of(column);
        let (icon, sorted) = match sort {
            ColumnSort::Ascending => (IconName::SortAscending, true),
            ColumnSort::Descending => (IconName::SortDescending, true),
            ColumnSort::Default => (IconName::ChevronsUpDown, false),
        };
        let arrow = Icon::new(icon)
            .size_3()
            .flex_none()
            .when(!sorted, |icon| icon.opacity(0.5));
        let label = div().min_w_0().truncate().child(column.name());
        let hover_color = cx.theme().foreground;
        h_flex()
            .id(("sort", col_ix))
            .flex_1()
            .min_w_0()
            .h(ROW_HEIGHT)
            .ml(-paddings.left)
            .mr(-paddings.right)
            .pl(paddings.left)
            .pr(paddings.right)
            .justify_between()
            .gap_1()
            .font_medium()
            .cursor_pointer()
            .hover(move |style| style.text_color(hover_color))
            .child(label)
            .child(arrow)
            // Unsorted, then descending, then ascending, as the table's own
            // arrow cycles.
            .on_click(cx.listener(move |table, _, window, cx| {
                let next = match table.delegate().sort_of(column) {
                    ColumnSort::Default => ColumnSort::Descending,
                    ColumnSort::Descending => ColumnSort::Ascending,
                    ColumnSort::Ascending => ColumnSort::Default,
                };
                table.delegate_mut().perform_sort(col_ix, next, window, cx);
                cx.notify();
            }))
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
        let column = self.columns[col_ix];
        let product = self.session.read(cx).stocktake().product(id);
        let content = match column {
            // The pick location, and how many overflow locations units were
            // also counted at, named in a tooltip.
            ProductColumn::Location => {
                let location = h_flex().gap_2().child(product.location().to_string());
                match product.overflow_len() {
                    0 => location.into_any_element(),
                    overflow => {
                        let buffers: Vec<SharedString> = product
                            .overflow_counts()
                            .map(|(location, _)| location.to_string().into())
                            .collect();
                        location
                            .id(("location", id.line()))
                            .child(
                                // gpui has no letter spacing, so the sign
                                // is set apart from the number by hand.
                                Tag::secondary().small().rounded(px(6.)).child(
                                    h_flex().gap_px().child("+").child(overflow.to_string()),
                                ),
                            )
                            .tooltip(move |window, cx| {
                                let buffers = buffers.clone();
                                Tooltip::element(move |_, cx| {
                                    v_flex()
                                        .child(
                                            div()
                                                .font_medium()
                                                .child("Bufferlokasjoner")
                                                .text_color(cx.theme().muted_foreground)
                                                .text_sm()
                                                .mb_1(),
                                        )
                                        .children(buffers.clone())
                                        .p_2()
                                })
                                .build(window, cx)
                            })
                            .into_any_element()
                    }
                }
            }
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
        let product = self.session.read(cx).stocktake().product(id);
        match self.columns[col_ix] {
            ProductColumn::Location => match product.overflow_len() {
                0 => product.location().to_string(),
                overflow => format!("{} +{overflow}", product.location())
                    .trim_start()
                    .to_string(),
            },
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
            ProductColumn::Status => CountStatus::label(product.is_counted()).to_string(),
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
