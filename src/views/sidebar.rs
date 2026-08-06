use crate::state::DbState;
use gpui::{Context, Entity, Render, SharedString, Window, div, prelude::*, svg};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, label::Label, v_flex};

pub struct Sidebar {
    db_state: Entity<DbState>,
    expanded: bool,
}

impl Sidebar {
    pub fn new(db_state: Entity<DbState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            db_state,
            expanded: true,
        }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.db_state.read(cx);
        let selected = state.selected_column_family().map(str::to_owned);
        let families = state.column_families().to_vec();
        let database = state
            .path()
            .map(|path| compact_path(&path.display().to_string(), 15))
            .unwrap_or_else(|| "No database opened".to_string());
        let db_state = self.db_state.clone();

        let tree_children = families.into_iter().map(|family| {
            let is_selected = selected.as_deref() == Some(family.as_str());
            let family_for_click = family.clone();
            let state = db_state.clone();
            div()
                .id(SharedString::from(format!("cf-{family}")))
                .w_full()
                .px_2()
                .py_1()
                .rounded_md()
                .when(is_selected, |this| this.bg(cx.theme().selection))
                .when(!is_selected, |this| {
                    this.hover(|this| this.bg(cx.theme().border))
                })
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            svg()
                                .path("icons/landmark.svg")
                                .size_4()
                                .text_color(cx.theme().foreground),
                        )
                        .child(Label::new(family)),
                )
                .on_click(cx.listener(move |_this, _event, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.select_column_family(family_for_click.clone(), cx)
                    });
                }))
        });

        let children = if self.expanded {
            v_flex()
                .pl_4()
                .gap_1()
                .children(tree_children)
                .into_any_element()
        } else {
            div().into_any_element()
        };

        v_flex()
            .size_full()
            .gap_1()
            .p_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .id("rocksdb-root")
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|this| this.bg(cx.theme().border))
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(Icon::new(if self.expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            }))
                            .child(
                                svg()
                                    .path("icons/rocket.svg")
                                    .size_4()
                                    .text_color(cx.theme().foreground),
                            )
                            .child(Label::new(database).text_sm()),
                    )
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    })),
            )
            .child(children)
    }
}

fn compact_path(path: &str, max_chars: usize) -> String {
    let chars = path.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return path.to_string();
    }

    let visible_len = max_chars.saturating_sub(3);
    format!("{}...", chars[..visible_len].iter().collect::<String>())
}
