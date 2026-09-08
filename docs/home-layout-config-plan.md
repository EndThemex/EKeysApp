# 主页可配置布局 实现方案（v0.1）

> 适用范围：`EKeysApp` 桌面端（eframe 0.33 + egui）
> 目标：将"主页"（`Page::Connect` 与 `Page::Settings` 之外的"主入口"）从硬编码布局改为**组件 + 区域 + 配置下发**驱动的可配置布局
> 阶段定位：阶段 04+（在当前架构上扩展；不破坏现有面板、协议、状态机）

---

## 1. 背景与目标

### 1.1 现状

- `app.rs::update()` 固定渲染 `TopBar + SideNav + CentralPanel(按 Page 渲染 panel) + StatusBar`。
- `Page` 枚举（`Page::Connect / Settings / Keymap / Lighting / Wifi / Voice / Log / About`）由 `SideNav` 选择。
- 主页（CentralPanel 区域）目前直接绑定到 `current_page`，**布局 = 选中的页面**，用户无法调整：
  - 主页放什么组件（"时间卡片" / "设备摘要" / "快捷入口" 等）
  - 组件放在哪里（顶部 / 左侧 / 右侧 / 网格坐标）
  - 组件大小（固定 vs 可拉伸）
- `LocalConfig` 已经支持 `last_port / auto_connect / window_size / language / theme`，但**没有任何主页布局相关字段**。

### 1.2 目标

让"主页"成为一块由**配置驱动**的"画布"：

1. **组件化**：将主页可放置的内容抽象为 `HomeWidget` 枚举（时间、欢迎语、设备摘要、最近错误、快捷入口…）。
2. **区域化**：把画布划分为固定网格区域（顶部/中部/侧栏/底栏 等），每个区域可放 1~N 个组件。
3. **可配置**：通过 JSON / 协议下发 / 未来云端，把布局写入 `LocalConfig` 的 `home_layout` 字段；UI 每帧按配置渲染。
4. **可扩展**：新增组件时只新增一个 `HomeWidget` 变体 + 一份渲染实现，**不修改 `app.rs` 中央调度**。
5. **可回退**：布局配置解析失败时，自动回退到内置默认布局，不卡死 UI。

### 1.3 非目标（明确不做）

- 不做"运行时拖拽"（先用配置下发；交互式编辑作为后续阶段）。
- 不改 `SideNav` / `TopBar` / `StatusBar` 三个全局 chrome。
- 不动 `Connect / Settings / Keymap / Lighting / Wifi / Voice / Log / About` 现有面板的内部结构（只解决"主页"这一块）。
- 不引入 `tokio` / `async` / 新的 GUI 框架。

---

## 2. 概念模型

### 2.1 区域（Zone）

主页画布 = **若干固定区域**的拼接。每个区域有：

```rust
pub struct Zone {
    pub id: ZoneId,             // TopBar | Hero | Quick | Sidebar | Status | ...
    pub kind: ZoneKind,         // Row | Column | Grid | Tabs(预留)
    pub slots: Vec<Slot>,       // 该区域内可放置的槽位
}
```

- `ZoneId::Hero` ── 中部主区（最显眼），单列纵向
- `ZoneId::Quick` ── 右侧快捷区
- `ZoneId::Status` ── 底部状态补充（区别于全局 StatusBar）
- `ZoneId::Welcome` ── 顶部欢迎语/标题区
- 后续可加 `ZoneId::Footer / ZoneId::SidebarLeft` 等

**关键约束**：每个 `Zone` 的位置和大小由本组件**写死**（即"区域是固定的，区域里的内容可变"），符合 egui Immediate Mode 的"用原生布局，不堆绝对坐标"原则。

### 2.2 槽位（Slot）

```rust
pub struct Slot {
    pub widget: HomeWidget,     // 这个槽位放什么
    pub span: u8,               // 跨多少格（仅 Grid zone 使用；其他 zone 固定 1）
    pub min_size: Option<[f32; 2]>, // 最小尺寸 hint（px）
}
```

`HomeWidget`（组件枚举，所有组件**大小由组件自身决定**，**位置由 Zone 决定**）：

```rust
pub enum HomeWidget {
    Clock,             // 时间组件（固定大小，eg：200x80）
    Greeting,          // 欢迎语 / 设备名（固定大小）
    DeviceSummary,     // 设备摘要卡片（固定大小，取决于字段多少）
    LastError,         // 最近错误提示（无错误时折叠）
    QuickActions,      // 快捷入口按钮（"打开设置 / 打开日志 / 重新连接"）
    Shortcut(ShortcutAction), // 单个快捷入口（绑定到某个 Page）
    Spacer(usize),     // 空白占位（像素）
    Empty,             // 显式空槽
}
```

> 关键设计原则：**组件只关心"自己长什么样、宽度自适应"；位置由 Zone 决定，组件不携带 `x, y` 坐标**。这与 egui 规则 2（优先用原生布局）一致。

### 2.3 布局文档（Layout）

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct HomeLayout {
    pub version: u32,                  // 布局 schema 版本，便于升级
    pub zones: Vec<Zone>,              // 区域定义（顺序 = 渲染顺序）
    #[serde(default)]
    pub hidden_widgets: Vec<HomeWidget>, // 用户主动隐藏的组件（Zone 中删除）
}

impl Default for HomeLayout {
    fn default() -> Self { /* 内置默认布局，见 §5 */ }
}
```

存储位置：

- 短期：追加到 `LocalConfig` 的 `home_layout: HomeLayout` 字段。
- 后续：通过协议 `CMD_CONFIG_SET` 下发（字段 id 预留；本阶段只做本地配置）。

---

## 3. 渲染管线

### 3.1 替换 `app.rs::update` 主页分支

把 `match page { Page::Connect => panel_connection::show(...) ... }` 中的 `Page::Home` 分支（**新增**）路由到 `panel_home::show`，其它页面行为完全不变。

```rust
// app.rs（伪代码）
let page = *self.handle.page.lock().unwrap();
match page {
    Page::Home        => panel_home::show(&self.handle, ui, &mut self.home_st),
    Page::Connect     => panel_connection::show(...),
    // ... 其他不变
}
```

`Page::Home` 是新增的"主页"页面，默认启动时落在 `Home`（与"启动跳到 Connect"的旧行为通过 `LocalConfig` 兼容：保留 `start_page` 字段，初次启动默认 Home）。

### 3.2 `panel_home::show` 总流程

```
panel_home::show(handle, ui, state)
  │
  ├─ 1) 读取 home_layout 快照（state.cached.clone()）
  │     - 首次进入或 handle.home_layout 变化时刷新缓存（避免每帧 clone）
  │     - 解析失败 → 用 default_layout() 回退并 log warn
  │
  ├─ 2) 按 zone.id 顺序，用 egui 原生布局渲染：
  │     ZoneId::Welcome → ui.horizontal(...).inside(Frame)
  │     ZoneId::Hero    → ui.vertical   (按 slot 顺序 add)
  │     ZoneId::Quick   → egui::SidePanel::right("home_quick") 或 ui.vertical
  │     ZoneId::Status  → ui.horizontal (底栏上方)
  │
  └─ 3) 每个 slot 渲染对应 HomeWidget：
         HomeWidget::Clock     → render_clock(ui)
         HomeWidget::Greeting  → render_greeting(ui, handle)
         ...
```

**每个组件**封装为 `fn render(ui: &mut Ui, handle: &AppHandle)`，全部位于 `ui/widgets/home/` 目录（新增）：

```
src/ui/widgets/home/
├── mod.rs              # re-export + HomeWidget 渲染入口 dispatch
├── clock.rs            # 时间组件（200x80 固定卡）
├── greeting.rs         # 欢迎语 / 设备名
├── device_summary.rs   # 设备摘要
├── last_error.rs       # 最近错误（无错误时不渲染）
├── quick_actions.rs    # 快捷入口按钮组
└── shortcut.rs         # 单个快捷入口
```

### 3.3 时间组件示例（`clock.rs`）

```rust
pub fn render(ui: &mut egui::Ui, _handle: &AppHandle) {
    // 大小固定 200x80（用户问的"大小固定，位置可配置"）
    let desired = egui::Vec2::new(200.0, 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());

    let now = chrono::Local::now();
    let time = now.format("%H:%M:%S").to_string();
    let date = now.format("%Y-%m-%d %a").to_string();

    ui.painter().text(
        rect.left_center() + egui::Vec2::new(16.0, -8.0),
        egui::Align2::LEFT_CENTER,
        time,
        egui::FontId::proportional(28.0),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        rect.left_center() + egui::Vec2::new(16.0, 14.0),
        egui::Align2::LEFT_CENTER,
        date,
        egui::FontId::proportional(13.0),
        ui.visuals().weak_text_color(),
    );

    // 动画（每秒刷一次）— 符合规则 9：基于时间、主动 request_repaint
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
}
```

> 关键：组件**不持有 `Rect` 也不跨帧缓存**，仅在 `update()` 内被调用；动画靠 `request_repaint_after` 驱动（见 egui 规则 9）。

### 3.4 ID 与状态隔离

- 每个组件用 `ui.push_id(format!("home_{:?}", widget))` 包裹，**避免不同 Zone 出现同类型组件时 ID 冲突**（egui 规则 5）。
- `panel_home::HomePanelState` 仅持有 `cached: HomeLayout` + 上次渲染时间戳；不持有 `Ui` / `Response`（egui 规则 1）。

---

## 4. 配置存储与下发

### 4.1 `LocalConfig` 新增字段

```rust
// config/mod.rs
pub struct LocalConfig {
    pub last_port: Option<String>,
    pub auto_connect: bool,
    pub window_size: Option<[f32; 2]>,
    pub language: Language,
    pub theme: Theme,
    #[serde(default)]
    pub home_layout: HomeLayout,    // ← 新增
    #[serde(default = "default_start_page")]
    pub start_page: Page,            // ← 新增：决定启动落在哪个 Page
}
```

- `#[serde(default)]` 保证旧配置文件自动获得默认布局（不破坏现有用户）。
- `start_page` 默认 `Page::Home`，让"主页"成为新启动入口。

### 4.2 AppHandle 共享布局

```rust
// state/mod.rs
pub struct AppHandle {
    // ... 既有字段 ...
    pub home_layout: Arc<Mutex<HomeLayout>>,   // ← 新增；UI 读 + UI 改都走它
}
```

- 启动时 `home_layout = local_config.home_layout.clone()`。
- 任何位置修改布局（如未来"右键 → 隐藏组件"）→ 修改后写回 `LocalConfig` 并 `save()`。

### 4.3 协议下发（后续阶段，**本阶段只预留**）

- 协议侧增加 `home_layout: HomeLayout` 字段（结构序列化后体积可控；用 `serde_json`）。
- 接收 `CMD_CONFIG_SET` 时与其它字段一并写入 `AppHandle::home_layout`，触发 UI 刷新。
- **本阶段**不实现设备侧下发，仅在 `protocol.rs` 的 `DeviceSettings` 增加字段占位（加 `#[serde(default)]`），为阶段 05/06 留口。

### 4.4 升级与回退

```rust
impl HomeLayout {
    pub fn resolve(stored: Option<HomeLayout>) -> Self {
        let s = stored.unwrap_or_default();
        if s.version != SCHEMA_VERSION {
            // 旧版本：迁移到当前版本；迁移失败回退 default
            return migrate(s).unwrap_or_default();
        }
        // 校验：若 zones 为空或 slot 引用未知 widget → 回退
        if s.zones.is_empty() {
            return Self::default();
        }
        s
    }
}
```

`SCHEMA_VERSION = 1`；后续每次变更递增。

---

## 5. 默认布局

```rust
// ui/widgets/home/default_layout.rs
pub fn default_layout() -> HomeLayout {
    HomeLayout {
        version: 1,
        zones: vec![
            Zone {
                id: ZoneId::Welcome,
                kind: ZoneKind::Row,
                slots: vec![Slot { widget: HomeWidget::Greeting, span: 1, min_size: None }],
            },
            Zone {
                id: ZoneId::Hero,
                kind: ZoneKind::Column,
                slots: vec![
                    Slot { widget: HomeWidget::Clock, span: 1, min_size: Some([200.0, 80.0]) },
                    Slot { widget: HomeWidget::DeviceSummary, span: 1, min_size: None },
                    Slot { widget: HomeWidget::LastError, span: 1, min_size: None },
                ],
            },
            Zone {
                id: ZoneId::Quick,
                kind: ZoneKind::Column,
                slots: vec![
                    Slot { widget: HomeWidget::QuickActions, span: 1, min_size: None },
                ],
            },
        ],
        hidden_widgets: vec![],
    }
}
```

`SidePanel::right("home_quick")` 渲染 `Quick` 区，`CentralPanel` 内按 `Welcome → Hero` 纵向堆叠。

---

## 6. 与现有架构的衔接

| 现有模块            | 改动                                                                                    |
| ------------------- | --------------------------------------------------------------------------------------- |
| `config/mod.rs`     | `LocalConfig` 新增 `home_layout` + `start_page`；旧字段不变                             |
| `state/mod.rs`      | `AppHandle` 新增 `home_layout: Arc<Mutex<HomeLayout>>`                                  |
| `app.rs`            | `match page` 增加 `Page::Home` 分支；`WxiApp` 新增 `home_st` 字段                       |
| `ui/mod.rs`         | `pub mod panel_home; pub mod widgets::home;`                                            |
| `ui/panel_home.rs`  | **新增**；签名同其他 panel：`show(&AppHandle, &mut Ui, &mut HomePanelState)`            |
| `ui/widgets/home/*` | **新增**；每个组件独立文件                                                              |
| `ui/sidenav.rs`     | 在导航首位插入 `🏠 主页` 入口                                                           |
| `protocol.rs`       | `DeviceSettings` 增加 `home_layout` 字段（`#[serde(default)]`，仅占位，**不立即生效**） |
| `Cargo.toml`        | 视情况引入 `chrono`（已在不少项目里使用；如已有 eframe 间接依赖，则无需新增）           |

**刻意不改**：`link/`、`protocol` 命令常量、`ui/topbar.rs`、`ui/statusbar.rs`、`ui/panel_connection.rs`（除增加"打开主页"按钮外不修改）。

---

## 7. 交互细节

### 7.1 启动流程

```
启动
  ├─ 读 LocalConfig
  ├─ home_layout = resolve(local_config.home_layout)
  ├─ 初始 page = local_config.start_page （默认 Home）
  ├─ SideNav 首位 = "🏠 主页"
  └─ HomePage 渲染 default_layout 或用户配置
```

### 7.2 快捷键

- `Ctrl+1` 切到主页（插在现有 `Ctrl+1~8` 之前；序号顺移：旧 1→2, ..., 8→9）
- `F5` 行为不变（刷配置）
- 主页内不做专属快捷键

### 7.3 空状态

- `LastError` 在 `handle.last_error.is_none()` 时**完全不渲染**（节省空间），不显示"暂无错误"
- `DeviceSummary` 在未连接时显示"未连接设备"占位，不显示真实字段
- `Greeting` 在未连接时降级为"欢迎使用 EKeys"

### 7.4 配置变更入口（后续阶段）

- 主页右上角放一个隐藏的 `⚙` 按钮（鼠标悬停时显示），点击打开"布局编辑"弹窗：
  - 勾选要显示的组件
  - 调整组件在 Zone 内的顺序（上下拖拽 → egui 暂不支持原生拖拽，使用 `↑/↓` 按钮）
  - 重置为默认布局
- 本阶段**只预留按钮占位**，不做实际编辑；保证接口稳定后下个阶段填。

---

## 8. 风险与缓解

| 风险                                                       | 缓解                                                                                                         |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 布局配置被改坏（手改 JSON / 下发错误）导致 UI 不显示       | `HomeLayout::resolve()` 校验；任意校验失败回退 `default_layout()` 并 log warn                                |
| 组件在 `egui::SidePanel` 内 `request_repaint_after` 死循环 | 时间组件只在 `Clock` widget 内调用 `request_repaint_after`；`SidePanel` 自身按需重绘（egui 规则 9）          |
| `Arc<Mutex<HomeLayout>>` 与 UI 锁死锁                      | 读：进入 `show()` 时一次性 `lock().unwrap().clone()` 到 `HomePanelState.cached`；后续只读缓存                |
| 新增组件时漏改 `dispatch` 报错                             | `HomeWidget` 渲染入口用 `match` 穷举 + `_ => log warn`；CI 用 `cargo clippy -W clippy::match_strap_arm` 防漏 |
| 启动慢（解析大布局）                                       | 布局 schema 极小（< 1KB），解析 < 1ms；不需异步                                                              |
| 协议侧字段未就绪但 App 侧已用                              | `DeviceSettings.home_layout: Option<HomeLayout>` + `#[serde(default)]`；设备不发送时 App 用本地              |

---

## 9. 落地步骤（建议 PR 拆解）

1. **PR1：基础结构**（不引入新功能）
   - `config/mod.rs`：加 `home_layout: HomeLayout` + `start_page` 字段，默认值
   - `state/mod.rs`：加 `home_layout: Arc<Mutex<HomeLayout>>`
   - `Page::Home` 枚举 + `panel_home.rs` 空壳（只渲染一句"Hello Home"）
   - `app.rs` `match page` 加分支
   - `SideNav` 首位加"🏠 主页"
   - `cargo check` / `cargo clippy` 通过

2. **PR2：组件实现**
   - `ui/widgets/home/{mod.rs, clock.rs, greeting.rs, device_summary.rs, last_error.rs, quick_actions.rs, shortcut.rs}`
   - `default_layout()` 落实
   - `HomeLayout::resolve()` + 校验/回退
   - 主页可见、可交互

3. **PR3：体验打磨**
   - 启动页 = Home（`start_page` 默认值生效）
   - 旧配置兼容（`#[serde(default)]`）
   - 错误/警告路径走通
   - 写一份"扩展新组件"README 片段

4. **PR4（后续阶段）**：协议下发 + 编辑器弹窗

---

## 10. 验收清单

- [ ] 首次启动，落到"主页"，显示默认布局（欢迎语 + 时钟 + 设备摘要 + 最近错误 + 快捷入口）
- [ ] 修改 `LocalConfig::home_layout` 字段（手改 JSON），重启后布局按配置渲染
- [ ] 把 `home_layout` 改成非法 JSON（`{}`）→ UI 仍然能渲染（用默认布局），并在 Log 看到 warn
- [ ] 删掉 `LocalConfig` 文件 → 重新生成，仍能正常使用
- [ ] 主页 `Clock` 每秒刷新；切到 Settings 页时不再触发主页 repaint
- [ ] `cargo check` / `cargo clippy` 无新增 warning
- [ ] SideNav / TopBar / StatusBar 行为与之前完全一致
- [ ] 现有 `Connect / Settings / Keymap / ...` 面板**未改动**

---

## 11. 后续演进（不在本方案范围）

- 协议侧把 `home_layout` 加入 `DeviceSettings`，通过 `CMD_CONFIG_SET` 下发（云端/局域网控制）
- `panel_home` 内嵌"布局编辑"弹窗，运行时拖拽
- 多套预设（"简洁 / 极客 / 调试"）一键切换
- Zone 增加 `Tabs` 类型，让 Hero 区域可切换"概览 / 图表 / 实时数据"
- 把 `Zone` 抽象升级为 `Dock`，支持停靠 / 浮动

---

## 12. 相关文档

- 现状 UI：[`ui-design.md`](./ui-design.md)
- 工程结构：[`project-structure-plan.md`](./project-structure-plan.md)
- 协议：[`desktop-app-protocol.md`](./desktop-app-protocol.md)
- egui 开发规则：`.trae/rules/rules.md`
