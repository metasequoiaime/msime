//! Pieces several operations share: the scheme and shuangpin profile a request names, the roots it reads from, and the candidate list shape.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::request::Request;
use super::BackendError;
use crate::shuangpin::profile::{default_profile, profile};
use crate::shuangpin::ShuangpinProfile;
use crate::types::{SchemeType, ShuangpinProfileKind, WordItem, WubiProfileKind};
use crate::RuntimePaths;

/// Where an operation reads. `dictionaries` is `resources` except inside a personal query, where it is the generation the user's overlay was replayed into.
#[derive(Clone, Copy)]
pub(super) struct Roots<'a> {
    pub(super) resources: &'a Path,
    pub(super) dictionaries: &'a Path,
    pub(super) scratch: &'a Path,
}

impl Roots<'_> {
    fn paths(&self) -> RuntimePaths {
        RuntimePaths {
            resources: self.resources.to_path_buf(),
            user_data: PathBuf::new(),
            cache: PathBuf::new(),
            dictionaries: self.dictionaries.to_path_buf(),
        }
    }

    /// A read-only file of the shipped release, under its current or legacy name.
    pub(super) fn resource(&self, name: &str) -> PathBuf {
        self.paths().resource(name)
    }

    /// A dictionary the user's overlay can change: the replayed copy inside a personal query, the shipped one otherwise.
    pub(super) fn dictionary(&self, name: &str) -> PathBuf {
        self.paths().dictionary(name)
    }

    /// The resource root must be an absolute path for anything that reads a file.
    pub(super) fn require_resources(&self) -> Result<(), BackendError> {
        if self.resources.as_os_str().is_empty() || !self.resources.is_absolute() {
            return Err(BackendError::ResourcesUnavailable);
        }
        Ok(())
    }
}

/// `scheme`：`pinyin`（缺省）、`shuangpin`、`wubi` 或 `wubi98`。两种五笔都是 `SchemeType::Wubi`，读哪一版码表由 [`wubi_profile`] 决定。
pub(super) fn scheme(request: &Request) -> Result<SchemeType, BackendError> {
    match request.string_or("scheme", "pinyin")? {
        "pinyin" => Ok(SchemeType::Quanpin),
        "shuangpin" => Ok(SchemeType::Shuangpin),
        "wubi" | "wubi98" => Ok(SchemeType::Wubi),
        _ => Err(BackendError::InvalidRequest),
    }
}

/// 五笔码表版本：`scheme` 为 `wubi98` 时是 98 版，其余（含 `wubi`）是 86 版。服务端沿用 `wubi` 指 86 版，所以 98 版另起方案名，而不是复用表示双拼方案的 `profile`。
pub(super) fn wubi_profile(request: &Request) -> Result<WubiProfileKind, BackendError> {
    Ok(match request.string_or("scheme", "pinyin")? {
        "wubi98" => WubiProfileKind::Wubi98,
        _ => WubiProfileKind::Wubi86,
    })
}

/// `profile`：双拼方案名。不认识的名字按小鹤，与 C++ 的 `GetShuangpinProfile` 相同；`custom` 也按小鹤，因为请求只带名字，不带用户的表。
pub(super) fn shuangpin_profile(
    request: &Request,
) -> Result<&'static ShuangpinProfile, BackendError> {
    let name = request.string_or("profile", "xiaohe")?;
    Ok(ShuangpinProfileKind::from_name(name)
        .and_then(profile)
        .unwrap_or_else(default_profile))
}

pub(super) fn candidate(item: &WordItem) -> Value {
    json!({
        "code": item.pinyin,
        "canonical_pinyin": item.canonical_pinyin,
        "word": item.word,
        "weight": item.weight,
        "fixed_position": item.fixed_position,
    })
}

/// `{"candidates":[{code, canonical_pinyin, word, weight, fixed_position}]}`.
pub(super) fn candidates(items: &[WordItem]) -> Value {
    json!({ "candidates": items.iter().map(candidate).collect::<Vec<_>>() })
}
