//! 后台读线程：按行分帧 → 协议帧/固件日志分流。

use std::io::Read;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::link::LinkEvent;
use crate::protocol;

/// 共享端口版的读循环（reader 与 writer 通过 Mutex 串行访问）
pub fn run_shared(port: Arc<Mutex<Box<dyn serialport::SerialPort>>>, tx: Sender<LinkEvent>) {
    let mut buf = [0u8; 512];
    let mut line = String::with_capacity(256);

    loop {
        let read_result = {
            let mut p = match port.lock() {
                Ok(g) => g,
                Err(_) => {
                    let _ = tx.send(LinkEvent::Error("串口锁被毒化".into()));
                    return;
                }
            };
            p.read(&mut buf)
        };

        match read_result {
            Ok(0) => {
                let _ = tx.send(LinkEvent::Error("串口对端关闭".into()));
                return;
            }
            Ok(n) => {
                for &b in &buf[..n] {
                    if b == b'\n' {
                        handle_line(&mut line, &tx);
                    } else if b != b'\r' {
                        // 协议 §1：剥离 \r
                        line.push(b as char);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => {
                let _ = tx.send(LinkEvent::Error(format!("串口读失败: {e}")));
                return;
            }
        }

        // 防止单行过长导致内存膨胀
        if line.len() > 16 * 1024 {
            line.clear();
            let _ = tx.send(LinkEvent::Error("单行过长，已丢弃".into()));
        }
    }
}

fn handle_line(line: &mut String, tx: &Sender<LinkEvent>) {
    let s = line.trim();
    if s.is_empty() {
        line.clear();
        return;
    }
    match protocol::try_parse_line(s) {
        None => {
            let _ = tx.send(LinkEvent::LogLine(s.to_string()));
        }
        Some(Ok(frame)) => {
            let _ = tx.send(LinkEvent::Frame(frame));
        }
        Some(Err(e)) => {
            let _ = tx.send(LinkEvent::LogLine(format!("[PARSE_ERR] {s} ({e})")));
        }
    }
    line.clear();
}

/// 周期（用于 read timeout）
#[allow(dead_code)]
pub fn read_timeout() -> Duration {
    Duration::from_millis(100)
}
