use crate::{
    state::{DbEvent, DbState},
    views::key_browser::format_bytes,
};
use gpui::{
    ClipboardItem, Context, Entity, Render, SharedString, Subscription, Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, IconNamed, Sizable, button::Button, h_flex, label::Label,
    text::TextView, v_flex,
};

struct PrettyJsonIcon;

impl IconNamed for PrettyJsonIcon {
    fn path(self) -> SharedString {
        "icons/pretty-json.svg".into()
    }
}

pub struct ValueViewer {
    db_state: Entity<DbState>,
    pretty_json: bool,
    _subscriptions: Vec<Subscription>,
}

impl ValueViewer {
    pub fn new(db_state: Entity<DbState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(&db_state, |_this, _state, event, cx| {
            if matches!(event, DbEvent::Updated) {
                cx.notify();
            }
        })];
        Self {
            db_state,
            pretty_json: false,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for ValueViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.db_state.read(cx);
        let Some(key) = state.selected_key().map(Vec::from) else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(Label::new("Select a key to inspect its value").text_sm())
                .into_any_element();
        };

        let key_text = format_bytes(&key);
        let key_for_copy = key_text.clone();
        let value = state.value().cloned();
        let body = match value {
            Some(value) => {
                let value_text = if self.pretty_json {
                    format_pretty_json(&value.value)
                } else {
                    format_bytes(&value.value)
                };
                let value_for_copy = value_text.clone();
                let size = value.value.len();
                let truncated = value.truncated;
                v_flex()
                    .size_full()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(Label::new(format!("{size} bytes")).text_sm())
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Button::new("pretty-json")
                                            .icon(Icon::new(PrettyJsonIcon))
                                            .small()
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.pretty_json = !this.pretty_json;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("copy-value")
                                            .icon(IconName::Copy)
                                            .small()
                                            .on_click(cx.listener(
                                                move |_this, _event, _window, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(
                                                            value_for_copy.clone(),
                                                        ),
                                                    );
                                                },
                                            )),
                                    ),
                            ),
                    )
                    .when(truncated, |this| {
                        this.child(
                            Label::new("Value exceeds 4 MiB; preview truncated")
                                .text_sm()
                                .text_color(cx.theme().yellow),
                        )
                    })
                    .child(
                        div()
                            .id("value-content")
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .overflow_y_scroll()
                            .p_2()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(
                                TextView::markdown("value-text", value_text)
                                    .selectable(true)
                                    .font_family("monospace")
                                    .text_sm(),
                            ),
                    )
                    .into_any_element()
            }
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(Label::new("Loading value…").text_sm())
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex_1()
                            .font_family("monospace")
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(key_text),
                    )
                    .child(
                        Button::new("copy-key")
                            .icon(IconName::Copy)
                            .small()
                            .on_click(cx.listener(move |_this, _event, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    key_for_copy.clone(),
                                ));
                            })),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}

fn format_pretty_json(bytes: &[u8]) -> String {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            return pretty;
        }
    }

    format_bytes(bytes)
}
