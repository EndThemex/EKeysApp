//! proto 区域翻译表：`src/protocol.rs` 中给 UI 展示用的文案
//! （固件功能友好名 / 动作标签 / 音频内容校验错误）。
//!
//! 保留前缀：`proto.*`。

use super::Entry;

pub(super) const ENTRIES: &[Entry] = &[
    // ---- 固件功能串 → 友好名（function_label）----
    (
        "proto.function.asr",
        "语音识别 (ASR)",
        "Voice Recognition (ASR)",
    ),
    (
        "proto.function.page_pc",
        "页面跳转: PC状态",
        "Page: PC Status",
    ),
    (
        "proto.function.page_music",
        "页面跳转: 音乐播放",
        "Page: Music Player",
    ),
    (
        "proto.function.page_audio",
        "页面跳转: 音效板",
        "Page: Sound Pad",
    ),
    (
        "proto.function.page_keymap",
        "页面跳转: 键映射",
        "Page: Keymap",
    ),
    (
        "proto.function.page_setting",
        "页面跳转: 设置",
        "Page: Settings",
    ),
    (
        "proto.function.page_ha",
        "页面跳转: 智能家居",
        "Page: Smart Home",
    ),
    (
        "proto.function.media_play",
        "媒体: 播放/暂停",
        "Media: Play/Pause",
    ),
    (
        "proto.function.media_next",
        "媒体: 下一曲",
        "Media: Next Track",
    ),
    (
        "proto.function.media_prev",
        "媒体: 上一曲",
        "Media: Previous Track",
    ),
    (
        "proto.function.media_vol_up",
        "媒体: 音量+",
        "Media: Volume Up",
    ),
    (
        "proto.function.media_vol_down",
        "媒体: 音量-",
        "Media: Volume Down",
    ),
    ("proto.function.media_mute", "媒体: 静音", "Media: Mute"),
    (
        "proto.function.mouse_left",
        "鼠标: 左键",
        "Mouse: Left Click",
    ),
    (
        "proto.function.mouse_right",
        "鼠标: 右键",
        "Mouse: Right Click",
    ),
    (
        "proto.function.mouse_middle",
        "鼠标: 中键",
        "Mouse: Middle Click",
    ),
    (
        "proto.function.mouse_scroll_up",
        "鼠标: 滚轮上",
        "Mouse: Scroll Up",
    ),
    (
        "proto.function.mouse_scroll_down",
        "鼠标: 滚轮下",
        "Mouse: Scroll Down",
    ),
    // ---- 动作标签（KeyAction::label）----
    ("proto.action.none", "未绑定", "Unbound"),
    ("proto.action.text", "文本:{text}", "Text: {text}"),
    // ---- 音频内容校验错误（validate_audio_content → Toast）----
    (
        "proto.audio.container_mp4",
        "MP4/M4A 容器（可能是改了扩展名的视频/音频文件）",
        "MP4/M4A container (possibly a renamed video/audio file)",
    ),
    ("proto.audio.container_riff", "RIFF（WAV）", "RIFF (WAV)"),
    (
        "proto.audio.container_unknown",
        "无法识别的格式",
        "unrecognized format",
    ),
    (
        "proto.audio.err_not_wav",
        "文件内容不是 WAV（实际为{actual}）。设备按扩展名选解码器，伪装文件会被播放成噪声。请转码后上传：ffmpeg -i 输入 -vn -acodec pcm_s16le 输出.wav",
        "File content is not WAV (actually {actual}). The device picks a decoder by file extension, so disguised files play as noise. Please transcode and re-upload: ffmpeg -i input -vn -acodec pcm_s16le output.wav",
    ),
    (
        "proto.audio.err_no_fmt",
        "WAV 缺少 fmt 块（文件头损坏），设备无法解码",
        "WAV is missing the fmt chunk (corrupted header); the device cannot decode it",
    ),
    ("proto.audio.codec_non_pcm", "非 PCM 编码", "Non-PCM codec"),
    (
        "proto.audio.err_codec",
        "WAV 编码为格式码 {code}（{codec_name}），设备仅支持 PCM（格式码 1）。请转码：ffmpeg -i 输入 -acodec pcm_s16le 输出.wav",
        "WAV encoding is format code {code} ({codec_name}); the device only supports PCM (format code 1). Transcode: ffmpeg -i input -acodec pcm_s16le output.wav",
    ),
    (
        "proto.audio.err_bits",
        "WAV 位深 {bits}bit，设备仅支持 16bit PCM。请转码：ffmpeg -i 输入 -acodec pcm_s16le 输出.wav",
        "WAV bit depth is {bits}bit; the device only supports 16bit PCM. Transcode: ffmpeg -i input -acodec pcm_s16le output.wav",
    ),
    (
        "proto.audio.err_channels",
        "WAV 声道数 {ch}，设备仅支持单声道/立体声",
        "WAV has {ch} channels; the device only supports mono/stereo",
    ),
    (
        "proto.audio.err_not_mp3",
        "文件内容不是 MP3（实际为{actual}）。请转码后上传：ffmpeg -i 输入 -acodec libmp3lame 输出.mp3",
        "File content is not MP3 (actually {actual}). Please transcode and re-upload: ffmpeg -i input -acodec libmp3lame output.mp3",
    ),
];
