//! chrome 区域翻译表：侧栏 `sidenav.rs` / 顶栏 `topbar.rs` / 底栏 `statusbar.rs`。
//!
//! 保留前缀：`nav.*`（侧栏）、`top.*`（顶栏）、`status.*`（底栏）。

use super::Entry;

pub(super) const ENTRIES: &[Entry] = &[
    // ---- 侧栏：分组标题 ----
    ("nav.group.device", "设备", "Device"),
    ("nav.group.system", "系统", "System"),
    // ---- 侧栏：导航项 ----
    ("nav.device", "设备", "Device"),
    (
        "nav.device.hint",
        "设备参数配置 (Ctrl+1)",
        "Device settings (Ctrl+1)",
    ),
    ("nav.keymap", "键盘", "Keymap"),
    (
        "nav.keymap.hint",
        "按键盘自定义按键功能 (Ctrl+2)",
        "Customize key functions (Ctrl+2)",
    ),
    ("nav.lighting", "灯效", "Lighting"),
    (
        "nav.lighting.hint",
        "灯光效果 (Ctrl+3)",
        "Lighting effects (Ctrl+3)",
    ),
    ("nav.wifi", "WiFi", "WiFi"),
    (
        "nav.wifi.hint",
        "无线网络 (Ctrl+4)",
        "Wireless network (Ctrl+4)",
    ),
    ("nav.audio", "音效", "Audio"),
    (
        "nav.audio.hint",
        "音效板文件与键位绑定 (Ctrl+5)",
        "Sound pad files & key bindings (Ctrl+5)",
    ),
    ("nav.voice", "语音", "Voice"),
    (
        "nav.voice.hint",
        "语音设置 (Ctrl+6)",
        "Voice settings (Ctrl+6)",
    ),
    ("nav.log", "日志", "Log"),
    (
        "nav.log.hint",
        "协议与应用日志 (Ctrl+7)",
        "Protocol & app logs (Ctrl+7)",
    ),
    ("nav.about", "关于", "About"),
    (
        "nav.about.hint",
        "版本信息 (Ctrl+8)",
        "Version info (Ctrl+8)",
    ),
    // ---- 顶栏：连接状态胶囊 ----
    ("top.status.online", "在线", "Online"),
    ("top.status.reconnecting", "重连中", "Reconnecting"),
    ("top.status.connecting", "连接中", "Connecting"),
    ("top.status.disconnected", "未连接", "Disconnected"),
    // ---- 顶栏：端口 ----
    ("top.port.none", "(无)", "(none)"),
    (
        "top.port.ch340",
        "{name}（CH340 不支持）",
        "{name} (CH340 unsupported)",
    ),
    (
        "top.port.product",
        "{name}（{product}）",
        "{name} ({product})",
    ),
    ("top.port.unselected", "(未选择端口)", "(no port selected)"),
    ("top.port.show_all", "显示全部端口", "Show all ports"),
    (
        "top.port.show_all.hint",
        "关闭时只列出疑似 ESP32 的端口（Espressif 原生 USB 与常见 USB 转串口桥）",
        "When off, only ports that could be an ESP32 are listed (Espressif native USB and common USB-UART bridges)",
    ),
    ("top.scan", "扫描", "Scan"),
    // ---- 顶栏：连接 / 断开 ----
    ("top.connect", "● 连接", "● Connect"),
    ("top.disconnect", "■ 断开", "■ Disconnect"),
    (
        "top.connect.fail",
        "连接失败：{error}",
        "Failed to connect: {error}",
    ),
    // ---- 顶栏：自动连接 ----
    ("top.auto_connect", "启动时自动连接", "Connect on startup"),
    (
        "top.auto_connect.hint",
        "勾选后下次启动自动连接上次使用的端口",
        "Auto-connect to the last used port on next launch",
    ),
    (
        "top.auto_connect.ch340_warning",
        "⚠ 当前为 CH340 系列 USB 转串口芯片，不支持连接，请改用设备原生 USB CDC 接口",
        "⚠ This is a CH340-series USB-to-serial bridge, which is not supported. Use the device's native USB CDC port instead",
    ),
    (
        "top.auto_connect.ch340_skipped",
        "已跳过自动连接：{port} 为 CH340 系列芯片，请改用设备原生 USB CDC 接口",
        "Auto-connect skipped: {port} is a CH340-series chip, please use the device's native USB CDC port",
    ),
    // ---- 顶栏：停止重连 ----
    ("top.stop_reconnect", "✕ 停止重连", "✕ Stop reconnecting"),
    (
        "top.stop_reconnect.hint",
        "立即终止当前的自动重连流程",
        "Abort the current auto-reconnect process immediately",
    ),
    (
        "top.stop_reconnect.done",
        "已停止自动重连",
        "Auto-reconnect stopped",
    ),
    // ---- 顶栏：右侧操作 ----
    ("top.settings", "设置", "Settings"),
    ("top.refresh", "刷新", "Refresh"),
    (
        "top.refresh.hint",
        "重新拉取全量设置 (F5)",
        "Re-fetch all settings (F5)",
    ),
    // ---- 底栏：遥测 ----
    ("status.uptime", "运行时长 {uptime}", "Uptime {uptime}"),
    ("status.tx", "已发送 {tx}", "Sent {tx}"),
    ("status.rx", "已接收 {rx}", "Received {rx}"),
    // ---- 主题名（config::Theme::label）----
    ("theme.dark", "深色", "Dark"),
    ("theme.light", "浅色", "Light"),
];
