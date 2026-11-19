use crate::assets::{Assets, window_icon};
use crate::ui::init_default_theme;
use crate::views::RootView;
#[cfg(target_os = "macos")]
use gpui_kit::point;
use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
#[cfg(target_os = "macos")]
use gpui_kit::TitlebarOptions;
use gpui_kit::component::Root;

pub fn run() {
    gpui_kit::application()
        .with_assets(Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            init_default_theme(cx);
            cx.activate(true);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(900.), px(600.))),
                    icon: window_icon(),
                    #[cfg(target_os = "macos")]
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| RootView::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
        });
}
