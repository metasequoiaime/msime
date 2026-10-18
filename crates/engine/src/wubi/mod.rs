//! Wubi 86 (schemes-lang.md §3, overlays.md §3.1-§3.3): the scheme's key handling and the `wubi86` table provider. The mixed-pinyin fallback merge lives in `ime`, and the per-row producer routing of learning and removal lives in `session`.

pub mod code;
pub mod lookup;
pub mod provider;
pub mod scheme;
