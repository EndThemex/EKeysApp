//! settings 区域翻译表：`panel_settings.rs`（设备设置页）。
//!
//! 保留前缀：`settings.*`。

use super::Entry;

pub(super) const ENTRIES: &[Entry] = &[
    // ---- Tab 标签 ----
    ("settings.tab.display", "显示", "Display"),
    ("settings.tab.keyboard", "键盘", "Keyboard"),
    ("settings.tab.audio", "音频", "Audio"),
    ("settings.tab.power", "电源", "Power"),
    ("settings.tab.pc_status", "PC 状态", "PC Status"),
    ("settings.tab.firmware", "固件升级", "Firmware Update"),
    // ---- 页头 ----
    ("settings.page.title", "设备", "Device"),
    (
        "settings.page.loading",
        "正在读取设备配置…（若长时间无变化，请查看日志或点击顶栏「刷新」）",
        "Reading device configuration… (if nothing changes for a while, check the Log or click Refresh in the top bar)",
    ),
    // ---- 通用 ----
    ("settings.common.unknown", "（未知）", "(Unknown)"),
    ("settings.common.not_captured", "（未采集）", "(not sampled)"),
    ("settings.common.busy", "正在处理中", "Working…"),
    ("settings.common.device_offline", "设备未连接", "Device not connected"),
    (
        "settings.common.waiting_confirm",
        "等待设备确认…",
        "Waiting for device confirmation…",
    ),
    (
        "settings.common.read_file_fail",
        "读取文件失败：{error}",
        "Failed to read file: {error}",
    ),
    (
        "settings.common.request_build_fail",
        "请求数据生成失败",
        "Failed to build request payload",
    ),
    ("settings.common.browse", "浏览…", "Browse…"),
    // ---- 显示 tab ----
    ("settings.display.theme", "屏幕主题", "Screen theme"),
    ("settings.display.theme.dark", "深色", "Dark"),
    ("settings.display.theme.light", "浅色", "Light"),
    (
        "settings.display.backlight",
        "屏幕背光（范围 5~100）",
        "Backlight (range 5-100)",
    ),
    // ---- 键盘 tab ----
    ("settings.keyboard.mode", "连接方式", "Connection mode"),
    ("settings.keyboard.mode.usb", "USB 有线", "USB wired"),
    ("settings.keyboard.mode.bluetooth", "蓝牙", "Bluetooth"),
    ("settings.keyboard.mode.wireless_24g", "2.4G 无线", "2.4G wireless"),
    (
        "settings.keyboard.mode.unknown",
        "未知（{n}）",
        "Unknown ({n})",
    ),
    (
        "settings.keyboard.profile.active",
        "当前按键配置",
        "Active profile",
    ),
    (
        "settings.keyboard.profile.switch",
        "切换按键配置（共 8 组）",
        "Switch profile (8 profiles)",
    ),
    ("settings.keyboard.profile.name", "配置 {n}", "Profile {n}"),
    (
        "settings.keyboard.icon.custom",
        "已设置自定义图标",
        "Custom icon set",
    ),
    (
        "settings.keyboard.icon.path",
        "按键配置图标（建议 PNG，≤ 48×48，文件 ≤ 2 KB）",
        "Profile icon (PNG recommended, ≤ 48×48, ≤ 2 KB)",
    ),
    (
        "settings.keyboard.icon.path_hint",
        "选择 PNG 图片文件…",
        "Choose a PNG image file…",
    ),
    ("settings.keyboard.icon.filter", "PNG 图片", "PNG images"),
    ("settings.keyboard.icon.upload", "上传", "Upload"),
    ("settings.keyboard.icon.clear", "清除图标", "Clear icon"),
    // ---- 图标上传 / 清除 ----
    (
        "settings.icon.invalid_png",
        "所选文件不是有效的 PNG 图片",
        "The selected file is not a valid PNG image",
    ),
    (
        "settings.icon.parse_fail",
        "无法解析 PNG 图片：{error}",
        "Failed to decode PNG image: {error}",
    ),
    (
        "settings.icon.too_large",
        "图片过大（编码后 {bytes} 字节），请压缩到 48×48 以下",
        "Image too large ({bytes} bytes encoded); shrink it below 48×48",
    ),
    (
        "settings.icon.upload_done",
        "配置 {n} 的图标已更新",
        "Icon updated for profile {n}",
    ),
    (
        "settings.icon.device_rejected",
        "设备未接受新图标",
        "Device rejected the new icon",
    ),
    ("settings.icon.upload_fail", "上传失败：{error}", "Upload failed: {error}"),
    (
        "settings.icon.upload_timeout",
        "上传超时：{error}",
        "Upload timed out: {error}",
    ),
    (
        "settings.icon.clear_done",
        "配置 {n} 的图标已清除",
        "Icon cleared for profile {n}",
    ),
    (
        "settings.icon.clear_rejected",
        "设备未接受清除请求",
        "Device rejected the clear request",
    ),
    ("settings.icon.clear_fail", "清除失败：{error}", "Clear failed: {error}"),
    (
        "settings.icon.clear_timeout",
        "清除超时：{error}",
        "Clear timed out: {error}",
    ),
    // ---- 音频 tab ----
    (
        "settings.audio.volume",
        "音量（范围 0~100）",
        "Volume (range 0-100)",
    ),
    ("settings.audio.enable", "启用音频输出", "Enable audio output"),
    // ---- 电源 tab ----
    ("settings.power.policy", "节能策略", "Power policy"),
    ("settings.power.mode.performance", "性能优先", "Performance"),
    ("settings.power.mode.saving", "省电优先", "Power saving"),
    ("settings.power.mode.unknown", "模式 {n}", "Mode {n}"),
    // ---- PC 状态 tab ----
    ("settings.pc.group.push", "主机状态推送", "Host status push"),
    (
        "settings.pc.push.desc",
        "启用后，应用会定时把大写锁定、数字锁定、滚动锁定与网络连通状态推送给设备，设备屏幕上的指示灯即可随之刷新。默认关闭，避免无意间共享主机状态。",
        "When enabled, the app periodically pushes Caps Lock, Num Lock, Scroll Lock and network connectivity to the device, so the indicators on its screen stay in sync. Off by default to avoid sharing host status unintentionally.",
    ),
    (
        "settings.pc.enable",
        "向设备同步主机状态",
        "Sync host status to device",
    ),
    (
        "settings.pc.enable.hint",
        "勾选后立即生效；启用时只在「在线」状态下发送，频率约每秒一次；退出应用时会自动记住选择。",
        "Takes effect immediately; while enabled, status is sent only while online, about once per second. Your choice is remembered when the app exits.",
    ),
    (
        "settings.pc.toast.on",
        "已开始向设备推送主机状态",
        "Started pushing host status to the device",
    ),
    (
        "settings.pc.toast.off",
        "已停止向设备推送主机状态",
        "Stopped pushing host status to the device",
    ),
    ("settings.pc.state.disabled", "未启用", "Not enabled"),
    (
        "settings.pc.state.waiting",
        "等待设备连接…",
        "Waiting for device connection…",
    ),
    (
        "settings.pc.state.pushing",
        "正在推送（每秒一次）",
        "Pushing (once per second)",
    ),
    ("settings.pc.state.label", "当前状态：{status}", "Current status: {status}"),
    ("settings.pc.group.live", "主机实时状态", "Live host status"),
    ("settings.pc.empty", "（尚未采集）", "(not sampled yet)"),
    ("settings.pc.caps", "大写锁定 Caps", "Caps Lock"),
    ("settings.pc.num", "数字锁定 Num", "Num Lock"),
    ("settings.pc.scroll", "滚动锁定 Scroll", "Scroll Lock"),
    ("settings.pc.network", "网络连通", "Network"),
    ("settings.pc.lock.on", "已开启", "On"),
    ("settings.pc.lock.off", "未开启", "Off"),
    ("settings.pc.network.connected", "已连接", "Connected"),
    ("settings.pc.network.disconnected", "未连接", "Not connected"),
    // ---- 固件升级 tab：信息区 ----
    ("settings.fw.group.info", "当前固件", "Current firmware"),
    ("settings.fw.info.name", "设备名称", "Device name"),
    ("settings.fw.info.version", "固件版本", "Firmware version"),
    // ---- 固件升级 tab：OTA ----
    ("settings.fw.ota.group", "固件升级（OTA）", "Firmware Update (OTA)"),
    (
        "settings.fw.ota.desc",
        "选择固件文件后，应用会在本机临时开启一个 HTTP 服务，设备通过 Wi-Fi 下载并自动校验 MD5。升级期间请保持设备供电、不要断开 Wi-Fi；成功后设备会自动重启进入新固件。",
        "After you choose a firmware file, the app starts a temporary local HTTP server; the device downloads it over Wi-Fi and verifies the MD5 automatically. Keep the device powered and Wi-Fi connected during the upgrade; on success it restarts into the new firmware.",
    ),
    (
        "settings.fw.ota.pick_hint",
        "点击「浏览…」选择 .bin 固件文件",
        "Click “Browse…” to pick a .bin firmware file",
    ),
    ("settings.fw.ota.filter", "固件文件", "Firmware files"),
    (
        "settings.fw.ota.size",
        "大小：{bytes} 字节（{kb} KB）",
        "Size: {bytes} bytes ({kb} KB)",
    ),
    ("settings.fw.ota.md5", "MD5：{md5}", "MD5: {md5}"),
    ("settings.fw.ota.start", "开始升级", "Start upgrade"),
    ("settings.fw.ota.cancel", "取消升级", "Cancel upgrade"),
    (
        "settings.fw.ota.cancelled",
        "已取消：本地下载服务已关闭",
        "Cancelled: local download server stopped",
    ),
    (
        "settings.fw.ota.pick_first",
        "请先选择固件文件",
        "Select a firmware file first",
    ),
    (
        "settings.fw.ota.offline_hint",
        "提示：设备未连接。升级前请先在顶栏连接设备，并确认设备已加入与电脑相同的 Wi-Fi 网络（Wi-Fi 页可配置）。",
        "Note: the device is not connected. Connect it from the top bar first, and make sure it joined the same Wi-Fi network as this PC (configurable on the Wi-Fi page).",
    ),
    (
        "settings.fw.ota.confirmed",
        "设备已确认，正在从本机下载固件（{url}）…下载并校验通过后自动重启，全程约 1~2 分钟",
        "Device confirmed; downloading the firmware from this computer ({url})… It restarts automatically once the download and checksum pass, taking about 1-2 minutes",
    ),
    (
        "settings.fw.ota.triggered",
        "OTA 已触发，设备开始下载固件",
        "OTA triggered; the device started downloading the firmware",
    ),
    (
        "settings.fw.ota.rejected",
        "设备拒绝升级",
        "Device rejected the upgrade",
    ),
    (
        "settings.fw.ota.fail",
        "升级失败：{error}",
        "Upgrade failed: {error}",
    ),
    (
        "settings.fw.ota.request_fail",
        "升级请求失败：{error}",
        "Upgrade request failed: {error}",
    ),
    (
        "settings.fw.ota.request_sent",
        "已发送升级请求，等待设备确认…",
        "Upgrade request sent; waiting for device confirmation…",
    ),
    (
        "settings.fw.ota.server_fail",
        "启动本地下载服务失败：{error}",
        "Failed to start the local download server: {error}",
    ),
    (
        "settings.fw.ota.no_ip",
        "无法获取本机局域网 IP，请确认电脑已联网",
        "Cannot detect the local LAN IP; make sure this computer is online",
    ),
    // ---- 固件升级 tab：烧录模式 ----
    ("settings.fw.download.enter", "进入烧录模式", "Enter download mode"),
    (
        "settings.fw.download.desc",
        "设备将立即复位进入 USB 下载模式（无需按 BOOT 键），当前连接会断开，应用在烧录完成前不会自动重连，以免占用串口。烧录完成后请在顶栏重新连接设备。",
        "The device resets immediately into USB download mode (no BOOT button needed) and the current connection drops. The app will not auto-reconnect until flashing finishes, keeping the serial port free. Reconnect from the top bar after flashing.",
    ),
    (
        "settings.fw.download.request_sent",
        "已发送进入烧录模式请求，等待设备确认…",
        "Download mode request sent; waiting for device confirmation…",
    ),
    (
        "settings.fw.download.done",
        "设备已重启进入烧录模式，连接已断开。请使用 idf.py flash / esptool 烧录；烧录完成后在顶栏重新连接设备。",
        "The device rebooted into download mode and the connection was closed. Flash it with idf.py flash / esptool, then reconnect from the top bar.",
    ),
    (
        "settings.fw.download.toast_ok",
        "设备已进入烧录模式",
        "Device entered download mode",
    ),
    (
        "settings.fw.download.rejected",
        "设备拒绝进入烧录模式",
        "Device refused to enter download mode",
    ),
    (
        "settings.fw.download.fail",
        "进入烧录模式失败：{error}",
        "Failed to enter download mode: {error}",
    ),
    (
        "settings.fw.download.no_confirm",
        "未收到设备确认（响应可能在复位前丢失）。设备大概率已进入烧录模式，可直接尝试烧录；若未进入，请手动重连后重试。",
        "No confirmation from the device (the reply may have been lost during reset). It has most likely entered download mode — try flashing; if not, reconnect manually and retry.",
    ),
    (
        "settings.fw.download.no_confirm_toast",
        "未收到设备确认，可能已进入烧录模式",
        "No confirmation from the device; it may already be in download mode",
    ),
    // ---- 固件升级 tab：文件选择 ----
    (
        "settings.fw.pick.invalid",
        "所选文件不是有效的 ESP32 固件镜像（首字节非 0xE9）",
        "The selected file is not a valid ESP32 firmware image (first byte is not 0xE9)",
    ),
    ("settings.fw.pick.empty", "固件文件为空", "Firmware file is empty"),
];
