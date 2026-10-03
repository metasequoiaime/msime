//! Folding externally produced candidates back into the session.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// Apply a provider result returned for a previously copied OnlineQuery.
/// The query and candidate buffers are UTF-8 and are never retained.
///
/// # Safety
/// The caller must provide readable buffers of the stated lengths, or null pointers only with
/// zero lengths; buffers are read for the duration of this call and never retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_apply_online_candidate(
    handle: u64,
    query: *const u8,
    query_length: usize,
    candidate: *const u8,
    candidate_length: usize,
    source: u8,
) -> *mut c_char {
    response(|| {
        if query.is_null() || candidate.is_null() || query_length > 16384 || candidate_length > 4096
        {
            return Err("invalid online candidate buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let candidate =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(candidate, candidate_length) })
                .map_err(|_| "candidate is not UTF-8")?;
        with_session(handle, |session| {
            if source == 0 && !session.cloud_candidates_enabled() {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            if source == 1 && !session.ai_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            let applied = session
                .runtime
                .apply_online_candidate_owned(query, candidate, source)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "applied": applied, "view": session.runtime.view() }))
        })
    })
}

/// Parse a bounded host-fetched cloud response and apply it to its original query.
/// Invalid/no-result provider documents leave the current view unchanged.
///
/// # Safety
/// Both pointers must reference readable buffers of their stated lengths.
#[no_mangle]
pub unsafe extern "C" fn msime_client_apply_cloud_response(
    handle: u64,
    query: *const u8,
    query_length: usize,
    body: *const u8,
    body_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || body.is_null() || query_length > 16384 || body_length > 262144 {
            return Err("invalid cloud response buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let candidate = msime_input_runtime::cloud_candidate_from_response(query, unsafe {
            std::slice::from_raw_parts(body, body_length)
        });
        with_session(handle, |session| {
            let applied = if session.cloud_candidates_enabled() {
                if let Some(candidate) = candidate {
                    let OnlineCandidate {
                        query,
                        text,
                        source,
                    } = candidate;
                    session
                        .runtime
                        .apply_online_candidate_owned(query, &text, source)
                        .map_err(|e| e.to_string())?
                } else {
                    false
                }
            } else {
                false
            };
            Ok(json!({ "applied": applied, "view": session.runtime.view() }))
        })
    })
}

/// Apply an ordered JSON array of candidate strings for one online source.
///
/// # Safety
/// Both pointers must reference readable buffers of their stated lengths.
#[no_mangle]
pub unsafe extern "C" fn msime_client_apply_online_candidates(
    handle: u64,
    query: *const u8,
    query_length: usize,
    candidates: *const u8,
    candidates_length: usize,
    source: u8,
) -> *mut c_char {
    response(|| {
        if query.is_null()
            || candidates.is_null()
            || query_length > 16384
            || candidates_length > 16384
            || source > 1
        {
            return Err("invalid online candidates buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let candidates = serde_json::from_slice::<Vec<String>>(unsafe {
            std::slice::from_raw_parts(candidates, candidates_length)
        })
        .map_err(|_| "invalid online candidates document")?;
        with_session(handle, |session| {
            if source == 0 && !session.cloud_candidates_enabled() {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            if source == 1 && !session.ai_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            let applied = session
                .runtime
                .apply_online_candidates_owned(query, &candidates, source)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "applied": applied, "view": session.runtime.view() }))
        })
    })
}

/// Select one Han edge through Engine using the displayed candidate identity.
#[no_mangle]
pub extern "C" fn msime_client_select_edge(
    handle: u64,
    generation: u64,
    index: usize,
    edge: u8,
) -> *mut c_char {
    let edge = match edge {
        0 => CandidateEdge::FirstHan,
        1 => CandidateEdge::LastHan,
        _ => return response(|| Err("Invalid candidate edge".into())),
    };
    dispatch(
        handle,
        Action::SelectEdge(
            CandidateId {
                session: handle,
                generation,
                index,
            },
            edge,
        ),
    )
}
