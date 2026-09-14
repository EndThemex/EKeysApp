# OTA 升级流程

> 本文描述 EKeysApp ↔ EKeys 设备之间的 OTA 升级链路。涉及两个模块:
> - App 端: [`src/ota.rs`](../src/ota.rs)(MD5 / 本机 HTTP / LAN IP 探测)
> - 协议层: [`src/protocol.rs`](../src/protocol.rs) 中 `CMD_FIRMWARE_INFO`
>   (`0x0b`) 的 `FirmwareOtaReq` 请求体
>
> 完整协议字段 / 钳位规则见 [`docs/protocol-usage.md` §9](../docs/protocol-usage.md)。

---

## 1. 总览

```
┌─────────────────┐  ① 选 .bin     ┌────────────────────────┐
│   User (UI)     │ ─────────────▶ │   AppHandle / Panel    │
└─────────────────┘                │   (panel_about/ota)    │
                                   └───────────┬────────────┘
                                               │  ② bytes + md5
                                               ▼
                                   ┌────────────────────────┐
                                   │  ota::FirmwareServer   │
                                   │  一次性 HTTP, 5min TTL │
                                   └───────────┬────────────┘
                                               │  ③ GET /firmware.bin
                                               ▼
                                   ┌────────────────────────┐
                                   │  EKeys 设备            │
                                   │  边下边校验 MD5,        │
                                   │  写 ota 分区, 自动重启  │
                                   └────────────────────────┘
```

四步走:

1. 用户在 UI 选择 `.bin` 固件;
2. App 计算 MD5 + 在本机起一次性 HTTP 服务;
3. App 通过 `0x0b` 把 URL + checksum 下发到设备;
4. 设备流式下载,边下边校验,通过后写 ota 分区并重启。

## 2. App 侧

### 2.1 MD5

```rust
pub fn md5_hex(data: &[u8]) -> String  // 32 位小写 hex
```

与固件端 `hexToDigest` 对齐。**不要**换成 SHA-256 —— 协议层字段长度固定 32
字符。

### 2.2 局域网 IP 探测

```rust
pub fn local_lan_ip() -> Option<IpAddr>
```

原理:对公网地址(`8.8.8.8` / `192.168.0.1`)发起 UDP `connect`(不 send),
内核据此选定默认路由的出口网卡,`local_addr()` 即本机在该网段的 IP。

**约束**:

- 必须是设备能访问到的地址 —— App 与设备需在同一 LAN(或 PC 共享网络给设备);
- 如果 PC 同时连着 VPN / 多网卡,可能拿到错误的接口,UI 应允许手动改 IP。

### 2.3 一次性 HTTP 服务

```rust
pub struct FirmwareServer { pub port: u16, ... }
impl FirmwareServer {
    pub fn spawn(bytes: Arc<Vec<u8>>) -> std::io::Result<Self>
    pub fn stop(self) { /* ... */ }
}
```

- 绑定 `0.0.0.0:<临时端口>`(`TcpListener::bind((UNSPECIFIED, 0))`);
- 仅响应 `GET /firmware.bin`,返回预加载字节;
- **不解析**请求体、不写文件,只回显内存中的 bytes;
- 超时 5 分钟(`SERVER_AUTO_STOP`)或 `stop()` 后线程退出。

> **安全边界**:服务只回一个预加载的字节串,不落盘、不解析请求体。
> 局域网内任意主机都能 GET 到这台 PC 当前正在 OTA 的固件;这是协议层
> 显式的取舍 —— 固件校验保证一致性,不靠传输层鉴权。

### 2.4 UI 触发

调用链:

```
panel_about / panel_settings (UI)
   └─ UiEvent::OtaSelectFile          // rfd 选 .bin
   └─ UiEvent::OtaStartDownload { url, md5 }
        └─ app.rs::drain_ui_events
             └─ 0x0b Frame with FirmwareOtaReq
                  └─ 0x8b response → Toast 成功/失败
```

详见源码 `src/ota.rs` 与对应 panel 的实现。

## 3. 协议层(`0x0b`)

请求体 (`App → 设备`):

```json
{
  "cmd": 11,
  "seq": 17,
  "data": {
    "url": "http://192.168.1.10:54321/firmware.bin",
    "checksum": "d41d8cd98f00b204e9800998ecf8427e"
  }
}
```

| 字段       | 类型   | 说明 |
| ---------- | ------ | --- |
| `url`      | string | 必须是 App 端 `FirmwareServer::port` + `local_lan_ip` 拼出,且 path 必须为 `/firmware.bin` |
| `checksum` | string | MD5,小写 32 位 hex |

响应体 (`设备 → App`) 为标准响应(成功 `status=0`)。升级进度 / 结果由
设备主动通过 `0x0b` 推 `FirmwareInfo` 或自定义推送通道反馈。

> ⚠️ 当前 `CMD_FIRMWARE_INFO` 数据结构已定义,但 **App 调用层未接入**(见
> `docs/protocol-usage.md` §2.1 App TODO)。OTA 实际链路依赖 `panel_*` 自行
> 拼 `Frame::request(0x0b, ...)`。

## 4. 设备侧(参考)

> 此节是给固件维护者的提示,不约束 App 实现。

设备在收到 `0x0b` 后:

1. 用 URL 发起 HTTP GET,边收边累加 MD5;
2. 收完校验:一致 → 写入 ota 分区 → 重启;不一致 → 回 `status=1` + 错误码;
3. 总超时 120s(`kOtaTotalTimeoutMs`);App 端 HTTP 服务保留 5 分钟覆盖
   「设备先忙一会儿再开始下载」。

## 5. 故障排查

| 现象 | 可能原因 | 排查 |
| --- | --- | --- |
| Toast: 「设备拒绝下载」 | 设备不在同一 LAN / URL 路径不对 / MD5 不一致 | 用 `local_lan_ip()` 是否非 loopback;URL 必须 `/firmware.bin`;重新计算 MD5 |
| Toast: 「服务起不来」 | 端口被占用 / 防火墙拦截 | `FirmwareServer::spawn` 返回的 `io::Error` 透传到 UI |
| 下载卡住 | 设备中途掉电 / WiFi 不稳 | 设备侧 120s 超时后回错误;App 端 5min 后自动关闭服务,UI 可再次发起 |
| 升级后设备不重启 | 固件未通过 MD5 校验 / ota 分区写入失败 | 设备端日志(串口) + 串口捕获 `0x0b` 响应 |

## 6. 安全注意事项

- **临时 HTTP 服务暴露在 LAN**:任何同网段主机都能 GET 当前固件。**不要**
  在 App 启动时(非用户主动触发)拉起服务;只在用户点「开始下载」时 spawn,
  并在成功后 / 失败后立即 stop。
- **固件来源**:App 不内置任何固件,完全由用户选择文件;UI 需提示「请从
  官方 / 受信来源获取 .bin」,否则后果自负。
- **MD5 算法**:协议层固定 MD5,本仓库 `docs/protocol-usage.md` §12 列出
  升级到更强校验的检查项。
