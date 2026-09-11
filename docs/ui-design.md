# EKeysApp 交互页面设计文档（v0.1）

> 对接协议：[`desktop-app-protocol.md`](./desktop-app-protocol.md)
> 工程结构：[`project-structure-plan.md`](./project-structure-plan.md)
> GUI 框架：eframe 0.33 + egui

本文档定义桌面 App 的**页面结构、组件构成、用户交互流程**。阶段 04 只落地"已实现"页面；其余页面以占位形式存在，阶段 05/06 接入。

---

## 1. 设计原则

1. **状态优先**：连接状态、推送到达等"非用户主动"事件必须**主动反馈**到 UI（toast/角标/状态灯），而不是等用户去查日志。
2. **最小主路径**：用户从启动到完成一次配置（修改 + 持久化）的点击次数不超过 4 次。
3. **可逆**：所有 SET 操作先在本地做"待下发"编辑，预览面板预览 → 用户点"应用"才真正下发；提供"放弃修改"按钮。
4. **日志分层**：协议帧、固件日志、应用日志分别着色（绿/灰/蓝），便于排查。
5. **协议前向兼容**：UI 对未知字段保持"忽略 + 日志"，绝不崩溃（见协议 §8.4）。
6. **键盘友好**：所有面板支持 `Tab` 切换焦点；常用按钮提供快捷键（连接/刷新 `F5`、应用 `Ctrl+Enter`）。

---

## 2. 全局布局

```
┌─────────────────────────────────────────────────────────────────────┐
│ TopBar  [● Online · COM5 · 115200]  [⟳ Refresh]  [� Local Settings] │
├──────────────┬──────────────────────────────────────────────────────┤
│              │                                                      │
│  SideNav     │                                                      │
│  ──────────  │                  ContentArea                         │
│  🔌 Connect  │  (根据 SideNav 选中渲染不同 Panel)                   │
│  🎛 Settings │                                                      │
│  🎹 Keymap   │                                                      │
│  💡 Lighting │                                                      │
|  📶 WiFi     │                                                      │
│  🎤 Voice    │                                                      │
│  🎵 Audio    │                                                      │
│  📜 Log      │                                                      │
│  ℹ  About    │                                                      │
│              │                                                      │
├──────────────┴──────────────────────────────────────────────────────┤
│ StatusBar  [⏱ Uptime 12:34]  [📤 12 sent]  [📥 11 recv]  [❤️ hb 1s] │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.1 TopBar（顶部状态条）

| 元素             | 说明                                                      | 交互                       |
| ---------------- | --------------------------------------------------------- | -------------------------- |
| 连接状态指示灯 ● | 灰=未连接 / 黄=连接中 / 绿=Online / 红=Error / 红=Offline | 点击 → 切换到 Connect 页面 |
| 端口与波特率     | `COM5 · 115200`                                           | 点击 → 切换到 Connect 页面 |
| ⟳ Refresh 按钮   | 主动发 `CMD_CONFIG_GET`（0x07）重新拉取全量快照           | 离线时禁用                 |
| ⚙ Local Settings | 打开本地 App 偏好弹窗（语言、主题、最近端口）             | —                          |

### 2.2 SideNav（左侧导航）

固定宽 200px，按功能分组。选中项高亮，右箭头 → 表示子项。

```
🔌 连接                  → Connect Panel
🎛 设备设置              → Settings Panel（Tab 子页）
   ├ 🖥 显示
   ├ ⌨ 键盘
   ├ � 音频
   └ ⚡ 电源
🎹 键映射 (灰，阶段 05)  → Keymap Panel（占位）
💡 灯效   (灰，阶段 06)  → Lighting Panel（占位）
📶 WiFi   (灰，阶段 06)  → WiFi Panel（占位）
🎤 语音   (灰，阶段 06)  → Voice Panel（占位）
🎵 音效                  → Audio Panel（上传音频 + 11 键绑定 + 试播）
📜 日志                  → Log Panel
ℹ 关于                   → About Panel
```

**灰显规则**：对应字段尚未在固件生效（见协议 §3"生效阶段"列），点击 → toast"该功能将在固件阶段 XX 启用"。

### 2.3 StatusBar（底部状态条）

| 元素        | 来源                                                |
| ----------- | --------------------------------------------------- |
| 设备 Uptime | 最近一次心跳响应 `data.timestamp`，格式化为 `mm:ss` |
| 📤 已发送   | `transport::manager` 计数器                         |
| 📥 已接收   | `transport::manager` 计数器                         |
| ❤️ 心跳间隔 | Heartbeat 配置（默认 1s）                           |

---

## 3. 页面清单

| ID  | 页面     | 阶段 | 主组件                                            |
| --- | -------- | ---- | ------------------------------------------------- |
| P1  | Connect  | 04   | `PortSelector` + `ConnectionControls`             |
| P2  | Settings | 04   | `SettingsTabs` + `FieldEditor` + `DiffPreviewBar` |
| P3  | Keymap   | 05   | 占位                                              |
| P4  | Lighting | 06   | 占位                                              |
| P5  | WiFi     | 06   | 占位                                              |
| P6  | Voice    | 06   | 占位                                              |
| P7  | Log      | 04   | `LogViewer`                                       |
| P8  | About    | 04   | `AboutCard`                                       |
| P9  | Audio    | 09   | `StorageCard` + `UploadCard` + `FileList` + `PadBindings`（见 §3.1） |

### 3.1 P9 — Audio（音效板）页面

数据源：`AudioPadData`（`state::audio`），连接后 `auto_get` 拉取、进入页面边沿补拉，上传进度由后台线程写。

| 区块          | 内容                                                                 |
| ------------- | -------------------------------------------------------------------- |
| 顶部操作行    | 「刷新」（0x16 list + 0x17 get）、「停止播放」（0x17 stop）          |
| 设备存储卡片  | used / total 进度条 + 剩余空间（SPIFFS）                             |
| 上传卡片      | 本地路径 TextEdit + 设备端名 TextEdit（空 = 自动生成）+「开始上传」；上传中显示进度条 +「取消上传」 |
| 音频文件列表  | 文件名 + 大小 + 「试播」（0x17 play file）+「删除」（0x16 delete）   |
| 键位绑定      | K1~K11 每行 ComboBox（未绑定 + 文件列表）+ 「试播」「清除」（0x17 set） |

约束提示：单文件 ≤ 2MB、设备端名 `a-z0-9_` + `.mp3/.wav`、`begin` 预留 64KB 空间。上传在后台线程进行（可切页面），失败 / 取消自动 abort 回滚，完成 Toast 由 `panel_audio::tick_upload` 统一收尾。

---

## 4. P1 — Connect 页面
### 4.1 布局

```
┌─ Connection ──────────────────────────────────────────────┐
│                                                            │
│   端口 (Port)                                              │
│   [ COM5 (EKeys)  ▾ ]    [�]  (扫描按钮)                  │
│                                                            │
│   波特率 (Baud)                                            │
│   [ 115200 ▾ ]   (协议固定；下拉锁死为 115200)             │
│                                                            │
│   设备信息                                                 │
│   ┌──────────────────────────────────────────────┐        │
│   │ VID: 0x303A   PID: 0x4001   Manufacturer: Espressif │  │
│   │ Serial:  AA:BB:CC:DD:EE:FF                     │        │
│   └──────────────────────────────────────────────┘        │
│                                                            │
│   [ ● Connect ]   [ ■ Disconnect ]   (状态决定哪个启用)     │
│                                                            │
│   ────────────────────────────────────────────────         │
│   状态:  ● Online  (上次握手: 2s 前)                       │
│   最近错误: (无)                                           │
│                                                            │
│   高级: [✓] 启动时自动连接上次端口                          │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### 4.2 组件

| 组件                 | 说明                                                                   |
| -------------------- | ---------------------------------------------------------------------- |
| `PortSelector`       | `egui::ComboBox`：枚举端口 + 自动过滤 VID 0x303A；显示 `端口名 (描述)` |
| `ScanButton`         | 点击重新枚举（热插拔时使用）                                           |
| `ConnectionControls` | `Connect`/`Disconnect` 按钮（按 `ConnectionState` 互斥启用）           |
| `DeviceInfoCard`     | 显示选中端口的元信息（即使未连接也能展示 USB 描述符）                  |
| `StatusBlock`        | 状态指示 + 最后心跳时间 + 最近错误                                     |
| `AutoConnectToggle`  | "启动时自动连接"开关，持久化到本地 App 配置                            |

### 4.3 交互流程

```text
[启动] ─枚举─▶ 没有可用端口 ───▶ Toast "未检测到 EKeys 设备，请插入后点击扫描"
   │
   ├─枚举─▶ 有可用端口 ─▶ 默认选中"上次端口"（如有）或第一个
   │
   ▼
[点 Connect]
   │ ① 打开串口
   │ ② 发 0x07 (CMD_CONFIG_GET) 拉初始快照
   │ ③ 启动心跳
   │ ④ 成功 ─▶ 状态 → Online，跳到 Settings
   │    失败 ─▶ 状态 → Error，展示 error 文本；按钮可重试
```

**关键事件**：

- 扫描时拔插 → `transport::manager` 上抛 `DeviceListChanged` → TopBar 指示灯闪黄
- 连接中拔线 → 状态变红 Error；3s 后进入自动重连（指数退避 1s→2s→4s→5s）
- 收到 `0x87 seq=0` 主动推送 → 若当前不在 Settings 页，状态角标显示 `●`

---

## 5. P2 — Settings 页面（核心）

### 5.1 布局

```
┌─ Settings ────────────────────────────────────────────────────────┐
│  Tabs:  [🖥 Display] [⌨ Keyboard] [🔊 Audio] [⚡ Power]           │
├───────────────────────────────────────────────────────────────────┤
│                                                                    │
│   (根据 Tab 渲染对应的字段组)                                     │
│                                                                    │
│   ┌─ Display ────────────────────────────────────────────┐         │
│   │  TFT 主题      [ 深色  ▾ ]    (id=12)               │         │
│   │  TFT 背光      [====●====] 50  (5~100, id=13)        │         │
│   │  ⚠ 修改未下发                                          │         │
│   └─────────────────────────────────────────────────────�         │
│                                                                    │
├──────────────────────────────────────────────────────────────────┤
│ DiffPreviewBar                                                     │
│   待下发 3 项: tft_brightness:50, tft_theme:0, ...   [应用][放弃] │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Tab → 字段分组（与协议 §3 对齐）

| Tab        | 字段（id = 协议字段名）                                                                                                                                        | 组件                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| � Display  | `tft_theme`, `tft_brightness`                                                                                                                                  | `ComboBox`, `Slider`                       |
| ⌨ Keyboard | `work_mode`, `active_keymap_profile`, `active_profile_name`（只读）, `active_profile_has_custom_icon`（只读）                                                  | `RadioGroup`, `ComboBox`, `Label`, `Badge` |
| 🔊 Audio   | `device_volume`, `audio_enable`                                                                                                                                | `Slider`, `Switch`                         |
| � Power    | `power_mode`                                                                                                                                                   | `ComboBox`                                 |
| 📶 WiFi    | `wifi_switch`, `connect_host`, `wifi_ssid`, `wifi_password`                                                                                                    | 阶段 06                                    |
| 🎤 Voice   | `voice_enable`, `voice_trigger_key`, `voice_max_record_ms`, `voice_auto_enter`, `voice_cuid`, `voice_tencent_secret_id`, `voice_tencent_secret_key` | 阶段 06（阶段 08 迁移腾讯云）              |
| 🖥 PC      | `pc_status_mask`                                                                                                                                               | 阶段 05（位掩码编辑器）                    |

> **组件选型原则**：枚举/小范围 int → `ComboBox`；0~100 连续值 → `Slider`；位掩码 → 自定义 `BitMaskField`；字符串 → `TextEdit` (单行)；只读字段 → `Label` + 复制按钮。

### 5.3 核心交互：编辑 → 预览 → 应用

这是设置面板最关键的交互，单独说明。

```text
用户修改字段
   │
   ▼
本地内存标记"待下发"（不直接发 SET）
   │
   ▼
DiffPreviewBar 实时更新：
   "待下发 N 项:  tft_brightness: 50→80,  work_mode: USB→BLE"
   [应用 (Ctrl+Enter)]  [放弃 (Esc)]
   │
   ▼
点 [应用]
   │
   ├─ 校验（协议 §4.1）：tft_brightness 钳到 5~100；work_mode 限 0~2；字符串截断
   │
   ▼
发 0x08 (CMD_CONFIG_SET) 增量字段
   │
   ├─ 等 0x88 响应
   │     ├─ status=0 成功 ─▶ 等待 0x87 seq=0 全量推送刷新本地快照
   │     └─ status=1 失败 ─▶ Toast 显示 error 字段；DiffPreviewBar 保留待下发项
   │
   ▼
本地快照更新（来自 0x87 推送）→ UI 重新渲染 → DiffPreviewBar 清空
```

**关键细节**：

- 用户连续修改 → 只在 `[应用]` 时一次性下发；避免抖动导致设备频繁持久化
- 收到 `0x87` 推送时若 DiffPreviewBar 非空 → **不覆盖用户未应用的值**；推送仅刷新未修改的字段（精细合并：推送值 vs 本地草稿，键级比较）
- 字段值未变化（如用户拖滑块到原值）→ 不计入"待下发"，按钮禁用

### 5.4 副作用提示

协议 §4.2 列出字段变更会触发设备侧动作，UI 需要明确告知用户：

| 字段                    | UI 行为                                                           |
| ----------------------- | ----------------------------------------------------------------- |
| `tft_brightness`        | Toast "已应用，屏幕背光已调整"（绿色，2s 自动消失）               |
| `work_mode`             | Confirm Dialog "切换工作模式将重建键盘实例，是否继续？"（防误触） |
| `active_keymap_profile` | Toast "已切换到 Profile X"                                        |
| `wifi_*`                | 阶段 06：Toast "WiFi 配置已保存，将在下次重启生效"                |

### 5.5 组件

| 组件             | 职责                                           |
| ---------------- | ---------------------------------------------- |
| `SettingsTabs`   | Tab 切换；当前 Tab 持久化到本地 App 配置       |
| `FieldGroup`     | 一组字段的容器，含标题 + 折叠按钮              |
| `FieldEditor`    | 统一封装"标签 + 控件 + 校验提示 + 待下发标记"  |
| `DiffPreviewBar` | 底部固定条，显示变更列表 + 应用/放弃按钮       |
| `ConfirmDialog`  | 危险操作的二次确认（工作模式切换、恢复出厂等） |
| `Toast`          | 应用反馈（成功/警告/错误），队列展示           |

---

## 6. P7 — Log 页面

### 6.1 布局

```
┌─ Log ─────────────────────────────────────────────────────┐
│  Filter:  [✓ 协议帧] [✓ 固件日志] [✓ 应用日志]          │
│           级别: [All ▾]    搜索: [____________] [清空] │
├──────────────────────────────────────────────────────────┤
│  12:34:56.789  ▶ {"cmd":7,"seq":1}                       │
│  12:34:56.812  ◀ {"cmd":135,"seq":1,"status":0,...}     │
│  12:34:57.001  ℹ [I]MAIN: usb_cdc ready                  │
│  12:34:58.100  ⚠ [W]CFG: unknown field "foo" ignored    │
│  ...                                                     │
└──────────────────────────────────────────────────────────┘
```

### 6.2 组件

| 组件           | 职责                                                         |
| -------------- | ------------------------------------------------------------ |
| `LogFilter`    | 多选过滤（协议/固件/应用）+ 级别筛选 + 文本搜索              |
| `LogViewer`    | 滚动虚拟列表（egui `ScrollArea` + 按需构造 row）；按类型着色 |
| `LogRow`       | 单行：时间戳 + 方向箭头 + 内容（协议帧可点击展开 JSON 树）   |
| `ClearButton`  | 清空日志缓冲                                                 |
| `ExportButton` | 导出当前过滤结果为 `.log` 文件（阶段 04 可选）               |

### 6.3 配色约定

| 类型       | 颜色                          |
| ---------- | ----------------------------- |
| 协议发出 ▶ | 浅蓝                          |
| 协议接收 ◀ | 浅绿                          |
| 固件日志   | 浅灰（按级别再分 I/W/E 三档） |
| 应用日志   | 白色（按级别 `tracing` 着色） |

---

## 7. P8 — About 页面

```
┌─ About ─────────────────────────────────────────────────┐
│                                                          │
│     EKeys Desktop App                                    │
│     Version  0.1.0                                       │
│     Commit   abc1234                                     │
│     Built    2026-08-31                                  │
│                                                          │
│     固件协议版本  v0.1（阶段 04）                         │
│     对接设备      ESP32-S3 USB CDC                       │
│                                                          │
│     [查看协议文档]  [打开项目主页]  [检查更新]           │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 8. 跨页面交互与事件流

### 8.1 全局事件总线（应用层）

```
Transport (reader thread)
    │
    ├─▶ ConnectionState 变化        ─▶ TopBar / Connect 页
    ├─▶ Heartbeat timeout           ─▶ TopBar / StatusBar
    ├─▶ 0x87 seq=0 推送             ─▶ Settings 页 / DiffPreviewBar
    ├─▶ 0x0a 心跳响应               ─▶ StatusBar (Uptime)
    ├─▶ Firmware log line           ─▶ Log 页
    └─▶ 应用错误 (打开失败/编码失败)  ─▶ Toast + Log 页
```

事件通过 `AppHandle`（`Arc<Mutex<...>>` + `mpsc::Receiver<AppEvent>`）派发；UI 在每帧 `update()` 开头 drain 一次。

### 8.2 关键交互序列

**场景 A：用户首次连接并调整亮度**

```text
1. 启动 App → 启动页（Connect）
2. 选中 COM5 → 点 [Connect]
3. 自动跳转到 Settings / Display Tab（首次连接体验）
4. 拖动 TFT 背光滑块到 80
5. DiffPreviewBar 显示 "待下发 1 项"
6. 点 [应用] 或 Ctrl+Enter
7. Toast "已应用，屏幕背光已调整"
8. 设备推送 0x87 刷新本地快照
9. DiffPreviewBar 清空
```

**场景 B：连接中拔线**

```text
1. 设备断电/拔线
2. Transport 检测到 read 错误 → ConnectionState 变 Offline
3. TopBar 指示灯变红 + 闪烁（2s 周期）
4. StatusBar "❤️ 心跳超时" 红字
5. Toast "连接已断开，正在自动重连…" (持续直到恢复或手动取消)
6. 自动重连退避：1s → 2s → 4s → 5s（封顶）
7. 恢复成功 → Toast "已重新连接" + 跳回上次所在页面
```

**场景 C：用户在 Settings 改了一半，被 0x87 推送打断**

```text
1. 用户把 tft_brightness 50→80（待下发）
2. 设备主动推送 0x87 seq=0（可能因其他来源，比如设备重启后）
3. 合并策略：tft_brightness 不覆盖（用户有草稿），其它字段用推送值刷新
4. DiffPreviewBar 仍显示 "待下发 1 项: tft_brightness: 50→80"
5. 用户可继续编辑或放弃
```

**场景 D：危险操作确认**

```text
1. 用户在 Keyboard Tab 切换 work_mode
2. 弹出 ConfirmDialog：
   "切换工作模式将重建键盘实例，期间无法响应按键，是否继续？"
   [取消]  [继续]
3. 用户确认 → 字段进入待下发
4. DiffPreviewBar 显示该变更并打上 ⚠ 标记
5. 用户点 [应用] → 真正下发
```

---

## 9. 快捷键

| 快捷键       | 行为                          |
| ------------ | ----------------------------- |
| `F5`         | 刷新（重发 `CMD_CONFIG_GET`） |
| `Ctrl+Enter` | 应用待下发设置                |
| `Esc`        | 放弃待下发设置                |
| `Ctrl+L`     | 切换到 Log 页面               |
| `Ctrl+,`     | 打开 Local Settings 弹窗      |
| `Ctrl+Q`     | 退出应用                      |
| `Ctrl+1` ~ `Ctrl+9` | 快速切换 SideNav 页面（7=音效，8=日志，9=关于） |

---

## 10. 错误与空状态

| 场景                  | UI 表现                                                            |
| --------------------- | ------------------------------------------------------------------ |
| 启动时无任何端口      | Connect 页中央空状态卡片 + "请插入 EKeys 设备" 插画占位 + 扫描按钮 |
| 设备响应 `error` 字段 | Toast（红，5s）+ Log 页红字记录                                    |
| `CMD_CONFIG_GET` 超时 | TopBar 黄灯闪烁 + Toast "设备无响应" + 自动重试 1 次               |
| 收到未知字段          | Log 页黄字 `[W] unknown field "xxx" ignored`（不影响 UI）          |
| 收到未知命令响应      | Log 页灰字记录                                                     |

---

## 11. 阶段落地清单

### 阶段 04（当前，必须完成）

- [ ] P1 Connect：`PortSelector`、`ScanButton`、`ConnectionControls`、`DeviceInfoCard`、`StatusBlock`、`AutoConnectToggle`
- [ ] P2 Settings：`SettingsTabs` + 4 个 Tab（Display / Keyboard / Audio / Power）+ `FieldEditor` + `DiffPreviewBar` + `Toast`
- [ ] P7 Log：`LogViewer` + `LogFilter` + `LogRow` + `ClearButton`
- [ ] P8 About：静态卡片
- [ ] 全局：TopBar、SideNav、StatusBar、快捷键（F5、Ctrl+Enter、Esc）

### 阶段 05

- [ ] P3 Keymap 占位 + `CMD_KEYMAP_GET/SET`、`CMD_PROFILE_ICON_SET`
- [ ] Settings 增加 🖥 PC Tab + `BitMaskField` 组件

### 阶段 06

- [ ] P4 Lighting、P5 WiFi、P6 Voice 全部启用
- [ ] SideNav 灰显项移除

---

## 12. 与工程结构的对应关系

| UI 组件                      | 落地文件                                                               |
| ---------------------------- | ---------------------------------------------------------------------- |
| `PortSelector` 等            | `src/ui/panel_connection.rs`                                           |
| `SettingsTabs` 等            | `src/ui/panel_settings.rs`                                             |
| `LogViewer` 等               | `src/ui/panel_log.rs`                                                  |
| `AboutCard`                  | `src/ui/panel_about.rs`                                                |
| TopBar / SideNav / StatusBar | `src/ui/topbar.rs`、`src/ui/sidenav.rs`、`src/ui/statusbar.rs`（新增） |
| `Toast` / `ConfirmDialog`    | `src/ui/widgets/toast.rs`、`src/ui/widgets/confirm.rs`（新增）         |
| `AppEvent` 总线              | `src/state/push_event.rs` + `src/state/app_state.rs`                   |
| `DiffPreviewBar`             | `src/ui/panel_settings.rs` 内私有组件                                  |
