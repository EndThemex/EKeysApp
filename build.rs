//! 仅 Windows：将 `img/icon.ico` 嵌入到最终 exe 的资源段，
//! 让打包后的 .exe 在资源管理器、任务栏、标题栏里都显示同一图标。

#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("img/icon.ico");
    if let Err(e) = res.compile() {
        // 不让图标缺失直接阻断构建；改用 stderr 提示，仍可生成无图标的 exe。
        eprintln!("winres: failed to embed icon ({}). exe will use default icon.", e);
    }
}

#[cfg(not(windows))]
fn main() {}
