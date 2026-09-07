# 桌面 App ↔ 固件 协议使用说明

> 面向**固件侧开发者**。本文档描述当前桌面 App（`EKeysApp`，`src/protocol.rs`）已经实现并下发/解析的协议约定。
>
> 所有示例 JSON 都是真实运行中产生的形状（来自 [`protocol.rs`](../src/protocol.rs) 的 serde derive）。
>
> 适用版本：当前 `main` 分支；增量下发（`mask`）为推荐方式。

---

## 1. 物理层与帧格式

### 1.1 传输

- 串口（serial）一行一帧：`\n` 作为行分隔符。
- 行尾必须为 `\n`（LF），不要发 `\r\n` —— App 端的 `Frame::encode_line` 也只发 LF。
- 非 JSON 行（固件日志）混在流中，App 通过 `try_parse_line` 自动识别：
  - 以 `{` 开头 → 尝试解析为 JSON Frame；
  - 否则 → 归类为日志，**不会**触发协议错误。

### 1.2 Frame 顶层结构

```json
{
  "cmd": 8,
  "seq": 1,
  "data": { ... },
  "status": 0,
  "error": "..."
}
```

| 字段     | 类型    | 说明                                                     |
| -------- | ------- | -------------------------------------------------------- |
| `cmd`    | u8      | 命令 ID；响应帧 = 请求命令 `\| 0x80`                     |
| `seq`    | u32     | 请求序号；App 发请求时单调递增；**主动推送**用 `seq = 0` |
| `data`   | object? | 请求/响应体；可缺省                                      |
| `status` | u8?     | 仅响应帧携带：`0` = 成功，`1` = 失败                     |
| `error`  | string? | `status = 1` 时附带，App 会弹 Toast / 写日志             |

App 判定"响应帧"的规则：

```
is_response(cmd) = (cmd & 0x80) != 0
is_push(frame)   = is_response(frame.cmd) && frame.seq == 0
```

---

## 2. 命令一览

App 端已声明全部 **18 个命令常量**（与固件 `SerialProtocol.h` 对齐），但**实际下发/接收逻辑**目前只覆盖：

| 命令               | 常量                            | 值              | 方向       | App 当前状态                                                 |
| ------------------ | ------------------------------- | --------------- | ---------- | ------------------------------------------------------------ |
| 配置版本 GET / SET | `CMD_CONF_VERSION_GET` / `_SET` | `0x01` / `0x02` | App → 设备 | ✅ 数据结构定义完成；调用层未接入                            |
| 设备信息 GET       | `CMD_DEVICE_INFO_GET`           | `0x03`          | App → 设备 | ✅ **已接入**（连接后 `auto_get` 拉取，顶栏 + About 页展示） |
| 设备信息 SET       | `CMD_DEVICE_INFO_SET`           | `0x04`          | App → 设备 | ✅ 数据结构定义；调用未接入                                  |
| 键映射 GET / SET   | `CMD_KEYMAP_GET` / `_SET`       | `0x05` / `0x06` | App → 设备 | ✅ **已接入**（键映射面板：重新加载 = GET，下发 = SET）      |
| 配置 GET           | `CMD_CONFIG_GET`                | `0x07`          | App → 设备 | ✅ **已接入**                                                |
| 配置 SET           | `CMD_CONFIG_SET`                | `0x08`          | App → 设备 | ✅ **已接入**                                                |
| 物理按键上报       | `CMD_KEY_EVENT`                 | `0x09`          | 设备 → App | ❌ 固件侧未实现；App 暂不监听                                |
| 心跳               | `CMD_HEARTBEAT`                 | `0x0a`          | 双向       | ✅ **已接入**                                                |
| 固件信息 / OTA     | `CMD_FIRMWARE_INFO`             | `0x0b`          | 双向       | ✅ 数据结构定义；调用未接入                                  |
| 语音文本推送       | `CMD_VOICE_TEXT`                | `0x0c`          | 设备 → App | ✅ 数据结构定义；UI 路由未接入                               |
| PC 状态            | `CMD_PC_STATUS`                 | `0x0d`          | App → 设备 | ✅ 数据结构定义；调用未接入                                  |
| 音乐状态           | `CMD_MUSIC_STATUS`              | `0x0e`          | App → 设备 | ✅ 同上                                                      |
| 音乐控制           | `CMD_MUSIC_CONTROL`             | `0x0f`          | 设备 → App | ✅ 同上（固件 UI 链路未通）                                  |
| Profile 状态       | `CMD_PROFILE_STATE`             | `0x10`          | 双向       | ✅ 数据结构定义；调用未接入；**异类响应**                    |
| Profile 图标       | `CMD_PROFILE_ICON_SET`          | `0x11`          | App → 设备 | ✅ **已接入**（设置 → 键盘页：上传 / 清除图标）              |
| HA 状态            | `CMD_HA_STATUS`                 | `0x12`          | 设备 → App | ❌ 固件侧未实现                                              |

**响应帧**：`response_cmd(req) = req | 0x80`。**例外**：`CMD_PROFILE_STATE` 的响应帧 `cmd` 仍是 `0x10`（详见 §3.3 与 §5.6）。

---

## 3. `DeviceSettings` 字段全集

固件推送 `CONFIG_GET` 响应 / 全量 `CONFIG_SET` 时必须**完整填写以下全部 26 个字段**（可缺省则走 serde `Default`）。

| #   | 字段                             | 类型   | 备注                                      |
| --- | -------------------------------- | ------ | ----------------------------------------- |
| 0   | `wifi_switch`                    | i32    | 0/1                                       |
| 1   | `connect_host`                   | i32    |                                           |
| 2   | `wifi_ssid`                      | string | 允许空串                                  |
| 3   | `wifi_password`                  | string | **永远不要把明文密码回显给 App**（见 §7） |
| 4   | `work_mode`                      | i32    | 0=USB / 1=BLE / 2=2.4G                    |
| 5   | `rgb_mode`                       | i32    |                                           |
| 6   | `rgb_single_colar`               | i32    |                                           |
| 7   | `rgb_click_mode`                 | i32    |                                           |
| 8   | `rgb_brightness`                 | i32    |                                           |
| 9   | `tft_theme`                      | i32    |                                           |
| 10  | `tft_brightness`                 | i32    | **5~100**（App 端 `clamp()` 会强制钳位）  |
| 11  | `device_volume`                  | i32    |                                           |
| 12  | `audio_enable`                   | i32    |                                           |
| 13  | `power_mode`                     | i32    |                                           |
| 14  | `voice_enable`                   | i32    |                                           |
| 15  | `voice_trigger_key`              | i32    |                                           |
| 16  | `voice_max_record_ms`            | i32    |                                           |
| 17  | `voice_auto_enter`               | i32    |                                           |
| 18  | `voice_dev_pid`                  | i32    |                                           |
| 19  | `voice_cuid`                     | string |                                           |
| 20  | `voice_baidu_api_key`            | string |                                           |
| 21  | `voice_baidu_secret_key`         | string |                                           |
| 22  | `pc_status_mask`                 | i32    |                                           |
| 23  | `active_keymap_profile`          | i32    | 0~7（App 端 `clamp()` 钳位）              |
| 24  | `active_profile_name`            | string |                                           |
| 25  | `active_profile_has_custom_icon` | bool   |                                           |

> ⚠️ `#` 是 `FieldMask` 的 bit 编号。**新增字段必须同步在 App 端 `FIELD_COUNT` 与位号上追加，固件侧字段顺序保持一致**。位号定义见 `protocol.rs` 顶部 `F_*` 常量。

---

## 4. SET 命令：增量下发

App 端 SET 当前采用**纯增量下发**（与固件 `parseConfigSetCommand.cpp` 对齐）：

```json
{
  "cmd": 8,
  "seq": 1,
  "data": {
    "config": {
      /* 只包含有变化的字段 */
    }
  }
}
```

固件侧行为（详见固件协议文档 §5.2 / §5.3）：

- 按"字段是否出现在 `data.config` 中"判断增量；
- 未出现的字段保持不变；
- 出现的字段按字段类型走对应校验规则（如 `tft_brightness` 钳位 `5~100`、`work_mode` 越界忽略等）；
- 未知字段忽略。

### 4.1 增量下发示例

App 把 `tft_brightness` 改成 80、`work_mode` 改成 0：

```json
{
  "cmd": 8,
  "seq": 17,
  "data": {
    "config": {
      "tft_brightness": 80,
      "work_mode": 0
    }
  }
}
```

### 4.2 App 端"合法 0 / 空串"如何处理

App 内部 `diff()` / `apply()` 使用 `FieldMask` 显式标记"哪些字段有变化"——避免把"`work_mode = 0`（USB）"或"`wifi_ssid = ""`（清空）"误判为"无变化"。

**对外协议**：App 只把 mask 标记的字段放进 `data.config`，**不发送 `mask` 字段**。固件按"字段是否存在"判断增量即可。

如果未来升级协议让固件也支持 `mask` 字段（更快跳过不存在字段、避免未知字段干扰），只需要在 `apply_diff` 中追加 mask 输出，不需要改动 diff / apply / merge_push 内部实现。

### 4.3 固件侧必须遵守的钳位规则

| 字段                                                                 | 钳位规则                                         |
| -------------------------------------------------------------------- | ------------------------------------------------ |
| `tft_brightness`                                                     | `< 5 → 5`，`> 100 → 100`                         |
| `work_mode`                                                          | 越界忽略该字段                                   |
| `active_keymap_profile`                                              | 仅 `0~7`，越界忽略                               |
| `wifi_switch` / `connect_host` / `voice_enable` / `voice_auto_enter` | 归一化为 `0/1`                                   |
| `voice_trigger_key`                                                  | `< 0 → 0`，`> 11 → 11`                           |
| `voice_max_record_ms`                                                | 钳制到 `1000~60000`                              |
| `voice_dev_pid`                                                      | 钳制到 `0~65535`                                 |
| 字符串                                                               | 超过容量时截断；WiFi 密码和百度 Key 最大 64 字节 |
| 未知字段                                                             | 忽略                                             |

App 端 `DeviceSettings::clamp()` **已覆盖上表全部规则**（含字符串截断，按字节截断且不切断 UTF-8 字符边界）。App 在下发前对 diff 先做 `clamp()`，固件侧仍保留自己的钳位作为兜底。如果固件端将来修改钳位规则，需同步通知 App 维护者（钳位变更可能导致 diff 计算与实际下发值不同步）。

---

## 5. 推送（设备 → App）

固件**主动推送**全量快照的格式：

```json
{
  "cmd": 135, // response_cmd(0x07) = 0x87
  "seq": 0,
  "data": {
    /* 完整 DeviceSettings */
  }
}
```

App 收到后行为（见 `app.rs::handle_link_event`）：

1. 用 `serde_json::from_value::<DeviceSettings>` 解析；
2. **脱敏**：`wifi_password` / `voice_baidu_api_key` / `voice_baidu_secret_key` 替换为 `***`，App 不存储设备回传的密钥明文（见 §7）；
3. 写入 `settings`（App 内部快照）；
4. 对 `draft` 做 `merge_push`：草稿里**未改动的字段**用推送值刷新，**用户改过**的字段保留草稿值。
5. 日志面板打 `PUSH ← 全量快照`。

> 主动推送只发生在响应帧 + `seq=0` 的情况下。任何**非响应帧 + `seq != 0`** 会被 App 当作"未配对的请求响应"，转给 UI 而不写入快照。

### 5.1 异类推送（body 在帧顶层）

部分命令的 body 字段**不在 `data` 里**，而直接在帧顶层，需要单独解析路径：

| 命令                           | 顶层字段             | App 解析                           |
| ------------------------------ | -------------------- | ---------------------------------- |
| `0x10 Profile State`（含响应） | `profile_state`      | `parse_top_level::<ProfileState>`  |
| `0x0C Voice Text`              | `text` / `timestamp` | `parse_top_level::<VoiceTextPush>` |
| `0x0F Music Control`           | `music_control`      | `parse_top_level::<MusicControl>`  |

⚠️ **特别注意 `0x10`**：响应帧 `cmd` 仍是 `0x10`（**不是 `0x90`**），不带 `status` 字段，`profile_state` 在顶层。这是固件侧 `cmd_profile.cpp` 的实现例外，App 解析时按"命令名识别"，不能套"`is_response = cmd & 0x80`"。

示例（Profile 状态推送）：

```json
{
  "cmd": 16,
  "seq": 0,
  "profile_state": {
    "active_profile": 0,
    "profile_number": 1,
    "profile_name": "Profile 1",
    "has_custom_icon": true,
    "icon_path": "/icon1.png"
  }
}
```

示例（语音识别文本）：

```json
{
  "cmd": 12,
  "seq": 0,
  "text": "识别出的文字",
  "timestamp": 123456
}
```

> `parse_top_level::<T>(json_line)` 会先尝试在整行 JSON 上反序列化；如果目标是嵌套的（如 `ProfileState`），则先剥掉 wrapper（`profile_state` / `music_control`）再解析。

---

## 6. 心跳

- App 周期性发 `CMD_HEARTBEAT = 0x0a`；
- 设备响应：`{ "cmd": 0x8a, "seq": <原 seq>, "status": 0 }`；
- App 通过 `seq != 0` 把响应配对回原请求，**不写入 settings，不弹 Toast**；
- 超时由 `LinkManager` 状态机处理（`Reconnecting`）。

固件侧**不应**把心跳响应当成 `is_push`——`seq != 0` 已经过滤。

---

## 7. 敏感字段约定

`wifi_password`、`voice_baidu_api_key`、`voice_baidu_secret_key` 是敏感字段。

**App 端已实现脱敏（当前生效）**：

- App 在**接收**设备推送 / GET 响应时，对这 3 个字段统一替换为 `***`（`DeviceSettings::mask_sensitive()`，调用点在 `app.rs` 推送处理、`state::auto_get`、顶栏刷新三处）；
- App 端 `settings` / `draft` 中不会出现设备回传的密钥明文；用户需要修改时在 UI 输入新值，diff 中该字段用新值下发；
- 固件**接收 SET** 时不要把收到的明文密码回传到日志串口。

> 固件侧推送时建议直接回显 `***` 或 `""`，与 App 端脱敏行为一致，避免多余的明文传输。

---

## 8. 错误响应

固件返回失败时：

```json
{
  "cmd": 136, // response_cmd(0x08)
  "seq": 17,
  "status": 1,
  "error": "tft_brightness out of range"
}
```

App 收到后：

- `status() == Some(1)` ⇒ 写日志 + 弹错误 Toast；
- `status() == Some(0)` ⇒ 写日志 `ACK ← 0xNN`，不弹 Toast；
- 其他值 ⇒ 走 `error` 兜底提示。

> `Frame::status()` 仅对响应类帧（标准响应 `cmd&0x80` 或异类响应 `0x10`）返回状态；请求帧恒为 `None`。

---

## 9. 各命令请求 / 响应骨架（App 侧已声明的类型）

下表列出 `src/protocol.rs` 中所有命令的请求 / 响应 / 推送数据类型。这些类型**已定义且通过单测**，但目前**调用层未完全接入**（详见 §2 表格）。

| 命令                          | 请求类型（App → 设备）                                                                                | 响应 / 推送类型（设备 → App）                                                   |
| ----------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `0x01` `CMD_CONF_VERSION_GET` | （无 body）                                                                                           | `ConfVersionResp { version: u32 }`（位于 `data`）                               |
| `0x02` `CMD_CONF_VERSION_SET` | `ConfVersionSetReq { version: u32 }`（位于 `data`）                                                   | `ConfVersionResp`                                                               |
| `0x03` `CMD_DEVICE_INFO_GET`  | （无 body）                                                                                           | `DeviceInfo`（位于 `data.device_info`）                                         |
| `0x04` `CMD_DEVICE_INFO_SET`  | `DeviceInfoSetReq { device_name?, serial? }`（位于 `data`，缺省字段应**省略**）                       | `DeviceInfoSetResp { device_name, serial }`                                     |
| `0x05` `CMD_KEYMAP_GET`       | （无 body）                                                                                           | `keymap: Vec<FirmwareKeyEntry>`（**帧顶层**，存于 `Frame::extra`）              |
| `0x06` `CMD_KEYMAP_SET`       | `KeymapSetReq { keymap: Vec<FirmwareKeyEntry> }`（位于 `data`）                                       | 标准响应（`status=0` 即可）                                                     |
| `0x07` `CMD_CONFIG_GET`       | （无 body）                                                                                           | `DeviceSettings`（位于 `data`，26 字段）                                        |
| `0x08` `CMD_CONFIG_SET`       | `ConfigSetPayload { config: DeviceSettings }`（详见 §4）                                              | 标准响应 + 可能 `0x87` 推送                                                     |
| `0x09` `CMD_KEY_EVENT`        | —                                                                                                     | 固件未实现                                                                      |
| `0x0a` `CMD_HEARTBEAT`        | （无 body）                                                                                           | `HeartbeatResp { timestamp, device }`                                           |
| `0x0b` `CMD_FIRMWARE_INFO`    | 无 body（查询）/`FirmwareOtaReq { url, checksum }`（OTA）                                             | `FirmwareInfo { version, device, build_date, build_time }`                      |
| `0x0c` `CMD_VOICE_TEXT`       | —                                                                                                     | `VoiceTextPush { text, timestamp }`（**顶层**）                                 |
| `0x0d` `CMD_PC_STATUS`        | `PcStatusReq { pc_status: PcStatus }` 或 `PcStatusConfigReq { type="config", mask }`                  | 标准响应                                                                        |
| `0x0e` `CMD_MUSIC_STATUS`     | `MusicStatusReq { music_status: MusicStatus }`                                                        | 标准响应                                                                        |
| `0x0f` `CMD_MUSIC_CONTROL`    | —                                                                                                     | `MusicControl { action: MusicControlAction }`（**顶层**）                       |
| `0x10` `CMD_PROFILE_STATE`    | （无 body）                                                                                           | `ProfileState { ... }`（**顶层**，**异类响应**）                                |
| `0x11` `CMD_PROFILE_ICON_SET` | `ProfileIconSetPayload { profile_icon: ProfileIconSetReq }`（位于 `data`，**必须包 `profile_icon`**） | `ProfileIconSetResp { profile, profile_number, has_custom_icon, profile_name }` |
| `0x12` `CMD_HA_STATUS`        | —                                                                                                     | 固件未实现                                                                      |

### 9.1 字段序列化约定

- **Option 字段**统一用 `skip_serializing_if = "Option::is_none"`：App 不下发空字段，固件按"字段是否存在"判断增量。
- **顶层 body** 命令（`0x10` / `0x0c` / `0x0f`）的响应**不走 `data`**，解析时调用 `parse_top_level::<T>`。
- 字符串字段长度上限沿用固件协议（WiFi 密码/百度 Key 64 字节、`device_name`/`serial` 32 字节）。

### 9.2 命令路由建议

App 端 Link 层在收到 `Frame` 后，按以下优先级分发：

1. 异类响应 (`0x10` / `0x0c` / `0x0f`) → 走 `parse_top_level` 单独解析；
2. `is_response() && seq != 0` → 配对回原请求（`pending` 队列）；
3. `is_response() && seq == 0` → 主动推送（典型如 `0x87` 配置快照）；
4. 其它 → 未配对响应，记录日志 + Toast 提示。

### 9.3 键映射（0x05 / 0x06）模型映射

App 的 `KeymapData`（4 层 × 槽位 × 绑定表）与固件"每 Profile 11 个物理键"模型**不对等**，映射规则如下（已实现并单测）：

- **映射基准**：当前 active profile 的 **layer 0（Base）**；旋钮槽（`SlotKind::Encoder`）跳过（固件不支持）；
- 剩余按键按 `(row, col)` 升序编号为 `physical` 1~11（与 App 3 行 × 4 列布局一致）；
- `KeymapData::to_firmware_entries()` 生成 `0x06` 请求体；`apply_firmware_entries()` 解析 `0x05` 响应写回；
- **动作编码**（`KeyAction::to_firmware_strings`）：
  - `Keyboard(code)` → `normal = "0xNN"`（固件 `KeyNameTable` 可无损解析回 code）；
  - `Media / Mouse / Macro / LayerSwitch / Encoder` → 尽力编码为 `function` 字符串，但固件当前**无法解析**（`KeyNameTable.cpp` 只支持 a-z/0-9/Enter/Backspace/Space 与 `0xNN`），按键会变无效——**两端模型差异的固有限制**；
- **回读限制**：固件 `macro` 是 `+` 连接的键序列，App 的 `Macro` 是带延时步进模型，**无法无损还原** → 回读为"未绑定"。

### 9.4 Profile 图标（0x11）注意事项

- 请求 body **必须包在 `data.profile_icon`** 里（`ProfileIconSetPayload`），固件不接收裸 `ProfileIconSetReq`；
- `clear = true` 时删除图标，**不携带** `png_base64`（`skip_serializing_if` 保证）；
- 单帧 ≤ 2048 字节：Base64 膨胀 4/3，App 上传前校验 PNG 签名 + `image` 解码 + Base64 长度 ≤ 1400，超限弹错误 Toast；
- 固件**不校验 PNG 尺寸**（注释提到 48×48 但未强制），App 自行保证格式与大小；
- 成功后固件会再推一条 `cmd=0x10, seq=0` 的 Profile 状态。

---

## 10. 协议层 Rust API 一览（参考）

> 给固件侧开发者参考 App 端的对应实现，便于双向理解。

```rust
// 1. 帧
Frame::request(cmd, seq, data) -> Frame
frame.encode_line() -> Result<String, ProtocolError>   // 末尾追加 '\n'；失败不发送
frame.is_response() -> bool                            // cmd & 0x80 != 0
frame.is_response_like() -> bool                       // 标准响应 + 异类响应 0x10
frame.is_push()      -> bool                           // is_response && seq == 0
frame.status()       -> Option<u8>                     // 仅响应类帧可见；请求帧恒为 None
frame.extra_value("keymap") -> Option<&Value>          // 帧顶层未声明字段（0x05 响应的 keymap）

// 2. 解析
try_parse_line(line) -> Option<Result<Frame, ProtocolError>>
// 非 JSON 行返回 None，不算错误。

// 3. 命令 ID
CMD_CONF_VERSION_GET  // 0x01
CMD_CONF_VERSION_SET  // 0x02
CMD_DEVICE_INFO_GET   // 0x03
CMD_DEVICE_INFO_SET   // 0x04
CMD_KEYMAP_GET        // 0x05
CMD_KEYMAP_SET        // 0x06
CMD_CONFIG_GET        // 0x07
CMD_CONFIG_SET        // 0x08
CMD_KEY_EVENT         // 0x09
CMD_HEARTBEAT         // 0x0a
CMD_FIRMWARE_INFO     // 0x0b
CMD_VOICE_TEXT        // 0x0c
CMD_PC_STATUS         // 0x0d
CMD_MUSIC_STATUS      // 0x0e
CMD_MUSIC_CONTROL     // 0x0f
CMD_PROFILE_STATE     // 0x10  // 异类响应：cmd 不带 0x80
CMD_PROFILE_ICON_SET  // 0x11
CMD_HA_STATUS         // 0x12
response_cmd(req) -> u8  // req | 0x80

// 4. SET 请求体（仅 config，不带 mask）
ConfigSetPayload { config: &DeviceSettings }

// 5. 解析辅助
try_parse_line(line) -> Option<Result<Frame, ProtocolError>>
data_or_default::<T>(f) -> Result<T, ProtocolError>   // 从 Frame::data 读
parse_top_level::<T>(line) -> Result<T, ProtocolError>  // 整行解析（含异类 wrapper）
is_top_level_cmd(cmd) -> bool   // 0x10 / 0x0c / 0x0f
is_response_like(cmd) -> bool   // 标准响应 + 0x10

// 6. FieldMask
FieldMask::empty() -> Self
FieldMask::all()    -> Self             // 低 FIELD_COUNT 位全 1
mask.set(bit) / mask.test(bit) / mask.bits() -> u64
mask.intersect(other) / mask.union(other)

// 7. DeviceSettings 工具
DeviceSettings::clamp() -> bool   // 覆盖固件全部钳位规则（含字符串截断）
device.mask_sensitive()          // wifi_password / voice_baidu_* → "***"
device.diff(&other) -> (DeviceSettings, FieldMask)
device.apply(&diff, mask)
DeviceSettings::merge_push(&new, &old, old_mask, new_mask, &mut draft)

// 8. Keymap ↔ 固件映射
keymap.to_firmware_entries() -> Vec<FirmwareKeyEntry>       // 0x06 请求体
keymap.apply_firmware_entries(&entries) -> bool             // 0x05 响应写回
KeyAction::to_firmware_strings()   -> (normal, macro, function)
KeyAction::from_firmware_strings(n, m, f) -> KeyAction
hid_to_name(code) -> Option<String>   // 0x04→"a" 等（KeyNameTable 子集）
name_to_hid(name)  -> Option<u16>     // "a"→0x04，"0x2A"→0x2A

// 9. 0x11 Profile 图标（body 必须包 profile_icon）
ProfileIconSetPayload { profile_icon: ProfileIconSetReq { profile, clear, png_base64? } }
```

---

## 11. 兼容性矩阵

| 固件阶段 | App 期望固件支持的字段                             | 备注                  |
| -------- | -------------------------------------------------- | --------------------- |
| 阶段 04  | Profile 字段（#23~25）                             | 已生效                |
| 阶段 05  | + `pc_status_mask`（#22）；Keymap 命令**尚未**对接 | Keymap 数据当前仅本地 |
| 阶段 06  | + WiFi（#0~3）+ Voice（#14~21）                    | 全部生效              |

未到阶段的字段固件可忽略；`CONFIG_GET` 响应 / 主动推送仍应返回完整 26 字段（缺省走 `Default`），便于 App 端 UI 始终有合理初始值。

---

## 12. 协议层扩展检查表

新增字段 / 新增命令时需同步：

- [ ] `protocol.rs` 顶部追加 `F_*` 常量 + 增加 `FIELD_COUNT`
- [ ] `DeviceSettings` 结构体追加字段
- [ ] `diff()` / `apply()` / `merge_push()` 三个宏各加一行 `cmp!` / `apply_field!` / `merge_field!`
- [ ] `clamp()` 如有范围字段需同步钳位规则
- [ ] `protocol.rs` 测试模块补单测（合法 0 / 空串场景）
- [ ] 文档本节更新（§3 字段表 + §11 兼容性矩阵）
- [ ] 通知固件侧更新字段表
- [ ] 新增命令：`protocol.rs` 加 `CMD_*` 常量 + Req/Resp/Push 结构体 + 单测 + §9 类型表

> 三处宏的字段列表仍可能漂移，但已有自动化守护：
>
> - `field_constants_are_contiguous` 检查 `F_*` 连续无空洞；
> - `diff_and_apply_cover_all_fields` / `merge_push_covers_all_fields` 强制 diff/apply/merge_push 覆盖全部字段。
>
> **新增字段务必三处同时更新**，测试会拦住漏项。后续仍可考虑用 build.rs 生成以彻底消除手写风险（已记入改进项）。
