use crate::state::DbState;
use crate::ui::{compact_path, mono_text};
use gpui_kit::{Context, Entity, Render, SharedString, Window, div, prelude::*, svg};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, h_flex, label::Label, menu::{ContextMenuExt, PopupMenuItem},
    v_flex,
};

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
        let database = state
            .path()
            .map(|path| compact_path(&path.display().to_string(), 28))
            .unwrap_or_else(|| "No database opened".to_string());
        let db_state = self.db_state.clone();

        let tree_children = state.column_families().iter().map(|family| {
            let is_selected = selected.as_deref() == Some(family.as_str());
            let family_for_click = family.clone();
            let family_for_export = family.clone();
            let state = db_state.clone();
            let export_state = db_state.clone();
            div()
                .id(SharedString::from(format!("cf-{family}")))
                .w_full()
                .px_2()
                .py_1()
                .rounded_md()
                .when(is_selected, |this| this.bg(cx.theme().accent))
                .when(!is_selected, |this| {
                    this.hover(|this| this.bg(cx.theme().accent.opacity(0.5)))
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
                        .child(Label::new(family.clone())),
                )
                .on_click(cx.listener(move |_this, _event, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.select_column_family(family_for_click.clone(), cx)
                    });
                }))
                .context_menu(move |menu, _window, _cx| {
                    if !is_selected {
                        return menu;
                    }
                    menu.item({
                        let export_state = export_state.clone();
                        let family_for_export = family_for_export.clone();
                        PopupMenuItem::new("Export").on_click(move |_event, _window, cx| {
                            export_state.update(cx, |state, cx| {
                                state.export_column_family(family_for_export.clone(), cx);
                            });
                        })
                    })
                })
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
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .id("rocksdb-root")
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|this| this.bg(cx.theme().accent.opacity(0.5)))
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
                            .child(
                                mono_text(div(), cx)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(database),
                            ),
                    )
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    })),
            )
            .child(children)
    }
}
