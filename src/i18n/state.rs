use super::Entry;

// state/mod.rs + link/mod.rs + link/reader.rs 的用户可见文案
// （Toast / 返回给 UI 的错误串）；日志类字符串保持中文，不进本表。
pub(super) const ENTRIES: &[Entry] = &[
    // ---- 连接 / 重连 Toast ----
    ("state.connect.connected", "已连接", "Connected"),
    (
        "state.connect.wch_bridge",
        "{name} 为 WCH USB 转串口芯片（{vid}:{pid}，CH340 系列），不支持连接，请改用设备原生 USB CDC 端口",
        "{name} is a WCH USB-to-serial chip ({vid}:{pid}, CH340 series) and is not supported. Please use the device's native USB CDC port",
    ),
    (
        "state.reconnect.unexpected_disconnect",
        "检测到 {port} 异常断开，开始自动重连…",
        "Unexpected disconnect on {port} detected, starting auto-reconnect…",
    ),
    (
        "state.reconnect.stopped_ch340",
        "已停止自动重连：{port} 为 CH340 系列芯片，请改用设备原生 USB CDC 端口",
        "Auto-reconnect stopped: {port} is a CH340-series chip. Please use the device's native USB CDC port",
    ),
    (
        "state.reconnect.auto_ok",
        "已自动重连到 {port}",
        "Auto-reconnected to {port}",
    ),
    (
        "state.reconnect.gave_up",
        "{port} 重连失败（已尝试 {attempts} 次），请检查设备后手动重连",
        "Failed to reconnect to {port} after {attempts} attempts. Check the device, then reconnect manually",
    ),
    (
        "state.reconnect.trying",
        "重连 {port} 第 {attempt} 次…",
        "Reconnecting to {port}, attempt {attempt}…",
    ),
    // ---- 通用错误（返回给 UI 的 Err 串） ----
    ("state.not_connected", "未连接", "Not connected"),
    // ---- 键映射（0x05） ----
    (
        "state.keymap.req_fail",
        "0x05 请求失败: {e}",
        "0x05 request failed: {e}",
    ),
    (
        "state.keymap.missing_keymap",
        "0x05 响应缺 keymap 字段: cmd={cmd} status={status}",
        "0x05 response missing keymap field: cmd={cmd} status={status}",
    ),
    (
        "state.keymap.parse_fail",
        "0x05 keymap 解析失败: {e}",
        "0x05 keymap parse failed: {e}",
    ),
    (
        "state.keymap.fw_upgrade_required",
        "设备固件不支持按方案拉取键映射（0x05 无 profile 回显），请升级固件",
        "Device firmware does not support fetching keymaps per profile (0x05 response has no profile echo). Please upgrade the firmware",
    ),
    // ---- Profile 名称（0x15） ----
    (
        "state.profile.name_req_fail",
        "0x15 请求失败: {e}",
        "0x15 request failed: {e}",
    ),
    (
        "state.profile.name_rejected",
        "设备未接受新名称",
        "Device did not accept the new name",
    ),
    // ---- 音效板（0x16 / 0x17） ----
    (
        "state.audio.list_req_fail",
        "0x16 list 请求失败: {e}",
        "0x16 list request failed: {e}",
    ),
    (
        "state.audio.rejected",
        "设备拒绝",
        "Request rejected by device",
    ),
    (
        "state.audio.list_missing_data",
        "0x16 list 响应缺 data 字段",
        "0x16 list response missing data field",
    ),
    (
        "state.audio.list_parse_fail",
        "0x16 list 解析失败: {e}",
        "0x16 list parse failed: {e}",
    ),
    (
        "state.audio.pads_req_fail",
        "0x17 get 请求失败: {e}",
        "0x17 get request failed: {e}",
    ),
    (
        "state.audio.pads_missing_data",
        "0x17 get 响应缺 data 字段",
        "0x17 get response missing data field",
    ),
    (
        "state.audio.pads_missing_pads",
        "0x17 get 响应缺 pads 字段",
        "0x17 get response missing pads field",
    ),
    (
        "state.audio.pads_parse_fail",
        "0x17 get 解析失败: {e}",
        "0x17 get parse failed: {e}",
    ),
    (
        "state.audio.pad_set_req_fail",
        "0x17 set 请求失败: {e}",
        "0x17 set request failed: {e}",
    ),
    (
        "state.audio.play_req_fail",
        "0x17 play 请求失败: {e}",
        "0x17 play request failed: {e}",
    ),
    (
        "state.audio.play_rejected",
        "播放被拒绝",
        "Playback rejected",
    ),
    (
        "state.audio.stop_req_fail",
        "0x17 stop 请求失败: {e}",
        "0x17 stop request failed: {e}",
    ),
    (
        "state.audio.delete_req_fail",
        "0x16 delete 请求失败: {e}",
        "0x16 delete request failed: {e}",
    ),
    (
        "state.audio.bad_name",
        "文件名不合法（a-z0-9_ + .mp3/.wav）: {name}",
        "Invalid file name (a-z0-9_ + .mp3/.wav): {name}",
    ),
    (
        "state.audio.size_out_of_range",
        "文件大小超出范围（上限 {max_kb} KB）",
        "File size out of range (max {max_kb} KB)",
    ),
    (
        "state.audio.upload_busy",
        "已有上传任务进行中",
        "An upload is already in progress",
    ),
    (
        "state.audio.upload_spawn_fail",
        "启动上传线程失败: {e}",
        "Failed to start the upload thread: {e}",
    ),
    (
        "state.audio.upload_begin_rejected",
        "begin 被拒绝: {e}",
        "begin rejected: {e}",
    ),
    (
        "state.audio.upload_begin_fail",
        "begin 请求失败: {e}",
        "begin request failed: {e}",
    ),
    (
        "state.audio.upload_block_rejected",
        "第 {index} 块被拒绝: {e}",
        "Block {index} rejected: {e}",
    ),
    (
        "state.audio.upload_block_fail",
        "第 {index} 块请求失败: {e}",
        "Block {index} request failed: {e}",
    ),
    (
        "state.audio.upload_end_rejected",
        "end 被拒绝: {e}",
        "end rejected: {e}",
    ),
    (
        "state.audio.upload_end_fail",
        "end 请求失败: {e}",
        "end request failed: {e}",
    ),
    // ---- Link 层（串口打开 / reader 错误 → Error Toast） ----
    (
        "link.open_fail",
        "打开串口失败: {e}",
        "Failed to open serial port: {e}",
    ),
    (
        "link.reader.lock_poisoned",
        "串口锁被毒化",
        "Serial port lock poisoned",
    ),
    (
        "link.reader.peer_closed",
        "串口对端关闭",
        "Serial port peer closed",
    ),
    (
        "link.reader.read_fail",
        "串口读失败: {e}",
        "Serial port read failed: {e}",
    ),
    (
        "link.reader.line_too_long",
        "单行过长，已丢弃",
        "Line too long, dropped",
    ),
];
