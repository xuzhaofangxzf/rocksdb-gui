use super::key_table::KeyTableDelegate;
use crate::{
    state::{BrowseMode, DbEvent, DbState, LoadState},
    storage::SearchMode,
    ui::{data_panel_styled, mono_text, muted_label},
    util::format_bytes,
};
use gpui_kit::{
    App, Context, Entity, Focusable, Render, SharedString, Subscription, Window, div, prelude::*,
    px, rems,
};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    resizable::{resizable_panel, v_resizable},
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    table::{DataTable, TableEvent, TableState},
    text::{TextView, TextViewStyle},
    v_flex,
};

#[derive(Clone, Debug)]
struct SearchOption {
    label: SharedString,
    mode: SearchMode,
}

impl SelectItem for SearchOption {
    type Value = SearchMode;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.mode
    }
}

fn search_options() -> Vec<SearchOption> {
    vec![
        SearchOption {
            label: "Prefix".into(),
            mode: SearchMode::Prefix,
        },
        SearchOption {
            label: "Key contains".into(),
            mode: SearchMode::KeyContains,
        },
        SearchOption {
            label: "Value contains".into(),
            mode: SearchMode::ValueContains,
        },
    ]
}

fn table_layout_signature(columns: &[String], include_value: bool) -> String {
    format!("{}:{}", columns.join("\x1f"), include_value)
}

fn wrap_json_code_block(json: &str) -> String {
    let fence = if json.contains("```") { "````" } else { "```" };
    format!("{fence}json\n{json}\n{fence}")
}

#[derive(Clone)]
struct KeyListDelegate {
    items: Vec<crate::storage::KeyEntry>,
    selected_key: Option<Vec<u8>>,
}

impl ListDelegate for KeyListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let entry = self.items.get(ix.row)?;
        let selected = self.selected_key.as_deref() == Some(entry.key.as_slice());
        Some(
            ListItem::new(ix)
                .w_full()
                .px_2()
                .py_2()
                .rounded_sm()
                .when(selected, |this| this.bg(cx.theme().accent))
                .child(mono_text(div(), cx).child(format_bytes(&entry.key))),
        )
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }
}

pub struct KeyBrowser {
    db_state: Entity<DbState>,
    search_state: Entity<InputState>,
    search_mode_state: Entity<SelectState<Vec<SearchOption>>>,
    list_state: Entity<ListState<KeyListDelegate>>,
    table_state: Entity<TableState<KeyTableDelegate>>,
    synced_data_revision: u64,
    table_layout_signature: Option<String>,
    detail_pretty_json: bool,
    _subscriptions: Vec<Subscription>,
}

impl KeyBrowser {
    pub fn new(db_state: Entity<DbState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search keys and values…")
        });
        let search_mode_state =
            cx.new(|cx| SelectState::new(search_options(), Some(IndexPath::new(0)), window, cx));
        let list_state = cx.new(|cx| {
            ListState::new(
                KeyListDelegate {
                    items: Vec::new(),
                    selected_key: None,
                },
                window,
                cx,
            )
        });
        let table_db_state = db_state.clone();
        let table_state = cx.new(|cx| {
            TableState::new(
                KeyTableDelegate {
                    db_state: Some(table_db_state),
                    ..Default::default()
                },
                window,
                cx,
            )
            .cell_selectable(true)
            .row_header(false)
            .row_selectable(false)
        });
        let mut subscriptions = Vec::new();
        let state = db_state.clone();
        subscriptions.push(cx.subscribe_in(
            &search_state,
            window,
            move |_this, input, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let prefix = input.read(cx).value().to_string();
                    state.update(cx, |state, cx| state.set_search_term(prefix, cx));
                }
            },
        ));
        let state = db_state.clone();
        subscriptions.push(cx.subscribe_in(
            &search_mode_state,
            window,
            move |_this, _select, event: &SelectEvent<Vec<SearchOption>>, _window, cx| {
                if let SelectEvent::Confirm(Some(mode)) = event {
                    state.update(cx, |state, cx| state.set_search_mode(*mode, cx));
                }
            },
        ));
        search_state.focus_handle(cx).focus(window, cx);
        subscriptions.push(cx.subscribe(&db_state, move |this, state, event, cx| {
            if !matches!(event, DbEvent::Updated) {
                return;
            }
            let revision = state.read(cx).data_revision();
            if revision == this.synced_data_revision {
                cx.notify();
                return;
            }
            cx.spawn(async move |handle, cx| {
                let _ = handle.update(cx, |this, cx| {
                    this.sync_delegates(cx);
                    cx.notify();
                });
            })
            .detach();
        }));
        subscriptions.push(cx.subscribe(&list_state, |this, list, event, cx| {
            if this.db_state.read(cx).browse_mode() != BrowseMode::List {
                return;
            }
            let index = match event {
                ListEvent::Select(ix) | ListEvent::Confirm(ix) => Some(ix.row),
                _ => None,
            };
            if let Some(index) = index {
                let key = list
                    .read(cx)
                    .delegate()
                    .items
                    .get(index)
                    .map(|entry| entry.key.clone());
                if let Some(key) = key {
                    this.db_state
                        .update(cx, |state, cx| state.select_key(key, cx));
                }
            }
        }));
        subscriptions.push(cx.subscribe(&table_state, |this, _table, event, cx| {
            if let TableEvent::SelectCell(row_ix, col_ix) = event {
                this.db_state.update(cx, |state, cx| {
                    state.select_table_cell(*row_ix, *col_ix, cx);
                });
            }
        }));
        Self {
            db_state,
            search_state,
            search_mode_state,
            list_state,
            table_state,
            synced_data_revision: 0,
            table_layout_signature: None,
            detail_pretty_json: false,
            _subscriptions: subscriptions,
        }
    }

    fn sync_delegates(&mut self, cx: &mut Context<Self>) {
        let revision = self.db_state.read(cx).data_revision();
        if revision == self.synced_data_revision {
            return;
        }
        self.synced_data_revision = revision;

        let (
            browse_mode,
            loading,
            selected_key,
            selected_cell,
            items,
            table_rows,
            table_columns,
            include_value,
        ) = {
            let db = self.db_state.read(cx);
            (
                db.browse_mode(),
                matches!(db.load_state(), LoadState::Loading),
                db.selected_key().map(Vec::from),
                db.selected_table_cell(),
                db.keys().to_vec(),
                db.table_rows().to_vec(),
                db.table_columns().to_vec(),
                db.table_include_value_column(),
            )
        };

        match browse_mode {
            BrowseMode::List => {
                self.list_state.update(cx, |list, cx| {
                    list.delegate_mut().items = items;
                    list.delegate_mut().selected_key = selected_key;
                    cx.notify();
                });
            }
            BrowseMode::Table => {
                let layout = table_layout_signature(&table_columns, include_value);
                let columns_changed = self.table_layout_signature.as_deref() != Some(layout.as_str());
                self.table_layout_signature = Some(layout);
                self.table_state.update(cx, |table, _cx| {
                    let delegate = table.delegate_mut();
                    delegate.rows = table_rows;
                    delegate.columns = table_columns;
                    delegate.include_value_column = include_value;
                    delegate.selected_key = selected_key;
                    delegate.selected_cell = selected_cell;
                    delegate.loading = loading;
                });
                if columns_changed {
                    self.table_state
                        .update(cx, |table, cx| table.refresh(cx));
                } else {
                    self.table_state.update(cx, |_, cx| cx.notify());
                }
            }
        }
    }
}

impl Render for KeyBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.db_state.read(cx);
        let has_more = state.next_key().is_some();
        let loading = matches!(state.load_state(), LoadState::Loading);
        let key_count = state.keys().len();
        let browse_mode = state.browse_mode();
        let table_mode = browse_mode == BrowseMode::Table;
        let details_visible = table_mode && state.table_details_visible();
        let detail_text = state.table_detail_text().unwrap_or_default();
        let pretty_detail = serde_json::from_str::<serde_json::Value>(&detail_text)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok());
        let detail_is_json = pretty_detail.is_some();
        let detail_display = if self.detail_pretty_json {
            pretty_detail.unwrap_or_else(|| detail_text.clone())
        } else {
            detail_text
        };
        let detail_title = state
            .selected_table_cell()
            .map(|(_, col_ix)| {
                if col_ix == 0 {
                    "Key".to_string()
                } else {
                    state
                        .table_columns()
                        .get(col_ix - 1)
                        .cloned()
                        .unwrap_or_else(|| "value".to_string())
                }
            })
            .unwrap_or_else(|| "Cell details".to_string());
        let data_panel = if table_mode {
            if key_count == 0 {
                data_panel_styled("key-table-empty", cx)
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(muted_label(
                                if loading {
                                    "Loading records…"
                                } else {
                                    "No matching keys"
                                },
                                cx,
                            )),
                    )
                    .into_any_element()
            } else {
                data_panel_styled("key-table", cx)
                    .child(div().size_full().child(DataTable::new(&self.table_state)))
                    .into_any_element()
            }
        } else {
            data_panel_styled("key-list", cx)
                .child(List::new(&self.list_state).size_full())
                .into_any_element()
        };

        let data_body = if details_visible {
            let db_state = self.db_state.clone();
            let detail_content = if self.detail_pretty_json && detail_is_json {
                TextView::markdown(
                    "table-cell-detail-json",
                    wrap_json_code_block(&detail_display),
                )
                .selectable(true)
                .style(TextViewStyle::default().paragraph_gap(rems(0.)))
                .into_any_element()
            } else {
                mono_text(div(), cx)
                    .text_sm()
                    .child(detail_display)
                    .into_any_element()
            };
            let details_panel = v_flex()
                .size_full()
                .gap_2()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_2()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            Label::new(detail_title)
                                .text_sm()
                                .text_color(cx.theme().foreground),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .when(detail_is_json, |this| {
                                    this.child(
                                        Button::new("pretty-cell-json")
                                            .label(if self.detail_pretty_json {
                                                "Raw"
                                            } else {
                                                "Pretty JSON"
                                            })
                                            .compact()
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(
                                                |this, _event, _window, cx| {
                                                    this.detail_pretty_json =
                                                        !this.detail_pretty_json;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                })
                                .child(
                                    Button::new("close-cell-details")
                                        .label("Close")
                                        .compact()
                                        .small()
                                        .ghost()
                                        .on_click(cx.listener(
                                            move |this, _event, _window, cx| {
                                                this.detail_pretty_json = false;
                                                db_state.update(cx, |state, cx| {
                                                    state.hide_table_cell_details(cx);
                                                });
                                            },
                                        )),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        // Match the proven scrolling structure used by ValueViewer.
                        // Combining both axes here prevents TextView's multi-line
                        // content from reporting its full vertical extent.
                        .overflow_y_scrollbar()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .p_2()
                        .child(detail_content),
                );

            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    v_resizable("table-cell-details-split")
                        .child(resizable_panel().child(data_panel))
                        .child(
                            resizable_panel()
                                .size(px(220.))
                                .size_range(px(120.)..px(600.))
                                .child(details_panel),
                        ),
                )
                .into_any_element()
        } else {
            data_panel
        };

        v_flex()
            .size_full()
            .gap_3()
            .p_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div().flex_1().min_w(px(200.)).child(
                            Input::new(&self.search_state)
                                .w_full()
                                .cleanable(true)
                                .prefix(Icon::new(IconName::Search)),
                        ),
                    )
                    .child(
                        Select::new(&self.search_mode_state)
                            .w(px(36.))
                            .h(px(32.))
                            .menu_width(px(168.))
                            .small()
                            .appearance(false)
                            .icon(Icon::new(IconName::Settings2)),
                    )
                    .child({
                        let db_state = self.db_state.clone();
                        Button::new("browse-mode-table")
                            .label("Table")
                            .compact()
                            .small()
                            .tooltip("Table view")
                            .when(table_mode, |this| this.primary())
                            .when(!table_mode, |this| this.ghost())
                            .on_click(cx.listener(move |_this, _event, _window, cx| {
                                db_state.update(cx, |state, cx| {
                                    state.set_browse_mode(BrowseMode::Table, cx);
                                });
                            }))
                    })
                    .child({
                        let db_state = self.db_state.clone();
                        Button::new("browse-mode-list")
                            .label("List")
                            .compact()
                            .small()
                            .tooltip("List + value panel")
                            .when(!table_mode, |this| this.primary())
                            .when(table_mode, |this| this.ghost())
                            .on_click(cx.listener(move |_this, _event, _window, cx| {
                                db_state.update(cx, |state, cx| {
                                    state.set_browse_mode(BrowseMode::List, cx);
                                });
                            }))
                    })
                    .child({
                        let state = self.db_state.clone();
                        Button::new("refresh-keys")
                            .icon(IconName::Redo2)
                            .ghost()
                            .small()
                            .tooltip("Reload keys")
                            .on_click(cx.listener(move |_this, _event, _window, cx| {
                                state.update(cx, |state, cx| state.refresh(cx));
                            }))
                    })
                    .child({
                        let state = self.db_state.clone();
                        Button::new("first-page")
                            .icon(IconName::ArrowUp)
                            .ghost()
                            .small()
                            .tooltip("Jump to first page")
                            .on_click(cx.listener(move |_this, _event, _window, cx| {
                                state.update(cx, |state, cx| state.jump_to_first(cx));
                            }))
                    }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new(format!("{key_count} keys"))
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .when(loading, |this| {
                        this.child(muted_label("Loading…", cx))
                    }),
            )
            .child(data_body)
            .when(has_more, |this| {
                let state = self.db_state.clone();
                this.child(
                    Button::new("load-more")
                        .label("Load more keys")
                        .icon(IconName::ArrowDown)
                        .ghost()
                        .small()
                        .on_click(cx.listener(move |_this, _event, _window, cx| {
                            state.update(cx, |state, cx| state.load_more(cx));
                        })),
                )
            })
    }
}