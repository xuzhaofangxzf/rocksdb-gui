use crate::platform::sync_native_window_chrome;
use gpui_kit::{App, Window};
use gpui_kit::component::{Theme, ThemeMode};

pub fn init_default_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
}

pub fn apply_theme_mode(mode: ThemeMode, window: &mut Window, cx: &mut App) {
    Theme::change(mode, Some(window), cx);
    sync_native_window_chrome(window, mode.is_dark(), cx);
    cx.refresh_windows();
}

pub fn toggle_theme(is_dark: bool, window: &mut Window, cx: &mut App) {
    let next = if is_dark {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    apply_theme_mode(next, window, cx);
}
