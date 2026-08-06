use super::{
    key_browser::KeyBrowser,
    open_database::{OpenDatabaseButton, empty_state, open_database},
    sidebar::Sidebar,
    value_viewer::ValueViewer,
};
use crate::state::{DbEvent, DbState, LoadState};
use gpui::{Context, Entity, Render, SharedString, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName, IconNamed, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
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
        let subscriptions = vec![cx.subscribe(&db_state, |_this, _state, event, cx| {
            if matches!(event, DbEvent::Updated) {
                cx.notify();
            }
        })];
        Self {
            db_state,
            empty_open_button,
            sidebar,
            key_browser,
            value_viewer,
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
        let state_for_open = self.db_state.clone();
        let open_button = Button::new("open-database-top")
            .label("Open RocksDB")
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
            .on_click(cx.listener(move |_this, _event, window, cx| {
                Theme::change(
                    if is_dark {
                        ThemeMode::Light
                    } else {
                        ThemeMode::Dark
                    },
                    Some(window),
                    cx,
                );
            }));

        let header = h_flex()
            .w_full()
            .h(px(36.))
            .flex_none()
            .items_center()
            .justify_between()
            .pl(px(TITLEBAR_LEFT_INSET))
            .pr_2()
            .child(h_flex().items_center().gap_2().child(open_button))
            .child(h_flex().gap_2().items_center().child(theme_button));

        let content = match load_state {
            LoadState::Closed => {
                empty_state(self.empty_open_button.clone().into_any_element()).into_any_element()
            }
            LoadState::Opening => empty_state(div().child("Opening database…")).into_any_element(),
            LoadState::Failed => {
                let message = error.unwrap_or_else(|| "Unable to open database".to_string());
                empty_state(
                    v_flex()
                        .items_center()
                        .gap_2()
                        .child(Label::new(message).text_sm().text_color(cx.theme().red))
                        .child(self.empty_open_button.clone()),
                )
                .into_any_element()
            }
            LoadState::Ready | LoadState::Loading => h_flex()
                .size_full()
                .min_h_0()
                .child(
                    h_resizable("main-layout-split")
                        .child(
                            resizable_panel()
                                .size(px(200.))
                                .size_range(px(150.)..px(320.))
                                .child(self.sidebar.clone()),
                        )
                        .child(
                            resizable_panel().child(
                                h_resizable("database-content-split")
                                    .child(
                                        resizable_panel()
                                            .size(px(600.))
                                            .size_range(px(380.)..px(900.))
                                            .child(self.key_browser.clone()),
                                    )
                                    .child(resizable_panel().child(self.value_viewer.clone())),
                            ),
                        ),
                )
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(header)
            .child(div().flex_1().min_h_0().child(content))
    }
}
