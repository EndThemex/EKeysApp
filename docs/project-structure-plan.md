# EKeysApp 工程结构计划（v0.2，简化版）

> 对接协议：[`desktop-app-protocol.md`](./desktop-app-protocol.md)
> UI 设计：[`ui-design.md`](./ui-design.md)
> 当前阶段：阶段 04（USB CDC + 设置面板）

本文档在 v0.1 基础上做了**收敛**：把"协议编解码"压成一个模块、把"传输+连接"合为一个 `link/` 层、把"小部件"折进 `ui/`。目标是每个目录一眼能看懂职责，避免过早细分。

---

## 1. 调整要点（相对 v0.1）

| v0.1                                              | v0.2                                                                    | 原因                                   |
| ------------------------------------------------- | ----------------------------------------------------------------------- | -------------------------------------- |
| `protocol/{cmd,frame,codec,settings}.rs` 4 个文件 | `protocol/mod.rs` 单文件，命令常量 + Frame + 编解码 + Settings 全部内聚 | 阶段 04 协议很简单，过早拆分会增加跳转 |
| `transport/` 与 `connection/` 两层                | 合并为 `link/`（端口枚举 + 读写 + 状态机 + 心跳）                       | 强相关，合并后心智负担更低             |
| `ui/widgets/{toast,confirm}.rs`                   | 折进 `ui/widgets.rs` 单文件                                             | 小部件数量少，统一管理                 |
| `state/{app_state,push_event}.rs`                 | 单 `state/mod.rs`：`AppHandle` + 事件枚举                               | 内聚                                   |
| `util/{logger,platform}.rs`                       | 仅保留 `util/log.rs`（tracing 初始化 + UI 转发）                        | 阶段 04 用不到平台工具函数             |

总文件数：v0.1 约 24 个源文件 → v0.2 约 14 个。

---

## 2. 目录结构（v0.2 最终态）

```
EKeysApp/
├── Cargo.toml
├── README.md
├── .gitignore
├── fonts/
│   └── MapleMono-NF-CN-ExtraLight.ttf
├── docs/
│   ├── desktop-app-protocol.md
│   ├── project-structure-plan.md     # 本文档
│   └── ui-design.md
└── src/
    ├── main.rs                       # 入口：装载字体 + 启动 App
    ├── app.rs                        # eframe::App 实现 + AppHandle 装配
    │
    ├── protocol.rs                   # 命令常量 / Frame / 编解码 / DeviceSettings
    │
    ├── link/
    │   ├── mod.rs                    # ConnectionState + LinkManager（对外门面）
    │   ├── serial.rs                 # 串口枚举与打开
    │   ├── reader.rs                 # 后台读线程：按行分帧 → 协议/日志分流
    │   └── heartbeat.rs              # 定时发 0x0a，连续失败降级 Offline
    │
    ├── state/
    │   └── mod.rs                    # AppHandle：UI 可读的共享状态 + 事件 channel
    │
    ├── config/
    │   └── mod.rs                    # 本地 App 偏好（最近端口、UI 选项）
    │
    ├── ui/
    │   ├── mod.rs                    # 主题常量 + 共享布局辅助
    │   ├── fonts.rs                  # 字体注册（从 main.rs 抽离）
    │   ├── topbar.rs                 # 顶部状态条
    │   ├── sidenav.rs                # 左侧导航
    │   ├── statusbar.rs              # 底部状态条
    │   ├── panel_connection.rs       # P1 连接页
    │   ├── panel_settings.rs         # P2 设置页（核心）
    │   ├── panel_log.rs              # P7 日志页
    │   ├── panel_about.rs            # P8 关于页
    │   └── widgets.rs                # Toast / ConfirmDialog / FieldEditor / DiffPreviewBar
    │
    └── util/
        └── log.rs                    # tracing 初始化 + 日志缓冲（供 panel_log 消费）
```

> 阶段 04 只需实现上列所有文件。`panel_keymap.rs` 等阶段 05/06 的面板暂不创建，到阶段再按同模式新增。

---

## 3. 模块职责（一句话版）

### 3.1 `src/main.rs`

- `eframe::run_native` 启动；构造 `AppHandle`，传入 `App::new(cc, handle)`

### 3.2 `src/app.rs`

- 实现 `eframe::App`：每帧 drain `AppHandle.events` → 触发 UI 更新；按 `current_page` 渲染对应 panel；绘制 topbar/sidenav/statusbar 外壳

### 3.3 `src/protocol.rs`

- 命令 ID 常量（与协议 §2 对齐）
- `Frame { cmd, seq, data }` / `Response { status, error, data }` / `PushSnapshot { settings }`
- `encode(&Frame) -> String`（追加 `\n`）、`try_parse_line(&str) -> Option<Result<Frame, Error>>`
- `DeviceSettings` 结构 + 默认值 + `clamp(&mut self)`（协议 §4.1）
- **纯函数，零 IO**

### 3.4 `src/link/mod.rs` — `LinkManager`

对外暴露：

```rust
pub struct LinkManager { /* 内部：serial port handle + threads + channels */ }
impl LinkManager {
    pub fn list_ports() -> Vec<PortInfo>;          // 枚举 + VID 0x303A 过滤
    pub fn open(port: &str) -> Result<Self>;
    pub fn close(&mut self);
    pub fn send(&self, frame: Frame);              // 走 writer 线程
    pub fn request(&self, cmd: u8, data: ...) -> Result<Response>;  // seq 配对 + 超时
    pub fn state(&self) -> ConnectionState;
    pub fn events(&self) -> Receiver<LinkEvent>;   // 协议帧 / 日志 / 状态变化 / 错误
}
pub enum LinkEvent {
    Frame(Frame),           // 协议帧（含响应与 seq=0 主动推送）
    LogLine(String),        // 固件日志原文
    State(ConnectionState),
    Error(String),
}
pub enum ConnectionState { Disconnected, Connecting, Online, Reconnecting, Error(String) }
```

### 3.5 `src/link/serial.rs`

- 基于 `serialport` crate：`list_ports`（含 VID/PID/serial/description 元数据）、`open(name, baud)`、原始 `read`/`write` 同步接口

### 3.6 `src/link/reader.rs`

- 持有 `Box<dyn SerialPort>` + `mpsc::Sender<LinkEvent>`
- 循环 `read()` → 按字节累加到行缓冲 → `\n` 切行
  - 以 `{` 开头 → `protocol::try_parse_line` → `Frame` 或错误
  - 其它 → `LogLine`
- 任一错误 → 推送 `State(Error)` 并退出循环

### 3.7 `src/link/heartbeat.rs`

- 独立线程：`interval.tick()` → 发 `Frame { cmd: CMD_HEARTBEAT, seq: 0 }`（注意：心跳的 seq 由 manager 自己递增）
- 跟踪"上次成功响应时间"，超过 `timeout * 3` 推送 `State(Reconnecting)`

### 3.8 `src/state/mod.rs` — `AppHandle`

```rust
pub struct AppHandle {
    pub settings: Arc<Mutex<DeviceSettings>>,      // 设备最新快照
    pub draft:     Arc<Mutex<DeviceSettings>>,      // 用户编辑未下发的草稿
    pub state:     Arc<Mutex<ConnectionState>>,
    pub log_buf:   Arc<Mutex<LogBuffer>>,           // 给 panel_log 用
    pub link_events: Receiver<LinkEvent>,
    pub ui_events: Sender<UiEvent>,                 // Toast / Confirm / 切页请求
}
```

- `App` 在每帧开头 `try_recv` 一批事件，分发到对应组件
- UI 主动动作（点 Connect、点应用）也通过 `ui_events` 发送，`App` 收到后调用 `LinkManager`

### 3.9 `src/config/mod.rs`

- 本地 JSON：`{ "last_port": "COM5", "auto_connect": true, "window_size": [W, H] }`
- 通过 `dirs` crate 拿 `%APPDATA%/wxi/config.json`（Win）/ `~/.config/wxi/config.json`（Unix）
- 启动时加载，退出时序列化保存

### 3.10 `src/ui/*` 面板

每个 `panel_*.rs` 都实现统一签名：

```rust
pub fn show(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    handle: &AppHandle,
) -> PanelAction;

pub enum PanelAction {
    None,
    RequestConnect(String),
    RequestDisconnect,
    RequestApply(Diff),       // 仅 settings
    RequestDiscardDraft,      // 仅 settings
    RequestConfirm(/* ... */), // 危险操作
}
```

面板**不直接**操作 `LinkManager`；所有副作用通过 `PanelAction` 返回，由 `app.rs` 调用。这种"面板 = 纯渲染 + 返回意图"模式让单测和重构更容易。

### 3.11 `src/ui/widgets.rs`

- `Toast::show(ctx, msg, kind, ttl)`
- `ConfirmDialog::show(ctx, title, body) -> Option<bool>`
- `FieldEditor` —— 协议字段到控件的统一封装（slider/combobox/switch/text），含"待下发"标记
- `DiffPreviewBar` —— 底部固定条，显示变更列表 + Apply/Discard

### 3.12 `src/util/log.rs`

- `init_tracing()`：写日志到 `LogBuffer`（容量 5000 行 ring buffer）而非 stderr
- `LogEntry { ts, level, kind, text }`

---

## 4. 数据流总览

```
┌───────────── Reader thread (link/reader.rs) ─────────────┐
│  serial read → 按行切分 → { → protocol → Frame          │
│                                → else  → LogLine        │
│  任一异常 → State(Error)                                │
└─────────────────────────────────────────────────────────┘
        │
        ▼  mpsc::Sender<LinkEvent>
┌──────── AppHandle.link_events ──────────────────────────┐
│  App.update 每帧 drain →                               │
│    Frame(req)         → 协议层响应（按 seq 配对）     │
│    Frame(seq=0,push)   → 合并到 settings + UI 刷新     │
│    LogLine             → LogBuffer::push               │
│    State(s)            → state::update + UI 提示      │
│    Error(e)            → Toast + LogBuffer             │
└─────────────────────────────────────────────────────────┘
        ▲
        │  UiEvent (Connect / Apply / Discard)
        │
┌──────── UI (app.rs + panels/*.rs) ─────────────────────┐
│  渲染 → 用户操作 → PanelAction → UiEvent              │
└─────────────────────────────────────────────────────────┘
```

---

## 5. 依赖清单（最终）

```toml
[dependencies]
eframe      = { version = "0.33.3", features = ["default"] }
serialport  = "4.6"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
tracing     = "0.1"
thiserror   = "2"
dirs        = "5"

[profile.release]
opt-level = 3
lto       = "thin"
```

阶段 04 **不引入** `tokio`：用 `std::thread` + `std::sync::mpsc`。体积更小、心智更轻。阶段 06 再视情况评估。

---

## 6. 命名与约定

1. **模块命名**：snake_case 文件，对应模块同名
2. **协议字段**：与固件一致（`lower_snake_case`），命令常量 `SCREAMING_SNAKE_CASE`
3. **错误处理**：底层（`protocol`/`link`）用 `thiserror`；UI 层用 `String`/`anyhow::Error` 简化
4. **UI 不直连 IO**：所有写操作经过 `PanelAction` → `App` → `LinkManager`
5. **日志通道**：固件日志原文一字不动，UI 仅做染色
6. **前向兼容**：`try_parse_line` 未知字段/命令不报错，仅打 tracing::warn
7. **线程模型**：reader / heartbeat / writer 各一线程；UI 线程永不阻塞

---

## 7. 阶段 04 落地清单

按以下顺序实现，每完成一项打勾：

1. **协议层**：`src/protocol.rs` —— 命令常量、Frame、Codec、DeviceSettings、clamp
2. **Link 骨架**：`link/mod.rs` 定义 `LinkManager`/`LinkEvent`/`ConnectionState`，先空实现
3. **Link 串口**：`link/serial.rs` 枚举 + 打开 + 原始读写
4. **Link reader**：`link/reader.rs` 按行分帧
5. **Link heartbeat**：`link/heartbeat.rs` 周期发 0x0a
6. **AppHandle**：`state/mod.rs` 共享状态 + channel
7. **UI 骨架**：`app.rs` + `ui/topbar.rs` + `ui/sidenav.rs` + `ui/statusbar.rs` + `panel_about.rs`
8. **Connect 面板**：`panel_connection.rs` —— 端口选择 + 连接/断开
9. **Settings 面板**：`panel_settings.rs` + `widgets.rs`（FieldEditor + DiffPreviewBar）
10. **Log 面板**：`panel_log.rs` + `util/log.rs`
11. **快捷键 + Toast + Confirm**：收尾

每步可独立 `cargo build` 通过。

---

## 8. 不在本计划内

- `panel_keymap.rs` / `panel_lighting.rs` / `panel_wifi.rs` / `panel_voice.rs`：阶段 05/06 再加
- `link/tcp.rs` + UDP 发现：阶段 06 再加
- 多语言：先中文硬编码，按需引入 `rust-i18n`
- 应用图标打包：默认 eframe 图标够用

---

## 9. 与 UI 设计文档的对应

| UI 组件 / 页面                | 落地文件                                             |
| ----------------------------- | ---------------------------------------------------- |
| TopBar / SideNav / StatusBar  | `ui/topbar.rs` / `ui/sidenav.rs` / `ui/statusbar.rs` |
| P1 Connect                    | `ui/panel_connection.rs`                             |
| P2 Settings + Diff            | `ui/panel_settings.rs` + `ui/widgets.rs`             |
| P7 Log                        | `ui/panel_log.rs`                                    |
| P8 About                      | `ui/panel_about.rs`                                  |
| Toast / Confirm / FieldEditor | `ui/widgets.rs`                                      |
| 事件总线                      | `state/mod.rs`（`AppHandle`）                        |
| 协议字段                      | `protocol.rs`（`DeviceSettings`）                    |
| 连接状态机                    | `link/mod.rs`（`ConnectionState`）                   |

---

确认本计划后即可按 §7 顺序逐项落地。
