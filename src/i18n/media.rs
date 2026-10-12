use super::Entry;

/// 内容页翻译：前缀 `audio.` / `voice.` / `wifi.`
/// （panel_audio.rs / panel_voice.rs / panel_wifi.rs）。
pub(super) const ENTRIES: &[Entry] = &[
    // ---- Audio（panel_audio）----
    ("audio.title", "音效板", "Sound Pad"),
    (
        "audio.subtitle",
        "为 11 个矩阵键绑定音频文件，设备音效页（导航环）按键即播",
        "Bind audio files to the 11 matrix keys; press a key on the device Sound page (nav ring) to play",
    ),
    ("audio.stop", "停止播放", "Stop"),
    ("audio.stop_fail", "停止失败：{e}", "Stop failed: {e}"),
    ("audio.refresh", "刷新", "Refresh"),
    ("audio.refresh_fail", "刷新失败：{e}", "Refresh failed: {e}"),
    ("audio.storage_title", "设备存储（SPIFFS）", "Device Storage (SPIFFS)"),
    (
        "audio.storage_empty",
        "尚未读取（连接设备后自动拉取，或点击右上角「刷新」）",
        "Not read yet (auto-fetched after connecting, or click Refresh at the top right)",
    ),
    (
        "audio.storage_usage",
        "已用 {used} / {total} KB（剩余 {free} KB）",
        "{used} / {total} KB used ({free} KB free)",
    ),
    ("audio.upload_title", "上传音频文件", "Upload Audio File"),
    (
        "audio.upload_progress",
        "{name}（{sent} / {total} KB）",
        "{name} ({sent} / {total} KB)",
    ),
    ("audio.upload_cancel", "取消上传", "Cancel Upload"),
    (
        "audio.upload_hint",
        "上传在后台进行，可切换页面；取消会回滚设备端临时文件",
        "Upload runs in the background; you can switch pages. Cancelling rolls back the temporary file on the device.",
    ),
    ("audio.upload_local_file", "本地文件", "Local File"),
    (
        "audio.upload_path_hint",
        "点击「浏览…」选择 .mp3 / .wav 文件",
        "Click \"Browse…\" to pick an .mp3 / .wav file",
    ),
    ("audio.upload_browse", "浏览…", "Browse…"),
    ("audio.upload_filter", "音频文件", "Audio Files"),
    ("audio.upload_device_name", "设备端名", "Device Name"),
    (
        "audio.upload_name_hint",
        "留空: 自动从文件名生成（a-z0-9_ + .mp3/.wav）",
        "Leave empty: generated from the file name (a-z0-9_ + .mp3/.wav)",
    ),
    ("audio.upload_start", "开始上传", "Start Upload"),
    (
        "audio.upload_limit",
        "限制：单文件 ≤ {max} KB，文件名 a-z0-9_ + .mp3/.wav",
        "Limit: ≤ {max} KB per file; name must use a-z0-9_ + .mp3/.wav",
    ),
    (
        "audio.upload_need_path",
        "请先填写本地文件路径",
        "Enter the local file path first",
    ),
    ("audio.upload_read_fail", "读取文件失败：{e}", "Failed to read file: {e}"),
    ("audio.upload_empty_file", "文件为空", "File is empty"),
    (
        "audio.upload_too_large",
        "文件 {size} KB 超过上限 {max} KB",
        "File is {size} KB, over the {max} KB limit",
    ),
    (
        "audio.upload_name_gen_fail",
        "无法从文件名「{name}」生成合法设备端名，请手动填写",
        "Cannot derive a valid device name from \"{name}\"; enter one manually",
    ),
    (
        "audio.upload_name_invalid",
        "设备端名不合法（a-z0-9_ + .mp3/.wav）：{name}",
        "Invalid device name (a-z0-9_ + .mp3/.wav): {name}",
    ),
    (
        "audio.upload_no_space",
        "设备剩余空间不足（剩 {free} KB，需要 {size} KB + {headroom} KB 预留）",
        "Not enough device space ({free} KB free; needs {size} KB + {headroom} KB reserved)",
    ),
    (
        "audio.upload_not_started",
        "上传未开始：{e}",
        "Upload did not start: {e}",
    ),
    (
        "audio.upload_cancelled",
        "上传 {name} 已取消",
        "Upload of {name} cancelled",
    ),
    (
        "audio.upload_fail",
        "上传 {name} 失败：{e}",
        "Failed to upload {name}: {e}",
    ),
    (
        "audio.upload_done",
        "上传 {name} 完成（{size} KB）",
        "Uploaded {name} ({size} KB)",
    ),
    (
        "audio.files_title",
        "音频文件（{count}）",
        "Audio Files ({count})",
    ),
    (
        "audio.files_header",
        "名称 · 大小 · 试播 / 删除",
        "Name · Size · Preview / Delete",
    ),
    ("audio.files_empty", "暂无文件", "No files yet"),
    ("audio.files_delete", "删除", "Delete"),
    ("audio.files_delete_fail", "删除失败：{e}", "Delete failed: {e}"),
    (
        "audio.files_deleted",
        "已删除 {name}，引用它的键位已清空",
        "Deleted {name}; keys referencing it were cleared",
    ),
    ("audio.files_preview", "试播", "Preview"),
    ("audio.preview_fail", "试播失败：{e}", "Preview failed: {e}"),
    (
        "audio.pads_title",
        "键位绑定（K1 ~ K11）",
        "Key Bindings (K1 ~ K11)",
    ),
    (
        "audio.pads_no_files",
        "设备上还没有音频文件，先上传后再绑定",
        "No audio files on the device yet; upload some first",
    ),
    (
        "audio.pads_hint",
        "绑定即改即发；设备音效页按键播放，音量跟随设备「音频」设置",
        "Changes apply immediately; keys play on the device Sound page, volume follows the device Audio settings",
    ),
    ("audio.pads_unbound", "（未绑定）", "(unbound)"),
    (
        "audio.pads_clear_fail",
        "清除绑定失败：{e}",
        "Failed to clear binding: {e}",
    ),
    ("audio.pads_bind_fail", "绑定失败：{e}", "Binding failed: {e}"),
    (
        "audio.pads_key_unbound",
        "K{key} 未绑定文件",
        "K{key} has no file bound",
    ),
    ("audio.pads_clear", "清除", "Clear"),
    // ---- Voice（panel_voice）----
    ("voice.title", "语音识别", "Voice Recognition"),
    (
        "voice.subtitle",
        "腾讯云一句话识别服务（采样率 16 kHz，中文）",
        "Tencent Cloud sentence recognition (16 kHz sample rate, Chinese)",
    ),
    ("voice.enable", "启用语音识别", "Enable voice recognition"),
    ("voice.trigger_key", "触发键编号", "Trigger key number"),
    (
        "voice.max_record_ms",
        "最长录音时长（毫秒）",
        "Max recording duration (ms)",
    ),
    (
        "voice.auto_enter",
        "按下触发键后立即进入识别",
        "Start recognition right after pressing the trigger key",
    ),
    (
        "voice.api_config",
        "腾讯云 API 配置",
        "Tencent Cloud API Config",
    ),
    (
        "voice.secret_id",
        "SecretId（最多 64 个字符）",
        "SecretId (max 64 characters)",
    ),
    (
        "voice.secret_key",
        "SecretKey（最多 64 个字符）",
        "SecretKey (max 64 characters)",
    ),
    // ---- WiFi（panel_wifi）----
    ("wifi.title", "无线网络", "Wi-Fi"),
    (
        "wifi.reboot_hint",
        "修改后下次重启生效",
        "Takes effect after the next restart",
    ),
    ("wifi.enable", "启用无线网络", "Enable Wi-Fi"),
    (
        "wifi.connect_channel",
        "启用与电脑的连接通道",
        "Enable the connection channel to the computer",
    ),
    (
        "wifi.ssid_label",
        "网络名称（最多 32 个字符）",
        "Network name (max 32 characters)",
    ),
    ("wifi.ssid_hint", "请输入 WiFi 名称…", "Enter Wi-Fi name…"),
    (
        "wifi.password_label",
        "网络密码（最多 64 个字符）",
        "Network password (max 64 characters)",
    ),
    ("wifi.password_hint", "请输入 WiFi 密码…", "Enter Wi-Fi password…"),
];
