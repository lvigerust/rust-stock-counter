//! The stock list as a table, with the counted-quantity cell of the product
//! being counted turned into an input.

use gpui_kit::component::{
    input::{Input, InputState},
    table::{Column, TableDelegate, TableState},
};
use gpui_kit::{Div, Stateful, px};
use stocktake::{ProductId, Stocktake};
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

    fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::SystemQuantity | Self::CountedQuantity | Self::Difference
        )
    }

    fn column(self) -> Column {
        // Column widths are table geometry, which the table API takes in pixels.
        let (key, name, width) = match self {
            Self::Location => ("location", "Lokasjon", 96.),
            Self::ItemNumber => ("item-number", "Varenummer", 112.),
            Self::Name => ("name", "Produkt", 300.),
            Self::SystemQuantity => ("system-quantity", "På lager", 96.),
            Self::CountedQuantity => ("counted-quantity", "Telt", 96.),
            Self::Difference => ("difference", "Differanse", 104.),
            Self::Status => ("status", "Status", 120.),
        };
        Column::new(key, name)
            .width(px(width))
            .min_width(px(64.))
            .movable(false)
            .when(self.is_numeric(), |column| column.text_right())
    }
}

/// The most recently counted product, and how many counts came before it, so
/// counting the same product twice in a row flashes it twice.
#[derive(Clone, Copy)]
pub struct LastCounted {
    pub id: ProductId,
    pub generation: usize,
}

pub struct ProductTable {
    stocktake: Entity<Stocktake>,
    /// The products shown, in display order.
    rows: Vec<ProductId>,
    count_input: Entity<InputState>,
    /// The product whose counted quantity is being entered.
    counting: Option<ProductId>,
    last_counted: Option<LastCounted>,
}

impl ProductTable {
    pub fn new(stocktake: Entity<Stocktake>, count_input: Entity<InputState>, cx: &App) -> Self {
        let rows = stocktake.read(cx).search("");
        Self {
            stocktake,
            rows,
            count_input,
            counting: None,
            last_counted: None,
        }
    }

    pub fn set_rows(&mut self, rows: Vec<ProductId>) {
        self.rows = rows;
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

impl TableDelegate for ProductTable {
    fn columns_count(&self, _: &App) -> usize {
        ProductColumn::ALL.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        ProductColumn::ALL[col_ix].column()
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
            .child(column.column().name)
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
                let strength = flash(("counted-flash", last.generation), window, cx);
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
            ProductColumn::Name => div()
                .truncate()
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
