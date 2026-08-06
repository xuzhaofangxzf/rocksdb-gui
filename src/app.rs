use crate::assets::Assets;
use crate::views::RootView;
#[cfg(target_os = "macos")]
use gpui::point;
use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

pub fn run() {
    let app = gpui_platform::application().with_assets(Assets);
    app.run(|cx| {
        gpui_component::init(cx);
        cx.activate(true);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(600.))),
                #[cfg(target_os = "macos")]
                titlebar: Some(gpui::TitlebarOptions {
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
