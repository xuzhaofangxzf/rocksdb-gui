use crate::state::DbState;
use crate::ui::empty_workspace;
use gpui_kit::{App, AppContext, Entity, Render, SharedString, Window, div, prelude::*};
use gpui_kit::component::{IconName, button::Button, button::ButtonVariants};

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
    fn render(&mut self, _window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let state = self.db_state.clone();
        Button::new(self.id.clone())
            .label("Open database")
            .icon(IconName::FolderOpen)
            .primary()
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                open_database(state.clone(), cx);
            }))
    }
}

pub fn welcome_state(button: impl IntoElement, cx: &App) -> impl IntoElement {
    empty_workspace(
        "Open a RocksDB database",
        "Choose a local database directory to browse column families, keys, and values.",
        button,
        cx,
    )
}

pub fn opening_state(cx: &App) -> impl IntoElement {
    empty_workspace(
        "Opening database",
        "Reading column families and preparing the first page of keys…",
        div(),
        cx,
    )
}
