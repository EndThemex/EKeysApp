# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> 关于「协议层」版本（`docs/protocol-usage.md` 顶部声明）：本仓库 `protocol.rs` 顶部
> `PROTOCOL_VERSION` 为唯一来源。App 与固件版本号**相互独立**，协议版本变更通过
> SemVer 体现在主仓库 `Cargo.toml`，并同步在协议文档顶部。

---

## [Unreleased]

### Added

- 开源治理：`LICENSE` / `CODE_OF_CONDUCT` / `CONTRIBUTING` / `SECURITY` 文档。
- GitHub Actions：CI（windows + ubuntu，`check` / `clippy` / `test` / `fmt`）。
- `.github/`：Issue 与 PR 模板。
- `docs/architecture.md`、`docs/build.md`、`docs/ota.md` 子文档。
- `protocol.rs` 顶部新增 `PROTOCOL_VERSION` 常量；`docs/protocol-usage.md`
  顶部同步标注并新增「App TODO / Firmware TODO」拆分表。

### Changed

- `Cargo.toml` 补齐 `authors` / `license` / `repository` / `keywords` /
  `categories` / `rust-version`，为 crates.io 发布做准备。
- `README.md` 增加徽章占位、跨平台现状说明、协议开放声明、致谢与下载说明。
- `.gitignore` 增加编辑器 / 操作系统噪声文件规则。

---

## [0.1.0] - TBD

> 此版本尚未发布。当前所有变更累积在 `[Unreleased]`,首次打 tag 时
> 再把上述变更折叠到带发布日期的版本段,并在底部追加 release link。

### Added

- 连接管理：原生 USB CDC 串口、CH340 等 WCH 桥接识别 + 自动拒绝、心跳保活、
  指数退避自动重连（1→2→4→5s，5 次后放弃）、启动自动连接上次端口。
- 全量配置 GET / SET：25 字段 `DeviceSettings`，增量下发 + 协议侧钳位。
- 8 个功能页：设备设置、键映射（4 层 × 11 物理键）、灯效、WiFi、语音（腾讯云
  一句话识别）、音效板（上传 + 绑定 + 试播）、日志、关于。
- 键映射编辑：本地 `KeymapData` ↔ 固件 `FirmwareKeyEntry` 模型映射，FUN 组合键、
  Profile 名称 / 自定义图标，进入页面边沿拉取。
- OTA：本地一次性 HTTP 服务 + MD5 校验，局域网下发 `.bin` 固件。
- PC 状态推送：键盘 Lock / 网络 / CPU / 内存，每秒 diff-based 推送到设备
  （`0x0D`），Settings → PC 状态可开关。
- 协议 / 固件 / 应用 三类日志分色展示（Tx 蓝 / Rx 绿 / Firmware 灰白 /
  App 浅黄）；Toast / Confirm 弹窗。
- 敏感字段自动脱敏：WiFi 密码、腾讯云 SecretId / SecretKey 设备回传自动替换
  为 `***`，UI 用 `preview_mask` 显示首尾。
- 本地持久化：`%APPDATA%/wxi/config.json`（最近端口、自动连接、窗口尺寸、
  语言、主题、PC 状态推送开关）。
- 主题：深 / 浅双主题统一切换；图标用 Phosphor + Maple Mono CN 字体混排。

### Notes

- 当前协议层版本：**v1.0**（见 `docs/protocol-usage.md` 顶部）。
- 平台支持：**Windows 是唯一已验证目标**。macOS / Linux 仍在路线图，
  CI 仅做编译验证，不保证运行时功能完整。

[Unreleased]: https://github.com/EndThemex/EKeysApp/compare/HEAD
