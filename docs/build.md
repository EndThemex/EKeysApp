# 构建 / 运行 / 调试

> 本文覆盖 EKeysApp 的完整构建链路。Windows 是已验证目标;
> macOS / Linux 仍在路线图(见末尾「跨平台」一节)。

---

## 1. 前置依赖

| 工具 | 最低版本 | 用途 |
| --- | --- | --- |
| Rust toolchain | stable, **MSRV 1.85** | 编译 App |
| Visual Studio Build Tools | 2022(C++ workload) | Windows MSVC 链接 |
| Windows SDK | 10.0.19041+ | `windows-sys` 调用 |

`Cargo.toml` 已锁定 `rust-version`,toolchain 自动检查。

安装 Rust(任一):

```bash
# rustup(推荐)
curl https://sh.rustup.rs -sSf | sh

# Windows: winget
winget install Rustlang.Rustup
```

Windows 上额外需要:

```bash
# Visual Studio Build Tools(C++ workload)
winget install Microsoft.VisualStudio.2022.BuildTools
# 勾选 "Desktop development with C++"
```

## 2. 常用命令

```bash
# 开发构建并启动
cargo run

# Release 构建
cargo build --release
# 产物:target/release/wxi.exe (Windows)
#       target/release/wxi   (macOS / Linux)

# 严格 clippy
cargo clippy --all-targets --locked -- -D warnings

# 协议层单测(字段 / diff / merge_push 守护)
cargo test --lib protocol

# 全部测试
cargo test --all-targets --locked

# 格式化
cargo fmt --all
cargo fmt --all -- --check
```

## 3. 后台线程清单

| 线程 | 入口 | 备注 |
| --- | --- | --- |
| reader | `src/link/reader.rs` | 同步阻塞读串口;按行解析 → `Frame` |
| router | reader 内联 | 同步分发到 `LinkEvent` |
| writer | `src/link/serial.rs` | 单写者,`link_rx` → 串口 |
| heartbeat | `src/link/heartbeat.rs` | 1Hz 心跳 |
| audio upload | `src/state/mod.rs::audio_upload_worker` | `0x16` 分块上传 |
| OTA HTTP | `src/ota.rs::serve_once` | 一次性 server,本机 IP |
| pc_status poll | `src/pc_status.rs` | Windows only,1Hz 采集 + diff |

线程拓扑图见 [`docs/architecture.md` §6](./architecture.md#后台线程清单)。

## 4. 运行参数

```bash
# 默认启动,使用 %APPDATA%/wxi/config.json
cargo run

# 调试日志:debug 级别输出到 stderr + 日志页
RUST_LOG=wxi=debug cargo run

# 单步 trace(会很吵)
RUST_LOG=wxi=trace cargo run
```

## 5. 跨平台编译

### 5.1 Linux

```bash
# 安装工具链
rustup target add x86_64-unknown-linux-gnu

# 编译
cargo build --release --target x86_64-unknown-linux-gnu

# 串口访问:把当前用户加入 dialout
sudo usermod -aG dialout $USER  # 重新登录后生效
```

`pc_status.rs` 在 Linux 上需要重新实现(用 `/proc/net/dev` 等),目前是
Windows-only stub,**仅能编译不能跑**。

### 5.2 macOS

```bash
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

串口: `/dev/tty.usbmodem*`;同样 `pc_status` 待补。

### 5.3 CI 当前矩阵

- `ubuntu-latest`:`fmt` + `check` + `clippy` + `test`(不保证运行时)
- `windows-latest`:`check` + `clippy` + `test`(已验证运行时)

## 6. Release 打包

- **当前**:`.github/workflows/release.yml` 提供手动触发的 Windows 打包,
  产出 `wxi-<ver>-windows-x86_64.zip`。
- **候选策略**:`cargo-dist` / `cargo-packager` 自动生成安装包 / 多平台;
  接入前请先在 `discussions` 讨论包格式与签名方案。
- **代码签名**:当前未签名;Win11 SmartScreen 会拦截,README 需要写明
  「首次运行请点'更多信息 → 仍要运行'」。

## 7. 调试技巧

### 7.1 协议帧追踪

打开「日志」页,过滤设为 `TX / RX` 即可看到所有 App ↔ 设备帧。比 wireshark
+ USB 抓包轻量。

### 7.2 锁相关 panic

若 `unwrap()` 一个 `Mutex`,几乎一定是上一帧没释放锁。按堆栈找「持锁跨过
事件发送 / 持锁调用 LinkManager」的代码段。常见反模式:

```rust
// ❌ 持锁调用会再次加锁的函数
let mut s = handle.settings.lock().unwrap();
handle.attempt_connect();   // 内部也会 lock handle.state

// ✅ 读出即释放
let snapshot = handle.settings.lock().unwrap().clone();
drop(snapshot);              // ← clone 后立刻 drop 不可省略,需要明确作用域
handle.attempt_connect();
```

### 7.3 UI 不刷新

- 确认面板在动画 / 计时器处调 `ctx.request_repaint()`;
- 空闲时 `app.rs::update` 末尾的 `request_repaint_after(100ms)` 兜底 10Hz。

### 7.4 主题切换没生效

- 确认 `WxiApp::new` 中字体安装之后调用了 `apply_theme(ctx, theme)` 一次;
- Local Settings 弹窗里切换时,AppHandle 会再调一次。

### 7.5 字体 / 图标问题

- 漏挂字体 fallback:`fonts.rs::install` 必须把 `maple_cn` 加到 `phosphor`
  家族;漏掉会出现 □。
- 「WiFi → WF」:确认没把图标字符与文本写在同一个 `ui.label("WiFi")`;
  走 `IconTextButton` / `paint_icon_text_in` / `paint_icon_at`。
