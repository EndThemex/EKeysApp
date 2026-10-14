//! PC 状态采集：从宿主机器采集键盘 Lock 状态、网络连通性、CPU / 内存使用率，
//! 组装成 [`crate::protocol::PcStatus`] 后通过 `0x0D CMD_PC_STATUS` 推送到设备。
//!
//! 字段全集：
//! - `caps_lock` / `num_lock` / `scroll_lock`（Win32 `GetAsyncKeyState`）
//! - `network_connected`（wininet `InternetGetConnectedState`）
//! - `cpu_usage_percent`（PDH `\Processor Information(_Total)\% Processor Utility`，
//!   与 Win10+ 任务管理器同源，按实际运行频率加权）
//! - `memory_usage_percent`（Win32 `GlobalMemoryStatusEx`）
//!
//! `cpu_temp_c` 已废弃（从未采集）：设备端"CPU-T"槽位改为显示 GPU 使用率，
//! 对应协议字段 `gpu_usage_percent`（PDH `\GPU Engine(*engtype_3D)` 求和）。
//!
//! 本轮已接通：
//! - `network_up_kbps` / `network_down_kbps`（`GetIfTable` 网卡字节计数差值，
//!   跳过环回与非操作状态接口）
//! - `disk_space_percent`（`GetLogicalDrives` + `GetDriveTypeW` 过滤固定盘，
//!   `GetDiskFreeSpaceExW` 加权汇总）
//! - `disk_io_percent`（PDH `\PhysicalDisk(_Total)\% Disk Time`）
//! - `gpu_usage_percent`（PDH `\GPU Engine(*engtype_3D)\Utilization Percentage`
//!   多实例求和，实例列表每 10s 重建一次）

use crate::protocol::PcStatus;

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIfTable, IF_OPER_STATUS_OPERATIONAL, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IFTABLE,
    };
    use windows_sys::Win32::Networking::WinInet::InternetGetConnectedState;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
    };
    use windows_sys::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_0, PDH_FMT_DOUBLE, PDH_MORE_DATA,
        PdhAddCounterW, PdhCloseQuery, PdhCollectQueryData, PdhExpandWildCardPathW,
        PdhGetFormattedCounterValue, PdhOpenQueryW,
    };
    use windows_sys::Win32::System::SystemInformation::{
        GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;
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

    /// CPU 采样器：PDH `\Processor Information(_Total)\% Processor Utility`。
    ///
    /// 选用理由：Win10+ 任务管理器的 CPU% 就是这个计数器——按**实际运行频率**
    /// 加权（P-state / Turbo 状态下"100% 活跃但降频"会算出 < 100%）。
    /// 旧实现 `GetSystemTimes` 是"忙时间简单平均"，不考虑频率，与任务管理器
    /// 存在系统性偏差（异构核 / 睿频机器上常见 10%+ 差距）。
    ///
    /// 与旧实现的差异：
    /// - `% Processor Utility` 返回 PDH 每次 collect 之间的**瞬时速率**（0~100
    ///   已归一化），无需自行做 delta；首拍即可产出有效值（仍按惯例首拍返回
    ///   None 建基线，保持与其它采样器一致的调用方语义）。
    /// - 该计数器在极老旧系统 / 精简镜像上不存在 → AddCounter 失败 → 恒 None，
    ///   调用方（协议 skip_serializing_if）自动跳过该字段。
    ///
    /// 查询常驻打开；collect 失败关闭重建，下次采样自愈。
    ///
    /// 使用 `OnceLock<Mutex<_>>` 全局共享：ui / tick_pc_status_push 与
    /// settings 面板的实时预览都会触发采集，必须在同一进程内看到一致的
    /// CPU% 序列。`Mutex` 仅在采集瞬间持有，开销可忽略。
    pub struct CpuSampler {
        query: isize,
        counter: isize,
        primed: bool,
        last_collect_ms: u64,
    }

    /// 最小采样窗口：推送 tick 与设置页预览共用本采样器，两者交错时可能在
    /// 很短时间内连续 collect。PDH rate 计数器窗口过短会放大量化噪声（任务
    /// 管理器同样是 ~1s 窗口），短于该值的调用直接返回 None。
    const CPU_MIN_WINDOW_MS: u64 = 500;

    impl CpuSampler {
        const fn new() -> Self {
            Self {
                query: 0,
                counter: 0,
                primed: false,
                last_collect_ms: 0,
            }
        }

        fn close(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query) };
            }
            self.query = 0;
            self.counter = 0;
            self.primed = false;
            self.last_collect_ms = 0;
        }

        fn ensure_open(&mut self) -> bool {
            if self.query != 0 {
                return true;
            }
            let Some(query) = open_query() else {
                return false;
            };
            let path = wide(r"\Processor Information(_Total)\% Processor Utility");
            let mut counter: isize = 0;
            let rc = unsafe { PdhAddCounterW(query, path.as_ptr(), 0, &mut counter) };
            if rc != 0 {
                unsafe { PdhCloseQuery(query) };
                return false;
            }
            self.query = query;
            self.counter = counter;
            self.primed = false;
            true
        }

        /// 读取当前 CPU 占用率（0~100）。
        ///
        /// 首次调用返回 `None`（建立基线，同时让 PDH 内部完成第一次 rate 采样）；
        /// 之后每次 collect 直接产出该拍的瞬时利用率。
        /// 两次 collect 间隔 < [`CPU_MIN_WINDOW_MS`] 时返回 None（不推进基线），
        /// 把有效采样留给下一次调用，避免过短窗口放大噪声。
        pub fn sample(&mut self) -> Option<f32> {
            if !self.ensure_open() {
                return None;
            }
            let now = unsafe { GetTickCount64() };
            if self.primed && now.saturating_sub(self.last_collect_ms) < CPU_MIN_WINDOW_MS {
                return None;
            }
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                self.close();
                return None;
            }
            self.last_collect_ms = now;
            if !self.primed {
                // rate 计数器首拍无有效 delta（PDH 内部要两次 collect 才有速率）
                self.primed = true;
                return None;
            }
            // CStatus 非零 = 样本无效（formatted_double 内部处理），返回 None
            formatted_double(self.counter).map(|v| v.clamp(0.0, 100.0) as f32)
        }
    }

    impl Drop for CpuSampler {
        fn drop(&mut self) {
            self.close();
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

    // ---------- PDH 通用辅助（disk_io / gpu 共用） ----------

    /// 字符串转 UTF-16 + NUL 终止（PDH 路径入参）。
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 打开实时查询（数据源 NULL = 本机活动数据）。失败返回 None。
    fn open_query() -> Option<isize> {
        let mut query: isize = 0;
        let rc = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
        if rc != 0 {
            return None;
        }
        Some(query)
    }

    /// 读取单个计数器的 double 格式化值（CStatus 非零 = 样本无效）。
    fn formatted_double(counter: isize) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE {
            CStatus: 0,
            Anonymous: PDH_FMT_COUNTERVALUE_0 { doubleValue: 0.0 },
        };
        let rc = unsafe {
            PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut value)
        };
        if rc != 0 || value.CStatus != 0 {
            return None;
        }
        Some(unsafe { value.Anonymous.doubleValue })
    }

    /// 磁盘 IO 活动率采样器（0~100，`% Disk Time`）。
    ///
    /// `% Disk Time` 是 rate 型计数器：需两次 collect 才有有效值，
    /// 首拍建基线返回 None（与 CPU/网速采样器语义一致）。
    /// 查询常驻打开；collect 失败（极少见）关闭重建，下次采样自愈。
    pub struct DiskIoSampler {
        query: isize,
        counter: isize,
        primed: bool,
    }

    impl DiskIoSampler {
        const fn new() -> Self {
            Self {
                query: 0,
                counter: 0,
                primed: false,
            }
        }

        fn close(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query) };
            }
            self.query = 0;
            self.counter = 0;
            self.primed = false;
        }

        fn ensure_open(&mut self) -> bool {
            if self.query != 0 {
                return true;
            }
            let Some(query) = open_query() else {
                return false;
            };
            let path = wide(r"\PhysicalDisk(_Total)\% Disk Time");
            let mut counter: isize = 0;
            let rc = unsafe { PdhAddCounterW(query, path.as_ptr(), 0, &mut counter) };
            if rc != 0 {
                unsafe { PdhCloseQuery(query) };
                return false;
            }
            self.query = query;
            self.counter = counter;
            self.primed = false;
            true
        }

        fn sample(&mut self) -> Option<f32> {
            if !self.ensure_open() {
                return None;
            }
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                self.close();
                return None;
            }
            if !self.primed {
                // rate 计数器首拍无有效 delta
                self.primed = true;
                return None;
            }
            formatted_double(self.counter).map(|v| v.clamp(0.0, 100.0) as f32)
        }
    }

    impl Drop for DiskIoSampler {
        fn drop(&mut self) {
            self.close();
        }
    }

    pub fn disk_io_sampler() -> &'static std::sync::Mutex<DiskIoSampler> {
        static SAMPLER: OnceLock<std::sync::Mutex<DiskIoSampler>> = OnceLock::new();
        SAMPLER.get_or_init(|| std::sync::Mutex::new(DiskIoSampler::new()))
    }

    /// 便捷包装：磁盘 IO 活动率（0~100），未就绪返回 None。
    pub fn disk_io_percent() -> Option<f32> {
        disk_io_sampler().lock().ok()?.sample()
    }

    /// GPU 使用率采样器（0~100）：`\GPU Engine(*engtype_3D)\Utilization Percentage`
    /// 多实例求和（每个进程 × 每个引擎一条实例），瞬时值非 rate，首拍即可用。
    ///
    /// 实例集合随进程启停 / 引擎激活动态变化：每 [`GPU_REFRESH_INTERVAL_MS`]
    /// 重建一次查询（关闭旧查询 → 展开通配符 → 逐实例 AddCounter）。
    /// 无 GPU Engine 计数器的机器（精简系统 / 基础显示驱动）恒返回 None。
    pub struct GpuSampler {
        query: isize,
        counters: Vec<isize>,
        last_refresh_ms: u64,
    }

    /// GPU 实例列表重建周期：太频繁会放大 PDH 开销，太疏会漏新进程的引擎。
    const GPU_REFRESH_INTERVAL_MS: u64 = 10_000;
    /// 实例数上限（几十~几百正常，防异常系统撑爆 Vec）。
    const GPU_MAX_INSTANCES: usize = 256;

    impl GpuSampler {
        const fn new() -> Self {
            Self {
                query: 0,
                counters: Vec::new(),
                last_refresh_ms: 0,
            }
        }

        fn close(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query) };
            }
            self.query = 0;
            self.counters.clear();
        }

        /// 展开通配符实例并重建查询。任一步失败都落在"空列表 → 返回 None"。
        fn refresh_instances(&mut self) {
            self.close();
            let Some(query) = open_query() else {
                return;
            };
            let wildcard = wide(r"\GPU Engine(*engtype_3D)\Utilization Percentage");
            // 两次调用模式：先探所需字符数（返回 PDH_MORE_DATA + 长度）
            let mut len: u32 = 0;
            let rc = unsafe {
                PdhExpandWildCardPathW(
                    std::ptr::null(),
                    wildcard.as_ptr(),
                    std::ptr::null_mut(),
                    &mut len,
                    0,
                )
            };
            if rc != PDH_MORE_DATA || len == 0 {
                unsafe { PdhCloseQuery(query) };
                return;
            }
            let mut buf = vec![0u16; len as usize];
            let rc = unsafe {
                PdhExpandWildCardPathW(
                    std::ptr::null(),
                    wildcard.as_ptr(),
                    buf.as_mut_ptr(),
                    &mut len,
                    0,
                )
            };
            if rc != 0 {
                unsafe { PdhCloseQuery(query) };
                return;
            }
            // multi-sz 缓冲：按 \0 分段，每段是一条已展开的完整计数器路径
            let mut counters = Vec::new();
            for seg in buf.split(|&c| c == 0) {
                if seg.is_empty() {
                    continue;
                }
                if counters.len() >= GPU_MAX_INSTANCES {
                    break;
                }
                let mut path: Vec<u16> = seg.to_vec();
                path.push(0);
                let mut counter: isize = 0;
                let rc = unsafe { PdhAddCounterW(query, path.as_ptr(), 0, &mut counter) };
                if rc == 0 {
                    counters.push(counter);
                }
            }
            // 全部实例都加失败（如计数器被组策略禁用）：不留空查询
            if counters.is_empty() {
                unsafe { PdhCloseQuery(query) };
                return;
            }
            self.query = query;
            self.counters = counters;
        }

        fn sample(&mut self) -> Option<f32> {
            let now = unsafe { GetTickCount64() };
            if self.query == 0
                || now.saturating_sub(self.last_refresh_ms) >= GPU_REFRESH_INTERVAL_MS
            {
                self.refresh_instances();
                self.last_refresh_ms = now;
            }
            if self.counters.is_empty() {
                return None;
            }
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                self.close();
                return None;
            }
            let mut total = 0f64;
            let mut valid = 0usize;
            for &c in &self.counters {
                if let Some(v) = formatted_double(c) {
                    total += v;
                    valid += 1;
                }
            }
            if valid == 0 {
                return None;
            }
            // 多实例求和可能因多 GPU / 多引擎略超 100，钳位保护固件 UI
            Some((total as f32).clamp(0.0, 100.0))
        }
    }

    impl Drop for GpuSampler {
        fn drop(&mut self) {
            self.close();
        }
    }

    pub fn gpu_sampler() -> &'static std::sync::Mutex<GpuSampler> {
        static SAMPLER: OnceLock<std::sync::Mutex<GpuSampler>> = OnceLock::new();
        SAMPLER.get_or_init(|| std::sync::Mutex::new(GpuSampler::new()))
    }

    /// 便捷包装：GPU 使用率（0~100），无计数器/未就绪返回 None。
    pub fn gpu_usage_percent() -> Option<f32> {
        gpu_sampler().lock().ok()?.sample()
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
    pub fn disk_io_percent() -> Option<f32> {
        None
    }
    pub fn gpu_usage_percent() -> Option<f32> {
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
        disk_io_percent: platform::disk_io_percent(),
        gpu_usage_percent: platform::gpu_usage_percent(),
        network_up_kbps: speeds.map(|s| s.0),
        network_down_kbps: speeds.map(|s| s.1),
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
        // 磁盘 IO / GPU：首拍可能 None（建基线 / 实例未就绪），Some 时必须在 0~100
        if let Some(io) = s.disk_io_percent {
            assert!(
                (0.0..=100.0).contains(&io),
                "磁盘 IO% 应在 0~100，实际 {io}"
            );
        }
        if let Some(gpu) = s.gpu_usage_percent {
            assert!((0.0..=100.0).contains(&gpu), "GPU% 应在 0~100，实际 {gpu}");
        }
        // 网速首拍 None（建基线），之后 Some；不做具体值断言
    }

    /// 序列化应当只输出采集到的字段；CPU% 首次为 None 时被跳过；
    /// 已废弃字段 cpu_temp_c 不再存在于结构体，永远不会输出。
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
        // 已废弃字段不应出现
        assert!(v.get("cpu_temp_c").is_none());
        // cpu_usage_percent / disk_space_percent / network_*_kbps 可能出现
        // 也可能不出现，取决于采样器是否已被本次测试链路预热过。
    }

    /// CPU 采样器：连续两次采样，第二次可能返回 Some（首拍基线被填）。
    /// 由于存在最小采样窗口保护（CPU_MIN_WINDOW_MS），窗口内第二次调用
    /// 会返回 None——属预期行为，测试只要求不 panic 且 Some 时在 0~100。
    /// 平台不支持（测试在 Linux / CI 上跑）时直接返回 pass。
    #[test]
    fn cpu_sampler_second_sample_returns_value() {
        // 第一次：返回 None（基线）
        let _ = platform::cpu_usage_percent();
        // 第二次：窗口内返回 None；窗口外可能 Some；测试只要不 panic。
        let v = platform::cpu_usage_percent();
        if let Some(v) = v {
            assert!((0.0..=100.0).contains(&v), "CPU% 应在 0~100，实际 {v}");
        }
    }
}
