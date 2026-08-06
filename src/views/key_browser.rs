use crate::{
    state::{DbEvent, DbState},
    storage::SearchMode,
};
use gpui::{
    App, Context, Entity, Focusable, Render, SharedString, Subscription, Window, div, prelude::*,
    px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    select::{Select, SelectEvent, SelectItem, SelectState},
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
                .border_b_1()
                .border_color(cx.theme().border)
                .when(selected, |this| this.bg(cx.theme().selection))
                .child(
                    div()
                        .font_family("monospace")
                        .text_sm()
                        .child(format_bytes(&entry.key)),
                ),
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
    _subscriptions: Vec<Subscription>,
}

impl KeyBrowser {
    pub fn new(db_state: Entity<DbState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
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
        let list_for_state = list_state.clone();
        subscriptions.push(cx.subscribe(&db_state, move |_this, state, event, cx| {
            if matches!(event, DbEvent::Updated) {
                let db = state.read(cx);
                let items = db.keys().to_vec();
                let selected_key = db.selected_key().map(Vec::from);
                list_for_state.update(cx, |list, cx| {
                    list.delegate_mut().items = items;
                    list.delegate_mut().selected_key = selected_key;
                    cx.notify();
                });
                cx.notify();
            }
        }));
        subscriptions.push(cx.subscribe(&list_state, |this, list, event, cx| {
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
        Self {
            db_state,
            search_state,
            search_mode_state,
            list_state,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for KeyBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.db_state.read(cx);
        let has_more = state.next_key().is_some();
        let loading = matches!(state.load_state(), crate::state::LoadState::Loading);

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .h(px(40.))
                    .p_1()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div().flex_1().min_w(px(220.)).h_full().child(
                            Input::new(&self.search_state)
                                .w_full()
                                .appearance(true)
                                .cleanable(true)
                                .prefix(Icon::new(IconName::Search)),
                        ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .child(
                                Select::new(&self.search_mode_state)
                                    .w(px(36.))
                                    .h(px(32.))
                                    .menu_width(px(150.))
                                    .small()
                                    .appearance(false)
                                    .icon(Icon::new(IconName::Settings2)),
                            )
                            .child({
                                let state = self.db_state.clone();
                                Button::new("refresh-keys")
                                    .icon(IconName::Redo2)
                                    .h(px(32.))
                                    .small()
                                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                                        state.update(cx, |state, cx| state.refresh(cx));
                                    }))
                            })
                            .child({
                                let state = self.db_state.clone();
                                Button::new("first-page")
                                    .icon(IconName::ArrowUp)
                                    .h(px(32.))
                                    .small()
                                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                                        state.update(cx, |state, cx| state.jump_to_first(cx));
                                    }))
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(Label::new(format!("{} keys loaded", state.keys().len())).text_sm())
                    .when(loading, |this| {
                        this.child(
                            Label::new("Loading…")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            .child(
                div()
                    .id("key-list")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(List::new(&self.list_state).size_full()),
            )
            .when(has_more, |this| {
                let state = self.db_state.clone();
                this.child(
                    Button::new("load-more")
                        .label("Load more")
                        .icon(IconName::ArrowDown)
                        .small()
                        .on_click(cx.listener(move |_this, _event, _window, cx| {
                            state.update(cx, |state, cx| state.load_more(cx));
                        })),
                )
            })
    }
}

pub fn format_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) if text.chars().all(|character| !character.is_control()) => text.to_string(),
        _ => format!("0x{}", hex(bytes)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_printable_utf8_as_text() {
        assert_eq!(format_bytes(b"records/00000001"), "records/00000001");
    }

    #[test]
    fn formats_binary_bytes_as_hex() {
        assert_eq!(format_bytes(&[0, 1, 0xff]), "0x0001ff");
    }

    #[test]
    fn formats_control_text_as_hex() {
        assert_eq!(format_bytes(b"line\nbreak"), "0x6c696e650a627265616b");
    }
}
