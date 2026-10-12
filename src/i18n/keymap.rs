//! keymap 区域翻译表：`panel_keymap.rs`。
//!
//! 保留前缀：`keymap.*`。

use super::Entry;

pub(super) const ENTRIES: &[Entry] = &[
    // ---- 页头 / 图例 ----
    ("keymap.title", "按键映射", "Key Mapping"),
    (
        "keymap.subtitle",
        "为每个按键自定义触发行为：可设为普通键、组合键、文本片段或内置功能",
        "Customize each key: plain key, combo, text snippet, or built-in function",
    ),
    ("keymap.unset", "未设置", "Not set"),
    ("keymap.legend.synced", "已同步", "Synced"),
    ("keymap.legend.pending", "待同步", "Pending sync"),
    ("keymap.legend.selected", "已选中", "Selected"),
    ("keymap.legend.fun1", "FUN 键 1", "FUN Key 1"),
    ("keymap.legend.fun2", "FUN 键 2", "FUN Key 2"),
    (
        "keymap.legend.hover",
        "键帽左上「F1」/ 右上「F2」= 配置了对应 FUN 层行为；右下三角 = 该键本身是 FUN 键；左下琥珀小点 = 有未同步的修改",
        "Top-left \"F1\" / top-right \"F2\" = FUN layer action set; bottom-right triangle = key is a FUN key; bottom-left amber dot = unsynced changes",
    ),
    // ---- 下发 / 应用 ----
    (
        "keymap.apply.error.device",
        "设备未接受新的按键映射",
        "Device did not accept the new key mapping",
    ),
    ("keymap.apply.error.fail", "下发失败：{msg}", "Apply failed: {msg}"),
    (
        "keymap.apply.error.timeout",
        "下发超时：{error}",
        "Apply timed out: {error}",
    ),
    (
        "keymap.apply.ok",
        "已同步 {count} 项按键映射",
        "Synced {count} key mappings",
    ),
    // ---- 顶部控制条 ----
    ("keymap.profile.label", "配置：", "Profile:"),
    (
        "keymap.profile.fallback",
        "配置 {profile}",
        "Profile {profile}",
    ),
    ("keymap.rename.discard", "放弃重命名", "Discard rename"),
    (
        "keymap.rename.start",
        "为当前配置改名",
        "Rename current profile",
    ),
    ("keymap.rename.hint", "输入新名称", "Enter new name"),
    ("keymap.rename.confirm", "保存新名称", "Save new name"),
    (
        "keymap.profile.loaded",
        "已加载配置 {profile} 的 {count} 个按键映射",
        "Loaded {count} key mappings for profile {profile}",
    ),
    (
        "keymap.profile.load_failed",
        "加载配置 {profile} 失败：{error}（显示本地缓存）",
        "Failed to load profile {profile}: {error} (showing local cache)",
    ),
    ("keymap.reload", "重新加载", "Reload"),
    (
        "keymap.reload.ok",
        "已从设备同步 {count} 个按键的映射",
        "Synced mappings for {count} keys from device",
    ),
    (
        "keymap.reload.fail",
        "重新加载失败：{error}",
        "Reload failed: {error}",
    ),
    ("keymap.export", "导出", "Export"),
    ("keymap.export.ok", "已导出到 {path}", "Exported to {path}"),
    (
        "keymap.rename.sync_fail",
        "设备同步名称失败：{error}（仅本地生效）",
        "Failed to sync name to device: {error} (local only)",
    ),
    // ---- FUN 分配条 ----
    ("keymap.fun.unassigned", "未分配", "Unassigned"),
    ("keymap.fun.key_n", "键 {n}", "Key {n}"),
    ("keymap.fun.bar_title", "FUN 组合键", "FUN Combo"),
    (
        "keymap.fun.bar_hint",
        "按住 FUN 键再按其它键，触发该键的 FUN 层行为。",
        "Hold a FUN key and press another key to trigger its FUN layer action.",
    ),
    (
        "keymap.fun.bar_tip",
        "右键键帽可快速分配",
        "Right-click a keycap to assign quickly",
    ),
    // ---- 屏幕 / 键盘占位 ----
    (
        "keymap.screen.label",
        "屏幕布局占位",
        "Screen layout placeholder",
    ),
    (
        "keymap.screen.pending",
        "（待接入屏幕布局）",
        "(screen layout pending)",
    ),
    ("keymap.keyboard.label", "键盘布局", "Keyboard layout"),
    (
        "keymap.keyboard.no_profile",
        "暂无可用的按键配置",
        "No key profile available",
    ),
    (
        "keymap.keyboard.no_base_layer",
        "当前配置缺少基础按键层",
        "Current profile is missing the base key layer",
    ),
    // ---- 键帽 hover / 右键菜单 ----
    (
        "keymap.key.hover.fun_note",
        " · 本键为 FUN 键（按住不产生输出）",
        " · This key is a FUN key (no output while held)",
    ),
    (
        "keymap.key.hover.assignable",
        " · 右键可设为 FUN 键",
        " · Right-click to assign as FUN key",
    ),
    (
        "keymap.key.hover",
        "「{label}」当前行为：{behavior}{combo_info}{fun_note}",
        "{label}: {behavior}{combo_info}{fun_note}",
    ),
    (
        "keymap.ctx.current",
        "当前为 FUN 键 {side}",
        "Currently FUN key {side}",
    ),
    (
        "keymap.ctx.cancel",
        "取消 FUN 分配",
        "Remove FUN assignment",
    ),
    (
        "keymap.ctx.set_fun1",
        "设为 FUN 键 1",
        "Set as FUN key 1",
    ),
    (
        "keymap.ctx.set_fun2",
        "设为 FUN 键 2",
        "Set as FUN key 2",
    ),
    // ---- 旋钮 ----
    ("keymap.status.pending", "待下发", "Pending sync"),
    ("keymap.status.applied", "已应用", "Applied"),
    (
        "keymap.encoder.hover",
        "「{label}」旋钮 第 {row} 行 第 {col} 列 · {status} · 当前行为：{behavior}",
        "{label} knob, row {row}, col {col} · {status} · Action: {behavior}",
    ),
    // ---- Drawer ----
    ("keymap.channel.click", "单击", "Click"),
    ("keymap.drawer.save", "保存", "Save"),
    (
        "keymap.drawer.saved",
        "已为「{label}」{chan}通道设为 {action}（待同步）",
        "Set {label}: {chan}channel = {action} (pending sync)",
    ),
    ("keymap.drawer.clear", "清除", "Clear"),
    ("keymap.drawer.close", "关闭", "Close"),
    ("keymap.drawer.title", "按键功能", "Key Function"),
    (
        "keymap.drawer.empty",
        "点击键盘图上的任意按键，开始设置触发行为",
        "Click any key on the keyboard to start configuring its action",
    ),
    (
        "keymap.drawer.pos",
        "位置：第 {row} 行 第 {col} 列",
        "Position: row {row}, col {col}",
    ),
    (
        "keymap.drawer.id",
        "按键标识：{label}",
        "Key ID: {label}",
    ),
    (
        "keymap.drawer.fun_self",
        "本键是 FUN 键 {side}（按住不产生输出）",
        "This key is FUN key {side} (no output while held)",
    ),
    ("keymap.drawer.fun_cancel", "取消分配", "Unassign"),
    ("keymap.drawer.fun_role", "FUN 角色：", "FUN role:"),
    ("keymap.drawer.set_fun1", "设为 FUN1", "Set as FUN1"),
    (
        "keymap.drawer.set_fun1_hint",
        "按住该键时触发其它键的 FUN1 层行为",
        "Holding this key triggers other keys' FUN1 layer actions",
    ),
    ("keymap.drawer.set_fun2", "设为 FUN2", "Set as FUN2"),
    (
        "keymap.drawer.set_fun2_hint",
        "按住该键时触发其它键的 FUN2 层行为",
        "Holding this key triggers other keys' FUN2 layer actions",
    ),
    ("keymap.drawer.channel", "编辑通道：", "Edit channel:"),
    (
        "keymap.channel.fun1_hint",
        "按住 FUN 键 1 时按下该键触发的行为",
        "Action when pressing this key while holding FUN key 1",
    ),
    (
        "keymap.channel.fun2_hint",
        "按住 FUN 键 2 时按下该键触发的行为",
        "Action when pressing this key while holding FUN key 2",
    ),
    (
        "keymap.channel.click_hint",
        "直接按下该键触发的行为",
        "Action when pressing this key directly",
    ),
    (
        "keymap.channel.fun_key_self",
        "本键是 FUN 键（按住不产生输出），不能配置 FUN 层行为",
        "This key is a FUN key (no output while held); FUN layer actions can't be configured",
    ),
    (
        "keymap.channel.need_fun",
        "请先分配 FUN 键 {side}（键盘下方 FUN 分配条或右键键帽）",
        "Assign FUN key {side} first (FUN bar below the keyboard or right-click a keycap)",
    ),
    (
        "keymap.drawer.current",
        "当前行为：{action}",
        "Current action: {action}",
    ),
    ("keymap.drawer.trigger", "触发行为：", "Action:"),
    ("keymap.kind.none", "未绑定", "Unbound"),
    ("keymap.kind.key", "普通键", "Plain key"),
    ("keymap.kind.combo", "组合键", "Combo"),
    ("keymap.kind.chord", "多键同按", "Multi-key press"),
    ("keymap.kind.text", "文本注入", "Text injection"),
    ("keymap.kind.function", "固件功能", "Firmware function"),
    (
        "keymap.drawer.save_hint",
        "改动需点击下方「保存」按钮才会同步到设备。",
        "Click Save below to sync changes to the device.",
    ),
    // ---- 捕获条 ----
    (
        "keymap.capture.chord_title",
        "正在记录同按按键…",
        "Recording chord keys…",
    ),
    (
        "keymap.capture.chord_hint",
        "依次按下要加入的键（可含 Ctrl/Shift/Alt）· 按 Esc 结束",
        "Press keys in order to add (Ctrl/Shift/Alt allowed) · Esc to finish",
    ),
    ("keymap.capture.title", "正在记录按键…", "Recording key…"),
    ("keymap.capture.hint", "按 Esc 退出", "Esc to exit"),
    ("keymap.capture.cancel", "取消", "Cancel"),
    (
        "keymap.capture.cancel_hint",
        "退出按键捕获",
        "Exit key capture",
    ),
    (
        "keymap.capture.idle_chord_title",
        "记录同按按键",
        "Record chord keys",
    ),
    (
        "keymap.capture.idle_chord_sub",
        "按下键盘上的键，直接加入同按组合",
        "Press keys to add them to the chord",
    ),
    (
        "keymap.capture.idle_title",
        "记录一次按键",
        "Record a key press",
    ),
    (
        "keymap.capture.idle_sub",
        "把键盘按下的键作为此按键的触发行为",
        "Use the key you press as this key's action",
    ),
    // ---- 组合键 / 同按 ----
    ("keymap.combo.mods", "修饰键：", "Modifiers:"),
    ("keymap.combo.main", "主键：", "Main key:"),
    (
        "keymap.chord.label",
        "同时按下以下按键：",
        "Press these keys together:",
    ),
    (
        "keymap.chord.keep_one",
        "至少需要保留一个按键",
        "At least one key is required",
    ),
    ("keymap.chord.remove", "移除该键", "Remove this key"),
    ("keymap.chord.add", "添加按键", "Add key"),
    ("keymap.chord.pick", "选择按键…", "Choose a key…"),
    // ---- 文本注入 ----
    (
        "keymap.text.hint",
        "按键触发时输出的文本（仅限英文与符号）",
        "Text output when the key is pressed (letters and symbols only)",
    ),
    ("keymap.text.counter", "{count} / 128 字符", "{count} / 128 chars"),
    (
        "keymap.text.warn",
        "⚠ 含中文或特殊符号，设备端可能无法输出",
        "⚠ Contains Chinese or special symbols; the device may not output them",
    ),
    (
        "keymap.text.example",
        "例如：常用邮箱、口令、命令行片段",
        "e.g. email, password, or CLI snippet",
    ),
    // ---- 固件功能 ----
    ("keymap.fn.asr", "语音识别（ASR）", "Voice recognition (ASR)"),
    ("keymap.fn.page.pc", "跳转: PC状态页", "Jump: PC status page"),
    (
        "keymap.fn.page.music",
        "跳转: 音乐播放页",
        "Jump: Music player page",
    ),
    (
        "keymap.fn.page.audio",
        "跳转: 音效板页",
        "Jump: Sound pad page",
    ),
    (
        "keymap.fn.page.keymap",
        "跳转: 键映射页",
        "Jump: Keymap page",
    ),
    (
        "keymap.fn.page.setting",
        "跳转: 设置页",
        "Jump: Settings page",
    ),
    (
        "keymap.fn.page.ha",
        "跳转: 智能家居页",
        "Jump: Smart home page",
    ),
    ("keymap.fn.select", "请选择…", "Please select…"),
    ("keymap.fn.custom", "自定义: {f}", "Custom: {f}"),
    ("keymap.fn.builtin", "内置功能：", "Built-in function:"),
    (
        "keymap.fn.custom_label",
        "自定义功能串：",
        "Custom function string:",
    ),
    (
        "keymap.fn.custom_hint",
        "如：语音识别 / Ctrl+c …",
        "e.g. voice recognition / Ctrl+c …",
    ),
    (
        "keymap.fn.support_note",
        "设备支持：语音识别、二级页跳转（6个页面）、组合键串（如 Ctrl+c）、单独修饰键；其它未识别的串按键无效果，但会原样保留不丢失。",
        "Device supports: voice recognition, page jumps (6 pages), key combos (e.g. Ctrl+c), and lone modifiers; unrecognized strings do nothing but are kept as-is.",
    ),
    // ---- DiffPreviewBar ----
    ("keymap.diff.off", "关闭", "Off"),
    (
        "keymap.diff.count",
        "{count} 项待同步到设备",
        "{count} changes pending sync",
    ),
    (
        "keymap.diff.discard",
        "放弃修改（Esc）",
        "Discard changes (Esc)",
    ),
    (
        "keymap.diff.apply",
        "同步到设备（Ctrl+Enter）",
        "Sync to device (Ctrl+Enter)",
    ),
    (
        "keymap.diff.profile",
        "切换到「配置 {profile}」",
        "Switch to profile {profile}",
    ),
    (
        "keymap.diff.fun_keys",
        "FUN 键 1 → {f1}，FUN 键 2 → {f2}",
        "FUN key 1 → {f1}, FUN key 2 → {f2}",
    ),
    ("keymap.diff.tag_fun1", "FUN1 组合：", "FUN1 combo: "),
    ("keymap.diff.tag_fun2", "FUN2 组合：", "FUN2 combo: "),
    (
        "keymap.diff.binding",
        "{layer_tag}第 {row} 行 第 {col} 列：{from} → {to}",
        "{layer_tag}row {row}, col {col}: {from} → {to}",
    ),
];
