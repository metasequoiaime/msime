//! The msime-cloud query protocol: one JSON request in, one JSON response out, answered from a resource directory and a per-request scratch directory.
//!
//! msime-cloud runs `msime-backend-engine <resources> <scratch>` once per request and writes the request to its stdin (`internal/engine/client.go` there). This module is that process's behaviour; it replaces the C++ bridge `native/main.cpp` that linked the archived MSIME-Engine, and keeps its request and response shapes so the Go server is unchanged. Errors are the four codes the server maps to HTTP statuses: `invalid_request` (400), `invalid_dictionary_entry` (400), `resources_unavailable` (503) and anything else (502).
//!
//! Paths only ever come from the process arguments, never from the request, and nothing is kept between requests.

mod catalogs;
mod common;
mod dictionary_checks;
mod input;
mod local_modes;
mod personal;
mod request;
mod text_tools;
mod validation;

use std::path::Path;

use serde_json::{json, Value};

use crate::assets;

use common::Roots;
use request::Request;
pub use request::MAXIMUM_REQUEST_BYTES;

/// Why a request produced no answer. `code` is the `error` string the server reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendError {
    /// The request is malformed or out of bounds.
    InvalidRequest,
    /// A personal dictionary entry failed validation.
    InvalidDictionaryEntry,
    /// The resource directory lacks a file the operation reads, or the file cannot be opened.
    ResourcesUnavailable,
    /// The operation failed after its input was accepted.
    EngineFailure,
    UnknownOperation,
}

impl BackendError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidDictionaryEntry => "invalid_dictionary_entry",
            Self::ResourcesUnavailable => "resources_unavailable",
            Self::EngineFailure => "engine_failure",
            Self::UnknownOperation => "unknown_operation",
        }
    }

    pub fn response(self) -> Value {
        json!({ "error": self.code() })
    }
}

impl From<rusqlite::Error> for BackendError {
    fn from(_: rusqlite::Error) -> Self {
        Self::EngineFailure
    }
}

pub(crate) type Outcome = std::result::Result<Value, BackendError>;

/// Turns Simplified Chinese text into Traditional. The conversion tables live in `msime-client-core`, which this crate does not depend on, so the process that hosts the protocol supplies it.
pub type SimplifiedToTraditional = fn(&str) -> String;

/// Answer one request. `resources` holds the pinned dictionary release; `scratch` is a directory the caller created for this request alone and removes afterwards.
pub fn execute(
    request: &Value,
    resources: &Path,
    scratch: &Path,
    simplified_to_traditional: SimplifiedToTraditional,
) -> Value {
    let roots = Roots {
        resources,
        dictionaries: resources,
        scratch,
    };
    let answer = Request::new(request).and_then(|request| {
        if request.operation() == "convert" {
            return text_tools::convert(&request, roots, simplified_to_traditional);
        }
        route(&request, roots)
    });
    match answer {
        Ok(response) => response,
        Err(error) => error.response(),
    }
}

/// Parse the raw request bytes and answer them. Bytes past `MAXIMUM_REQUEST_BYTES` or anything that is not a JSON object is an invalid request, as in the C++ bridge.
pub fn execute_raw(
    raw: &[u8],
    resources: &Path,
    scratch: &Path,
    simplified_to_traditional: SimplifiedToTraditional,
) -> Value {
    if raw.len() > MAXIMUM_REQUEST_BYTES {
        return BackendError::InvalidRequest.response();
    }
    match serde_json::from_slice::<Value>(raw) {
        Ok(request) if request.is_object() => {
            execute(&request, resources, scratch, simplified_to_traditional)
        }
        _ => BackendError::InvalidRequest.response(),
    }
}

/// Answer one parsed request against `roots`. A personal query calls this again with `dictionaries` pointing at the user's replayed generation.
fn route(request: &Request, roots: Roots) -> Outcome {
    match request.operation() {
        "unicode" => local_modes::unicode(request),
        "datetime" => local_modes::date_time(request),
        "romaji" => local_modes::romaji(request),
        "emoji" | "kaomoji" | "jianpin" | "quick" => local_modes::local_mode(request, roots),
        "english" => local_modes::english(request, roots),
        "gloss" => local_modes::gloss(request, roots),
        "segmentation" => input::segmentation(request),
        "candidates" | "cloud_candidates" => input::candidates_for_input(request, roots),
        "catalog" => catalogs::catalog(request, roots),
        "dictionary" => catalogs::dictionary(request, roots),
        "helpcode" => text_tools::helpcode(request, roots),
        "annotate" => text_tools::annotate(request, roots),
        "annotate_batch" => text_tools::annotate_batch(request, roots),
        "japanese" => text_tools::japanese(request, roots),
        "personal_query" | "personal_rank" | "personal_delete" => {
            personal::personal(request, roots)
        }
        "validate_snapshot" => personal::validate_snapshot(roots),
        "listed_pinyin_batch" => dictionary_checks::listed_pinyin_batch(
            request,
            &roots.dictionary(assets::MAIN_DICTIONARY),
        ),
        "listed_english_batch" => dictionary_checks::listed_english_batch(
            request,
            &roots.dictionary(assets::ENGLISH_DICTIONARY),
        ),
        "pinyin_weight_medians" => {
            dictionary_checks::pinyin_weight_medians(&roots.dictionary(assets::MAIN_DICTIONARY))
        }
        "validate_dictionary" => validation::validate_dictionary(request),
        "validate_dictionary_batch" => validation::validate_dictionary_batch(request),
        _ => Err(BackendError::UnknownOperation),
    }
}

#[cfg(test)]
mod tests;
