//! Shared host orchestration; the Engine remains the owner of composition state.
//! Views are cached values. UI selection carries both session and view identity.

/// Character width conversion used by host-specific mode selectors.
/// Converts printable ASCII to Unicode fullwidth forms and back.
pub mod character_width {
    pub fn to_fullwidth(input: &str) -> String {
        input
            .chars()
            .map(|c| {
                if c == ' ' {
                    '\u{3000}'
                } else if ('!'..='~').contains(&c) {
                    char::from_u32(c as u32 + 0xfee0).unwrap()
                } else {
                    c
                }
            })
            .collect()
    }
    pub fn to_halfwidth(input: &str) -> String {
        input
            .chars()
            .map(|c| {
                if c == '\u{3000}' {
                    ' '
                } else if ('！'..='～').contains(&c) {
                    char::from_u32(c as u32 - 0xfee0).unwrap()
                } else {
                    c
                }
            })
            .collect()
    }
}

pub use chinese_ime_lm::{CandidateFacts, Reranker, SentenceModel, DICTIONARY_SOURCES};
use msime_client_core::preferences::TouchKeyboardLayout;
/// 每页候选数的上限，与共享偏好的校验一致；宿主和网页引擎从这里取，不各写一份。
pub use msime_client_core::preferences::MAX_CANDIDATE_PAGE_SIZE;
/// View 里报告的已解析标点样式，类型与 client-core 的解析函数同源。
pub use msime_client_core::punctuation::{PunctuationMarks, QuoteMarks};
use msime_engine::host::{
    local_mode_counts_as_typing, Command, EngineResult, EngineSnapshot, OnlineQuerySnapshot,
    Session,
};
pub use msime_engine::ordering::CandidateScorer;
use msime_engine::{
    CandidateEdge, CommandTableEntry, CommandTranslationQuery, MentionEntry, QuickPhraseEntry,
    SharedKeymap,
};
pub use msime_engine::{GlideKeyboard, GlidePoint, KeyGrid};
/// 脚本插件的 `/` 指令：宿主把会运行的插件声明的指令按执行顺序经 [`Runtime::set_plugin_commands`] 交给引擎，由引擎决定哪些生效。
pub use msime_engine::{PluginCommand, PluginCommandQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
// The Unix socket providers below are the only consumers of these imports.
#[cfg(unix)]
use serde_json::{json, Value};
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Windows cloud-candidate settle delay, matching the native Server behavior.
pub const WINDOWS_CLOUD_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(500);

fn needs_dangling_segment_delimiter_backspace(raw: &str, caret: usize) -> bool {
    let bytes = raw.as_bytes();
    if caret > bytes.len() {
        return false;
    }
    if caret < bytes.len() && bytes[caret] == b'\'' {
        return caret > 0 && bytes[caret - 1] == b'\'';
    }
    caret == bytes.len() && caret > 0 && bytes[caret - 1] == b'\''
}

// Everything is re-exported: the crate has always presented one flat surface to
// host-api and the desktop shell, and this split is for readers of the source.
mod engine;
mod multilingual;
mod providers;
mod runtime;
mod transform;
mod types;

pub use engine::*;
pub use providers::*;
pub use runtime::*;
// 候选变换只在桌面有公开的东西，移动端的 `transform` 只有几个 crate 内的空方法。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub use transform::*;
pub use types::*;

#[cfg(test)]
mod tests;
