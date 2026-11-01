//! 背单词模式：词书、复习排程与复习进度。
//!
//! 复习是用户自己打开的，不在打字时发生。宿主可以在候选窗以外被动地显示今日待复习的张数（菜单栏、托盘菜单这类用户自己点开才看得到的地方，经 [`session::due_count`]），但组字期间从不显示，也不在候选窗里出现。这里的东西都碰不到 Engine 和组字状态机：宿主显示卡片、报告用户怎么答、读回进度行和这个计数要的数字，就是全部接口，而且有意只到这里为止：候选排序和学习归 C++ Engine，Rust 里再长出一个和它悄悄意见不合的排程器，正是这条边界要防的。决定和理由见 `.agents/notes/implemented/feature/2026-11-01-vocabulary-due-count.md`。
//!
//! The split follows where the data comes from and how it fails, the same rule
//! [`crate::dictionary`] follows:
//!
//! - [`wordbook`] is the word list itself — bundled or imported, read-only once loaded.
//! - [`import`] turns a user's CSV/TXT file into wordbook entries, reporting bad rows per line
//!   without ever echoing them.
//! - [`schedule`] is the spaced-repetition arithmetic, pure and date-driven.
//! - [`progress`] is the per-user review state on disk, which is the only part that is written.
//!
//! Per-card state is deliberately NOT a preference. The shared preference document is capped at
//! 16 KiB by the iOS bridge, and a few thousand studied cards are far past that; the progress
//! store is its own file beside `typing-statistics.json` in the host-supplied directory.

pub mod builtin;
pub mod import;
pub mod library;
pub mod personal;
pub mod progress;
pub mod read_aloud;
pub mod schedule;
pub mod session;
pub mod wordbook;
