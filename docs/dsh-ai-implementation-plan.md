# DSH 插件能力 + 键盘 AI 页面 实施方案

> 状态：方案评审稿（2026-10-10）
> 范围：DSH 插件（新） / EKeysApp（Rust 桌面中间层） / EKeys 固件（ESP32）
> 前置决策（已确认）：
>
> 1. 插件 ↔ APP 走 **APP 内置 127.0.0.1 HTTP API（REST + SSE）**
> 2. 按键事件回传内容为 **键 ID(1~11) + 按下/抬起 + 时间戳**（原始边沿）
> 3. APP 端**新增 AI 面板**用于观测链路

---

## 1. 目标

构建一条三层链路，让 AI（DeepSeek Harness 会话）可以主动调用键盘硬件能力：

- **下行**：AI 发送文字 → 键盘屏幕（新增 AI 页面）多行显示；AI 控制指定按键的 LED 颜色。
- **上行**：用户在键盘上的按键事件 → APP → 插件 → 注入 AI 会话，作为 AI 的输入。

APP 作为中间层，向下通过串口 JSON 行协议管理固件，向上暴露本地 HTTP API 给 DSH 插件。

### 总体架构

```
┌─────────────────────┐
│  DeepSeek AI 会话    │  模型推理 · Agent 循环
└─────────┬───────────┘
          │ tool 调用                ▲ followup 注入
┌─────────▼───────────┐              │
│  DSH 插件 dsh-ekeys │  defineTool · SSE 监听
└─────────┬───────────┘              │
          │ HTTP 127.0.0.1           │ SSE 推送
┌─────────▼───────────┐              │
│  EKeysApp 桌面中间层 │  本地 HTTP API · 串口 JSON 转发
└─────────┬───────────┘              │
          │ 串口 0x18/0x19/0x1A      │ 0x09 按键推送
┌─────────▼───────────┐              │
│  EKeys 键盘固件      │──────────────┘
│  AI 页面 · 单键 LED · 按键扫描      │
└─────────────────────┘
```

下行 = 显示 / 灯光控制（品牌主路径）；上行 = 按键事件回传（异步事件流）。

---

## 2. 现状盘点（代码调研结论）

| 层         | 现状                                                                                                                                                                      | 关键证据                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| 固件 EKeys | LVGL 8.3 + SquareLine 生成 11 屏；`DisplayTask::navigateNow()` 按 `ui_screen_tag_t` 切屏；命令经 `CommandRegistry::registerHandler` 注册；已有 TCP 通道与串口复用同一分发 | `src/ui/ui.c` L31-L57、`src/protocol/registration.cpp` L25-L38                |
| 协议       | 0x01~0x17 已定义；**0x09 按键上报双端均为「仅定义未接通」**；串口行缓冲 2048B                                                                                             | `src/protocol/SerialProtocol.h` L24-L49、`EKeysApp/src/protocol.rs` L137-L138 |
| 灯光       | 11 键各 1 颗 LED（`RGBDriver::kLedCount = 11`）；`setHighlight` 只有 bool 高亮，**无任意颜色单键接口**；全局灯效每帧重绘会覆盖临时状态                                    | `src/rgb/RGBLightControl.h` L84-L108、`src/rgb/RGBDriver.h` L29               |
| APP        | 同步 `request()` / 异步 `request_async()` / 跨线程 `requester()` 齐备；`handle_link_event` 尚不识别 0x09；**无任何本地服务器，无 tokio/axum 依赖**                        | `src/link/mod.rs` L528-L562、`Cargo.toml` L15-L48                             |
| DSH 插件   | 生态为 npm 包 + Cordis 框架；`ctx.tools.register(defineTool({...}))` 注册工具；按键回传无官方专门 API，社区通行 `agent.followup()`                                        | dshai.org、deepseek-harness `docs/cookbook/adding-a-tool.md`                  |

结论：三层均有可复用挂点，空白点只有四处——**0x09 链路、单键颜色接口、APP 本地 API、插件本身**。

---

## 3. 协议扩展（双端同步实现）

| 命令                 | 方向                      | data 载荷                                                      | 说明                       |
| -------------------- | ------------------------- | -------------------------------------------------------------- | -------------------------- |
| `0x18 CMD_AI_TEXT`   | APP → 固件                | `{op:"set"\|"append"\|"clear", text}`                          | AI 屏文本控制              |
| `0x19 CMD_AI_LED`    | APP → 固件                | `{op:"set"\|"clear"\|"clear_all", key:1..11, color:"#RRGGBB"}` | 单键 LED 控制              |
| `0x1A CMD_AI_EVENT`  | APP → 固件                | `{enable:bool}`                                                | 按键上报开关               |
| `0x09 CMD_KEY_EVENT` | 固件 → APP（push, seq=0） | `{key_id, pressed, ts_ms}`                                     | **首次接通**，物理按键边沿 |

约定：

- 响应帧沿用 `cmd | 0x80` + `status/error` 标准格式。
- **长文本分帧**：行缓冲 2048B，APP 侧按 ≤1400B 切块用 `append` 下发；JSON 中文按 UTF-8 原样编码（serde_json 不转 `\u` 转义）。
- 0x09 使用标准 `data` 字段，无需改 `is_top_level_cmd`。
- `PROTOCOL_VERSION` 保持 v1（新增命令属向下兼容扩展）。
- 文档同步：`EKeysApp/docs/protocol-usage.md` 命令表、固件 `FEATURE_DOC.md` §5.3。

---

## 4. 固件侧实施（EKeys）

### 4.1 AI 屏幕

- 新增 `src/ui/ui_AIScreen.c/h`，沿用 SquareLine 生成文件结构；参照 `ui_FlipClock` 的「独立文件」做法，`ui.c` 只加 init/destroy 两行（注意 SquareLine 重新生成的覆盖风险）。
- `ui_screen_tag_t` 枚举（`src/ui/ui_KeyMapped.h`）追加 `UI_SCREEN_AI`。
- 屏幕内容：
  - 多行 `lv_label`：`\n` 分行 + `LV_LABEL_LONG_WRAP` 自动换行；中文用现有 `FontCKJGT` CJK 字体族。
  - 顶部状态条：`等待连接中`（默认）/ `已连接(USB)` / `已连接(TCP)`——由 `SerialProtocol` 新增的 `last_app_frame_ms`（收到任意合法 APP 帧刷新）+ `line_sink_` 状态判断。

### 4.2 导航接入

- `DisplayTask::navigateNow()`（`src/tasks/DisplayTask.cpp` L419）支持新 tag；`isSecondaryScreen()` 归类。
- 入口：主屏/设置屏菜单加「AI」项（具体注册点实现时按 MainTask 菜由确认，见 §10 待定项）。
- **AI 页建议禁用空闲自动回主屏**，避免显示内容被超时清掉。

### 4.3 命令处理

- 新增 `src/protocol/commands/cmd_ai.cpp/h`，在 `registration.cpp::registerAllCommandHandlers()` 注册 0x18/0x19/0x1A。
- 文本命令经 `message_types.h` 消息队列投递到 DisplayTask 线程操作 LVGL（**LVGL 对象只能在 UI 任务线程碰**）；灯光命令直接写 `RGBLightControl`。

### 4.4 单键灯光

- `RGBLightControl` 新增 `ai_color_[11]`（可选 RGB override 数组），`renderFrame()` 中优先级**高于**全局灯效模式与 `highlight_[]`。
- 生命周期：AI 页退出或收到 `clear_all` 时清除，恢复全局灯效。

### 4.5 按键事件上报

- 钩子：`KeyEventDispatcher::onKeyEdge(key_id, pressed)`（`src/keymap/KeyEventDispatcher.cpp`）。
- 条件：`当前屏 == UI_SCREEN_AI && 0x1A enable` 时调用新增的 `SerialProtocol::sendKeyEvent()`（仿 `sendMusicControl` 实现，seq=0 push）。

---

## 5. APP 侧实施（EKeysApp）

### 5.1 协议层

`src/protocol.rs`：新增 3 个 CMD 常量、0x09 推送数据结构与解析。

### 5.2 本地 HTTP 服务（新模块 `src/server/mod.rs`）

- 实现：`std::net::TcpListener` 独立线程 + `tiny_http`（纯同步，不引入 tokio）。
- 安全：仅绑定 `127.0.0.1`，所有请求校验 `Authorization: Bearer <token>`。
- API：
  | 方法 | 路径 | 用途 |
  |---|---|---|
  | GET | `/v1/status` | APP / 设备 / AI 页连接状态 |
  | POST | `/v1/display` | `{text, mode: replace\|append\|clear}` 下发屏幕文本 |
  | POST | `/v1/led` | `{key: 1..11, color: "#RRGGBB"\|null}` 单键灯光 |
  | POST | `/v1/keys` | `{enable: bool}` 按键上报开关 |
  | GET | `/v1/events` | SSE：按键事件 + 连接状态变化 |
- 设备调用：持有**克隆的 `LinkRequester`**（跨线程句柄，`LinkManager::requester()`），在 HTTP 线程同步 `request()`（3s 超时）。**绝不在线程中触碰 UI 状态锁。**

### 5.3 0x09 消费

`src/app.rs::handle_link_event` 识别 0x09 push → 写入按键事件环形缓冲 + 广播给 SSE 客户端 + Log 面板打日志。

### 5.4 AI 面板

- `Page::Ai` + `src/ui/panel_ai.rs` + `AiPanelState`。
- 内容：服务开关 / 端口 / token（一键复制）、设备与 AI 页连接状态、最近按键事件列表、下发历史。
- 按 ui-style 规范接入：`sidenav::ITEMS` 注册导航项、`icons.rs` 注册图标、`app.rs` 初始化 state、`card()` 分组。

### 5.5 配置与验收

- `src/config/mod.rs` 持久化 `{enabled, port, token}`，首次启动生成 token。
- 完成后 `cargo check` + `cargo clippy` 零新增告警；深浅主题 + 800×500 目视自检（ui-style §15）。

---

## 6. DSH 插件实施（新仓库 `dsh-ekeys`）

### 6.1 包结构（官方规范）

```
dsh-ekeys/
├── package.json        # dsh.bundle → cordis.patch.yml；keywords: dsh-plugin
├── cordis.patch.yml    # - insert: [{id: ekeys, name: dsh-ekeys}]
├── index.ts            # apply(ctx) 入口
└── README.md           # 含配置块
```

### 6.2 注册工具（`defineTool`）

```ts
import type { Context } from "@deepseek-ai/cordis";
import { defineTool } from "@deepseek-ai/dsh-tools";

export const name = "ekeys";
export const inject = ["tools"];

export function apply(ctx: Context) {
  ctx.tools.register(
    defineTool({
      name: "ekeys_display_text",
      description: "在 EKeys 键盘的 AI 页面上显示文字（支持多行中文）",
      parameters: {
        text: {
          type: "string",
          required: true,
          description: "要显示的文本，\\n 换行",
        },
        mode: {
          type: "string",
          required: false,
          description: "replace | append | clear",
        },
      },
      async execute(args) {
        /* POST /v1/display */
      },
    }),
  );
  // ekeys_set_key_light {key_id, color?}  → POST /v1/led
  // ekeys_get_status   {}                 → GET  /v1/status
}
```

- 工具描述写清「APP 未启动 / 键盘未连接」的报错语义，让 AI 能自行降级。
- 可选第四工具 `ekeys_clear_screen`（等价 `mode: clear`，视需要）。

### 6.3 按键事件回传

- 插件激活时起 SSE 长连 `GET /v1/events`，收到事件调 `agent.followup()` 注入会话。
- 注意：followup 入参需含 `kind` / `source.kind` 对象字段；**精确 schema 官方未定稿，实现时以 `dsh-agent-message` / `dsh-weixin` 插件源码为准**。

### 6.4 发布

- 包名 `dsh-ekeys`；`package.json` keywords 加 `dsh-plugin`、`deepseek-harness`。
- GitHub topic `dsh-plugin`；README 一句话描述 + 配置块（注册表自动抓取，约 24h 上架）。
- 装机：`npx @deepseek-ai/dsh plugin add dsh-ekeys`。

---

## 7. 实施顺序（里程碑）

| 里程碑             | 内容                                               | 出口条件                                       |
| ------------------ | -------------------------------------------------- | ---------------------------------------------- |
| **M1 协议 + 固件** | 0x18/19/1A handler、AI 屏、LED override、0x09 推送 | 串口工具手动发帧可见屏显/灯光变化，按键有 push |
| **M2 APP**         | 协议常量 → 0x09 消费 → HTTP API → AI 面板          | curl 走通 display/led/events，面板可见按键事件 |
| **M3 插件**        | tools 封装 + SSE 回传                              | DSH 会话中调用工具生效，按键进会话             |
| **M4 联调**        | 全场景验收 + 两端协议文档同步                      | §8 全部通过                                    |

---

## 8. 验收场景

- [ ] AI 发中文多行文本 → 键盘屏完整显示（含 >1400B 长文本分帧）
- [ ] AI 点亮键 5 红色 → LED 变色；切屏 / `clear_all` → 恢复全局灯效
- [ ] 进入 AI 屏默认「等待连接中」，APP 连接后变「已连接」
- [ ] AI 页按下/抬起任意键 → APP 面板实时可见 → AI 会话收到 followup
- [ ] 文本 `clear` 后屏幕恢复默认提示
- [ ] APP 未启动、键盘未连接、设备超时三类错误返回可读文案
- [ ] `cargo check` / `cargo clippy` 零新增告警；固件编译通过
- [ ] 深浅主题下面板可读；800×500 窗口不截断

---

## 9. 风险与对策

| 风险                           | 对策                                            |
| ------------------------------ | ----------------------------------------------- |
| 2048B 行缓冲截断长文本         | APP 侧统一分帧 + `append`；单帧 ≤1400B          |
| 同步 `request()` 阻塞 UI       | 只在 HTTP 专用线程调用，禁入 UI 线程            |
| SquareLine 重新生成覆盖 `ui.c` | 参照 `ui_FlipClock` 独立文件，`ui.c` 改动最小化 |
| 全局灯效覆盖单键色             | override 最高优先级 + 生命周期清除              |
| `agent.followup` API 未定稿    | 实现期对照社区插件源码，锁定依赖版本            |
| DSH 生态演进快                 | 插件锁定 `@deepseek-ai/*` 依赖版本              |
| 0x09 首次接通两端不一致        | 先冻结 §3 载荷格式，双端同迭代实现              |

---

## 10. 待定项

- **AI 页在键盘上的进入方式**：主屏菜单项 or 按键组合？——决定固件导航改动的具体位置，其余可直接开工。
- AI 页是否完全禁用空闲回主屏（本方案建议禁用）。
- 插件是否需要第 4 个 `ekeys_clear_screen` 工具（可并入 display 的 `mode` 参数）。
