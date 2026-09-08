# EKeysApp

桌面端控制应用，基于 Rust + [egui](https://github.com/emilk/egui)（eframe 0.33）。通过 USB CDC 串口与 `EKeys` 键盘固件通信，用于参数配置、键映射编辑、灯效 / WiFi / 语音参数下发与日志查看。

> 配套文档：
>
> - 协议约定：[`docs/protocol-usage.md`](./docs/protocol-usage.md)
> - 工程结构：[`docs/project-structure-plan.md`](./docs/project-structure-plan.md)
> - 交互设计：[`docs/ui-design.md`](./docs/ui-design.md)

## 特性

- 串口连接管理（VID `0x303A` 过滤、状态机、自动重连、心跳保活）
- 全量配置 GET/SET（26 字段 `DeviceSettings`，增量下发 + 字段掩码）
- 键映射（Keymap）编辑：本地模型 ↔ 固件模型映射、回读
- 灯效 / WiFi / 语音 / Profile 图标等子页面
- 协议 / 固件 / 应用三类日志分色展示 + Toast / Confirm 弹窗
- 敏感字段（WiFi 密码、语音 Key）自动脱敏
- 本地持久化：最近端口、自动连接、窗口尺寸、语言、主题

## 环境

- Rust 2024 edition
- Windows（已使用 `windows_subsystem = "windows"` 隐藏 release 版控制台；其他平台理论可编译但未验证）

## 构建运行

```bash
# 开发构建并启动
cargo run

# Release 构建
cargo build --release
# 产物位于 target/release/wxi.exe
```

## 快捷键

| 快捷键     | 动作                                                                                    |
| ---------- | --------------------------------------------------------------------------------------- |
| `F5`       | 跳到设置页                                                                              |
| `Ctrl+L`   | 跳到日志页                                                                              |
| `Ctrl+1~8` | 切换到对应导航页（Connect / Settings / Keymap / Lighting / WiFi / Voice / Log / About） |

> 当 Keymap 面板进入"按下任意键捕获"模式时，全局快捷键让路以免吞键。

## 工程结构

```
EKeysApp/
├── Cargo.toml
├── docs/                       # 协议、结构、UI 设计文档
├── fonts/                      # 内嵌字体（MapleMono NF CN）
├── img/                        # 程序图标（编译期嵌入）
└── src/
    ├── main.rs                 # 入口：装载字体 + 启动 App
    ├── app.rs                  # eframe::App 实现，事件分发
    ├── protocol.rs             # 命令常量、Frame 编解码、DeviceSettings
    ├── link/                   # 串口 + reader/writer/router/heartbeat 线程
    │   ├── mod.rs              # LinkManager、ConnectionState
    │   ├── serial.rs           # 串口枚举与打开
    │   ├── reader.rs           # 后台读线程
    │   └── heartbeat.rs        # 心跳
    ├── state/mod.rs            # AppHandle：共享状态 + UI 事件 channel
    ├── config/mod.rs           # 本地偏好（%APPDATA%/wxi/config.json）
    ├── ui/                     # 面板与共享组件
    │   ├── topbar.rs           # 顶部状态条
    │   ├── sidenav.rs          # 左侧导航
    │   ├── statusbar.rs        # 底部状态条
    │   ├── panel_connection.rs # P1 连接
    │   ├── panel_settings.rs   # P2 设置（核心）
    │   ├── panel_keymap.rs     # P3 键映射
    │   ├── panel_lighting.rs   # P4 灯效
    │   ├── panel_wifi.rs       # P5 WiFi
    │   ├── panel_voice.rs      # P6 语音
    │   ├── panel_log.rs        # P7 日志
    │   ├── panel_about.rs      # P8 关于
    │   ├── widgets.rs          # Toast / Confirm / FieldEditor / DiffPreviewBar
    │   ├── fonts.rs            # 字体注册
    │   └── icons.rs            # 图标常量
    └── util/log.rs             # tracing 初始化 + 日志缓冲
```

## 关键依赖

| Crate                            | 用途                           |
| -------------------------------- | ------------------------------ |
| `eframe` 0.33                    | 窗口与渲染                     |
| `egui-phosphor`                  | 图标字体                       |
| `serialport` 4.6                 | 串口枚举与读写                 |
| `serde` / `serde_json`           | Frame / 配置 JSON 序列化       |
| `tracing` / `tracing-subscriber` | 日志                           |
| `thiserror`                      | 错误类型                       |
| `dirs`                           | 平台配置目录                   |
| `image` / `base64`               | 程序图标解码、Profile 图标上传 |

## 协议简述

- 物理层：串口（LF 行分隔），一帧一行 JSON；
- `cmd` 最高位表示响应（`req | 0x80`），`seq = 0` 表示设备主动推送；
- 推送的全量配置走 `response_cmd(CMD_CONFIG_GET) = 0x87`；
- 异类命令（`0x10` / `0x0c` / `0x0f`）body 在帧顶层，单独解析；
- SET 增量下发：`data.config` 只放有变化的字段，由 `FieldMask` 标记。

完整字段表、命令清单与钳位规则见 [`docs/protocol-usage.md`](./docs/protocol-usage.md)。

## 开发约定

- UI 渲染 / 业务意图解耦：面板不直接操作 `LinkManager`，所有副作用经 `AppHandle` 事件 channel；
- 每帧通过 `drain_link_events` / `drain_ui_events` 拉取事件再渲染，不在 UI 线程阻塞 IO；
- 资源（字体、图标、纹理）按需缓存，避免每帧重建；
- 动画以 `ctx.input(|i| i.time)` 驱动，无动画时不调用 `request_repaint`。

详见 `.trae/rules/rules.md`。
