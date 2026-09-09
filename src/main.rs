//! 入口：装载字体 + 启动 App。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod link;
mod pc_status;
mod protocol;
mod state;
mod ui;
mod util;

use eframe::egui::{IconData, Vec2};
use std::sync::Arc;

/// 直接用编译期嵌入 exe 的 `img/ekeys.ico` 生成窗口/程序图标。
/// `include_bytes!` 把图标字节打包进二进制，运行时无需外部文件。
fn load_icon() -> Option<Arc<IconData>> {
    const ICO_BYTES: &[u8] = include_bytes!("../img/icon.png");

    if let Ok(img) = image::load_from_memory(ICO_BYTES) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        return Some(Arc::new(IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        }));
    }

    // 回退：生成 64x64 品牌色方块图标
    const S: usize = 64;
    let mut rgba = vec![0u8; S * S * 4];
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&[0x4F, 0x8C, 0xFF, 0xFF]);
    }
    Some(Arc::new(IconData {
        rgba,
        width: S as u32,
        height: S as u32,
    }))
}

fn main() -> eframe::Result {
    util::log::init_tracing();

    let cfg = config::load();
    let handle = state::AppHandle::new();

    // 把 LocalConfig 注入 AppHandle
    *handle.last_port.lock().unwrap() = cfg.last_port.clone();
    *handle.auto_connect.lock().unwrap() = cfg.auto_connect;
    *handle.local_config.lock().unwrap() = cfg.clone();
    // PC 状态推送开关：默认关闭；持久化字段，跨启动保留用户选择。
    handle
        .pc_status_push_enabled
        .store(cfg.pc_status_push, std::sync::atomic::Ordering::Relaxed);

    let mut options = eframe::NativeOptions::default();
    if let Some([w, h]) = cfg.window_size {
        options.viewport.inner_size = Some(Vec2::new(w, h));
    }
    options.viewport.min_inner_size = Some(Vec2::new(960.0, 600.0));
    options.viewport.icon = load_icon();

    eframe::run_native(
        "EKeys",
        options,
        Box::new(move |cc| {
            let app = app::WxiApp::new(cc, handle);
            Ok(Box::new(app))
        }),
    )
}
