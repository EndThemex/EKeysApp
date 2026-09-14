# Security Policy

## Supported Versions

下表展示本项目当前获得安全更新的版本。EKeysApp 仍处早期,**仅最新发布的
minor 版本** 会收到修复;更早的版本请自行升级。

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅        |
| < 0.1   | ❌        |

> 协议层版本(`docs/protocol-usage.md` 顶部声明的 `PROTOCOL_VERSION`)与
> App 版本号独立追踪;协议侧的安全相关变更随 App 主版本号发布。

## Reporting a Vulnerability

**请不要在 GitHub Issues / Discussions 中公开披露安全问题**。在补丁发布前
披露细节可能让所有正在使用该软件的用户暴露风险。

请通过以下任一私密渠道报告:

- **GitHub Private Vulnerability Reporting**(首选):
  [Report a vulnerability](https://github.com/EndThemex/EKeysApp/security/advisories/new)

请在报告中尽量包含:

1. 受影响版本(Cargo.lock 或 About 面板版本号);
2. 复现步骤或 PoC;
3. 影响评估(数据泄露 / 代码执行 / DoS / 协议绕过 等);
4. 你是否愿意协助修复并署名。

## What to Expect

- **72 小时内**确认收到;
- 7 天内给出**初步评估**(严重性 + 计划);
- 修复发布周期取决于严重性:
  - Critical:48 小时 hotfix;
  - High:下一个 minor 版本;
  - Medium / Low:下一个 minor 或 patch。
- 修复发布后会在 [`CHANGELOG.md`](./CHANGELOG.md) 标注 `Security:` 前缀,
  并在 GitHub Security Advisories 同步公告。

## Scope

本仓库范围内:

- App 与协议层 Rust 代码(`src/**`);
- CI / 构建脚本 / GitHub Actions;
- 文档中嵌入的脚本 / 命令。

**Out of scope**:

- 第三方固件实现 —— 它们独立维护,但欢迎通过本项目渠道转发给我们认识;
- 用户自行 fork 的下游分支;
- 物理攻击 / 供应链攻击 —— 涉及硬件与生产流程,本项目仅做软件协调。

## Sensitive Fields

App 已经在协议层与 UI 层对以下字段做脱敏处理,详见
[`docs/protocol-usage.md` §7](./docs/protocol-usage.md):

- `wifi_password`
- `voice_tencent_secret_id`
- `voice_tencent_secret_key`

如发现设备回传 / 日志中**意外**出现上述字段明文,请按上方流程报告。
