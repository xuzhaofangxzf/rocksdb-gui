use crate::{
    record_format::table_cell_preview,
    state::{DbState, TableRow},
};
use gpui_kit::{
    App, Entity, SharedString, Window, div, prelude::*, px,
};
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    table::{Column, ColumnFixed, TableDelegate, TableState},
};

const MAX_FIT_WIDTH: f32 = 480.;
const MIN_FIT_WIDTH: f32 = 72.;
const APPROX_CHAR_WIDTH: f32 = 7.2;
const CELL_HORIZONTAL_PADDING: f32 = 24.;

#[derive(Clone, Default)]
pub struct KeyTableDelegate {
    pub db_state: Option<Entity<DbState>>,
    pub rows: Vec<TableRow>,
    pub columns: Vec<String>,
    pub include_value_column: bool,
    pub selected_key: Option<Vec<u8>>,
    pub selected_cell: Option<(usize, usize)>,
    #[allow(dead_code)]
    pub loading: bool,
}

impl KeyTableDelegate {
    fn value_column_index(&self) -> Option<usize> {
        if !self.include_value_column {
            return None;
        }
        Some(1 + self.columns.len())
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize) -> Option<&str> {
        let row = self.rows.get(row_ix)?;
        if col_ix == 0 {
            return Some(row.key_label.as_str());
        }
        let field_ix = col_ix - 1;
        if field_ix < self.columns.len() {
            let column = self.columns.get(field_ix)?;
            return row.json_fields.get(column).map(|value| value.as_ref());
        }
        if self.include_value_column && col_ix == self.value_column_index()? {
            return row.plain_value.as_deref();
        }
        None
    }

    fn fitted_width(&self, col_ix: usize) -> f32 {
        let header = if col_ix == 0 {
            "Key"
        } else if let Some(column) = self.columns.get(col_ix - 1) {
            column
        } else {
            "value"
        };
        let max_chars = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(row_ix, _)| self.cell_text(row_ix, col_ix))
            .map(|text| text.chars().count())
            .chain(std::iter::once(header.chars().count()))
            .max()
            .unwrap_or(0);
        (max_chars as f32 * APPROX_CHAR_WIDTH + CELL_HORIZONTAL_PADDING)
            .clamp(MIN_FIT_WIDTH, MAX_FIT_WIDTH)
    }
}

impl TableDelegate for KeyTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        1 + self.columns.len() + usize::from(self.include_value_column)
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        if col_ix == 0 {
            return Column::new("key", "Key")
                .width(px(self.fitted_width(col_ix)))
                .max_width(px(MAX_FIT_WIDTH))
                .fixed(ColumnFixed::Left);
        }
        let field_ix = col_ix - 1;
        if field_ix < self.columns.len() {
            let name = self.columns[field_ix].clone();
            return Column::new(SharedString::from(name.clone()), SharedString::from(name))
                .width(px(self.fitted_width(col_ix)))
                .max_width(px(MAX_FIT_WIDTH));
        }
        Column::new("value", "value")
            .width(px(self.fitted_width(col_ix)))
            .max_width(px(MAX_FIT_WIDTH))
    }

    fn fit_column(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        _: &mut gpui_kit::Context<TableState<Self>>,
    ) -> Option<gpui_kit::Pixels> {
        Some(px(self.fitted_width(col_ix)))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _cx: &mut gpui_kit::Context<TableState<Self>>,
    ) -> impl gpui_kit::IntoElement {
        let text = self
            .cell_text(row_ix, col_ix)
            .map(table_cell_preview)
            .unwrap_or_default();
        let selected = self.selected_cell == Some((row_ix, col_ix));
        let db_state = self.db_state.clone();

        h_flex()
            .size_full()
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .overflow_x_hidden()
                    .child(text),
            )
            .when(selected, |this| {
                this.child(
                    Button::new(SharedString::from(format!(
                        "show-cell-details-{row_ix}-{col_ix}"
                    )))
                        .label("…")
                        .compact()
                        .small()
                        .ghost()
                        .tooltip("Show full cell value")
                        .on_click(move |_event, _window, cx| {
                            if let Some(db_state) = db_state.as_ref() {
                                db_state.update(cx, |state, cx| {
                                    state.show_table_cell_details(row_ix, col_ix, cx);
                                });
                            }
                        }),
                )
            })
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        self.cell_text(row_ix, col_ix)
            .map(|value| SharedString::from(table_cell_preview(value)))
    }
}