use gpui_kit::{App, Window, WindowAppearance};

#[cfg(target_os = "windows")]
mod windows;

/// Keep the native window frame (title bar and outer border) in sync with the app theme.
pub fn sync_native_window_chrome(window: &Window, dark: bool, cx: &mut App) {
    let appearance = if dark {
        WindowAppearance::Dark
    } else {
        WindowAppearance::Light
    };
    cx.set_window_appearance(Some(appearance));

    #[cfg(target_os = "windows")]
    windows::sync_window_chrome(window, dark);
}
