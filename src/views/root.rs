use super::{
    key_browser::KeyBrowser,
    open_database::{OpenDatabaseButton, open_database, opening_state, welcome_state},
    sidebar::Sidebar,
    value_viewer::ValueViewer,
};
use crate::platform::sync_native_window_chrome;
use crate::state::{BrowseMode, DbEvent, DbState, LoadState};
use crate::ui::{compact_path, muted_label, toggle_theme};
use gpui_kit::{Context, Entity, Render, SharedString, Subscription, Window, div, prelude::*, px};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, IconNamed, Sizable,
    alert::Alert,
    button::{Button, ButtonVariants},
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

struct DatabasePlusIcon;

impl IconNamed for DatabasePlusIcon {
    fn path(self) -> SharedString {
        "icons/database-plus.svg".into()
    }
}

#[cfg(target_os = "macos")]
const TITLEBAR_LEFT_INSET: f32 = 80.;
#[cfg(not(target_os = "macos"))]
const TITLEBAR_LEFT_INSET: f32 = 8.;

pub struct RootView {
    db_state: Entity<DbState>,
    empty_open_button: Entity<OpenDatabaseButton>,
    sidebar: Entity<Sidebar>,
    key_browser: Entity<KeyBrowser>,
    value_viewer: Entity<ValueViewer>,
    last_shell_load_state: Option<LoadState>,
    last_shell_browse_mode: Option<BrowseMode>,
    _subscriptions: Vec<Subscription>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let db_state = cx.new(|_| DbState::new());
        let empty_open_button =
            cx.new(|_| OpenDatabaseButton::new(db_state.clone(), "open-database-empty"));
        let sidebar = cx.new(|cx| Sidebar::new(db_state.clone(), window, cx));
        let key_browser = cx.new(|cx| KeyBrowser::new(db_state.clone(), window, cx));
        let value_viewer = cx.new(|cx| ValueViewer::new(db_state.clone(), window, cx));
        let subscriptions = vec![cx.subscribe(&db_state, move |this, state, event, cx| {
            if !matches!(event, DbEvent::Updated) {
                return;
            }
            let (load_state, browse_mode) =
                state.read_with(cx, |db, _| (db.load_state(), db.browse_mode()));
            let needs_shell_update = this.last_shell_load_state.as_ref() != Some(&load_state)
                || this.last_shell_browse_mode.as_ref() != Some(&browse_mode);
            if needs_shell_update {
                this.last_shell_load_state = Some(load_state);
                this.last_shell_browse_mode = Some(browse_mode);
                cx.notify();
            }
        })];
        sync_native_window_chrome(window, cx.theme().is_dark(), cx);

        Self {
            db_state,
            empty_open_button,
            sidebar,
            key_browser,
            value_viewer,
            last_shell_load_state: None,
            last_shell_browse_mode: None,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.db_state.read(cx);
        let load_state = state.load_state();
        let error = state.error().map(str::to_owned);
        let is_dark = cx.theme().is_dark();
        let database_open = matches!(load_state, LoadState::Ready | LoadState::Loading);
        let state_for_open = self.db_state.clone();

        let open_button = Button::new("open-database-top")
            .label(if database_open {
                "Open another database"
            } else {
                "Open database"
            })
            .icon(Icon::new(DatabasePlusIcon))
            .ghost()
            .compact()
            .small()
            .on_click(move |_event, _window, cx| {
                open_database(state_for_open.clone(), cx);
            });
        let theme_button = Button::new("toggle-theme")
            .icon(Icon::new(if is_dark {
                IconName::Sun
            } else {
                IconName::Moon
            }))
            .ghost()
            .compact()
            .small()
            .tooltip(if is_dark {
                "Switch to light theme"
            } else {
                "Switch to dark theme"
            })
            .on_click(cx.listener(move |_this, _event, window, cx| {
                toggle_theme(is_dark, window, cx);
            }));

        let header_left = if let Some(path) = state.path() {
            h_flex()
                .items_center()
                .gap_2()
                .child(open_button)
                .child(
                    muted_label(compact_path(&path.display().to_string(), 48), cx)
                        .overflow_x_hidden(),
                )
                .into_any_element()
        } else {
            h_flex().items_center().gap_2().child(open_button).into_any_element()
        };

        let header = h_flex()
            .w_full()
            .h(px(36.))
            .flex_none()
            .items_center()
            .justify_between()
            .pl(px(TITLEBAR_LEFT_INSET))
            .pr_2()
            .child(header_left)
            .child(theme_button);

        let content = match load_state {
            LoadState::Closed => welcome_state(self.empty_open_button.clone(), cx).into_any_element(),
            LoadState::Opening => opening_state(cx).into_any_element(),
            LoadState::Failed => {
                let message = error.unwrap_or_else(|| "Unable to open database".to_string());
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        v_flex()
                            .gap_3()
                            .w_full()
                            .max_w(px(480.))
                            .items_center()
                            .child(Alert::error("open-database-error", message))
                            .child(self.empty_open_button.clone()),
                    )
                    .into_any_element()
            }
            LoadState::Ready | LoadState::Loading => {
                let table_mode = state.browse_mode() == BrowseMode::Table;
                let key_browser = self.key_browser.clone();
                let value_viewer = self.value_viewer.clone();
                h_flex()
                    .size_full()
                    .min_h_0()
                    .child(
                        h_resizable("main-layout-split")
                            .child(
                                resizable_panel()
                                    .size(px(220.))
                                    .size_range(px(160.)..px(320.))
                                    .child(self.sidebar.clone()),
                            )
                            .child(resizable_panel().child(if table_mode {
                                key_browser.into_any_element()
                            } else {
                                h_resizable("database-content-split")
                                    .child(
                                        resizable_panel()
                                            .size(px(520.))
                                            .size_range(px(360.)..px(900.))
                                            .child(key_browser),
                                    )
                                    .child(resizable_panel().child(value_viewer))
                                    .into_any_element()
                            })),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(header)
            .child(div().flex_1().min_h_0().child(content))
    }
}
