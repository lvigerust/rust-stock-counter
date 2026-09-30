mod tasks;

use gpui_kit::*;
use tasks::TaskList;

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        gpui_kit::init(cx);

        // Window geometry is a platform boundary, so physical pixels are intentional here.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(460.), px(560.)),
                cx,
            ))),
            window_min_size: Some(size(px(360.), px(420.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Tasks".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        open_window(options, cx, |window, cx| {
            cx.new(|cx| TaskList::new(window, cx))
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}
