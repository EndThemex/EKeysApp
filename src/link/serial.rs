//! USB CDC 串口枚举与打开。

use serde::Serialize;
use serialport::{SerialPortInfo, SerialPortType};

/// ESP32-S3 USB CDC VID
pub const ESP32_S3_VID: u16 = 0x303A;

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
    serialport::new(name, 115_200)
        .timeout(std::time::Duration::from_millis(100))
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .flow_control(serialport::FlowControl::None)
        .open()
}
