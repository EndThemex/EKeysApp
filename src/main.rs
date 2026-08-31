//! 入口：装载字体 + 启动 App。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod link;
mod protocol;
mod state;
mod ui;
mod util;

use eframe::egui::Vec2;

fn main() -> eframe::Result {
    util::log::init_tracing();

    let cfg = config::load();
    let handle = state::AppHandle::new();

    // 把 LocalConfig 注入 AppHandle
    *handle.last_port.lock().unwrap() = cfg.last_port.clone();
    *handle.auto_connect.lock().unwrap() = cfg.auto_connect;
    *handle.local_config.lock().unwrap() = cfg.clone();

    let mut options = eframe::NativeOptions::default();
    if let Some([w, h]) = cfg.window_size {
        options.viewport.inner_size = Some(Vec2::new(w, h));
    }
    options.viewport.min_inner_size = Some(Vec2::new(960.0, 600.0));

    eframe::run_native(
        "wxi — EKeys Desktop App",
        options,
        Box::new(move |cc| {
            let app = app::WxiApp::new(cc, handle);
            Ok(Box::new(app))
        }),
    )
}
