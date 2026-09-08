//! eframe::App 实现：装配各面板、drain Link 事件、快捷键、Toast/Confirm。

use eframe::egui;

use crate::link::LinkEvent;
use crate::protocol::{
    CMD_CONFIG_GET, CMD_PROFILE_STATE, DeviceSettings, ProfileState, response_cmd, top_level,
};
use crate::state::{AppHandle, LogKind, Page, ToastKind, UiConfirmKind, UiEvent};
use crate::ui::{
    panel_about, panel_connection, panel_keymap, panel_lighting, panel_log, panel_settings,
    panel_voice, panel_wifi, sidenav, statusbar, topbar,
    widgets::{ConfirmOutcome, Toast, push_toast, show_confirm, show_local_settings, show_toasts},
};

pub struct WxiApp {
    pub handle: AppHandle,
    pub connect_st: panel_connection::ConnectPanelState,
    pub settings_st: panel_settings::SettingsPanelState,
    pub keymap_st: panel_keymap::KeymapPanelState,
    pub lighting_st: panel_lighting::LightingPanelState,
    pub wifi_st: panel_wifi::WifiPanelState,
    pub voice_st: panel_voice::VoicePanelState,
    pub log_st: panel_log::LogPanelState,
    pub toasts: Vec<Toast>,
    pub confirm_open: bool,
    pub confirm_title: String,
    pub confirm_body: String,
    pub confirm_kind: Option<UiConfirmKind>,
    pub current_port: Option<String>,
    pub local_settings_open: bool,
    pub last_inner_size: [f32; 2],
}

impl WxiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, handle: AppHandle) -> Self {
        crate::ui::fonts::install(&cc.egui_ctx);
        // 启动时应用主题（语言/主题由本地配置驱动）
        crate::ui::apply_theme(&cc.egui_ctx, handle.theme());
        let mut connect_st = panel_connection::ConnectPanelState::default();
        connect_st.refresh();
        let log_st = panel_log::LogPanelState {
            show_tx: true,
            show_rx: true,
            show_fw: true,
            show_app: true,
            level: panel_log::LevelFilter::All,
            search: String::new(),
        };
        Self {
            handle,
            connect_st,
            settings_st: panel_settings::SettingsPanelState::default(),
            keymap_st: panel_keymap::KeymapPanelState::default(),
            lighting_st: panel_lighting::LightingPanelState::default(),
            wifi_st: panel_wifi::WifiPanelState::default(),
            voice_st: panel_voice::VoicePanelState::default(),
            log_st,
            toasts: vec![],
            confirm_open: false,
            confirm_title: String::new(),
            confirm_body: String::new(),
            confirm_kind: None,
            current_port: None,
            local_settings_open: false,
            last_inner_size: [960.0, 600.0],
        }
    }

    fn drain_link_events(&mut self) {
        // 通过 LinkManager::poll_events 拉取一批事件：
        // 内部完成 seq 响应配对（request() 依赖）、心跳 ack 标记与状态同步，
        // 只把 UI 关心的事件（未配对帧 / 推送 / 状态 / 日志 / 错误）透传出来。
        let evs = self
            .handle
            .with_link(|lm| lm.poll_events())
            .unwrap_or_default();
        for e in evs {
            self.handle_link_event(e);
        }
    }

    fn handle_link_event(&mut self, e: LinkEvent) {
        match e {
            LinkEvent::Frame(f) => {
                // 异类顶层帧（0x10/0x0C/0x0F）cmd 最高位不为 1，不会命中 is_push()，
                // 必须先分流，否则会掉进下方错误分支误报"未知错误" Toast。
                if crate::protocol::is_top_level_cmd(f.cmd) {
                    self.handle_top_level_frame(f);
                } else if f.is_push() {
                    // 设备主动推送的全量配置快照（0x87 seq=0）
                    match f.data.as_ref() {
                        Some(data) => {
                            match serde_json::from_value::<DeviceSettings>(data.clone()) {
                                Ok(mut new_snap) => {
                                    self.handle.apply_settings_snapshot(&mut new_snap);
                                    self.handle.log_kind(LogKind::Rx, "PUSH ← 全量快照");
                                }
                                Err(e) => {
                                    self.handle
                                        .log_kind(LogKind::App, format!("推送快照解析失败: {e}"));
                                }
                            }
                        }
                        None => {
                            self.handle.log_kind(LogKind::App, "推送快照缺 data 字段");
                        }
                    }
                } else {
                    if f.status() == Some(0) {
                        // 迟到的 0x87 响应（request 已超时、未配对）：快照仍有效，
                        // 落地展示而不是只打一条 ACK 把数据丢掉。
                        if f.cmd == response_cmd(CMD_CONFIG_GET) {
                            if let Some(data) = f.data.as_ref() {
                                match serde_json::from_value::<DeviceSettings>(data.clone()) {
                                    Ok(mut snap) => {
                                        self.handle.apply_settings_snapshot(&mut snap);
                                        self.handle
                                            .log_kind(LogKind::Rx, "Rx ← 全量快照（迟到响应）");
                                        return; // handled，避免重复 ACK 日志
                                    }
                                    Err(e) => {
                                        self.handle.log_kind(
                                            LogKind::App,
                                            format!("迟到快照解析失败: {e}"),
                                        );
                                    }
                                }
                            }
                        }
                        self.handle
                            .log_kind(LogKind::Rx, format!("ACK ← 0x{:02X}", f.cmd));
                    } else {
                        let msg = f.error.unwrap_or_else(|| "未知错误".into());
                        self.handle.log_kind(LogKind::App, format!("错误: {msg}"));
                        push_toast(&mut self.toasts, ToastKind::Error, msg, 5000);
                    }
                }
            }
            LinkEvent::LogLine(s) => {
                self.handle.log_kind(LogKind::Firmware, s);
            }
            LinkEvent::State(s) => {
                // 状态变化必须同步进共享 state：顶栏/侧栏/连接页 UI 直接读这里
                *self.handle.state.lock().unwrap() = s.clone();
                self.handle
                    .log_kind(LogKind::App, format!("状态变更: {s:?}"));

                // 被动断开（reader 异常退出时由 router 转发）：卸掉旧 LM，
                // 并按上次连接的端口自动调度重连任务。
                // - 主动断开（用户点"断开"）走 detach_link() 直接改 state，
                //   不会经 router 转发到这里；所以这里的 Disconnected 一定
                //   是 reader 异常退出导致的。
                // - 区分点：handle.link 是否仍持有 LM。被动断开时 LM 还在。
                if matches!(s, crate::link::ConnectionState::Disconnected)
                    && self.handle.link.lock().unwrap().is_some()
                {
                    self.handle.detach_link();
                    // 先取出端口释放锁再调度：if let 的 scrutinee 临时 MutexGuard
                    // 会存活到块尾，schedule_reconnect 内部再锁其它互斥体时
                    // 极易形成同类自死锁（参照 panel_connection 的教训）。
                    let last_port = self.handle.last_port.lock().unwrap().clone();
                    if let Some(port) = last_port {
                        self.handle.schedule_reconnect(port);
                    }
                }
            }
            LinkEvent::Error(e) => {
                self.handle
                    .log_kind(LogKind::App, format!("Link 错误: {e}"));
                push_toast(&mut self.toasts, ToastKind::Error, e, 5000);
            }
        }
    }

    /// 异类顶层帧分发：body 在帧顶层而非 `data`，各命令单独解析。
    fn handle_top_level_frame(&mut self, f: crate::protocol::Frame) {
        match f.cmd {
            // 0x10 Profile State：查询响应（seq≠0）与切换/连接推送（seq=0）同格式，
            // 都用于刷新 Settings 页的当前 Profile 展示。
            CMD_PROFILE_STATE => match top_level::<ProfileState>(&f) {
                Ok(ps) => {
                    // 记录刷新前的本地 active_profile，用于判断是否需要重拉键映射
                    let prev_profile = self.handle.keymap.lock().unwrap().active_profile;
                    self.handle.apply_profile_state(&ps);
                    self.handle.log_kind(
                        LogKind::Rx,
                        format!(
                            "Rx ← Profile: #{} \"{}\"",
                            ps.active_profile, ps.profile_name
                        ),
                    );
                    // 设备端切了 Profile（0x06/0x08 只作用于激活 Profile）→
                    // 重拉 0x05 键映射，否则键映射页展示的还是旧 Profile 的数据
                    if ps.active_profile != prev_profile {
                        if let Err(e) = self.handle.refresh_keymap_from_device() {
                            self.handle.log_kind(
                                LogKind::App,
                                format!("Profile 切换后重拉键映射失败: {e}"),
                            );
                        }
                    }
                }
                Err(e) => {
                    self.handle
                        .log_kind(LogKind::App, format!("Profile 状态解析失败: {e}"));
                }
            },
            // 0x0C 语音文本 / 0x0F 音乐控制：推送链路尚未接入 UI，
            // router 已记录帧头日志，这里只吞掉避免误报错误。
            _ => {}
        }
    }

    fn drain_ui_events(&mut self) {
        while let Ok(ev) = self.handle.ui_rx.try_recv() {
            match ev {
                UiEvent::Toast(k, t) => {
                    let ttl = match k {
                        ToastKind::Error => 5000,
                        ToastKind::Warning => 4000,
                        _ => 2500,
                    };
                    push_toast(&mut self.toasts, k, t, ttl);
                }
                UiEvent::Navigate(p) => {
                    *self.handle.page.lock().unwrap() = p;
                }
                UiEvent::ConfirmYes(kind) => match kind {
                    UiConfirmKind::SwitchWorkMode => {
                        self.confirm_title = "切换工作模式".into();
                        self.confirm_body =
                            "切换工作模式将重建键盘实例，期间无法响应按键，是否继续？".into();
                        self.confirm_kind = Some(kind);
                        self.confirm_open = true;
                    }
                },
                UiEvent::ConfirmNo(_) => {
                    self.confirm_open = false;
                    self.confirm_kind = None;
                }
                UiEvent::OpenLocalSettings => {
                    self.local_settings_open = true;
                }
                UiEvent::CurrentPort(p) => {
                    // 顶栏端口名显示；与 handle.last_port 解耦（后者是"上次端口"，用于自动连接）
                    self.current_port = if p.is_empty() { None } else { Some(p) };
                }
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Keymap 面板处于"按下任意键捕获"模式时，全局快捷键必须让路，
        // 否则 Ctrl+1~8 / Esc 会被吃掉，捕获不到用户实际按的键。
        let capturing = *self.handle.capture_keyboard.lock().unwrap();
        if capturing {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            let _ = self.handle.ui_tx.send(UiEvent::Navigate(Page::Settings));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)) {
            let _ = self.handle.ui_tx.send(UiEvent::Navigate(Page::Log));
        }
        let nav = ctx.input(|i| {
            if !i.modifiers.ctrl {
                None
            } else if i.key_pressed(egui::Key::Num1) {
                Some(Page::Connect)
            } else if i.key_pressed(egui::Key::Num2) {
                Some(Page::Settings)
            } else if i.key_pressed(egui::Key::Num3) {
                Some(Page::Keymap)
            } else if i.key_pressed(egui::Key::Num4) {
                Some(Page::Lighting)
            } else if i.key_pressed(egui::Key::Num5) {
                Some(Page::Wifi)
            } else if i.key_pressed(egui::Key::Num6) {
                Some(Page::Voice)
            } else if i.key_pressed(egui::Key::Num7) {
                Some(Page::Log)
            } else if i.key_pressed(egui::Key::Num8) {
                Some(Page::About)
            } else {
                None
            }
        });
        if let Some(p) = nav {
            let _ = self.handle.ui_tx.send(UiEvent::Navigate(p));
        }
    }
}

impl eframe::App for WxiApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // 退出时保存 LocalConfig（含窗口大小、语言、主题）
        let lc = self.handle.local_config.lock().unwrap().clone();
        let cfg = crate::config::LocalConfig {
            last_port: self.handle.last_port.lock().unwrap().clone(),
            auto_connect: *self.handle.auto_connect.lock().unwrap(),
            window_size: Some(self.last_inner_size),
            language: lc.language,
            theme: lc.theme,
        };
        crate::config::save(&cfg);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_link_events();
        self.drain_ui_events();
        // 重连状态机：每帧驱动；time-to-next-try 之前直接 return
        self.handle.tick_reconnect();
        self.handle_shortcuts(ctx);

        // chrome（顶栏/侧栏/底栏）统一底色，与内容区形成清晰分区
        let chrome = ctx.style().visuals.extreme_bg_color;

        egui::TopBottomPanel::top("topbar")
            .frame(egui::Frame::new().fill(chrome).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 6,
                bottom: 6,
            }))
            .show(ctx, |ui| {
                topbar::show(&self.handle, ui, self.current_port.as_deref());
            });

        egui::SidePanel::left("sidenav")
            .resizable(false)
            .exact_width(188.0)
            .frame(egui::Frame::new().fill(chrome).inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: 10,
                bottom: 10,
            }))
            .show(ctx, |ui| {
                sidenav::show(&self.handle, ui);
            });

        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::new().fill(chrome).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 4,
            }))
            .show(ctx, |ui| {
                statusbar::show(&self.handle, ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            // 外层统一加垂直滚动：保证任何面板在窗口缩到很窄 / 很高时都不会
            // 被截断——页面内部已经自带 ScrollArea 的（Settings/WiFi/Voice/
            // Lighting/Log）会被两层滚动自然组合；其它面板的内容超出可视区
            // 时直接滚动显示。
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let page = *self.handle.page.lock().unwrap();
                    match page {
                        Page::Connect => {
                            panel_connection::show(&self.handle, ui, &mut self.connect_st)
                        }
                        Page::Settings => {
                            panel_settings::show(&self.handle, ui, &mut self.settings_st)
                        }
                        Page::Keymap => panel_keymap::show(&self.handle, ui, &mut self.keymap_st),
                        Page::Lighting => {
                            panel_lighting::show(&self.handle, ui, &mut self.lighting_st)
                        }
                        Page::Wifi => panel_wifi::show(&self.handle, ui, &mut self.wifi_st),
                        Page::Voice => panel_voice::show(&self.handle, ui, &mut self.voice_st),
                        Page::Log => panel_log::show(&self.handle, ui, &mut self.log_st),
                        Page::About => panel_about::show(&self.handle, ui),
                    }
                });
        });

        if self.confirm_open {
            let outcome = show_confirm(
                ctx,
                &self.confirm_title,
                &self.confirm_body,
                &mut self.confirm_open,
            );
            match outcome {
                ConfirmOutcome::Yes => {
                    if let Some(k) = self.confirm_kind.take() {
                        match k {
                            UiConfirmKind::SwitchWorkMode => {
                                self.settings_st.pending_confirm = Some(k);
                            }
                        }
                    }
                    self.confirm_open = false;
                }
                ConfirmOutcome::No => {
                    self.confirm_kind = None;
                    self.confirm_open = false;
                }
                ConfirmOutcome::None => {}
            }
        }

        if self.local_settings_open {
            let close = show_local_settings(ctx, &self.handle, &mut self.local_settings_open);
            if close == Some(false) {
                self.local_settings_open = false;
            }
        }

        show_toasts(ctx, &mut self.toasts);
        let size = ctx.input(|i| i.screen_rect().size());
        self.last_inner_size = [size.x, size.y];
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
