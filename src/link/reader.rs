//! 后台读线程：按行分帧 → 协议帧/固件日志分流。

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::link::LinkEvent;
use crate::protocol;
use crate::state::LogKind;
use crate::util::log::SharedLog;

/// 共享端口版的读循环（reader 与 writer 通过 Mutex 串行访问）。
/// `stop` 置位后退出，保证 `LinkManager::close()` 的 join 能返回。
pub fn run_shared(
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    tx: Sender<LinkEvent>,
    stop: Arc<AtomicBool>,
    log: SharedLog,
) {
    log.push(LogKind::App, "reader 线程启动".to_string());
    let mut buf = [0u8; 512];
    // 按字节缓冲，整行收齐后再统一 UTF-8 解码：逐字节 `b as char` 会把
    // UTF-8 多字节序列拆成 Latin-1 字符，固件中文日志变成 "ä»å¤©" 乱码。
    let mut line: Vec<u8> = Vec::with_capacity(256);

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let read_result = {
            let mut p = match port.lock() {
                Ok(g) => g,
                Err(_) => {
                    log.push(LogKind::App, "串口锁被毒化".to_string());
                    let _ = tx.send(LinkEvent::Error("串口锁被毒化".into()));
                    return;
                }
            };
            p.read(&mut buf)
        };

        match read_result {
            Ok(0) => {
                log.push(LogKind::App, "串口对端关闭".to_string());
                let _ = tx.send(LinkEvent::Error("串口对端关闭".into()));
                return;
            }
            Ok(n) => {
                for &b in &buf[..n] {
                    if b == b'\n' {
                        let s = String::from_utf8_lossy(&line).into_owned();
                        handle_line(&s, &tx, &log);
                        line.clear();
                    } else if b != b'\r' {
                        // 协议 §1：剥离 \r
                        line.push(b);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // 关键：空闲时必须让出锁窗口，不能立刻重新 lock。
                // Windows 的 std::sync::Mutex（SRWLock）不公平：reader 释放后
                // 立即重拿，writer 会持续抢锁失败、被饿死数秒（实测 Tx 实际
                // 写入比入队晚 6s+，设备心跳响应成批迟到 → 心跳假超时反复
                // 进 Reconnecting）。sleep 打断 lock convoy，writer 在此窗口
                // 必然能拿到锁；空闲→有数据的首字节延迟增加 ≤ 一个 sleep，
                // 对本应用无感。
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) => {
                log.push(LogKind::App, format!("串口读失败: {e}"));
                let _ = tx.send(LinkEvent::Error(format!("串口读失败: {e}")));
                return;
            }
        }

        // 防止单行过长导致内存膨胀
        if line.len() > 16 * 1024 {
            line.clear();
            log.push(LogKind::App, "单行过长，已丢弃".to_string());
            let _ = tx.send(LinkEvent::Error("单行过长，已丢弃".into()));
        }
    }
}

fn handle_line(line: &str, tx: &Sender<LinkEvent>, log: &SharedLog) {
    let s = line.trim();
    if s.is_empty() {
        return;
    }
    match protocol::try_parse_line(s) {
        None => {
            // 非 JSON 行 → 视为固件日志原样转发；Rx 视角在这里已无意义，
            // 由 UI 面板按 Firmware 类别着色。
            let _ = tx.send(LinkEvent::LogLine(s.to_string()));
        }
        Some(Ok(_frame)) => {
            // 协议帧：router 线程会写一条 Rx 日志（含 cmd/seq/status），
            // 这里只做转发，避免重复。
            let _ = tx.send(LinkEvent::Frame(_frame));
        }
        Some(Err(e)) => {
            // 看起来像 JSON 但解析失败：写到 Firmware 行里给用户看，同时
            // 单独留一条 App 日志方便在日志面板里按关键字过滤。
            log.push(LogKind::App, format!("协议帧解析失败: {s} ({e})"));
            let _ = tx.send(LinkEvent::LogLine(format!("[PARSE_ERR] {s} ({e})")));
        }
    }
}

/// 周期（用于 read timeout）
#[allow(dead_code)]
pub fn read_timeout() -> Duration {
    Duration::from_millis(100)
}
