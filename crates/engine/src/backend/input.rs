//! Segmentation and candidates for pinyin, shuangpin and wubi input, answered by the engine's schemes and providers directly: the same `QueryRequest` a session builds for that input, without the session's learning, local modes or frequency adjustment.

use std::path::PathBuf;

use serde_json::Value;

use super::common::{candidates, scheme, shuangpin_profile, wubi_profile, Roots};
use super::request::Request;
use super::{BackendError, Outcome};
use crate::ime::registry::ProviderRegistry;
use crate::quanpin::scheme::QuanpinScheme;
use crate::shuangpin::scheme::ShuangpinScheme;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{
    CandidateSource, QueryRequest, SchemeSet, SchemeType, WordItem, WubiProfileKind,
};
use crate::wubi::scheme::WubiScheme;
use crate::{assets, RuntimePaths};

/// The query the scheme builds for `text`; invalid input is an invalid request.
pub(super) fn build_query(
    scheme: SchemeType,
    profile: &'static ShuangpinProfile,
    text: &str,
) -> Result<QueryRequest, BackendError> {
    let query = match scheme {
        SchemeType::Shuangpin => {
            let mut input = ShuangpinScheme::new(profile);
            input.set_raw_input(text, text);
            input.build_request()
        }
        SchemeType::Wubi => {
            let mut input = WubiScheme::new();
            input.set_raw_input(text);
            input.build_request()
        }
        _ => {
            let mut input = QuanpinScheme::new();
            input.set_raw_input(text, text);
            input.build_request()
        }
    };
    if !query.valid {
        return Err(BackendError::InvalidRequest);
    }
    Ok(query)
}

/// `{"raw": "ni'hao", "normalized": "ni'hao"}`. Shuangpin is normalised to quanpin syllables.
pub(super) fn segmentation(request: &Request) -> Outcome {
    let query = build_query(
        scheme(request)?,
        shuangpin_profile(request)?,
        request.text(),
    )?;
    Ok(serde_json::json!({
        "raw": query.raw_segmentation,
        "normalized": query.normalized_segmentation,
    }))
}

/// 服务端接受的三种方案的 provider，读 `roots` 下的词库；五笔读 `wubi` 这一版码表。`scratch` 充当用户目录和缓存目录：provider 写下的东西不会活过这次请求。
pub(super) fn registry(
    roots: Roots,
    profile: &'static ShuangpinProfile,
    wubi: WubiProfileKind,
) -> Result<ProviderRegistry, BackendError> {
    roots.require_resources()?;
    if roots.scratch.as_os_str().is_empty() || !roots.scratch.is_absolute() {
        return Err(BackendError::ResourcesUnavailable);
    }
    if !roots.dictionary(assets::MAIN_DICTIONARY).is_file() {
        return Err(BackendError::ResourcesUnavailable);
    }
    let paths = RuntimePaths {
        resources: roots.resources.to_path_buf(),
        user_data: roots.scratch.to_path_buf(),
        cache: roots.scratch.to_path_buf(),
        dictionaries: roots.dictionaries.to_path_buf(),
    };
    let enabled = SchemeSet::of(&[SchemeType::Quanpin, SchemeType::Shuangpin, SchemeType::Wubi]);
    let mut providers = ProviderRegistry::new(
        enabled,
        profile.kind,
        &paths,
        PathBuf::new(),
        PathBuf::new(),
        PathBuf::new(),
        PathBuf::new(),
    );
    providers.set_wubi_profile(wubi);
    Ok(providers)
}

/// Candidates for the whole input, best first, with the segmentation they were read for. `cloud_candidates` keeps only dictionary rows whose key is the entire normalised input: the cloud contract has no replacement span, so prefixes and generated phrases are never safe there.
pub(super) fn candidates_for_input(request: &Request, roots: Roots) -> Outcome {
    let (items, query) = query_candidates(request, roots)?;
    let mut items = items;
    if request.operation() == "cloud_candidates" {
        items.retain(|item| {
            let key = if item.canonical_pinyin.is_empty() {
                &item.pinyin
            } else {
                &item.canonical_pinyin
            };
            item.source == CandidateSource::Database && *key == query.normalized_segmentation
        });
    }
    items.truncate(request.limit());
    let mut response = candidates(&items);
    response["raw_segmentation"] = Value::from(query.raw_segmentation);
    response["normalized_segmentation"] = Value::from(query.normalized_segmentation);
    Ok(response)
}

/// Every candidate the providers give for the request's input, unlimited, and the query they answered.
pub(super) fn query_candidates(
    request: &Request,
    roots: Roots,
) -> Result<(Vec<WordItem>, QueryRequest), BackendError> {
    let scheme = scheme(request)?;
    let profile = shuangpin_profile(request)?;
    let query = build_query(scheme, profile, request.text())?;
    let mut providers = registry(roots, profile, wubi_profile(request)?)?;
    let items = providers.query(&query);
    Ok((items, query))
}
