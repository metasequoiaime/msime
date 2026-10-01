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
use msime_engine::host::{
    local_mode_counts_as_typing, CandidateEdge, Command, CommandTableEntry,
    CommandTranslationQuery, EngineResult, EngineSnapshot, MentionEntry, OnlineQuerySnapshot,
    Session,
};
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
mod providers;
mod runtime;
mod types;

pub use engine::*;
pub use providers::*;
pub use runtime::*;
pub use types::*;

#[cfg(test)]
mod tests;
