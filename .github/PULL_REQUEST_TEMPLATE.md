## 改动说明

<!-- 一两句话:这个 PR 解决了什么问题 / 实现了什么功能 -->

## 关联 Issue

<!-- 用 "Closes #123" / "Fixes #456" / "Refs #789" 关联 -->

## 改动范围

<!-- 勾选涉及范围 -->
- [ ] App UI(`src/ui/**`)
- [ ] 协议层(`src/protocol.rs`)
- [ ] Link / 串口(`src/link/**`)
- [ ] 文档(`README.md` / `docs/**` / `.trae/rules/**`)
- [ ] CI / 构建(`.github/` / `Cargo.toml`)
- [ ] 单测
- [ ] 其他:

## 协议层检查表(若涉及协议层)

- [ ] `F_*` 常量 + `FIELD_COUNT` 同步
- [ ] `DeviceSettings` 字段追加
- [ ] `diff()` / `apply()` / `merge_push()` 三处宏同步
- [ ] `clamp()` 同步
- [ ] 单测覆盖(守护测试)
- [ ] `docs/protocol-usage.md` §3 / §9 / §11 同步
- [ ] 通知固件侧维护者

## 自检

- [ ] `cargo check --all-targets` 通过
- [ ] `cargo clippy --all-targets -- -D warnings` 通过
- [ ] `cargo test --all-targets` 通过
- [ ] `cargo fmt --all` 已跑
- [ ] UI 改动:深 / 浅主题各看一次;800×500 窗口无截断
- [ ] 文档改动:链接已校对

## 截图 / 录屏

<!-- UI / 协议可见行为变化建议附图 -->

## 备注

<!-- 任何想说明的取舍 / TODO / 风险 / 向后兼容性 -->
