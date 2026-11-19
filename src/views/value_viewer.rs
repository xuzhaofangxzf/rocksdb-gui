use crate::{
    state::{DbEvent, DbState},
    ui::{mono_text, muted_label},
    util::format_bytes,
};
use gpui_kit::{
    ClipboardItem, Context, Entity, Render, SharedString, Subscription, Window, div, prelude::*,
    rems,
};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, IconNamed, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    text::{TextView, TextViewStyle},
    v_flex,
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
    last_selected_key: Option<Vec<u8>>,
    last_value_len: Option<usize>,
    formatted: Option<CachedFormat>,
    _subscriptions: Vec<Subscription>,
}

struct CachedFormat {
    key: Vec<u8>,
    value_len: usize,
    pretty_json: bool,
    format: ValueFormat,
}

impl ValueViewer {
    pub fn new(db_state: Entity<DbState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(&db_state, |this, state, event, cx| {
            if !matches!(event, DbEvent::Updated) {
                return;
            }
            let state = state.read(cx);
            let selected_key = state.selected_key().map(Vec::from);
            let value_len = state.value().map(|value| value.value.len());
            if this.last_selected_key == selected_key && this.last_value_len == value_len {
                return;
            }
            this.last_selected_key = selected_key;
            this.last_value_len = value_len;
            this.formatted = None;
            cx.notify();
        })];
        Self {
            db_state,
            pretty_json: false,
            last_selected_key: None,
            last_value_len: None,
            formatted: None,
            _subscriptions: subscriptions,
        }
    }

    fn formatted_value(&mut self, key: &[u8], value: &crate::storage::ValueEntry) -> &ValueFormat {
        let needs_refresh = self.formatted.as_ref().is_none_or(|cached| {
            cached.key.as_slice() != key
                || cached.value_len != value.value.len()
                || cached.pretty_json != self.pretty_json
        });
        if needs_refresh {
            self.formatted = Some(CachedFormat {
                key: key.to_vec(),
                value_len: value.value.len(),
                pretty_json: self.pretty_json,
                format: format_value(&value.value, self.pretty_json),
            });
        }
        &self.formatted.as_ref().expect("formatted value cache").format
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
                .child(muted_label("Select a key to inspect its value", cx))
                .into_any_element();
        };

        let key_text = format_bytes(&key);
        let key_for_copy = key_text.clone();
        let body = match state.value() {
            Some(value) => {
                let formatted = self.formatted_value(&key, value);
                let value_for_copy = formatted.copy.clone();
                let highlighted = formatted.highlighted;
                let display = formatted.display.clone();
                let size = value.value.len();
                let truncated = value.truncated;
                v_flex()
                    .size_full()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(muted_label(format!("{size} bytes"), cx))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Button::new("pretty-json")
                                            .icon(Icon::new(PrettyJsonIcon))
                                            .ghost()
                                            .small()
                                            .tooltip(if self.pretty_json {
                                                "Show raw bytes"
                                            } else {
                                                "Pretty-print JSON"
                                            })
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.pretty_json = !this.pretty_json;
                                                this.formatted = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("copy-value")
                                            .icon(IconName::Copy)
                                            .ghost()
                                            .small()
                                            .tooltip("Copy value")
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
                                .text_color(cx.theme().warning),
                        )
                    })
                    .child(
                        div()
                            .id("value-content")
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .overflow_y_scrollbar()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .when(!highlighted, |this| this.p_2().bg(cx.theme().secondary))
                            .child(if highlighted {
                                TextView::markdown("value-text", display)
                                    .selectable(true)
                                    .style(TextViewStyle::default().paragraph_gap(rems(0.)))
                                    .into_any_element()
                            } else {
                                mono_text(div(), cx)
                                    .text_sm()
                                    .child(display)
                                    .into_any_element()
                            }),
                    )
                    .into_any_element()
            }
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(muted_label("Loading value…", cx))
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        mono_text(div(), cx)
                            .flex_1()
                            .text_color(cx.theme().foreground)
                            .child(key_text),
                    )
                    .child(
                        Button::new("copy-key")
                            .icon(IconName::Copy)
                            .ghost()
                            .small()
                            .tooltip("Copy key")
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

struct ValueFormat {
    display: String,
    copy: String,
    highlighted: bool,
}

fn format_value(bytes: &[u8], pretty_json: bool) -> ValueFormat {
    if pretty_json {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) {
            let expanded = unwrap_json_strings(json);
            if let Ok(pretty) = serde_json::to_string_pretty(&expanded) {
                return ValueFormat {
                    display: wrap_json_code_block(&pretty),
                    copy: pretty,
                    highlighted: true,
                };
            }
        }
    }

    let text = format_bytes(bytes);
    ValueFormat {
        display: text.clone(),
        copy: text,
        highlighted: false,
    }
}

/// Recursively parse string values that contain JSON objects or arrays.
fn unwrap_json_strings(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(parsed) if parsed.is_object() || parsed.is_array() => unwrap_json_strings(parsed),
            _ => serde_json::Value::String(s),
        },
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items.into_iter().map(unwrap_json_strings).collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, unwrap_json_strings(value)))
                .collect(),
        ),
        other => other,
    }
}

fn wrap_json_code_block(json: &str) -> String {
    let fence = if json.contains("```") { "````" } else { "```" };
    format!("{fence}json\n{json}\n{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_nested_json_strings() {
        let input = serde_json::json!({
            "timeStamp": "2026-09-16T05:44:58.3611709Z",
            "sMessage": "{\"ReceiveTicket\":639251342983611448,\"Context\":\"{\\\"measured_depth\\\":0.0}\"}",
            "WorkOrderId": null
        });

        let expanded = unwrap_json_strings(input);
        let message = expanded.get("sMessage").expect("sMessage");
        assert!(message.is_object());
        let context = message.get("Context").expect("Context");
        assert!(context.is_object());
        assert_eq!(context.get("measured_depth").and_then(|v| v.as_f64()), Some(0.0));
    }

    #[test]
    fn unwrap_leaves_plain_strings() {
        let input = serde_json::json!({
            "name": "hello",
            "code": "123"
        });

        let expanded = unwrap_json_strings(input);
        assert_eq!(expanded.get("name").and_then(|v| v.as_str()), Some("hello"));
        assert_eq!(expanded.get("code").and_then(|v| v.as_str()), Some("123"));
    }
}
