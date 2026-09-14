# EKeysApp 架构

> 给新加入贡献者的「5 分钟读懂」地图。深入某一层请按目录跳到对应模块。

---

## 1. 顶层数据流

```
                        ┌─────────────────────────────────────────────┐
                        │                 UI Thread                   │
                        │  (eframe 每帧重建,80%~100% 立即模式)        │
                        │                                             │
   鼠标 / 键盘 ───────▶ │  ┌──────────┐    ┌──────────────────────┐   │
                        │  │ ui/*     │    │  AppHandle (Arc<…>)  │   │
                        │  │ 面板     │ ──▶│  shared state        │   │
                        │  └────┬─────┘    │  (Mutex<…>)          │   │
                        │       │          └──────────┬───────────┘   │
                        │       │ ui_tx              │               │
                        │       ▼                     │               │
                        │  ┌──────────────────────┐   │               │
                        │  │ app.rs::drain_*      │   │               │
                        │  │ (事件分发 / 主题/快捷键)│   │               │
                        │  └──────────┬───────────┘   │               │
                        └─────────────┼───────────────┼───────────────┘
                                      │               │
                          LinkEvent   │               │  spawn task
                          (rx)        │               │  (后台线程)
                                      ▼               ▼
                        ┌─────────────────────────────────────────────┐
                        │                Link Layer                   │
                        │  reader ─ router ─ writer ─ heartbeat       │
                        │  (全部 std::thread,共享 LinkManager)        │
                        └──────────────┬──────────────────────────────┘
                                       │ USB CDC / 串口
                                       ▼
                                 [ EKeys 设备 ]
```

## 2. 模块职责

| 模块 | 职责 | 不允许 |
| --- | --- | --- |
| `src/main.rs` | 装载字体 / 启动 eframe / 注入 LocalConfig | 任何业务逻辑 |
| `src/app.rs` | `WxiApp::update` 装配 chrome + 当前页;`drain_link_events` / `drain_ui_events`;主题与快捷键 | 直接调用 `LinkManager` 方法(只走事件) |
| `src/protocol.rs` | 命令常量 / `Frame` 编解码 / `DeviceSettings` / `KeymapData`;**纯函数** | 任何 IO / `Mutex` / `tracing` 业务宏 |
| `src/link/` | 后台 reader / writer / router / heartbeat 线程;状态机 (`Online / Reconnecting / Idle`) | 直接读 UI 状态;UI 状态经 `AppHandle` |
| `src/state/mod.rs` | `AppHandle`(共享 Arc);`UiEvent` / `LinkEvent` / `Page` / `UiConfirmKind` / `ToastKind` | 业务规则 |
| `src/config/mod.rs` | `%APPDATA%/wxi/config.json` 加载与保存 | 协议字段 |
| `src/ota.rs` | 本机一次性 HTTP 服务 + MD5 校验;局域网 OTA 下发 | UI 渲染 |
| `src/pc_status.rs` | Windows 平台下采集键盘 Lock / 网络 / CPU / 内存;diff 打包为 `PcStatusReq` | UI 渲染 |
| `src/ui/mod.rs` | 主题常量 + `apply_theme` + `card()` 容器 | 业务逻辑 |
| `src/ui/fonts.rs` | 字体注册 + 图标 / 文本混排 helper | 协议 |
| `src/ui/icons.rs` | Phosphor 图标常量集中导出 | 业务 |
| `src/ui/topbar.rs` | 顶部状态条 + 连接块 | 长事务逻辑 |
| `src/ui/sidenav.rs` | 左侧导航 | 业务 |
| `src/ui/statusbar.rs` | 底部状态条 | 业务 |
| `src/ui/panel_*.rs` | 每个页面对应一个文件 + `*PanelState` 结构体 | 直接调用 `LinkManager` |
| `src/ui/widgets.rs` | `Toast` / `ConfirmDialog` / `LocalSettings` / `DiffPreviewBar` | 协议细节 |
| `src/util/log.rs` | `tracing` 初始化 + 共享日志缓冲(供 Log 页消费) | UI |

## 3. 共享状态契约

所有「跨线程可见」的状态都挂在 `AppHandle` 上,典型形式:

```rust
pub struct AppHandle {
    pub settings: Mutex<DeviceSettings>,   // 设备最新快照(已脱敏)
    pub draft:    Mutex<DeviceSettings>,    // Settings 页用户编辑中的草稿
    pub state:    Mutex<ConnectionState>,   // 连接状态机
    pub keymap:   Mutex<KeymapData>,        // 当前 Profile 的键映射
    pub device_info: Mutex<DeviceInfo>,
    pub auto_get: Mutex<AutoGetState>,
    pub logs:    Mutex<VecDeque<LogEntry>>, // 给 Log 页消费
    pub toasts:  Mutex<Vec<Toast>>,
    pub page:    Mutex<Page>,
    pub ui_tx:   Sender<UiEvent>,
    pub link_tx: Sender<LinkCmd>,
    // ...
}
```

**核心约定**:

1. **锁粒度尽量小**:读出 `clone()` 后立刻 drop guard;持锁状态不要再调用
   任何会再次加锁同一互斥体的函数(典型反例见 `topbar.rs::show` 注释)。
2. **跨线程通知走 channel**:`UiEvent` 经 `ui_tx` 流向 `app.rs::drain_ui_events`,
   避免「Panel 直接 `*handle.page.lock() = p`」。
3. **`Atom` 用于纯标记位**:`capture_keyboard` / `pc_status_push_enabled` 用
   `AtomicBool`,避免锁。

## 4. 事件总线

### 4.1 `LinkEvent`(`link` → `app`)

| 变体 | 触发 | App 处理 |
| --- | --- | --- |
| `Connected` | 串口打开成功 | 进入 `Online`,发 `auto_get` 与 `TimeSet` |
| `Disconnected` | 串口关闭 / 读线程退出 | 进入 `Reconnecting` / `Idle` |
| `Frame(Frame)` | 收到一帧 | 按 §5 路由 |
| `Log(LogEntry)` | reader 解析出协议 / 固件 / 应用日志 | 写入 `logs` 缓冲 + Toast(若需) |
| `Toast(ToastKind, String)` | reader 想直接告知用户 | 入 `toasts` |

### 4.2 `UiEvent`(`ui` → `app`)

| 变体 | 触发方 | App 处理 |
| --- | --- | --- |
| `Navigate(Page)` | sidenav / 快捷键 | 写入 `page`,触发进入边沿拉取 |
| `Connect / Disconnect` | topbar | 调用 `LinkManager` |
| `SendFrame(Frame)` | 各 panel 下发命令 | 走 `link_tx` |
| `SettingsCommit(DeviceSettings)` | panel_settings | 应用 + clamp + diff + 下发 |
| `Toast(ToastKind, String)` | 各 panel | 入 `toasts` |
| `Confirm(UiConfirmKind)` | 危险操作 | 弹 ConfirmDialog,按用户回答触发后续 |
| `Audio*` | panel_audio | 控制上传线程 |

## 5. 帧路由(`app.rs::handle_link_event`)

```
Frame(cmd, seq, data, status, error)
  │
  ├─ is_response_like() && seq != 0
  │    └─ pending 队列出队 / 配对 (SettingsCommit / ConfigGet 等)
  │
  ├─ is_response_like() && seq == 0   // 主动推送
  │    └─ 0x87 全量快照 → mask_sensitive + merge_push
  │       0x10 profile_state → refresh keymap
  │       0x0c voice text → 语音页 / Toast
  │       0x0f music control → 关于页
  │
  └─ 其他(请求帧 / 未配对响应)
       └─ 写日志 + Toast 提示
```

异类响应(`0x10 / 0x0c / 0x0f`)走 `parse_top_level::<T>`,不套用
`is_response = cmd & 0x80` 的标准规则。

## 6. 后台线程清单

| 线程 | 入口 | 主要职责 |
| --- | --- | --- |
| reader | `link::reader::run` | 读串口 → 行解析 → `Frame` 入 router |
| router | `link::reader` 内联(同步分发) | 解析 JSON + 派发到 `LinkEvent` |
| writer | `link::serial::write_loop` | 单写者,从 `link_rx` 收帧写入串口 |
| heartbeat | `link::heartbeat::run` | 周期发 `0x0a`,超时触发状态机切到 `Reconnecting` |
| audio upload | `state::audio_upload_worker` | `0x16` 分块上传(begin/data×N/end) |
| OTA HTTP | `ota::serve_once` | 一次性 HTTP,本机 IP + 文件 MD5 校验 |
| pc_status poll | `pc_status::run` | 1Hz 采集 → diff → `0x0d` 下发 |

> 详见 [docs/build.md](./build.md#后台线程清单) 的线程拓扑小节。

## 7. UI 装配(`app.rs::update`)

```
TopBottomPanel::top("topbar")      → topbar::show(handle, ui)
SidePanel::left("sidenav")         → sidenav::show(handle, ui)
CentralPanel::default              → 强制 ScrollArea::vertical(auto_shrink=false)
                                       └─ match current_page { … }
TopBottomPanel::bottom("statusbar")→ statusbar::show(handle, ui)
```

- Keymap 页面**不**进入 CentralPanel;在 `ctx` 上注册 top/central/bottom 三段
  (实现见 `panel_keymap.rs` 顶部注释)。
- Toast / Confirm 由独立的 `Window` 在 `update` 末尾统一调用
  `widgets::show_toasts` / `show_confirm`。
- `ctx.request_repaint_after(100ms)` 在 `update` 末尾调用,**保底 10 Hz**;
  有动画 / 计时器的面板自行 `request_repaint()` 拉高频率。

## 8. 持久化

| 数据 | 路径 | 触发保存 |
| --- | --- | --- |
| LocalConfig | `%APPDATA%/wxi/config.json` | LocalSettings 弹窗确认 / 窗口尺寸变化 |
| Device Settings | **不持久化**(始终以设备为权威) | — |
| KeymapData | **不持久化**(页面进入时 GET) | — |

> 「设备回传的密钥明文不落盘」是协议层 §7 的硬约束,本地配置文件也不应出现。
> 当前实现依赖 `DeviceSettings::mask_sensitive()` 在写入共享 state 前
> 完成脱敏。

## 9. 调试建议

- **协议帧追踪**:打开日志页 → 设置过滤为 `TX/RX`,可实时看到 App ↔ 设备
  的全部帧;比 wireshark + USB 抓包轻量得多。
- **状态机断点**:`LinkManager::state` / `state::AutoGetState` 是连接 + 自动
  拉取的两个核心状态机;出现「连不上 / 拉不到配置」时优先看这里。
- **锁相关 panic**:若 `unwrap()` 一个 `Mutex`,几乎一定是上一帧没释放锁;
  按堆栈找「持锁跨过 await / 持锁调用 LockManager」的代码段。
- **UI 不刷新**:确认对应面板有 `ctx.request_repaint()`;空闲时 `update`
  末尾的 `request_repaint_after(100ms)` 会兜底。
