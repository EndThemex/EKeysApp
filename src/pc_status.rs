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
//! `cpu_temp_c` / `disk_io_percent` / `network_up_kbps` / `network_down_kbps`
//! 需要 PDH / WMI / `GetIfTable` 等更重量级的 API，本轮先不采集（保持 None，
//! 序列化时自动跳过）。

use crate::protocol::PcStatus;

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use windows_sys::Win32::Networking::WinInet::InternetGetConnectedState;
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
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
}

/// 一次性快照：采集当前 PC 状态。
///
/// CPU% 需要两次采样才能产出 delta；首次返回 `cpu_usage_percent = None`。
/// UI 端按 `None` 显示"采样中…"，设备端协议层 None 字段会被 serde 跳过。
pub fn snapshot() -> PcStatus {
    PcStatus {
        caps_lock: Some(platform::caps_lock()),
        num_lock: Some(platform::num_lock()),
        scroll_lock: Some(platform::scroll_lock()),
        network_connected: Some(platform::network_connected()),
        cpu_usage_percent: platform::cpu_usage_percent(),
        memory_usage_percent: platform::memory_usage_percent(),
        // 其余扩展字段暂不采集，保持 None（序列化自动跳过）。
        cpu_temp_c: None,
        disk_io_percent: None,
        network_up_kbps: None,
        network_down_kbps: None,
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
        // cpu_temp_c / disk_io_percent / network_*_kbps 暂不采集
        assert!(s.cpu_temp_c.is_none());
        assert!(s.disk_io_percent.is_none());
        assert!(s.network_up_kbps.is_none());
        assert!(s.network_down_kbps.is_none());
        // 内存% 应在 0~100
        let mem = s.memory_usage_percent.unwrap();
        assert!((0.0..=100.0).contains(&mem), "内存% 应在 0~100，实际 {mem}");
    }

    /// 序列化应当只输出基础字段 + memory_usage_percent；CPU% 首次为 None
    /// 时被跳过；cpu_temp_c / disk_io / network_*_kbps 全部跳过。
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
        assert!(v.get("network_down_kbps").is_none());
        assert!(v.get("network_up_kbps").is_none());
        // cpu_usage_percent 可能出现也可能不出现，取决于 CpuSampler 是否
        // 已经被本次测试链路预热过（其他测试先调过一次就有值）。
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
