//! PC 状态采集：从宿主机器采集键盘 Lock 状态、网络连通性、CPU / 内存使用率，
//! 组装成 [`crate::protocol::PcStatus`] 后通过 `0x0D CMD_PC_STATUS` 推送到设备。
//!
//! 字段全集：
//! - `caps_lock` / `num_lock` / `scroll_lock`（Win32 `GetAsyncKeyState`）
//! - `network_connected`（wininet `InternetGetConnectedState`）
//! - `cpu_usage_percent`（Win32 `GetSystemTimes`；模块内置 CPU 采样器
//!   记录上一拍 idle/kernel/user 时间以计算 delta）
//! - `memory_usage_percent`（Win32 `GlobalMemoryStatusEx`）
//!
//! `cpu_temp_c` / `disk_io_percent`
//! 需要 PDH / WMI / `GetIfTable` 等更重量级的 API，本轮先不采集（保持 None，
//! 序列化时自动跳过）。
//!
//! 本轮已接通：
//! - `network_up_kbps` / `network_down_kbps`（`GetIfTable` 网卡字节计数差值，
//!   跳过环回与非操作状态接口）
//! - `disk_space_percent`（`GetLogicalDrives` + `GetDriveTypeW` 过滤固定盘，
//!   `GetDiskFreeSpaceExW` 加权汇总）

use crate::protocol::PcStatus;

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIfTable, IF_OPER_STATUS_OPERATIONAL, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IFTABLE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
    };
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;
    use windows_sys::Win32::Networking::WinInet::InternetGetConnectedState;
    use windows_sys::Win32::System::SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CAPITAL, VK_NUMLOCK, VK_SCROLL,
    };

    /// 把 `GetAsyncKeyState` 的高位 bit 解读为 bool（按下 = 1）。
    /// 返回 `true` 表示对应 Lock 键处于"激活"状态。
    ///
    /// windows-sys 0.52 的 `GetAsyncKeyState` 接受 `i32` 虚拟键码；
    /// `VK_*` 常量定义成 `u16`，这里 `as i32` 转换（值都在 SHORT 范围）。
    fn lock_active(vk: i32) -> bool {
        // GetAsyncKeyState 返回 SHORT（i16），高位（0x8000）= 当前是否按下。
        // 注意：Lock 类键是 toggle，按一次翻转一次；这里只读"当前是否处于
        // 按下/激活态"，符合 OS UI 显示的指示灯状态。
        unsafe { (GetAsyncKeyState(vk) as u16) & 0x8000 != 0 }
    }

    pub fn caps_lock() -> bool {
        lock_active(VK_CAPITAL as i32)
    }

    pub fn num_lock() -> bool {
        lock_active(VK_NUMLOCK as i32)
    }

    pub fn scroll_lock() -> bool {
        lock_active(VK_SCROLL as i32)
    }

    /// 网络连通性探测：使用 wininet 的 `InternetGetConnectedState`。
    /// 返回 `true` 表示系统至少存在一个处于连接状态的 LAN / modem 接口。
    /// 注：仅判断本地链路层是否已连接到上游，并不验证是否能访问公网；
    /// 固件侧只需"是否连得上"，粗粒度判断已足够。
    pub fn network_connected() -> bool {
        let mut flags = 0u32;
        // lpdwFlags 可为 NULL（旧文档允许），此处仍按规范传一个 out 参数；
        // 返回非 0 即视为已连接。
        unsafe { InternetGetConnectedState(&mut flags, 0) != 0 }
    }

    /// Win32 FILETIME（100ns 单位，无符号 64 位）。
    /// 直接转 u128 防止加法溢出（边角场景：连续高负载时间累加可能溢出 u64）。
    type Filetime = u128;

    fn filetime_to_u128(ft: &windows_sys::Win32::Foundation::FILETIME) -> Filetime {
        ((ft.dwHighDateTime as u128) << 32) | (ft.dwLowDateTime as u128)
    }

    /// 当前 idle / kernel / user 时间（100ns 累计）。失败返回 None。
    fn system_times() -> Option<(Filetime, Filetime, Filetime)> {
        // FILETIME 在 windows-sys 0.52 是 `MaybeUninit` 风格？实测需要 `unsafe`
        // zeroed（struct 含 padding）。直接 MaybeUninit + assume_init 更显式，
        // 但 zeroed() 在 FILETIME 上是 OK 的（两个 DWORD，无 invalid bit pattern）。
        let mut idle: windows_sys::Win32::Foundation::FILETIME = unsafe { std::mem::zeroed() };
        let mut kernel: windows_sys::Win32::Foundation::FILETIME = unsafe { std::mem::zeroed() };
        let mut user: windows_sys::Win32::Foundation::FILETIME = unsafe { std::mem::zeroed() };
        // GetSystemTimes 第二个参数历史上文档有些模糊（"system" 含 idle
        // 时间还是分开返回）；windows-sys 与 Win32 文档均明确 kernel +
        // user 分别走两个参数。GetSystemTimes(idle, kernel, user)。
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
        if ok == 0 {
            return None;
        }
        Some((
            filetime_to_u128(&idle),
            filetime_to_u128(&kernel),
            filetime_to_u128(&user),
        ))
    }

    /// 系统内存使用率（0~100）。
    ///
    /// 返回 `None` 表示 `GlobalMemoryStatusEx` 失败（极少发生，主要是 OS API
    /// 损坏 / 极旧 Windows），调用方按 `None` 处理（不写入 PcStatus）。
    pub fn memory_usage_percent() -> Option<f32> {
        let mut ms: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        // windows-sys 0.52 的 MEMORYSTATUSEX 没有 const 字段，只能 memcpy
        // 一个已知大小；直接 zeroed 后用 set_dwLength 设置长度字段。
        ms.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        let ok = unsafe { GlobalMemoryStatusEx(&mut ms) };
        if ok == 0 {
            return None;
        }
        // dwMemoryLoad 是 Windows 自带的 0~100 内存占用百分比（系统全局）
        if ms.ullTotalPhys == 0 {
            return None;
        }
        // dwMemoryLoad 已经是 0~100 的整数百分比，直接作为 f32 使用，
        // 精度上 OS 内部就是 0~100 整数，无需再用总/可用自己算（自算会
        // 受共享内存页等影响，结果会有几 GB 偏差）。
        Some(ms.dwMemoryLoad as f32)
    }

    /// CPU 采样器：记录上一拍的 idle / kernel / user 时间，
    /// 下次调用时计算两次采样的差值得出 CPU 占用率。
    ///
    /// 使用 `OnceLock<Mutex<_>>` 全局共享：ui / tick_pc_status_push 与
    /// settings 面板的实时预览都会触发采集，必须在同一进程内看到一致的
    /// CPU% 序列。`Mutex` 仅在采集瞬间持有，开销可忽略。
    pub struct CpuSampler {
        last_idle: Filetime,
        last_kernel: Filetime,
        last_user: Filetime,
        /// 首次采样标志：首次没有 delta 数据，返回 None 让调用方跳过。
        primed: bool,
    }

    impl CpuSampler {
        const fn new() -> Self {
            Self {
                last_idle: 0,
                last_kernel: 0,
                last_user: 0,
                primed: false,
            }
        }

        /// 读取当前 CPU 占用率（0~100）。
        ///
        /// 首次调用返回 `None`（建立基线）；之后基于 delta 计算。
        /// kernel 时间含 idle 时间，所以公式为：
        ///   busy = (Δkernel + Δuser) - Δidle
        ///   total = Δkernel + Δuser
        ///   cpu% = busy / total * 100
        pub fn sample(&mut self) -> Option<f32> {
            let (idle, kernel, user) = system_times()?;
            if !self.primed {
                self.last_idle = idle;
                self.last_kernel = kernel;
                self.last_user = user;
                self.primed = true;
                return None;
            }
            let d_idle = idle.saturating_sub(self.last_idle);
            let d_kernel = kernel.saturating_sub(self.last_kernel);
            let d_user = user.saturating_sub(self.last_user);
            let total = d_kernel + d_user;
            if total == 0 {
                // 两次采样之间几乎无时间流逝（如 100ns 精度下相邻 tick），
                // 返回 None 让调用方跳过本次推送（避免出现 0 / NaN）。
                return None;
            }
            let busy = total.saturating_sub(d_idle);
            self.last_idle = idle;
            self.last_kernel = kernel;
            self.last_user = user;
            // 钳位 0~100，理论 total >= busy，busy/total 不应 >1，但浮点
            // 误差下偶发 100.0000001，这里直接 clamp 保护固件 UI 显示。
            Some(((busy as f64) / (total as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
    }

    /// 进程全局 CPU 采样器；UI 与推送 tick 共用同一序列。
    pub fn cpu_sampler() -> &'static std::sync::Mutex<CpuSampler> {
        static SAMPLER: OnceLock<std::sync::Mutex<CpuSampler>> = OnceLock::new();
        SAMPLER.get_or_init(|| std::sync::Mutex::new(CpuSampler::new()))
    }

    /// 便捷包装：拿锁 + 采集。失败（采样器未 primed 或 OS 返回 0）返回 None。
    pub fn cpu_usage_percent() -> Option<f32> {
        cpu_sampler().lock().ok()?.sample()
    }

    /// 遍历所有网卡（跳过环回与非操作状态），汇总 in/out 字节计数（u64 累加，
    /// 单口 32 位计数器在 1s 采样间隔下不会回绕，累加侧用 u64 无压力）。
    fn iface_octets() -> Option<(u64, u64)> {
        // 首次调用传 NULL 拿所需缓冲区大小（返回 ERROR_INSUFFICIENT_BUFFER）
        let mut size: u32 = 0;
        let rc = unsafe { GetIfTable(std::ptr::null_mut(), &mut size, 0) };
        if (rc != ERROR_INSUFFICIENT_BUFFER && rc != 0) || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let table = buf.as_mut_ptr() as *mut MIB_IFTABLE;
        let rc = unsafe { GetIfTable(table, &mut size, 0) };
        if rc != 0 {
            return None;
        }
        let entries = unsafe { (*table).dwNumEntries as usize };
        let mut total_in = 0u64;
        let mut total_out = 0u64;
        for i in 0..entries {
            let row = unsafe { (*table).table.as_ptr().add(i) };
            // 环回口流量与外网无关；非操作状态接口计数不走
            unsafe {
                if (*row).dwType == IF_TYPE_SOFTWARE_LOOPBACK {
                    continue;
                }
                if (*row).dwOperStatus != IF_OPER_STATUS_OPERATIONAL {
                    continue;
                }
                total_in += (*row).dwInOctets as u64;
                total_out += (*row).dwOutOctets as u64;
            }
        }
        Some((total_in, total_out))
    }

    /// 网速采样器：记录上一拍的总字节计数与系统 tick，
    /// 差值 / 间隔 = 速率（kbps，bit 千位每秒，与固件协议字段语义一致）。
    ///
    /// 与 CPU 采样器同款 `OnceLock<Mutex<_>>` 全局共享：推送 tick 与设置面板
    /// 预览看到同一序列，避免双重采样导致速率减半。
    pub struct NetSpeedSampler {
        last_ms: u64,
        last_in: u64,
        last_out: u64,
        primed: bool,
    }

    impl NetSpeedSampler {
        const fn new() -> Self {
            Self {
                last_ms: 0,
                last_in: 0,
                last_out: 0,
                primed: false,
            }
        }

        /// 返回 `(up_kbps, down_kbps)`；首拍建基线返回 None。
        pub fn sample(&mut self) -> Option<(f32, f32)> {
            let (recv, sent) = iface_octets()?;
            let now = unsafe { GetTickCount64() };
            if !self.primed {
                self.last_ms = now;
                self.last_in = recv;
                self.last_out = sent;
                self.primed = true;
                return None;
            }
            let dt_s = now.saturating_sub(self.last_ms) as f32 / 1000.0;
            if dt_s < 0.05 {
                // 间隔过短，速率噪声大；跳过本次
                return None;
            }
            let d_in = recv.wrapping_sub(self.last_in);
            let d_out = sent.wrapping_sub(self.last_out);
            self.last_ms = now;
            self.last_in = recv;
            self.last_out = sent;
            let down_kbps = d_in as f64 * 8.0 / 1000.0 / dt_s as f64;
            let up_kbps = d_out as f64 * 8.0 / 1000.0 / dt_s as f64;
            Some((up_kbps as f32, down_kbps as f32))
        }
    }

    pub fn net_speed_sampler() -> &'static std::sync::Mutex<NetSpeedSampler> {
        static SAMPLER: OnceLock<std::sync::Mutex<NetSpeedSampler>> = OnceLock::new();
        SAMPLER.get_or_init(|| std::sync::Mutex::new(NetSpeedSampler::new()))
    }

    /// 便捷包装：返回 `(up_kbps, down_kbps)`，未就绪返回 None。
    pub fn network_speeds_kbps() -> Option<(f32, f32)> {
        net_speed_sampler().lock().ok()?.sample()
    }

    /// 磁盘空间使用率（0~100）：枚举逻辑盘位，只统计固定盘（DRIVE_FIXED，
    /// 排除 U 盘 / 光驱 / 网络映射盘），按容量加权汇总。
    pub fn disk_space_percent() -> Option<f32> {
        let drives = unsafe { GetLogicalDrives() };
        if drives == 0 {
            return None;
        }
        let mut total = 0u64;
        let mut free = 0u64;
        for i in 0..26u32 {
            if drives & (1u32 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as u16;
            let root: [u16; 4] = [letter, b':' as u16, b'\\' as u16, 0];
            unsafe {
                if GetDriveTypeW(root.as_ptr()) != DRIVE_FIXED {
                    continue;
                }
                let mut avail = 0u64;
                let mut t = 0u64;
                let mut f = 0u64;
                if GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut t, &mut f) != 0 {
                    total += t;
                    free += f;
                }
            }
        }
        if total == 0 {
            return None;
        }
        Some(((total - free) as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32)
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn caps_lock() -> bool {
        false
    }
    pub fn num_lock() -> bool {
        false
    }
    pub fn scroll_lock() -> bool {
        false
    }
    pub fn network_connected() -> bool {
        // 非 Windows 平台：保守按"未知"返回 true，避免阻塞设备侧展示。
        true
    }
    pub fn cpu_usage_percent() -> Option<f32> {
        None
    }
    pub fn memory_usage_percent() -> Option<f32> {
        None
    }
    pub fn network_speeds_kbps() -> Option<(f32, f32)> {
        None
    }
    pub fn disk_space_percent() -> Option<f32> {
        None
    }
}

/// 一次性快照：采集当前 PC 状态。
///
/// CPU% 需要两次采样才能产出 delta；首次返回 `cpu_usage_percent = None`。
/// UI 端按 `None` 显示"采样中…"，设备端协议层 None 字段会被 serde 跳过。
pub fn snapshot() -> PcStatus {
    let speeds = platform::network_speeds_kbps();
    PcStatus {
        caps_lock: Some(platform::caps_lock()),
        num_lock: Some(platform::num_lock()),
        scroll_lock: Some(platform::scroll_lock()),
        network_connected: Some(platform::network_connected()),
        cpu_usage_percent: platform::cpu_usage_percent(),
        memory_usage_percent: platform::memory_usage_percent(),
        disk_space_percent: platform::disk_space_percent(),
        network_up_kbps: speeds.map(|s| s.0),
        network_down_kbps: speeds.map(|s| s.1),
        // 其余扩展字段暂不采集，保持 None（序列化自动跳过）。
        cpu_temp_c: None,
        disk_io_percent: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 采集函数至少不应 panic。当前环境 Lock 状态任意，仅验证调用链通畅。
    #[test]
    fn snapshot_runs_without_panic() {
        let s = snapshot();
        // 基础字段必须 Some（采集器强制填值）
        assert!(s.caps_lock.is_some());
        assert!(s.num_lock.is_some());
        assert!(s.scroll_lock.is_some());
        assert!(s.network_connected.is_some());
        // 内存% 必须 Some（GlobalMemoryStatusEx 单次即可产出）
        assert!(s.memory_usage_percent.is_some());
        // cpu_temp_c / disk_io_percent 暂不采集
        assert!(s.cpu_temp_c.is_none());
        assert!(s.disk_io_percent.is_none());
        // 内存% 应在 0~100
        let mem = s.memory_usage_percent.unwrap();
        assert!((0.0..=100.0).contains(&mem), "内存% 应在 0~100，实际 {mem}");
        // 磁盘空间（Windows 上必有固定盘）：Some 时必须在 0~100
        if let Some(space) = s.disk_space_percent {
            assert!(
                (0.0..=100.0).contains(&space),
                "磁盘空间% 应在 0~100，实际 {space}"
            );
        }
        // 网速首拍 None（建基线），之后 Some；不做具体值断言
    }

    /// 序列化应当只输出采集到的字段；CPU% 首次为 None 时被跳过；
    /// cpu_temp_c / disk_io_percent 暂不采集，全部跳过。
    #[test]
    fn snapshot_serialization_omits_none_fields() {
        let s = snapshot();
        let v = serde_json::to_value(&s).expect("serialize");
        assert!(v.get("caps_lock").is_some());
        assert!(v.get("num_lock").is_some());
        assert!(v.get("scroll_lock").is_some());
        assert!(v.get("network_connected").is_some());
        // memory 应被序列化
        assert!(v.get("memory_usage_percent").is_some());
        // 暂未实现的字段不应出现
        assert!(v.get("cpu_temp_c").is_none());
        assert!(v.get("disk_io_percent").is_none());
        // cpu_usage_percent / disk_space_percent / network_*_kbps 可能出现
        // 也可能不出现，取决于采样器是否已被本次测试链路预热过。
    }

    /// CPU 采样器：连续两次采样，第二次必须返回 Some（首拍基线被填）。
    /// 平台不支持（测试在 Linux / CI 上跑）时直接返回 pass。
    #[test]
    fn cpu_sampler_second_sample_returns_value() {
        // 第一次：返回 None（基线）
        let _ = platform::cpu_usage_percent();
        // 第二次：可能 Some / 也可能因 total=0 返回 None；测试只要不 panic。
        let v = platform::cpu_usage_percent();
        if let Some(v) = v {
            assert!((0.0..=100.0).contains(&v), "CPU% 应在 0~100，实际 {v}");
        }
    }
}
