//! USB CDC 串口枚举与打开。

use serde::Serialize;
use serialport::{SerialPortInfo, SerialPortType};

/// ESP32-S3 USB CDC VID
pub const ESP32_S3_VID: u16 = 0x303A;

/// WCH（沁恒）USB 转串口桥 VID：CH340/CH340C/CH340K/CH341/CH343/CH910x 系列。
/// 桥接芯片每个往返带 10~40ms 驱动批量缓冲延迟，协议的多轮请求-响应（连接
/// 初始化 / 心跳 / 配置下发）会被显著放大，表现为连接期间 UI 卡顿与超时误报。
/// App 对此类端口拒绝连接并提示改用设备原生 USB CDC。
pub const WCH_VID: u16 = 0x1A86;

/// 检测端口是否为 WCH USB 转串口桥（非原生 CDC）。命中返回 (vid, pid)；
/// 端口已拔出 / 非 USB 端口 / 其它厂商 / 枚举失败 → None（放行，按原流程处理）。
pub fn is_wch_bridge(name: &str) -> Option<(u16, u16)> {
    let ports = serialport::available_ports().ok()?;
    ports
        .into_iter()
        .find(|p| p.port_name == name)
        .and_then(|p| match p.port_type {
            SerialPortType::UsbPort(info) if info.vid == WCH_VID => Some((info.vid, info.pid)),
            _ => None,
        })
}

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    pub name: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

impl From<&SerialPortInfo> for PortInfo {
    fn from(p: &SerialPortInfo) -> Self {
        let (vid, pid, manufacturer, product, serial_number) = match &p.port_type {
            SerialPortType::UsbPort(info) => (
                Some(info.vid),
                Some(info.pid),
                info.manufacturer.clone(),
                info.product.clone(),
                info.serial_number.clone(),
            ),
            _ => (None, None, None, None, None),
        };
        Self {
            name: p.port_name.clone(),
            manufacturer,
            product,
            serial_number,
            vid,
            pid,
        }
    }
}

/// 枚举所有端口；按 VID 0x303A 优先排序（EKeys 排前）
pub fn list_ports() -> Vec<PortInfo> {
    let Ok(mut ports) = serialport::available_ports() else {
        return vec![];
    };
    // serialport crate 4.x: 枚举本身通常已按名称排序；这里按"是否为 EKeys"分组
    ports.sort_by_key(|p| {
        let is_ekeys = matches!(&p.port_type, SerialPortType::UsbPort(u) if u.vid == ESP32_S3_VID);
        if is_ekeys { 0 } else { 1 }
    });
    ports.iter().map(PortInfo::from).collect()
}

/// 打开一个端口（固定 115200）
pub fn open(name: &str) -> serialport::Result<Box<dyn serialport::SerialPort>> {
    let mut port = serialport::new(name, 115_200)
        .timeout(std::time::Duration::from_millis(100))
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .open()?;
    /*
     * 固件使用 TinyUSB CDC（ARDUINO_USB_MODE=0），其 USBCDC::write 在
     * host 未断言 DTR 时会静默丢弃所有输出（tud_cdc_n_connected == DTR），
     * 导致 App 收不到任何回复/日志。serialport crate 打开端口默认不拉
     * DTR/RTS，必须显式断言。
     */
    port.write_data_terminal_ready(true)?;
    port.write_request_to_send(true)?;
    Ok(port)
}
