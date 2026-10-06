//! The stock list as a table.

use std::{cmp::Ordering, time::Instant};

use gpui_kit::component::{
    table::{Column, ColumnSort, TableDelegate, TableState},
    tooltip::Tooltip,
};
use gpui_kit::{Div, Edges, Pixels, Stateful, px};
use stocktake::{Product, ProductId, compare_locations, natural_cmp};
use ui::{Badge, Delta, Heading, SectionHeading, Text, flash, prelude::*};

use crate::{count_dialog::SYSTEM_QUANTITY, count_status::CountStatus, session::Session};

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
    /// The overflow locations and what was counted at each.
    OverflowLocations,
    ItemNumber,
    Name,
    /// Colour and style, which tell apart products with the same name.
    Description,
    SystemQuantity,
    CountedQuantity,
    /// What was counted at the overflow locations together.
    OverflowQuantity,
    Difference,
    Status,
}

impl ProductColumn {
    /// Every column, in the order the table shows them.
    pub const ALL: [Self; 10] = [
        Self::Location,
        Self::OverflowLocations,
        Self::ItemNumber,
        Self::Name,
        Self::Description,
        Self::SystemQuantity,
        Self::CountedQuantity,
        Self::OverflowQuantity,
        Self::Difference,
        Self::Status,
    ];

    /// The narrowest the product name gets; it takes whatever the window
    /// has beyond the other columns.
    const MIN_NAME_WIDTH: f32 = 320.;

    fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::SystemQuantity
                | Self::CountedQuantity
                | Self::OverflowQuantity
                | Self::Difference
        )
    }

    /// Whether the counter can hide the column. The product name stays: it's
    /// what the counter reads off the shelf, and it takes the width the other
    /// columns leave.
    pub fn is_hideable(self) -> bool {
        self != Self::Name
    }

    fn key(self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::OverflowLocations => "overflow-locations",
            Self::ItemNumber => "item-number",
            Self::Name => "name",
            Self::Description => "description",
            Self::SystemQuantity => "system-quantity",
            Self::CountedQuantity => "counted-quantity",
            Self::OverflowQuantity => "overflow-quantity",
            Self::Difference => "difference",
            Self::Status => "status",
        }
    }

    /// The column's header. The overflow locations' is plural once a
    /// product has more than one.
    fn name(self, several_overflow_locations: bool) -> &'static str {
        match self {
            Self::Location => "Lokasjon",
            Self::OverflowLocations if several_overflow_locations => "Bufferlokasjoner",
            Self::OverflowLocations => "Bufferlokasjon",
            Self::ItemNumber => "Varenummer",
            Self::Name => "Produkt",
            Self::Description => "Beskrivelse",
            Self::SystemQuantity => SYSTEM_QUANTITY,
            Self::CountedQuantity => "Talt",
            Self::OverflowQuantity => "Buffer",
            Self::Difference => "Differanse",
            Self::Status => "Status",
        }
    }

    /// Fixed widths for everything but the name, padding included. Column
    /// widths are table geometry, which the table API takes in pixels.
    fn fixed_width(self) -> Option<f32> {
        match self {
            Self::Location => Some(160.),
            Self::OverflowLocations => Some(220.),
            Self::ItemNumber => Some(156.),
            Self::Name => None,
            Self::Description => Some(240.),
            Self::SystemQuantity => Some(168.),
            Self::CountedQuantity => Some(136.),
            Self::OverflowQuantity => Some(136.),
            Self::Difference => Some(146.),
            Self::Status => Some(180.),
        }
    }
}

/// Which of the stocktake's products a table lists.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The whole stock list, narrowed by the search and the sidebar's
    /// filters, for counting.
    StockList,
    /// The counted products whose counted quantity differs from the system
    /// quantity.
    Differences,
}

impl Scope {
    /// The columns shown until the counter changes them. The overflow
    /// columns are a closer look most counts don't need; the location column
    /// already says how many overflow locations there are. The differences
    /// are all counted, so they leave out the status.
    fn default_columns(self) -> Vec<ProductColumn> {
        let shown: &[ProductColumn] = match self {
            Self::StockList => &[
                ProductColumn::Location,
                ProductColumn::ItemNumber,
                ProductColumn::Name,
                ProductColumn::Description,
                ProductColumn::SystemQuantity,
                ProductColumn::CountedQuantity,
                ProductColumn::Difference,
                ProductColumn::Status,
            ],
            Self::Differences => &[
                ProductColumn::Location,
                ProductColumn::ItemNumber,
                ProductColumn::Name,
                ProductColumn::Description,
                ProductColumn::SystemQuantity,
                ProductColumn::CountedQuantity,
                ProductColumn::Difference,
            ],
        };
        shown.to_vec()
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
    scope: Scope,
    /// The products in [`Self::scope`] matching the search, in the
    /// storage's walking order.
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
    pub fn new(session: Entity<Session>, scope: Scope, cx: &App) -> Self {
        let stocktake = session.read(cx).stocktake();
        let matches = match scope {
            Scope::StockList => stocktake.search(""),
            Scope::Differences => stocktake.differences(),
        };
        Self {
            session,
            scope,
            rows: matches.clone(),
            matches,
            columns: scope.default_columns(),
            sort: None,
            width: px(0.),
            last_counted: None,
        }
    }

    /// The column's header, in the table and in the columns menu.
    pub fn column_name(&self, column: ProductColumn, cx: &App) -> &'static str {
        let several_overflow_locations = self
            .session
            .read(cx)
            .stocktake()
            .products()
            .any(|(_, product)| product.overflow_len() > 1);
        column.name(several_overflow_locations)
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
                // By the first overflow location, those without one last.
                ProductColumn::OverflowLocations => {
                    return missing_last(
                        a.overflow_counts().next(),
                        b.overflow_counts().next(),
                        direction,
                        |(a, _), (b, _)| compare_locations(a, b),
                    );
                }
                ProductColumn::ItemNumber => natural_cmp(a.item_number(), b.item_number()),
                ProductColumn::Name => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
                ProductColumn::Description => a
                    .description()
                    .to_lowercase()
                    .cmp(&b.description().to_lowercase()),
                ProductColumn::SystemQuantity => a.system_quantity().cmp(&b.system_quantity()),
                ProductColumn::CountedQuantity => {
                    return missing_last(
                        a.counted_quantity(),
                        b.counted_quantity(),
                        direction,
                        i64::cmp,
                    );
                }
                ProductColumn::OverflowQuantity => {
                    return missing_last(
                        a.overflow_quantity(),
                        b.overflow_quantity(),
                        direction,
                        i64::cmp,
                    );
                }
                ProductColumn::Difference => {
                    return missing_last(a.difference(), b.difference(), direction, i64::cmp);
                }
                // Uncounted first when ascending: that's the work left.
                ProductColumn::Status => a.count_state().cmp(&b.count_state()),
            };
            directed(ordering, direction)
        });
    }
}

/// The counted quantity, or a muted dash while the product is uncounted.
fn render_counted_quantity(product: &Product, cx: &App) -> AnyElement {
    match product.counted_quantity() {
        Some(quantity) => quantity.to_string().into_any_element(),
        None => div()
            .text_color(cx.theme().muted_foreground)
            .child("–")
            .into_any_element(),
    }
}

/// The product's overflow locations, in location order.
fn overflow_locations(product: &Product) -> String {
    product
        .overflow_counts()
        .map(|(location, _)| location)
        .collect::<Vec<_>>()
        .join(", ")
}

fn directed(ordering: Ordering, direction: ColumnSort) -> Ordering {
    match direction {
        ColumnSort::Descending => ordering.reverse(),
        ColumnSort::Ascending | ColumnSort::Default => ordering,
    }
}

/// Products without a value to compare, such as uncounted ones in the
/// counted-quantity column, stay at the bottom whichever way the column is
/// sorted.
fn missing_last<T>(
    a: Option<T>,
    b: Option<T>,
    direction: ColumnSort,
    compare: impl FnOnce(&T, &T) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => directed(compare(&a, &b), direction),
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

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        let column = self.columns[col_ix];
        let (key, name) = (column.key(), self.column_name(column, cx));
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
        let label = div()
            .min_w_0()
            .truncate()
            .child(self.column_name(column, cx));
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
                                Badge::new().child(
                                    h_flex().gap_px().child("+").child(overflow.to_string()),
                                ),
                            )
                            .tooltip(move |window, cx| {
                                let buffers = buffers.clone();
                                Tooltip::element(move |_, _| {
                                    v_flex()
                                        .p_2()
                                        .child(SectionHeading::new("Bufferlokasjoner").mb_1())
                                        .children(buffers.clone())
                                })
                                .build(window, cx)
                            })
                            .into_any_element()
                    }
                }
            }
            ProductColumn::OverflowLocations => div()
                .min_w_0()
                .truncate()
                .child(overflow_locations(product))
                .into_any_element(),
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
            ProductColumn::Description => div()
                .min_w_0()
                .truncate()
                .text_color(cx.theme().muted_foreground)
                .child(product.description().to_string())
                .into_any_element(),
            ProductColumn::SystemQuantity => {
                product.system_quantity().to_string().into_any_element()
            }
            ProductColumn::CountedQuantity => render_counted_quantity(product, cx),
            ProductColumn::OverflowQuantity => product
                .overflow_quantity()
                .map(|quantity| quantity.to_string())
                .unwrap_or_default()
                .into_any_element(),
            ProductColumn::Difference => match product.difference() {
                Some(difference) => Delta::new(difference).into_any_element(),
                None => div().into_any_element(),
            },
            ProductColumn::Status => CountStatus::new(product.count_state()).into_any_element(),
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
        let empty = v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground);
        match self.scope {
            Scope::StockList => empty
                .child(Icon::new(IconName::SearchX).large())
                .child(Text::new("Ingen varer passer søket")),
            // Says what's left to count, as that's what could still turn up
            // a difference.
            Scope::Differences => {
                let uncounted = self.session.read(cx).stocktake().uncounted_len();
                let detail = match uncounted {
                    0 => "Alle varene stemmer med lagersystemet.".to_string(),
                    1 => "Alle talte varer stemmer med lagersystemet. 1 vare er ikke talt ennå."
                        .to_string(),
                    _ => format!(
                        "Alle talte varer stemmer med lagersystemet. \
                         {uncounted} varer er ikke talt ennå."
                    ),
                };
                empty
                    .child(Icon::new(IconName::CircleCheck).large())
                    .child(Heading::new("Ingen differanse"))
                    .child(Text::new(detail))
            }
        }
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
            ProductColumn::OverflowLocations => overflow_locations(product),
            ProductColumn::ItemNumber => product.item_number().to_string(),
            ProductColumn::Name => product.name().to_string(),
            ProductColumn::Description => product.description().to_string(),
            ProductColumn::SystemQuantity => product.system_quantity().to_string(),
            ProductColumn::CountedQuantity => product
                .counted_quantity()
                .map(|quantity| quantity.to_string())
                .unwrap_or_default(),
            ProductColumn::OverflowQuantity => product
                .overflow_quantity()
                .map(|quantity| quantity.to_string())
                .unwrap_or_default(),
            ProductColumn::Difference => product
                .difference()
                .map(|difference| difference.to_string())
                .unwrap_or_default(),
            ProductColumn::Status => CountStatus::label(product.count_state()).to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_values_sort_last_both_ways() {
        let mut values = vec![None, Some(3), Some(-1), None, Some(0)];
        values.sort_by(|a, b| missing_last(*a, *b, ColumnSort::Ascending, i64::cmp));
        assert_eq!(values, [Some(-1), Some(0), Some(3), None, None]);
        values.sort_by(|a, b| missing_last(*a, *b, ColumnSort::Descending, i64::cmp));
        assert_eq!(values, [Some(3), Some(0), Some(-1), None, None]);
    }
}
