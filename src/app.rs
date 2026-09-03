//! eframe::App 实现：装配各面板、drain Link 事件、快捷键、Toast/Confirm。

use eframe::egui;

use crate::link::LinkEvent;
use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, LogKind, Page, ToastKind, UiConfirmKind, UiEvent};
use crate::ui::{
    panel_about, panel_connection, panel_lighting, panel_log, panel_settings, panel_voice,
    panel_wifi, sidenav, statusbar, topbar,
    widgets::{ConfirmOutcome, Toast, push_toast, show_confirm, show_local_settings, show_toasts},
};

pub struct WxiApp {
    pub handle: AppHandle,
    pub connect_st: panel_connection::ConnectPanelState,
    pub settings_st: panel_settings::SettingsPanelState,
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
        // 拿到订阅 Receiver，drain 一批事件（不持锁 recv）
        let mut evs: Vec<LinkEvent> = vec![];
        {
            let slot = self.handle.link_events.lock().unwrap();
            if let Some(rx) = slot.as_ref() {
                while let Ok(e) = rx.try_recv() {
                    evs.push(e);
                }
            }
        }
        for e in evs {
            self.handle_link_event(e);
        }
    }

    fn handle_link_event(&mut self, e: LinkEvent) {
        match e {
            LinkEvent::Frame(f) => {
                if f.is_push() {
                    if let Some(data) = f.data.as_ref() {
                        if let Ok(new_snap) = serde_json::from_value::<DeviceSettings>(data.clone())
                        {
                            // 1. 记录旧快照
                            let old_snap = self.handle.settings.lock().unwrap().clone();
                            // 2. 用推送值刷新 settings
                            *self.handle.settings.lock().unwrap() = new_snap.clone();
                            // 3. 合并 draft（草稿优先）：仅刷新未修改字段
                            let mut draft = self.handle.draft.lock().unwrap();
                            DeviceSettings::merge_push(&new_snap, &old_snap, &mut draft);
                        }
                    }
                    self.handle.log_kind(LogKind::Rx, "PUSH ← 全量快照");
                } else {
                    if f.status() == Some(0) {
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
                self.handle
                    .log_kind(LogKind::App, format!("状态变更: {s:?}"));
            }
            LinkEvent::Error(e) => {
                self.handle
                    .log_kind(LogKind::App, format!("Link 错误: {e}"));
                push_toast(&mut self.toasts, ToastKind::Error, e, 5000);
            }
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
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            let _ = self.handle.ui_tx.send(UiEvent::Navigate(Page::Settings));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)) {
            let _ = self.handle.ui_tx.send(UiEvent::Navigate(Page::Log));
        }
        let nav = ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) {
                Some(Page::Connect)
            } else if i.key_pressed(egui::Key::Num2) {
                Some(Page::Settings)
            } else if i.key_pressed(egui::Key::Num3) {
                Some(Page::Lighting)
            } else if i.key_pressed(egui::Key::Num4) {
                Some(Page::Wifi)
            } else if i.key_pressed(egui::Key::Num5) {
                Some(Page::Voice)
            } else if i.key_pressed(egui::Key::Num6) {
                Some(Page::Log)
            } else if i.key_pressed(egui::Key::Num7) {
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
            let page = *self.handle.page.lock().unwrap();
            match page {
                Page::Connect => panel_connection::show(&self.handle, ui, &mut self.connect_st),
                Page::Settings => panel_settings::show(&self.handle, ui, &mut self.settings_st),
                Page::Lighting => panel_lighting::show(&self.handle, ui, &mut self.lighting_st),
                Page::Wifi => panel_wifi::show(&self.handle, ui, &mut self.wifi_st),
                Page::Voice => panel_voice::show(&self.handle, ui, &mut self.voice_st),
                Page::Log => panel_log::show(&self.handle, ui, &mut self.log_st),
                Page::About => panel_about::show(ui),
            }
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
