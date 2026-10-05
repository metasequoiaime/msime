//! Pieces several operations share: the scheme and shuangpin profile a request names, the roots it reads from, and the candidate list shape.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::request::Request;
use super::BackendError;
use crate::shuangpin::profile::profile;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{SchemeType, ShuangpinProfileKind, WordItem};
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

/// `scheme`: `pinyin` (the default), `shuangpin` or `wubi`.
pub(super) fn scheme(request: &Request) -> Result<SchemeType, BackendError> {
    match request.string_or("scheme", "pinyin")? {
        "pinyin" => Ok(SchemeType::Quanpin),
        "shuangpin" => Ok(SchemeType::Shuangpin),
        "wubi" => Ok(SchemeType::Wubi),
        _ => Err(BackendError::InvalidRequest),
    }
}

/// `profile`: a shuangpin layout name. An unknown name falls back to xiaohe, as the C++ `GetShuangpinProfile` did.
pub(super) fn shuangpin_profile(
    request: &Request,
) -> Result<&'static ShuangpinProfile, BackendError> {
    let name = request.string_or("profile", "xiaohe")?;
    let kind = ShuangpinProfileKind::from_name(name).unwrap_or(ShuangpinProfileKind::Xiaohe);
    Ok(profile(kind))
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
