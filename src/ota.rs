//! OTA 支持：固件 MD5 计算 + 本机临时 HTTP 文件服务 + 局域网地址探测。
//!
//! 流程与固件端对齐（EKeys `src/upgrade/Upgrade.cpp`）：
//! 1. App 选择 `.bin` 固件文件，计算 MD5（32 位 hex）；
//! 2. App 在 `0.0.0.0:<临时端口>` 起一个一次性 HTTP 服务，只响应
//!    `GET /firmware.bin`，把固件字节回给设备；
//! 3. 通过 `0x0B CMD_FIRMWARE_INFO` 下发 `url` + `checksum`，设备从本机
//!    HTTP 服务流式下载、边下边校验 MD5，通过后写入 ota 分区并重启。
//!
//! 安全边界：服务只回一个预加载的字节串，不落盘、不解析请求体；
//! 超时（默认 5 分钟）后自动关闭——设备端 OTA 总超时 120s，留足余量。

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 计算数据 MD5，返回 32 位小写十六进制字符串（与固件端 `hexToDigest` 对齐）。
pub fn md5_hex(data: &[u8]) -> String {
    use md5::Digest;
    let mut hasher = md5::Md5::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(32);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 探测本机局域网 IPv4（不发包，仅查路由表）。
///
/// 原理：对公网地址发起 UDP `connect`（不 send），内核据此选定默认路由的
/// 出口网卡，`local_addr()` 即为本机在该网段的 IP。设备与 PC 同处一个
/// 局域网时，该地址就是设备可访问的下载地址。
pub fn local_lan_ip() -> Option<IpAddr> {
    for probe in [Ipv4Addr::new(8, 8, 8, 8), Ipv4Addr::new(192, 168, 0, 1)] {
        let addr = SocketAddr::new(IpAddr::V4(probe), 80);
        if let Ok(sock) = UdpSocket::bind(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            0,
        )) {
            if sock.connect(addr).is_ok() {
                if let Ok(local) = sock.local_addr() {
                    if !local.ip().is_loopback() {
                        return Some(local.ip());
                    }
                }
            }
        }
    }
    None
}

/// 一次性固件 HTTP 服务句柄。`stop()` 或超时后后台线程退出。
pub struct FirmwareServer {
    pub port: u16,
    stop: Arc<AtomicBool>,
    /// 防止意外多次 stop；Drop 时也兜底停掉线程
    stopped: bool,
}

/// 服务自动关闭时长。设备端 OTA 总超时 120s（`kOtaTotalTimeoutMs`），
/// 5 分钟足够覆盖"设备先忙一会儿再开始下载"的场景。
const SERVER_AUTO_STOP: Duration = Duration::from_secs(300);

impl FirmwareServer {
    /// 起服务线程。`bytes` 为完整固件内容（已在选择文件时读入内存）。
    pub fn spawn(bytes: Arc<Vec<u8>>) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = Arc::clone(&stop);

        std::thread::Builder::new()
            .name("ota-http".into())
            .spawn(move || {
                let deadline = Instant::now() + SERVER_AUTO_STOP;
                while !stop2.load(Ordering::Relaxed) && Instant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // 单连接单请求：读完请求头（丢弃）→ 回固件字节
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                            drain_request(&mut stream);
                            serve_firmware(&mut stream, &bytes);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self {
            port,
            stop,
            stopped: false,
        })
    }

    /// 停止服务（幂等）。
    pub fn stop(&mut self) {
        if !self.stopped {
            self.stopped = true;
            self.stop.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for FirmwareServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 读取并丢弃 HTTP 请求头（到 `\r\n\r\n` 为止）。设备端 HTTPClient 只发 GET。
fn drain_request(stream: &mut std::net::TcpStream) {
    let mut buf = [0u8; 1024];
    let mut total = 0usize;
    while total < buf.len() {
        match stream.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                // 请求头结束即返回，不等待 body
                if let Ok(text) = std::str::from_utf8(&buf[..total]) {
                    if text.contains("\r\n\r\n") {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
}

/// 回写固件字节。设备只需要 200 + Content-Length + body。
fn serve_firmware(stream: &mut std::net::TcpStream, bytes: &[u8]) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(bytes);
    let _ = stream.flush();
}
