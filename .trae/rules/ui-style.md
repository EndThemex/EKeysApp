# EKeysApp UI 样式与布局规范

> 适用于 `src/ui/**` 的所有面板、组件、对话框。
> 与 `rules.md`（egui 通用规则）配合使用——本文只覆盖**视觉与布局约定**，不重复通用规则。

---

## 1. 整体窗口骨架（Chrome Layout）

主窗口固定为 **顶栏 + 侧栏 + 内容区 + 底栏** 四段式，由 `app.rs::update` 装配：

```
┌─ TopBottomPanel::top("topbar")     ─ 高度 ~24~32px ─┐
├─ SidePanel::left("sidenav")        ─ 宽 188px 固定 ─┤
│                                                    │
│                                                    │
│              CentralPanel                         │
│              └ ScrollArea::vertical               │
│                  └ 当前 Page 的 panel              │
│                                                    │
├─ TopBottomPanel::bottom("statusbar") ─ 高度 ~22px ┤
```

**关键约定**：

- 顶栏 / 侧栏 / 底栏统称 **chrome**，统一使用 `ctx.style().visuals.extreme_bg_color` 作为底色，与内容区形成清晰分区。
- 内容区在 `CentralPanel` 内**强制套一层 `ScrollArea::vertical().auto_shrink([false, false])`**，避免窄窗口截断。
- Keymap 页面特殊：自带 top/central/bottom 三段，**必须**在 `ctx` 上注册（不能套进 CentralPanel），见 `app.rs::update` 的 `if matches!(current_page, Page::Keymap)` 分支。
- 每个 Panel 的注册形如 `egui::TopBottomPanel::top("topbar").frame(...).show(ctx, |ui| topbar::show(...))`；frame 的 `inner_margin` 见 §3。
- `SidePanel` 一律 `resizable(false).exact_width(188.0)`。

---

## 2. 主题与配色

### 2.1 主题切换

由 `crate::ui::apply_theme(ctx, theme)` 统一应用，在 `WxiApp::new` 中**字体安装之后、首个窗口创建之前**调用一次。运行时切换（Local Settings 弹窗）也调用此函数即时生效。

### 2.2 主题常量（在 `ui/mod.rs::apply_theme` 中定义）

| 角色             | Dark                              | Light                          |
| ---------------- | --------------------------------- | ------------------------------ |
| `panel_fill`     | `0x1B 0x1F 0x26`                  | `0xF2 0xF4 0xF8`               |
| `window_fill`    | `0x20 0x25 0x2D`                  | `0xFF 0xFF 0xFF`               |
| `extreme_bg_color` (chrome) | `0x12 0x15 0x1A`        | `0xE4 0xE7 0xEC`               |
| `faint_bg_color` | `0x24 0x2A 0x33`                  | `0xE9 0xEC 0xF1`               |
| 文本（override） | `0xE8 0xEC 0xF2`                  | `0x1A 0x1D 0x24`               |

### 2.3 品牌主色

```rust
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x4F, 0x8C, 0xFF);
```

用于：选中态背景、按钮高亮、链接、Toast 主操作、Sidenav 选中项、Tab 选中。

- 链接颜色：`vis.hyperlink_color = ACCENT`。
- 选中文本：`vis.selection.bg_fill`：
  - Dark → `ACCENT.gamma_multiply(0.45)`
  - Light → `colors::mix(WHITE, ACCENT, 0.22)`（gamma_multiply 会变暗，不适用）
- 选中文本描边：`vis.selection.stroke = Stroke::new(1.0, ACCENT)`。

### 2.4 状态色（在 `ui/colors` 模块）

按"日志类型"和"连接状态"两类，**每种颜色都提供深 / 浅两套**，按主题用 `themed(dark, dark_c, light_c)` 切换。

| 语义       | Dark                | Light 变体 (`_L`)   |
| ---------- | ------------------- | ------------------- |
| TX（协议发出） | `0x78 0xBE 0xFF`    | `0x1D 0x5C 0xD6`    |
| RX（协议接收） | `0x8C 0xE6 0x96`    | `0x1E 0x8A 0x44`    |
| FW_INFO    | `0xD2 0xD7 0xE1`    | `0x5C 0x62 0x6E`    |
| FW_WARN    | `0xF5 0xC3 0x5A`    | `0xA5 0x66 0x00`    |
| FW_ERROR   | `0xF5 0x6E 0x6E`    | `0xC2 0x36 0x36`    |
| APP（应用） | `0xEC 0xF0 0xF6`    | `0x2A 0x30 0x3C`    |
| STATUS_GREY  | `0xA0 0xA5 0xAF`  | `0x6E 0x74 0x80`    |
| STATUS_YELLOW | `0xF0 0xD2 0x5A`  | `0xA0 0x74 0x00`    |
| STATUS_GREEN  | `0x64 0xD7 0x64`  | `0x18 0x8A 0x38`    |

**取色工具函数**（`ui/colors`）：

```rust
pub fn themed(dark: bool, dark_c: Color32, light_c: Color32) -> Color32
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32  // 线性插值
```

**状态胶囊底色派生**（`topbar.rs::show`）：

```rust
let chip_bg = if dark {
    color.gamma_multiply(0.22)
} else {
    colors::mix(egui::Color32::WHITE, color, 0.16)
};
```

新增"语义色"必须**同步深浅两套**并写到 `ui/colors` 模块。

---

## 3. 间距 / 圆角 / 字号

在 `apply_theme` 中统一设置：

| 角色            | 值                            |
| --------------- | ----------------------------- |
| 窗口圆角        | 10（`window_corner_radius`）  |
| 菜单圆角        | 8（`menu_corner_radius`）     |
| 控件圆角        | 6（五个 widget 状态）         |
| `item_spacing`  | `vec2(8.0, 8.0)`              |
| `button_padding`| `vec2(12.0, 5.0)`             |
| `menu_margin`   | 10 / 10 / 6 / 6               |
| Heading 字号    | 22.0                          |
| Body 字号       | 15.0                          |

### 3.1 Panel 内边距约定

| 区域      | inner_margin (左/右/上/下) |
| --------- | -------------------------- |
| TopBar    | 12 / 12 / 6 / 6            |
| SideNav   | 10 / 10 / 10 / 10          |
| StatusBar | 12 / 12 / 4 / 4            |
| Toast     | 14 / 14 / 8 / 8            |
| `card()`  | 14 / 14 / 12 / 12          |
| 状态胶囊  | 10 / 10 / 3 / 3            |

### 3.2 常用局部间距

- 卡片之间：`ui.add_space(10.0~14.0)`
- 字段标签与控件之间：`ui.add_space(6.0)`
- 段落标题下：`ui.add_space(4.0)`
- 侧栏导航项之间：`ui.spacing_mut().item_spacing.y = 2.0`
- 状态栏图标对之间：`ui.style_mut().spacing.item_spacing.x = 14.0`

### 3.3 字号分级

| 用途             | size  |
| ---------------- | ----- |
| H1（ui.heading） | 22    |
| Body             | 15    |
| 卡片 / 弹窗正文  | 14~15 |
| 导航项           | 14（选中 14.5） |
| 状态栏 / 顶栏   | 12~13 |
| 提示、版本号     | 11    |

---

## 4. 三大"基础容器"

### 4.1 `card()`（内容卡片，**首选**）

`src/ui/mod.rs` 导出：

```rust
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R
```

- 底色：`ui.visuals().window_fill`
- 描边：1px，`widgets.noninteractive.bg_stroke.color`
- 圆角：10
- inner_margin：14 / 14 / 12 / 12
- 自动 `set_min_width(available_width)`

**用法**：内容页内"分组 / 区块"统一用 `card()`，不要裸写 `egui::Frame`。

### 4.2 状态胶囊（Chrome 上的指示器）

```rust
egui::Frame::new()
    .fill(chip_bg)               // 主题相关派生色
    .corner_radius(CornerRadius::same(10))
    .inner_margin(Margin { left: 10, right: 10, top: 3, bottom: 3 })
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
            ui.label(RichText::new(label).strong().size(13.0).color(color));
        });
    });
```

仅用于"顶栏状态指示"；不要复用做其他用途。

### 4.3 `ui.group()`（卡片**内**的子分组）

卡片内若需再分段，用 `ui.group(|ui| { ... })` 形成内层视觉分隔（egui 默认 group 有细描边与浅底）。

---

## 5. 控件样式约定

### 5.1 按钮（Button）

| 类型           | 填充（`fill`）                          | 描边（`stroke`） | 文字色 | 圆角 | 最小尺寸           |
| -------------- | --------------------------------------- | ---------------- | ------ | ---- | ------------------ |
| 主操作         | `ACCENT`                                | 1px `ACCENT`     | WHITE  | 6    | `vec2(76.0, 24.0)` |
| 次操作         | `interactive_fill(dark)`                | `interactive_border(dark)` | 默认 | 6    | `vec2(76.0, 24.0)` |
| 主题次级（选中）| `faint_bg_color`                        | 默认             | 默认   | 8    | 自定               |
| 图标+文字按钮  | 走 `IconTextButton`，可空 fill/stroke   | 可选             | 默认   | 6/8  | 自定               |

```rust
// 顶栏中的两档按钮写法（参考 topbar.rs）
let interactive_fill = if dark { 0x242A33 } else { 0xE9ECF1 };
let interactive_border = Stroke::new(1.0, if dark { 0x444B56 } else { 0xAEB4BE });

egui::Button::new("■ 断开")
    .fill(interactive_fill)
    .stroke(interactive_border)
    .corner_radius(CornerRadius::same(6))
    .min_size(vec2(76.0, 24.0));
```

- `corner_radius` **统一用 `CornerRadius::same(N)`**，不要用旧的 `f32` 重载。
- 关闭/删除类操作仍使用次操作按钮样式（不渲染为红框）——危险感通过 `ConfirmDialog` 表达，不要靠颜色。
- 启用/禁用使用 `ui.add_enabled(cond, btn)`；不要在 button 内手动 `if !cond` 隐藏。

### 5.2 ComboBox

在 chrome 上时边框偏淡，需要**外层套 `egui::Frame`** 强制画出清晰轮廓：

```rust
egui::Frame::new()
    .fill(interactive_fill(dark))
    .stroke(interactive_border(dark))
    .corner_radius(CornerRadius::same(6))
    .show(ui, |ui| {
        egui::ComboBox::from_id_salt("topbar-port-combo")
            .selected_text(display)
            .width(90.0)
            .show_ui(ui, |cb| { ... });
    });
```

- 必须用 `id_salt(...)`，且 salt 字符串稳定（不要混入动态值）。
- 顶栏端口下拉宽度固定 90.0；其它按内容自适应。

### 5.3 TextEdit

- 单行文本：`.desired_width(280.0)`（Settings 页默认值）。
- 密码字段：`.password(true)`。
- 必填 ID 用 `id_salt("xxx")`；循环内用 `push_id(index)`。

### 5.4 Slider / DragValue

- 0~100 连续值：`Slider`。
- 数值/枚举允许任意值：`DragValue::new(&mut v).speed(N).range(...)`。
- 触发键、录音时长等：用 `DragValue`，**speed** 给出合理步长（trigger_key=1, ms=100）。

### 5.5 Checkbox

- 直接 `ui.checkbox(&mut v, "label")` 即可，**不要**自定义 wrap。
- 旁加提示：`response.on_hover_text("...")`。

### 5.6 IconTextButton

来自 `ui/fonts.rs`，**图标 + 文本按钮**的标配，比 `ui.button(format!("{icon} {text}"))` 更可靠（后者会让图标被 Proportional 字体"吃掉"）。

```rust
crate::ui::fonts::IconTextButton::new(
    crate::ui::icons::REFRESH,
    "刷新",
    13.0,
)
.fill(interactive_fill(dark))
.corner_radius(CornerRadius::same(6))
```

可用链式方法：`gap`、`fill`、`fg`、`min_size`、`corner_radius`、`selected`。

---

## 6. 图标规范（**重点**）

### 6.1 字体注册

`ui/fonts.rs::install(ctx)`：

- 注册 Maple Mono CN（`fonts/MapleMono-NF-CN-ExtraLight.ttf`）作为 `Proportional` 主体。
- 注册 Phosphor Regular 到**独立命名字体族** `"phosphor"`（不进入 Proportional 链）。
- `Proportional` 链：移除系统 `Proportional`、移除 `"phosphor"`、保留 `maple_cn`。
- `Monospace`：仅 `maple_cn`。
- `phosphor` 家族：`["phosphor", "maple_cn"]`（maple_cn 作为 fallback，避免 egui 找不到替代字形告警）。

**关键陷阱**（必须遵守）：

- Phosphor 对小写字母只有 **零宽占位字形**，所以 `"WiFi"` 落到含 Phosphor 的字体里会被渲染成 `"WF"`。
- 因此**永远不要**把图标字符与普通文本塞进同一个 `ui.label("...")` / `ui.button("...")` / `ui.text(...)`。
- 正确做法：
  - 图标 → `icon_font_id(size)` = `FontId::new(size, FontFamily::Name("phosphor".into()))`
  - 文本 → `FontId::proportional(size)`
  - 二者各自 layout 成 galley 后拼到一行（`paint_icon_text_in` / `IconTextButton` / `paint_icon_at`）。

### 6.2 图标集中管理

**所有图标常量集中在 `src/ui/icons.rs`**，禁止业务代码里直接写 `egui_phosphor::regular::X`：

```rust
pub use egui_phosphor::regular as r;
pub const NAV_SETTINGS: &str = r::SLIDERS_HORIZONTAL;
pub const NAV_KEYMAP:   &str = r::KEYBOARD;
pub const TAB_DISPLAY:  &str = r::MONITOR;
// ...
```

新增图标必须：

1. 在 `icons.rs` 加 `pub const XXX: &str = r::XXX;`（注释所在分组）。
2. 在 `egui_phosphor` 0.11 的 `regular` 模块里确认常量存在。
3. 不混用 `regular` / `fill` / `bold` / `light` / `thin`——本项目只用 `regular`。

### 6.3 图标尺寸约定

| 场景             | size  |
| ---------------- | ----- |
| 侧栏导航         | 14（选中 14.5） |
| 顶栏 IconTextButton | 13       |
| 状态栏指标       | 12      |
| Toast            | 13      |
| 品牌 LOGO        | 22      |

图标和文字混排时用 `IconTextButton::gap(6.0)`（默认 6.0），不要靠 Unicode 空格。

### 6.4 图标与文本混排 API

按场景选：

| 场景 | API |
| ---- | --- |
| 按钮 | `IconTextButton` |
| 任意 `Rect` 内居中绘制 | `fonts::paint_icon_text_in(ui, rect, icon, text, size, color, gap)` |
| 在已有控件旁追加图标 | `fonts::paint_icon_at(ui, anchor, icon, color, size)` |
| 纯图标标签 | `RichText::new(icon).font(icon_font_id(size))` |

---

## 7. 侧栏（SideNav）规范

由 `src/ui/sidenav.rs` 实现，所有 Panel **不允许重复绘制侧栏**。

- 宽 188px 固定，`.resizable(false).exact_width(188.0)`。
- 内边距 10 / 10 / 10 / 10。
- 顶部品牌区：图标（22px，`ACCENT` 着色） + "EKeys"（16px strong） + `v{version}`（11px weak）。
- 品牌下 `ui.add_space(10.0); ui.separator(); ui.add_space(10.0);`。
- 分组：每组一个 `ui.label(weak size 11)` 标题，组下 `add_space(4.0)`，再循环渲染项。
- 组与组之间：`add_space(12.0); separator; add_space(8.0);`。
- `ui.spacing_mut().item_spacing.y = 2.0` 让项更紧凑。

**NavItem 渲染**（必须用现有 `nav_item` 私有函数，不要重写）：

- 高 32px，宽 `available_width()`。
- 选中：`ACCENT` 底 + 白字（无圆角外框）。
- hover：使用 `vis.widgets.hovered.bg_fill`。
- 禁用：透明底，文字 `gamma_multiply(0.5)`。
- 文本和图标分开用 galley 绘制（避免字体互相污染），水平起点 `left + 10`，间距 6。

**新增导航项**：

1. 在 `ITEMS` 数组追加 `NavItem`（含 `page / label / icon / enabled / hint`）。
2. `icon` 必须来自 `crate::ui::icons::*`。
3. `enabled: false` 时点击会触发"该功能将在固件阶段 XX 启用"提示（看 ui-design.md §2.2）。

---

## 8. 顶栏（TopBar）规范

由 `src/ui/topbar.rs` 实现。

- `egui::MenuBar::new().ui(ui, |ui| { ... })` 包裹整段。
- 左到右元素顺序：
  1. 状态胶囊（最左，`add_space(0)`）。
  2. `add_space(6.0)`。
  3. 端口 ComboBox + 扫描按钮（`horizontal`）。
  4. `add_space(4.0)`。
  5. 连接 / 断开按钮组。
  6. `add_space(6.0)`。
  7. 端口名（`{port} · 115200`，`.weak()`）。
  8. 设备信息（仅 Online 时显示，`{device_name} · v{fw_version} · {device_id}`，`.weak().size(12.0)`）。
  9. `add_space(6.0)` + 自动连接 checkbox。
  10. CH340 警告（仅当当前端口 VID = `WCH_VID` 时显示）。
  11. 停止重连（仅 `pending_reconnect.is_some() && !is_online`）。
  12. **右侧**：`with_layout(Layout::right_to_left(Align::Center), ...)` 放「本地设置」+「刷新」。

**约束**：

- 任何在 chrome 上的交互控件（ComboBox、Button、IconTextButton）都**必须**显式给 fill / stroke，否则会"消失"在底色里。
- 连接按钮 `can_connect = !is_online && st.selected.is_some()`，断开 `is_online`。
- 自动连接 checkbox 改变只影响**下次启动**，不立即发起连接（防止多请求阻塞 UI）——同时设置 `st.auto_connect_done = true` 抑制自动连接块。
- 主动断开必须设 `st.auto_connect_done = true`，否则底部自动连接块会当帧把连接拉回来。

---

## 9. 底栏（StatusBar）规范

由 `src/ui/statusbar.rs` 实现。

- `egui::MenuBar::new().ui(ui, |ui| { ... })`。
- `ui.style_mut().spacing.item_spacing.x = 14.0;` 让指标之间宽松。
- 元素顺序（左→右）：Uptime · 已发送 · 已接收 …… 右侧：`心跳 N秒`。
- 文本统一 `.weak()`；图标 12px。
- 状态指示**不**放底栏（顶栏胶囊已经表达，避免重复）。

---

## 10. 内容页（Panel）规范

每个 Panel 是 `src/ui/panel_xxx.rs` + 同名 `*PanelState` 结构体。

### 10.1 通用签名

```rust
#[derive(Default)]
pub struct XxxPanelState { /* 跨帧状态：port list、search text、scroll offset 等 */ }

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut XxxPanelState) {
    // 渲染
}
```

- State 持有**真正需要跨帧的状态**（如端口扫描结果、用户输入、scroll 位置），**不要**缓存可以每帧重新计算的简单值。
- 不在 `update()` 内塞业务逻辑，复杂组件必须抽成 struct / 方法（参考 `rules.md` §6）。

### 10.2 页面结构模板

```rust
pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut XxxState) {
    // 1. 页头（一行标题 + 一行副标题）
    ui.heading("页面标题");
    ui.label("简短描述");
    ui.add_space(4.0);

    // 2. 卡片 1
    ui.add_space(8.0);
    crate::ui::card(ui, |ui| { /* ... */ });

    // 3. 卡片 2 ...
}
```

- 页面**首行**直接 `ui.heading(...)`（不放入 card），后续用 `card()` 分组。
- 字段密集时（Settings / Voice / WiFi 等）用 `settings_panel_scaffold(handle, ui, |ui, snapshot, draft| { ... })` 拿 draft（见 `ui/widgets.rs`），不要直接读 snapshot 后做"未下发 diff"。

### 10.3 Settings 风格字段

- 每个字段：`ui.label("字段名")` + `ui.add_space(4.0~6.0)` + 控件。
- 用 `ui.group(|ui| { ... })` 在 card 内再分段（"语音功能开关" / "腾讯云 API 配置"）。
- 标签前不加 `*` 必填标记——错误用 `ui.colored_label(warn_fg_color, "...")` 直接给提示。
- 校验在 `apply_settings_snapshot` / `build_config_payload` 中做，UI 端只负责"用户改了 → 写 draft"。

### 10.4 敏感字段渲染（密码 / Token）

参考 `panel_voice.rs::preview_mask`：

- `draft == snapshot` → 显示首尾可见的 `preview_mask`（如 `abc***xyz`），长度 ≤ 8 全 `*`。
- `draft != snapshot` → 显示 draft 明文。
- 写回时仍以真实值为准（不要被显示态污染 draft）。
- SecretKey 用 `TextEdit::password(true)`。

### 10.5 进入页面边沿拉取

在 `app.rs::update` 用 `last_page` 边沿检测，进入指定页面时调一次 `handle.refresh_xxx()`（`Keymap` / `Audio` 等已实现）。**新 Panel 需要进入边沿拉数据的，参照该模式**——失败静默（离线 / 旧固件），不弹错误。

### 10.6 滚动

页面**已经**被 `app.rs` 的外层 `ScrollArea::vertical().auto_shrink([false, false])` 包裹：

- 页面内不要再包一层 `ScrollArea`（除非有特殊内嵌需要，如 Keymap）。
- Keymap 是特例，自带滚动 + top/bottom 段——见 `app.rs` 中 `if matches!(current_page, Page::Keymap)` 分支。

---

## 11. 弹窗（Modal / Window）

### 11.1 Confirm Dialog

来自 `ui/widgets.rs::show_confirm`：

```rust
egui::Window::new(title)
    .open(open)
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .default_pos([0.0, 0.0])          // 防止 egui 记忆位置飘走
    .default_size([380.0, 180.0])
    .min_size([320.0, 140.0])
    .max_size([520.0, 320.0])
    .show(ctx, |ui| {
        ui.label(body);
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let primary = Button::new(RichText::new("继续").color(WHITE))
                .fill(ACCENT)
                .corner_radius(CornerRadius::same(6));
            if ui.add(primary).clicked() { outcome = Yes; }
            if ui.button("取消").clicked() { outcome = No; }
        });
    });
```

**约定**：

- 主操作按钮用 `ACCENT` + 白字；次操作默认按钮即可。
- 弹窗文案**写明后果**（不只是"是否继续？"），让用户在点击 Yes 前知道影响。
- 危险操作前要二次确认（工作模式切换、进入烧录模式、恢复出厂等），通过 `UiConfirmKind` 派发。

### 11.2 Local Settings 弹窗

- 居中，`default_size([420.0, 360.0])`。
- 用 `ui.group(|ui| { ui.strong("分组标题"); ... })` 切分：连接 / 语言 / 主题 / 窗口。
- 主题切换按钮**两段式**：未选中 `faint_bg_color` 底，选中 `ACCENT` 底 + 白字，圆角 8。

### 11.3 Toast

来自 `ui/widgets.rs::show_toasts` + `push_toast`：

- 锚点：`Align2::RIGHT_BOTTOM, [-12.0, -36.0]`（避免被状态栏遮挡）。
- 颜色映射：
  - Info: `Color32::from_rgb(80, 130, 180)` + `TOAST_INFO`
  - Success: `Color32::from_rgb(80, 160, 90)` + `TOAST_SUCCESS`
  - Warning: `Color32::from_rgb(200, 160, 60)` + `TOAST_WARNING`
  - Error: `Color32::from_rgb(200, 80, 80)` + `TOAST_ERROR`
- 容器：`corner_radius(8)`，inner_margin 14/14/8/8，`max_width(240.0)`。
- 内容用 `paint_icon_text_in(..., 13.0, WHITE, 6.0)`。
- TTL 默认：
  - Error: 5000ms
  - Warning: 4000ms
  - Info / Success: 2500ms
- 业务代码通过 `handle.ui_tx.send(UiEvent::Toast(kind, text))` 派发，由 `app.rs::drain_ui_events` 收尾；不要直接 push 到 `toasts` Vec。

---

## 12. 状态、事件与刷新

- **共享状态**：`AppHandle`（`Arc<...>`）持有 `Mutex<state>`、`Mutex<keymap>`、`Mutex<device_info>` 等；UI 通过 `handle.xxx.lock().unwrap()` 读取。
- **读出即释放**：`let v = handle.foo.lock().unwrap().clone();` 之后立刻 drop guard，**不要再持锁调用 `attempt_connect` / `schedule_reconnect` 等会再次锁同一互斥体的函数**——见 `topbar.rs::show` 中 `last_port` 取出后释放再调度的注释。
- **事件总线**：
  - `LinkEvent`（来自 reader）→ `app.rs::drain_link_events`。
  - `UiEvent`（来自 panel）→ `handle.ui_tx.send(...)` → `app.rs::drain_ui_events`。
- **页面导航**：`handle.ui_tx.send(UiEvent::Navigate(p))`，**不要**直接 `*handle.page.lock() = p`。
- **持续刷新**：`ctx.request_repaint_after(Duration::from_millis(100))`（已写在 `app.rs::update` 末尾）；有动画/计时器时各自 `request_repaint()`；空闲时该机制可保 10Hz 刷新。
- **动画**：用 `ctx.input(|i| i.time)`，不要按帧数假设 FPS。

---

## 13. 快捷键

由 `app.rs::handle_shortcuts` 统一处理，新快捷键在此注册：

| 快捷键 | 行为 |
| ------ | ---- |
| `F5`   | 跳到 Settings 并触发 `handle_refresh` |
| `Ctrl+L` | 跳到 Log |
| `Ctrl+1` ~ `Ctrl+8` | 跳到对应 SideNav 页面 |
| `Esc`   | 放弃待下发设置（Settings 页内） |
| `Ctrl+Enter` | 应用待下发设置（Settings 页内） |

- **Keymap 捕获模式**：当 `handle.capture_keyboard` 为 true 时，全局快捷键必须让路（`if capturing { return; }`），否则捕获不到用户实际按下的键。

---

## 14. 命名与文件组织

| 类型 | 路径 | 命名 |
| ---- | ---- | ---- |
| 入口 | `src/app.rs` | `WxiApp` |
| Panel | `src/ui/panel_xxx.rs` | `xxx::XxxPanelState` + `xxx::show(handle, ui, st)` |
| 共享小部件 | `src/ui/widgets.rs` | `Toast`、`ConfirmOutcome`、`show_*` |
| 图标常量 | `src/ui/icons.rs` | 全大写 `NAV_*` / `TAB_*` / `LOG_*` |
| 字体/Phosphor 帮助 | `src/ui/fonts.rs` | `install` / `paint_icon_*` / `IconTextButton` |
| 主题/颜色/卡片 | `src/ui/mod.rs` | `apply_theme` / `card` / `ACCENT` / `colors::*` |
| 顶/侧/底栏 | `src/ui/topbar.rs` / `sidenav.rs` / `statusbar.rs` | `show(handle, ui, ...)` |
| 协议层 | `src/protocol.rs` | `DeviceSettings` / `FieldMask` |
| 共享状态 | `src/state/*.rs` | `AppHandle` / `Page` / `UiEvent` / `UiConfirmKind` / `ToastKind` |

**禁止**：

- 在 Panel 文件里手写 `egui::Frame` 代替 `card()`。
- 在业务代码里直接写 `egui_phosphor::regular::X`——必须走 `icons::*`。
- 在 Panel 里写"另一份主题常量"——所有主题相关值都在 `ui/mod.rs::apply_theme` + `ui/colors`。
- 把 `egui::Context` / `Ui` 长期持有（违反 immediate mode）。

---

## 15. 编译前自检

新增/修改 UI 代码后必须：

1. `cargo check` 通过。
2. `cargo clippy` 无新增 warning（特别是 `clippy::borrow_interior_mutable_const` 这类与 `Arc<Mutex>` 相关的）。
3. 切换深 / 浅主题各看一遍：状态胶囊底色、ComboBox / 按钮边框、敏感字段掩码、Toast 颜色均应可读且对比足够。
4. 缩放窗口到 800×500：所有页面（含 Keymap 自带滚动）均不应被截断。
5. 中文 + 英文混排不出现 □（漏挂字体 fallback）或 "WiFi" → "WF"（图标字体污染文本）。
6. 启动时无 "Failed to find replacement characters" 警告（fonts.rs 已通过把 maple_cn 挂到 phosphor 家族规避）。
7. 关闭 / 重新打开 Local Settings 不残留焦点。
8. 离线状态下所有页面（除依赖连接的字段）仍可正常浏览，不能整页崩溃。

---

## 16. 变更流程

修改 UI 时：

1. 先读 `ui/mod.rs` + `ui/widgets.rs` + 相关 `panel_xxx.rs`，理解现有视觉惯例。
2. 优先复用 `card()` / `IconTextButton` / `paint_icon_text_in` / `show_confirm` / `show_toasts`。
3. 颜色用 `colors::themed(dark, ...)` / `colors::mix(...)` / `ACCENT` / `visuals.*`，不要硬编码新 RGB。
4. 新增图标先在 `icons.rs` 注册。
5. 提交前跑 `cargo check` + `cargo clippy` + 主题切换目视。
6. 与本文不一致时，**优先以代码现状为准**（本文是规范也是描述）；若是新引入的样式，请顺手把本文一并更新。
