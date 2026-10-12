//! P7 Audio 页面：音效板（Sound Pad）——文件管理 + 11 键绑定。
//!
//! 数据全部来自 `handle.audio`（`AudioPadData`）：连接后 `auto_get` 拉取、
//! 进入页面边沿补拉、上传线程写进度。单键 set / 试播 / 删除走同步请求
//! （与 panel_settings 图标上传同风格，最长 1s 卡顿可接受）；
//! 大文件上传走后台线程（`start_audio_upload`），进度条每帧刷新。

use eframe::egui;

use crate::i18n;
use crate::protocol::{
    AUDIO_FILE_MAX_BYTES, sanitize_audio_name, valid_audio_name, validate_audio_content,
};
use crate::state::{AppHandle, ToastKind, UiEvent};

/// 固件侧 `begin` 要求的剩余空间 headroom（cmd_audio.cpp kFreeHeadroomBytes）。
const FREE_HEADROOM_BYTES: u32 = 64 * 1024;

#[derive(Default)]
pub struct AudioPanelState {
    /// 上传：本地音频文件路径（panel_settings 图标上传同款交互，无原生对话框）
    pub upload_path: String,
    /// 上传：设备端目标文件名（空 = 由本地文件名自动生成）
    pub upload_name: String,
}

fn toast(handle: &AppHandle, kind: ToastKind, text: impl Into<String>) {
    let _ = handle.ui_tx.send(UiEvent::Toast(kind, text.into()));
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut AudioPanelState) {
    ui.horizontal(|ui| {
        ui.heading(i18n::t("audio.title"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(i18n::t("audio.stop")).clicked() {
                if let Err(e) = handle.audio_stop() {
                    toast(
                        handle,
                        ToastKind::Error,
                        i18n::t("audio.stop_fail").replace("{e}", &e.to_string()),
                    );
                }
            }
            if ui.button(i18n::t("audio.refresh")).clicked() {
                if let Err(e) = handle
                    .refresh_audio_files()
                    .and_then(|_| handle.refresh_audio_pads())
                {
                    toast(
                        handle,
                        ToastKind::Error,
                        i18n::t("audio.refresh_fail").replace("{e}", &e.to_string()),
                    );
                }
            }
        });
    });
    ui.label(
        egui::RichText::new(i18n::t("audio.subtitle"))
            .weak()
            .small(),
    );
    ui.add_space(4.0);

    storage_card(handle, ui);
    ui.add_space(6.0);
    upload_card(handle, ui, st);
    ui.add_space(6.0);
    files_card(handle, ui);
    ui.add_space(6.0);
    pads_card(handle, ui);
}

/// 存储占用卡片：used / total 进度条 + 剩余空间。
fn storage_card(handle: &AppHandle, ui: &mut egui::Ui) {
    let (total, used, free) = {
        let a = handle.audio.lock().unwrap();
        (a.total_bytes, a.used_bytes, a.free_bytes)
    };
    ui.group(|ui| {
        ui.label(i18n::t("audio.storage_title"));
        if total == 0 {
            ui.label(egui::RichText::new(i18n::t("audio.storage_empty")).weak());
            return;
        }
        let frac = if total > 0 {
            used as f32 / total as f32
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(frac).show_percentage().text(
                i18n::t("audio.storage_usage")
                    .replace("{used}", &(used / 1024).to_string())
                    .replace("{total}", &(total / 1024).to_string())
                    .replace("{free}", &(free / 1024).to_string()),
            ),
        );
    });
}

/// 上传卡片：本地路径 + 设备端文件名 + 进度条（后台线程）+ 取消。
fn upload_card(handle: &AppHandle, ui: &mut egui::Ui, st: &mut AudioPanelState) {
    let upload = handle.audio.lock().unwrap().upload.clone();
    ui.group(|ui| {
        ui.label(i18n::t("audio.upload_title"));
        match upload {
            Some(u) => {
                // 进行中：进度条 + 取消。finished 由 tick_upload 统一收尾。
                let frac = if u.total > 0 {
                    u.sent as f32 / u.total as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(frac).show_percentage().text(
                        i18n::t("audio.upload_progress")
                            .replace("{name}", &u.name)
                            .replace("{sent}", &(u.sent / 1024).to_string())
                            .replace("{total}", &(u.total / 1024).to_string()),
                    ),
                );
                if ui.button(i18n::t("audio.upload_cancel")).clicked() {
                    u.cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                ui.label(
                    egui::RichText::new(i18n::t("audio.upload_hint"))
                        .weak()
                        .small(),
                );
            }
            None => {
                egui::Grid::new("audio-upload-grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .min_col_width(64.0)
                    .show(ui, |ui| {
                        ui.label(i18n::t("audio.upload_local_file"));
                        ui.horizontal(|ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut st.upload_path)
                                    .desired_width(400.0),
                            );
                            // 输入框为空时显示一行提示文字（不占位符）
                            if st.upload_path.trim().is_empty() && !resp.has_focus() {
                                ui.painter().text(
                                    resp.rect.left_center() + egui::vec2(6.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    i18n::t("audio.upload_path_hint"),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    ui.visuals().weak_text_color(),
                                );
                            }
                            if ui.button(i18n::t("audio.upload_browse")).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(i18n::t("audio.upload_filter"), &["mp3", "wav"])
                                    .pick_file()
                                {
                                    st.upload_path = path.display().to_string();
                                }
                            }
                        });
                        ui.end_row();

                        ui.label(i18n::t("audio.upload_device_name"));
                        ui.horizontal(|ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut st.upload_name)
                                    .desired_width(400.0),
                            );
                            if st.upload_name.trim().is_empty() && !resp.has_focus() {
                                ui.painter().text(
                                    resp.rect.left_center() + egui::vec2(6.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    i18n::t("audio.upload_name_hint"),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    ui.visuals().weak_text_color(),
                                );
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("audio.upload_start")).clicked() {
                        start_upload(handle, st);
                    }
                });
                ui.label(
                    egui::RichText::new(
                        i18n::t("audio.upload_limit")
                            .replace("{max}", &(AUDIO_FILE_MAX_BYTES / 1024).to_string()),
                    )
                    .weak()
                    .small(),
                );
            }
        }
    });
}

/// 「开始上传」：读文件 → 生成/校验设备端名 → 本地预检 → 交后台线程。
fn start_upload(handle: &AppHandle, st: &mut AudioPanelState) {
    let path = st.upload_path.trim().to_string();
    if path.is_empty() {
        toast(
            handle,
            ToastKind::Warning,
            i18n::t("audio.upload_need_path"),
        );
        return;
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            toast(
                handle,
                ToastKind::Error,
                i18n::t("audio.upload_read_fail").replace("{e}", &e.to_string()),
            );
            return;
        }
    };
    if bytes.is_empty() {
        toast(handle, ToastKind::Error, i18n::t("audio.upload_empty_file"));
        return;
    }
    if bytes.len() as u32 > AUDIO_FILE_MAX_BYTES {
        toast(
            handle,
            ToastKind::Error,
            i18n::t("audio.upload_too_large")
                .replace("{size}", &(bytes.len() / 1024).to_string())
                .replace("{max}", &(AUDIO_FILE_MAX_BYTES / 1024).to_string()),
        );
        return;
    }
    // 内容预检：拦截伪装扩展名（如 .wav 实为 MP4）与设备不支持的编码
    let local_ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if let Err(e) = validate_audio_content(&local_ext, &bytes) {
        toast(handle, ToastKind::Error, e);
        return;
    }
    // 设备端文件名：手动填的优先，否则由本地文件名自动生成
    let device_name = match st.upload_name.trim() {
        "" => {
            let local = path.rsplit(['\\', '/']).next().unwrap_or(&path);
            match sanitize_audio_name(local) {
                Some(n) => n,
                None => {
                    toast(
                        handle,
                        ToastKind::Error,
                        i18n::t("audio.upload_name_gen_fail").replace("{name}", local),
                    );
                    return;
                }
            }
        }
        n => n.to_string(),
    };
    if !valid_audio_name(&device_name) {
        toast(
            handle,
            ToastKind::Error,
            i18n::t("audio.upload_name_invalid").replace("{name}", &device_name),
        );
        return;
    }
    // 本地预检剩余空间（固件 begin 时还会强校验 size + 64KB headroom）
    {
        let a = handle.audio.lock().unwrap();
        if a.total_bytes > 0 && a.free_bytes < bytes.len() as u32 + FREE_HEADROOM_BYTES {
            toast(
                handle,
                ToastKind::Error,
                i18n::t("audio.upload_no_space")
                    .replace("{free}", &(a.free_bytes / 1024).to_string())
                    .replace("{size}", &(bytes.len() / 1024).to_string())
                    .replace("{headroom}", &(FREE_HEADROOM_BYTES / 1024).to_string()),
            );
            return;
        }
    }
    match handle.start_audio_upload(device_name, bytes) {
        Ok(()) => {
            st.upload_path.clear();
            st.upload_name.clear();
        }
        Err(e) => toast(
            handle,
            ToastKind::Error,
            i18n::t("audio.upload_not_started").replace("{e}", &e.to_string()),
        ),
    }
}

/// 文件列表卡片：名称 + 大小 + 试播 + 删除。
fn files_card(handle: &AppHandle, ui: &mut egui::Ui) {
    let files = handle.audio.lock().unwrap().files.clone();
    ui.group(|ui| {
        ui.set_max_width(560.0);
        ui.horizontal(|ui| {
            ui.label(i18n::t("audio.files_title").replace("{count}", &files.len().to_string()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(i18n::t("audio.files_header"))
                        .weak()
                        .small(),
                );
            });
        });
        if files.is_empty() {
            ui.label(egui::RichText::new(i18n::t("audio.files_empty")).weak());
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for f in &files {
                    ui.horizontal(|ui| {
                        ui.label(&f.name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(i18n::t("audio.files_delete")).clicked() {
                                if let Err(e) = handle.delete_audio_file(&f.name) {
                                    toast(
                                        handle,
                                        ToastKind::Error,
                                        i18n::t("audio.files_delete_fail")
                                            .replace("{e}", &e.to_string()),
                                    );
                                } else {
                                    toast(
                                        handle,
                                        ToastKind::Success,
                                        i18n::t("audio.files_deleted").replace("{name}", &f.name),
                                    );
                                }
                            }
                            if ui.small_button(i18n::t("audio.files_preview")).clicked() {
                                if let Err(e) = handle.audio_play_file(&f.name) {
                                    toast(
                                        handle,
                                        ToastKind::Error,
                                        i18n::t("audio.preview_fail")
                                            .replace("{e}", &e.to_string()),
                                    );
                                }
                            }
                            ui.label(
                                egui::RichText::new(format!("{} KB", f.size / 1024))
                                    .weak()
                                    .small(),
                            );
                        });
                    });
                }
            });
    });
}

/// 键位绑定卡片：11 行 ComboBox（未绑定 + 文件列表）+ 试播 + 清除。
fn pads_card(handle: &AppHandle, ui: &mut egui::Ui) {
    let (files, pads) = {
        let a = handle.audio.lock().unwrap();
        (a.files.clone(), a.pads.clone())
    };
    ui.group(|ui| {
        ui.label(i18n::t("audio.pads_title"));
        if files.is_empty() {
            ui.label(egui::RichText::new(i18n::t("audio.pads_no_files")).weak());
        }
        egui::Grid::new("audio-pads-grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .min_col_width(220.0)
            .show(ui, |ui| {
                for (i, bound) in pads.iter().enumerate() {
                    let key = i + 1;
                    pad_row(ui, handle, key, bound, &files);
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
                // 奇数行补一格,避免末尾孤行拉伸
                if pads.len() % 2 == 1 {
                    ui.label("");
                    ui.end_row();
                }
            });
        ui.label(egui::RichText::new(i18n::t("audio.pads_hint")).weak());
    });
}

/// 键位绑定单行：K 编号 + ComboBox + 试播 + 清除。
fn pad_row(
    ui: &mut egui::Ui,
    handle: &AppHandle,
    key: usize,
    bound: &str,
    files: &[crate::protocol::AudioFileInfo],
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("K{key}")).strong().monospace());
        let selected_text = if bound.is_empty() {
            i18n::t("audio.pads_unbound").to_string()
        } else {
            bound.to_string()
        };
        egui::ComboBox::from_id_salt(("audio-pad", key))
            .selected_text(selected_text)
            .width(160.0)
            .show_ui(ui, |ui| {
                // 「未绑定」选项
                if ui
                    .selectable_label(bound.is_empty(), i18n::t("audio.pads_unbound"))
                    .clicked()
                {
                    if !bound.is_empty() {
                        if let Err(e) = handle.set_audio_pad(key as u8, "") {
                            toast(
                                handle,
                                ToastKind::Error,
                                i18n::t("audio.pads_clear_fail").replace("{e}", &e.to_string()),
                            );
                        }
                    }
                }
                for f in files {
                    if ui.selectable_label(*bound == f.name, &f.name).clicked() {
                        if *bound != f.name {
                            if let Err(e) = handle.set_audio_pad(key as u8, &f.name) {
                                toast(
                                    handle,
                                    ToastKind::Error,
                                    i18n::t("audio.pads_bind_fail").replace("{e}", &e.to_string()),
                                );
                            }
                        }
                    }
                }
            });
        if ui.small_button(i18n::t("audio.files_preview")).clicked() {
            if bound.is_empty() {
                toast(
                    handle,
                    ToastKind::Warning,
                    i18n::t("audio.pads_key_unbound").replace("{key}", &key.to_string()),
                );
            } else if let Err(e) = handle.audio_play_key(key as u8) {
                toast(
                    handle,
                    ToastKind::Error,
                    i18n::t("audio.preview_fail").replace("{e}", &e.to_string()),
                );
            }
        }
        if ui
            .add_enabled(
                !bound.is_empty(),
                egui::Button::new(i18n::t("audio.pads_clear")).small(),
            )
            .clicked()
        {
            if let Err(e) = handle.set_audio_pad(key as u8, "") {
                toast(
                    handle,
                    ToastKind::Error,
                    i18n::t("audio.pads_clear_fail").replace("{e}", &e.to_string()),
                );
            }
        }
    });
}

/// 每帧调用（app.rs update）：上传完成收尾 —— Toast + 清理进度。
///
/// 放在 app.rs 而不是 panel 内：上传完成时用户可能停在其它页面，
/// 这里保证 Toast 不丢。文件列表已由上传线程自行刷新。
pub fn tick_upload(handle: &AppHandle) {
    let done = {
        let a = handle.audio.lock().unwrap();
        match a.upload.as_ref() {
            Some(u) if u.finished => Some((u.name.clone(), u.error.clone(), u.total)),
            _ => None,
        }
    };
    if let Some((name, err, total)) = done {
        match err {
            Some(e) if e == "已取消" => {
                toast(
                    handle,
                    ToastKind::Warning,
                    i18n::t("audio.upload_cancelled").replace("{name}", &name),
                );
            }
            Some(e) => {
                toast(
                    handle,
                    ToastKind::Error,
                    i18n::t("audio.upload_fail")
                        .replace("{name}", &name)
                        .replace("{e}", &e),
                );
            }
            None => {
                toast(
                    handle,
                    ToastKind::Success,
                    i18n::t("audio.upload_done")
                        .replace("{name}", &name)
                        .replace("{size}", &(total / 1024).to_string()),
                );
            }
        }
        handle.audio.lock().unwrap().upload = None;
    }
}
