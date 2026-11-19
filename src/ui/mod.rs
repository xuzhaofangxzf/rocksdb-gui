mod theme;

use gpui_kit::{App, Div, ElementId, IntoElement, SharedString, Stateful, div, prelude::*, px};
use gpui_kit::component::{ActiveTheme, label::Label, v_flex};

pub use theme::{init_default_theme, toggle_theme};

/// Theme monospace styling for keys and raw values.
pub fn mono_text(div: Div, cx: &App) -> Div {
    div.font_family(cx.theme().mono_font_family.clone())
        .text_size(cx.theme().mono_font_size)
}

/// Bordered region for scrollable data (keys, values).
pub fn data_panel_styled(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

pub fn muted_label(text: impl Into<SharedString>, cx: &App) -> Label {
    Label::new(text)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

pub fn compact_path(path: &str, max_chars: usize) -> String {
    let chars = path.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return path.to_string();
    }

    let visible_len = max_chars.saturating_sub(3);
    format!("{}...", chars[..visible_len].iter().collect::<String>())
}

pub fn empty_workspace(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    action: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            v_flex()
                .gap_3()
                .w_full()
                .max_w(px(420.))
                .items_center()
                .child(Label::new(title).text_lg())
                .child(muted_label(detail, cx).text_center())
                .child(action),
        )
}
