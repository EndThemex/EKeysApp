# Contributing to EKeysApp

感谢你有兴趣让 EKeysApp 变得更好!本文档尽量短,把所有「动手前必须知道的事」放在最前面。

---

## 1. 行为准则

本项目采用 [Contributor Covenant](./CODE_OF_CONDUCT.md)。参与即表示你已阅读并同意遵守。

## 2. 在你提 Issue / PR 之前

- **Bug 报告**:先在 [GitHub Issues](https://github.com/EndThemex/EKeysApp/issues)
  里搜一下,确认没人提过。
- **协议相关问题**:请同时阅读 [`docs/protocol-usage.md`](./docs/protocol-usage.md)
  —— 固件 / App 两侧的字段钳位规则都在那里。
- **行为差异 / 兼容性**:请附上 App 版本(顶栏 About)、固件阶段(文档 §11),
  以及操作系统的位数(Win10/11 x64 等)。

## 3. 开发环境

- Rust **stable**, **MSRV = 1.85**(见 `Cargo.toml` 的 `rust-version`)。
- Windows:需要 Visual Studio Build Tools 2022 或更新(C++ workload)。
- macOS / Linux:可以 `cargo check`,但 `pc_status` 等模块依赖 Win32 API,
  跑不起来,先看 §7。

推荐 IDE:**VS Code + rust-analyzer**(本仓库 `.vscode/settings.json` 已配置)。

## 4. 常用命令

```bash
# 编译检查(必须在每个 PR 通过)
cargo check --all-targets

# 严格 clippy(CI 同等强度;CI 不强制 -D warnings,本地可加)
cargo clippy --all-targets --locked --no-deps

# 协议层单测
cargo test --lib protocol

# 全部测试
cargo test

# 格式化(必跑)
cargo fmt --all -- --check
cargo fmt --all
```

## 5. UI 改动自检清单

提交涉及 `src/ui/**` 的改动前,**必须**在本地肉眼过一遍:

1. `cargo run`,切换深 / 浅主题(Local Settings 弹窗)各看一次。
2. 缩放窗口到 **800×500**,确保所有页面(含 Keymap 自带滚动)不被截断。
3. 中英混排不出现 □ 字符(漏挂字体 fallback)。
4. 图标和文本不要写在同一个 `ui.label("...")` / `ui.button("...")` —— 见
   [`.trae/rules/ui-style.md`](./.trae/rules/ui-style.md) §6。
5. 新增控件要复用 `card()` / `IconTextButton` / `show_confirm` /
   `push_toast`,不要手写 `egui::Frame`。
6. 关闭 / 重开 Local Settings 弹窗,焦点不能残留。

## 6. 协议层扩展(必读)

`docs/protocol-usage.md` 末尾 §12 列出了**协议字段 / 命令扩展检查表**。简而言之:

- 新增字段:同步 `protocol.rs` 顶部 `F_*` 常量 + `FIELD_COUNT` +
  `DeviceSettings` 字段 + `diff()` / `apply()` / `merge_push()` 三处宏 +
  `clamp()` + 单测 + §3 字段表 + §11 兼容性矩阵。
- 新增命令:加 `CMD_*` 常量 + Req / Resp / Push 结构体 + 单测 + §9 类型表。

`protocol.rs` 内置三条自动化守护测试:

- `field_constants_are_contiguous`
- `diff_and_apply_cover_all_fields`
- `merge_push_covers_all_fields`

它们会在你漏更新时编译失败 / 测试红,务必在本地跑通再提交。

## 7. 跨平台开发

当前阶段 CI 跨平台矩阵仅做 `cargo check` / `clippy` / `test` 验证,
**不保证运行时功能完整**。如要让 macOS / Linux 跑起来:

- 把 `windows-sys` 强相关的代码(`pc_status.rs` 等)拆出
  `pc_status/windows.rs` / `pc_status/unix.rs` 并用 `#[cfg(...)]` 分流。
- 串口名差异:Windows `COMx`,macOS `/dev/tty.usbmodem*`,Linux `/dev/ttyACM*`。
- Linux 用户需要加入 `dialout` 组才能访问串口。

## 8. Commit / PR 风格

- **Commit message**:推荐 [Conventional Commits](https://www.conventionalcommits.org/)。
  ```
  feat(audio): 支持取消上传
  fix(keymap): 修复旋钮槽误映射到物理键
  docs(protocol): 补 PC Status 字段钳位
  refactor(ui): 抽出 theme helpers
  chore: 升级 eframe 0.33.3 → 0.33.4
  ```
- **PR 描述**:用仓库自带的 PR 模板,说清「为什么」比「做了什么」更重要。
- **一次 PR 只做一件事**:大改动请拆 PR,每个 PR ≤ ~500 行(代码 + 测试)
  比较舒服。
- **不要直接 push `main`**:所有改动走 PR,等 CI 绿 + 1 个 reviewer 通过。
- **CI 红 = 必修**:本仓库 CI = `cargo check` + `cargo clippy --no-deps`
  - `cargo test` + `cargo fmt --check`,任意一项失败都视为不达标。

## 9. 文档同步

- 改 UI → 同步更新 `.trae/rules/ui-style.md`(规范 / 描述双角色)。
- 改协议 → 同步更新 `docs/protocol-usage.md`(见 §6)。
- 改公开 API 或构建流程 → 同步更新 `README.md` / `docs/build.md`。
- 用户可见的变更 → 在 `CHANGELOG.md` `[Unreleased]` 追加一条。

## 10. 发布流程(维护者)

- 主仓库版本号通过 git tag 触发,**不在 CI 内自动 bump**。
- 新版本发布时把 `CHANGELOG.md` 的 `[Unreleased]` 折叠为带日期的版本段,
  并开新的 `[Unreleased]`。
- 协议层版本号 (`PROTOCOL_VERSION`) 由 `protocol.rs` 单点维护;
  跨主版本升级 = 协议不兼容,需要在 `docs/protocol-usage.md` 顶部升 vX.Y。

## 11. 第一次贡献不知道做啥?

浏览 [open issues](https://github.com/EndThemex/EKeysApp/issues) 寻找感兴趣的
方向,或在 [Discussions](https://github.com/EndThemex/EKeysApp/discussions)
里发个想法。

非常欢迎 **协议层单测** 与 **UI 自检清单** 相关的 PR —— 它们风险低但
项目受益明显。
