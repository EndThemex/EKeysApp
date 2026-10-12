//! i18n：显式 key 的中英双语翻译表。
//!
//! # 约定
//!
//! - key **全小写、点分命名**：`<区域>.<元素>`（如 `nav.keymap`、`settings.tab.display`）。
//! - 文案中的动态值用 `{name}` 占位，调用处
//!   `i18n::t("keymap.export.done").replace("{path}", &path)`。
//! - **每个区域一个表文件，只增自己区域的条目**（前缀见各表文件头注释），
//!   避免并行改动冲突；key 在不同表之间必须唯一，重复会在初始化时 panic。
//! - 缺失 key：debug 下 panic（尽早暴露），release 下回退 key 本身。
//!
//! # 语言状态
//!
//! 全局 `AtomicU8`，启动时 `main.rs` 从 `LocalConfig` 注入，语言下拉框变更时
//! 立即写入；UI 每帧重建，下一帧即为新语言。

mod chrome;
mod dialogs;
mod keymap;
mod media;
mod misc;
mod proto;
mod settings;
mod state;

use crate::config::Language;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// (key, 中文, English)
pub type Entry = (&'static str, &'static str, &'static str);

const LANG_ZH: u8 = 0;
const LANG_EN: u8 = 1;

static LANG: AtomicU8 = AtomicU8::new(LANG_ZH);

/// 切换界面语言（本地设置弹窗 / 启动加载时调用）。
pub fn set_language(lang: Language) {
    let v = match lang {
        Language::Chinese => LANG_ZH,
        Language::English => LANG_EN,
    };
    LANG.store(v, Ordering::Relaxed);
}

/// 合并各区域表并按 key 升序排序（排序保证 `t` 走二分查找）。
///
/// 排序时顺带做重复 key 检测：重复即 panic——同一个 key 出现两处会让
/// 翻译漂移，必须合并成一个。
fn entries() -> &'static [Entry] {
    static ALL: OnceLock<Vec<Entry>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v: Vec<Entry> = chrome::ENTRIES
            .iter()
            .chain(dialogs::ENTRIES)
            .chain(keymap::ENTRIES)
            .chain(media::ENTRIES)
            .chain(misc::ENTRIES)
            .chain(proto::ENTRIES)
            .chain(settings::ENTRIES)
            .chain(state::ENTRIES)
            .copied()
            .collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        for w in v.windows(2) {
            assert!(w[0].0 != w[1].0, "i18n key 重复：`{}`", w[0].0);
        }
        v
    })
}

/// 按指定语言查文案；key 不存在返回 `None`。
///
/// 测试走这个函数，避免读写全局 `LANG` 与其它用例竞态。
fn lookup(key: &str, english: bool) -> Option<&'static str> {
    let list = entries();
    let i = list.binary_search_by(|e| e.0.cmp(key)).ok()?;
    let (_, zh, en) = list[i];
    Some(if english { en } else { zh })
}

/// 取当前语言下的文案。
///
/// 英文缺失时回退中文（英文界面不会出现裸 key）；
/// key 本身不存在则是 bug，debug 下 panic。
pub fn t(key: &'static str) -> &'static str {
    let english = LANG.load(Ordering::Relaxed) == LANG_EN;
    if let Some(s) = lookup(key, english) {
        return s;
    }
    debug_assert!(false, "i18n 缺失 key：`{key}`");
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_non_empty() {
        // 重复 key 会在 entries() 初始化时 assert，这里确认能正常初始化。
        let list = entries();
        assert!(!list.is_empty());
        assert!(list.windows(2).all(|w| w[0].0 < w[1].0), "表未排序");
    }

    #[test]
    fn entries_have_non_empty_bodies_and_valid_keys() {
        for (k, zh, en) in entries() {
            assert!(!zh.trim().is_empty(), "key `{k}` 中文为空");
            assert!(!en.trim().is_empty(), "key `{k}` 英文为空");
            assert!(
                k.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_'),
                "key `{k}` 只允许小写字母 / 数字 / `.` / `_`"
            );
        }
    }

    #[test]
    fn placeholders_match_across_languages() {
        // 中英两侧的 `{占位符}` 集合必须一致，否则调用处 replace 会漏替换。
        fn ph(s: &str) -> Vec<&str> {
            let mut out = Vec::new();
            let mut rest = s;
            while let Some(a) = rest.find('{') {
                let Some(b) = rest[a..].find('}') else { break };
                out.push(&rest[a + 1..a + b]);
                rest = &rest[a + b + 1..];
            }
            out.sort_unstable();
            out
        }
        for (k, zh, en) in entries() {
            assert_eq!(ph(zh), ph(en), "key `{k}` 中英占位符不一致");
        }
    }

    #[test]
    fn lookup_returns_both_languages() {
        let list = entries();
        for (k, zh, en) in list {
            assert_eq!(lookup(k, false), Some(*zh), "key `{k}` 中文查不到");
            assert_eq!(lookup(k, true), Some(*en), "key `{k}` 英文查不到");
        }
        assert_eq!(lookup("no.such.key", true), None);
    }
}
