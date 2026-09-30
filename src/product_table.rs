//! The stock list as a table, with the counted-quantity cell of the product
//! being counted turned into an input.

use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, h_flex,
    input::{Input, InputState},
    table::{Column, TableDelegate, TableState},
};
use gpui_kit::{
    App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::stocktake::{ProductId, Stocktake};

/// Key context around the counted-quantity input, so Escape can cancel the
/// count after the input itself has ignored it.
pub const COUNT_CELL_CONTEXT: &str = "CountCell";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProductColumn {
    Location,
    ItemNumber,
    Barcode,
    Name,
    SystemQuantity,
    CountedQuantity,
    Status,
}

impl ProductColumn {
    const ALL: [Self; 7] = [
        Self::Location,
        Self::ItemNumber,
        Self::Barcode,
        Self::Name,
        Self::SystemQuantity,
        Self::CountedQuantity,
        Self::Status,
    ];

    fn is_numeric(self) -> bool {
        matches!(self, Self::SystemQuantity | Self::CountedQuantity)
    }

    fn column(self) -> Column {
        // Column widths are table geometry, which the table API takes in pixels.
        let (key, name, width) = match self {
            Self::Location => ("location", "Lokasjon", 96.),
            Self::ItemNumber => ("item-number", "Varenummer", 112.),
            Self::Barcode => ("barcode", "Strekkode", 136.),
            Self::Name => ("name", "Navn", 280.),
            Self::SystemQuantity => ("system-quantity", "På lager", 104.),
            Self::CountedQuantity => ("counted-quantity", "Telt", 104.),
            Self::Status => ("status", "Status", 112.),
        };
        Column::new(key, name)
            .width(px(width))
            .min_width(px(64.))
            .movable(false)
            .when(self.is_numeric(), |column| column.text_right())
    }
}

pub struct ProductTable {
    stocktake: Entity<Stocktake>,
    /// The products shown, in display order.
    rows: Vec<ProductId>,
    count_input: Entity<InputState>,
    /// The product whose counted quantity is being entered.
    counting: Option<ProductId>,
}

impl ProductTable {
    pub fn new(stocktake: Entity<Stocktake>, count_input: Entity<InputState>, cx: &App) -> Self {
        let rows = stocktake.read(cx).search("");
        Self {
            stocktake,
            rows,
            count_input,
            counting: None,
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

    fn render_counted_quantity(&self, id: ProductId, cx: &App) -> gpui_kit::AnyElement {
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

    fn render_status(&self, id: ProductId, cx: &App) -> impl IntoElement {
        let counted = self.stocktake.read(cx).product(id).is_counted();
        h_flex()
            .gap_1()
            .when(counted, |this| {
                this.child(
                    Icon::new(IconName::Check)
                        .small()
                        .text_color(cx.theme().success),
                )
                .child("Telt")
            })
            .when(!counted, |this| {
                this.text_color(cx.theme().muted_foreground)
                    .child("Ikke telt")
            })
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
            ProductColumn::Location => {
                SharedString::from(product.location().to_string()).into_any_element()
            }
            ProductColumn::ItemNumber => {
                SharedString::from(product.item_number().to_string()).into_any_element()
            }
            ProductColumn::Barcode => {
                SharedString::from(product.barcode().to_string()).into_any_element()
            }
            ProductColumn::Name => div()
                .truncate()
                .child(product.name().to_string())
                .into_any_element(),
            ProductColumn::SystemQuantity => {
                product.system_quantity().to_string().into_any_element()
            }
            ProductColumn::CountedQuantity => self.render_counted_quantity(id, cx),
            ProductColumn::Status => self.render_status(id, cx).into_any_element(),
        };
        h_flex()
            .size_full()
            .when(column.is_numeric(), |this| this.justify_end())
            .child(content)
            .into_any_element()
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, cx: &App) -> String {
        let Some(id) = self.product_at(row_ix) else {
            return String::new();
        };
        let product = self.stocktake.read(cx).product(id);
        match ProductColumn::ALL[col_ix] {
            ProductColumn::Location => product.location().to_string(),
            ProductColumn::ItemNumber => product.item_number().to_string(),
            ProductColumn::Barcode => product.barcode().to_string(),
            ProductColumn::Name => product.name().to_string(),
            ProductColumn::SystemQuantity => product.system_quantity().to_string(),
            ProductColumn::CountedQuantity => product
                .counted_quantity()
                .map(|quantity| quantity.to_string())
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
