use crate::state::DbState;
use gpui::{App, AppContext, Entity, Render, SharedString, Window, div, prelude::*};
use gpui_component::{IconName, button::Button, button::ButtonVariants, v_flex};

pub struct OpenDatabaseButton {
    db_state: Entity<DbState>,
    id: SharedString,
}

impl OpenDatabaseButton {
    pub fn new(db_state: Entity<DbState>, id: impl Into<SharedString>) -> Self {
        Self {
            db_state,
            id: id.into(),
        }
    }
}

pub fn open_database(state: Entity<DbState>, cx: &mut App) {
    let picker = cx.background_spawn(async {
        rfd::FileDialog::new()
            .set_title("Select a RocksDB directory")
            .pick_folder()
    });
    cx.spawn(async move |cx| {
        if let Some(path) = picker.await {
            state.update(cx, |state, cx| state.open(path, cx));
        }
    })
    .detach();
}

impl Render for OpenDatabaseButton {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = self.db_state.clone();
        Button::new(self.id.clone())
            .label("Open RocksDB")
            .icon(IconName::FolderOpen)
            .primary()
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                open_database(state.clone(), cx);
            }))
    }
}

pub fn empty_state(button: impl IntoElement) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .child(div().text_lg().child("Open a local RocksDB database"))
        .child(button)
}
