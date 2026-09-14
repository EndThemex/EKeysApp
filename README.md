# EKeysApp

桌面端控制应用，基于 Rust + [egui](https://github.com/emilk/egui)（eframe 0.33）。通过 USB CDC 串口与 `EKeys` 键盘固件通信，用于参数配置、键映射编辑、灯效 / WiFi / 语音 / 音效下发与日志查看。

> 协议约定：[`docs/protocol-usage.md`](./docs/protocol-usage.md)（协议版本 **v1.0**，见文档 §0）

[![CI](https://img.shields.io/github/actions/workflow/status/EndThemex/EKeysApp/ci.yml?branch=main&label=CI)](./.github/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-orange)](./Cargo.toml)
[![Protocol](https://img.shields.io/badge/Protocol-v1.0-informational)](./docs/protocol-usage.md)
[![Platform](https://img.shields.io/badge/Platform-Windows-lightgrey)](./docs/build.md#跨平台编译)
[![GitHub issues](https://img.shields.io/github/issues/EndThemex/EKeysApp)](https://github.com/EndThemex/EKeysApp/issues)

> 截图待维护者补全（建议放入 `docs/screenshots/`，README 顶部插入）。

---

## 目录

- [特性](#特性)
- [平台支持](#平台支持)
- [环境与构建](#环境与构建)
- [下载](#下载)
- [快捷键](#快捷键)
- [工程结构](#工程结构)
- [关键依赖](#关键依赖)
- [协议简述](#协议简述)
- [开源治理](#开源治理)
- [致谢](#致谢)
- [开发约定](#开发约定)

---

## 特性

- **连接管理**：原生 USB CDC 串口、CH340 等 WCH 桥接芯片识别 + 自动拒绝、心跳保活、指数退避自动重连（1→2→4→5s，5 次后放弃）、启动自动连接上次端口
- **全量配置 GET / SET**：25 字段 `DeviceSettings`，增量下发（仅 `data.config` 携带差异字段）+ 协议侧钳位
- **8 个功能页**：设备设置（多组卡片）、键映射（4 层 × 物理键 → 设备 11 键映射）、灯效、WiFi、语音（腾讯云一句话识别）、音效板（音频上传 + 11 键绑定 + 试播）、日志、关于
- **键映射编辑**：本地 `KeymapData` ↔ 固件 `FirmwareKeyEntry` 模型映射、FUN 组合键、Profile 名称 / 自定义图标、进入页面边沿拉取
- **OTA**：本地一次性 HTTP 服务 + MD5 校验，局域网下发 `.bin` 固件（详见 [`docs/ota.md`](./docs/ota.md)）
- **PC 状态推送**：采集键盘 Lock / 网络 / CPU / 内存，每秒 diff-based 推送到设备（`0x0D`），可在 Settings → PC 状态 开关
- **协议 / 固件 / 应用** 三类日志分色展示（Tx 蓝 / Rx 绿 / Firmware 灰白 / App 浅黄）；Toast / Confirm 弹窗
- **敏感字段自动脱敏**：WiFi 密码、腾讯云 SecretId / SecretKey 设备回传自动替换为 `***`，UI 用 `preview_mask` 显示首尾
- **本地持久化**：`%APPDATA%/wxi/config.json`（最近端口、自动连接、窗口尺寸、语言、主题、PC 状态推送开关）
- **主题**：深 / 浅双主题统一切换；图标用 Phosphor + Maple Mono CN 字体混排

---

## 平台支持

| 平台                  | 状态          | 说明                                                            |
| --------------------- | ------------- | --------------------------------------------------------------- |
| **Windows 10/11 x64** | ✅ 已验证     | 主开发目标，CI 跑通 `check / clippy / test`                     |
| **Windows 11 ARM64**  | ⚠️ 理论可编译 | 未实测；`cargo build --target aarch64-pc-windows-msvc` 应可出包 |
| **macOS**             | ⚠️ 仅编译验证 | CI 跑通编译；`pc_status` 等 Windows-only 模块需补全             |
| **Linux**             | ⚠️ 仅编译验证 | CI 跑通编译；运行时需加入 `dialout` 组 + 补 `pc_status`         |

详细跨平台构建指南见 [`docs/build.md`](./docs/build.md#跨平台编译)。

---

## 环境与构建

- Rust **stable**，**MSRV = 1.85**（见 `Cargo.toml`）
- Windows：Visual Studio Build Tools 2022（C++ workload）
- 其他系统依赖见 [`docs/build.md`](./docs/build.md)

```bash
# 开发构建并启动
cargo run

# Release 构建
cargo build --release
# Windows 产物位于 target/release/wxi.exe

# 严格 clippy + 全量测试
cargo clippy --all-targets --locked --no-deps
cargo test --all-targets --locked
```

完整命令清单、调试技巧、跨平台编译见 [`docs/build.md`](./docs/build.md)。

> 当前尚未发布预编译包，请通过 `cargo build --release` 自行产出
> `target/release/wxi.exe`。Windows SmartScreen 可能拦截未签名 exe，
> 请点「更多信息 → 仍要运行」。

---

## 快捷键

| 快捷键   | 动作                                       |
| -------- | ------------------------------------------ |
| `F5`     | 跳到设置页（并触发 `CMD_CONFIG_GET` 刷新） |
| `Ctrl+L` | 跳到日志页                                 |
| `Ctrl+1` | 设备设置（Settings）                       |
| `Ctrl+2` | 键盘（Keymap）                             |
| `Ctrl+3` | 灯效（Lighting）                           |
| `Ctrl+4` | WiFi                                       |
| `Ctrl+5` | 音效（Audio）                              |
| `Ctrl+6` | 语音（Voice）                              |
| `Ctrl+7` | 日志（Log）                                |
| `Ctrl+8` | 关于（About）                              |

> 当 Keymap 面板进入"按下任意键捕获"模式时，全局快捷键让路以免吞键。
> Settings 页内的"应用待下发" / "放弃修改"由面板自身处理，不在此表。

---

## 工程结构

```
EKeysApp/
├── Cargo.toml
├── Cargo.lock
├── LICENSE                     # MIT
├── CHANGELOG.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── README.md
├── docs/
│   ├── protocol-usage.md       # 协议约定（App ↔ 固件）
│   ├── architecture.md         # 模块 / 数据流 / 事件总线
│   ├── build.md                # 构建 / 调试 / 跨平台
│   └── ota.md                  # OTA 升级链路
├── fonts/
│   └── MapleMono-NF-CN-ExtraLight.ttf
├── img/
│   ├── icon.ico                # 程序图标（编译期嵌入）
│   └── icon.png
├── .github/
│   ├── workflows/
│   │   ├── ci.yml              # fmt / check / clippy / test
│   │   └── release.yml         # 手动触发 Windows 打包
│   ├── ISSUE_TEMPLATE/         # Bug / Feature / config
│   └── PULL_REQUEST_TEMPLATE.md
├── .trae/
│   └── rules/                  # 工程级规则（egui / UI 样式）
└── src/
    ├── main.rs                 # 入口：装载字体 + 启动 App
    ├── app.rs                  # eframe::App 实现，事件分发 / 快捷键 / 主题切换
    ├── protocol.rs             # 命令常量、Frame 编解码、DeviceSettings、KeymapData + PROTOCOL_VERSION
    ├── link/                   # 串口 + 后台线程（reader / writer / router / heartbeat）
    │   ├── mod.rs              # ConnectionState + LinkManager（对外门面）
    │   ├── serial.rs           # 串口枚举、打开、WCH 桥接检测
    │   ├── reader.rs           # 后台读线程
    │   └── heartbeat.rs        # 心跳线程
    ├── state/
    │   └── mod.rs              # AppHandle：共享状态 + UI 事件 channel + AudioUploadProgress
    ├── config/
    │   └── mod.rs              # 本地偏好（%APPDATA%/wxi/config.json）
    ├── ota.rs                  # 固件 MD5 + 本机 HTTP 服务 + 局域网 OTA 下发
    ├── pc_status.rs            # PC 状态采集（Lock / 网络 / CPU / 内存，Windows only）
    ├── ui/
    │   ├── mod.rs              # 主题常量 + apply_theme + card() 容器
    │   ├── fonts.rs            # 字体注册 + 图标 / 文本混排 helper
    │   ├── icons.rs            # Phosphor 图标常量集中导出
    │   ├── topbar.rs           # 顶部状态条 + 连接块（端口 / 扫描 / 状态胶囊 / 自动连接）
    │   ├── sidenav.rs          # 左侧导航（8 项）
    │   ├── statusbar.rs        # 底部状态条（Uptime / 收发计数 / 心跳）
    │   ├── panel_settings.rs   # 设备设置（多组卡片 + 草稿 + 字段钳位）
    │   ├── panel_keymap.rs     # 键映射（自带 top / central / bottom 三段）
    │   ├── panel_lighting.rs   # 灯效
    │   ├── panel_wifi.rs       # WiFi
    │   ├── panel_voice.rs      # 语音（敏感字段脱敏显示）
    │   ├── panel_audio.rs      # 音效板（上传进度 / 11 键绑定 / 试播）
    │   ├── panel_log.rs        # 日志（协议 / 固件 / 应用分色 + 过滤 + 搜索）
    │   ├── panel_about.rs      # 关于（应用版本 + 设备信息）
    │   └── widgets.rs          # Toast / ConfirmDialog / LocalSettings / DiffPreviewBar
    └── util/
        ├── mod.rs
        └── log.rs              # tracing 初始化 + 共享日志缓冲
```

模块职责、数据流、事件总线、后台线程清单见 [`docs/architecture.md`](./docs/architecture.md)。

---

## 关键依赖

| Crate                            | 用途                           |
| -------------------------------- | ------------------------------ |
| `eframe` 0.33                    | 窗口与渲染                     |
| `egui-phosphor` 0.11             | 图标字体（PUA，独立命名族）    |
| `serialport` 4.6                 | 串口枚举与读写                 |
| `serde` / `serde_json`           | Frame / 配置 JSON 序列化       |
| `tracing` / `tracing-subscriber` | 日志                           |
| `thiserror`                      | 错误类型                       |
| `dirs`                           | 平台配置目录                   |
| `image` / `base64`               | 程序图标解码、Profile 图标上传 |
| `md-5` / `rfd`                   | OTA 固件 MD5 校验 + 文件选择   |

Windows 上额外引入 `windows-sys`（与 `serialport` 4.x 对齐版本），提供键盘 Lock、网络状态、CPU 时间、系统内存等 Win32 调用。

---

## 协议简述

- **物理层**：串口（LF 行分隔），一帧一行 JSON；
- **响应帧**：响应命令 = 请求命令 `| 0x80`，`seq = 0` 表示设备主动推送；
- **异类命令**：`0x10` / `0x0c` / `0x0f` 响应 body 在帧顶层，单独解析；
- **SET 增量下发**：`data.config` 只放有变化的字段；`FieldMask` 在 App 内部用于精确 diff（不落盘、不下发）；
- **心跳**：`0x0a`，App 周期性发，固件响应配对 `seq != 0`；
- **TIME 同步**：连接后立即发 `0x13`（epoch + tz），固件立即 settimeofday；
- **音效板**：`0x16` 文件管理（list/begin/data/end/abort/delete）+ `0x17` 绑定与播放（get/set/play/stop），单文件 ≤ 2MB，每块 1024B；
- **协议版本**：**v1.0**（见 `src/protocol.rs` 顶部 `PROTOCOL_VERSION`）。

完整字段表（25 项）、命令清单（22 条）与钳位规则见 [`docs/protocol-usage.md`](./docs/protocol-usage.md)。

> 本协议**对第三方实现完全开放**，欢迎第三方固件 / App 兼容实现；详见协议文档 §0。

---

## 开源治理

| 文件                                                                     | 用途                                            |
| ------------------------------------------------------------------------ | ----------------------------------------------- |
| [`LICENSE`](./LICENSE)                                                   | MIT 协议                                        |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md)                                   | 贡献指南（环境 / 命令 / UI 自检 / 协议层扩展）  |
| [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)                             | 行为准则（Contributor Covenant v2.1）           |
| [`SECURITY.md`](./SECURITY.md)                                           | 安全漏洞报告渠道与处理流程                      |
| [`CHANGELOG.md`](./CHANGELOG.md)                                         | 版本变更记录（Keep a Changelog）                |
| [`.github/workflows/`](./.github/workflows/)                             | CI（fmt / check / clippy / test）+ Release 打包 |
| [`.github/ISSUE_TEMPLATE/`](./.github/ISSUE_TEMPLATE/)                   | Bug / Feature 报告模板                          |
| [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md) | PR 模板（含协议层扩展检查表）                   |

提 Issue / PR 前请先阅读对应的 `CONTRIBUTING.md` 与 `CODE_OF_CONDUCT.md`。

---

## 致谢

- [egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) — 即时模式 GUI 框架
- [egui-phosphor](https://github.com/n2/egui_phosphor) — Phosphor 图标字体
- [Maple Mono](https://github.com/subframe7536/Maple-font) — 中英文字体
- [serialport-rs](https://gitlab.com/susurrus/serialport-rs) — 跨平台串口访问
- 协议设计、固件实现参考 EKeys 团队

---

## 开发约定

- **UI 渲染 / 业务意图解耦**：面板不直接操作 `LinkManager`；所有副作用经 `AppHandle::ui_tx` 发 `UiEvent`，由 `app.rs::drain_ui_events` 收尾；
- **每帧 drain 事件**：不阻塞 UI 线程，IO 全在后台（`link/` 内的 reader / writer / router / heartbeat 线程）；
- **资源（字体 / 图标）**在 `WxiApp::new` 中安装一次；动画用 `ctx.input(|i| i.time)` 驱动，空闲时不调用 `request_repaint`；
- **共享状态**：通过 `Arc<Mutex<...>>` 在 `AppHandle` 上统一持有；锁粒度尽量小，读出即释放，避免持锁调用其他锁相关函数；
- **协议扩展检查表**：新增字段 / 命令时同步更新 `protocol.rs` 的 `F_*` 常量 / `FIELD_COUNT`、`DeviceSettings` 字段、`diff / apply / merge_push` 三处宏、`clamp()` 钳位、测试模块与本 README 协议简述。

详见 [`.trae/rules/rules.md`](./.trae/rules/rules.md) 与 [`.trae/rules/ui-style.md`](./.trae/rules/ui-style.md)。
