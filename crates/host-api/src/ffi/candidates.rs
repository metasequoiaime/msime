//! Folding externally produced candidates back into the session.
//!
//! 这些是旁路调用：选择加入生命周期的会话在这里只给核心发视图类事件（组字和候选的显示变化），不投递给脚本，免得把停顿时长和网络时序透露出去。runtime 报错时也照样推导，见 `HostSession::lifecycle_on_error`。
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
        if query.is_null()
            || candidate.is_null()
            || query_length > msime_input_runtime::MAX_ONLINE_QUERY_BYTES
            || candidate_length > 4096
            || source > 1
        {
            return Err(if source > 1 {
                "invalid online candidate source"
            } else {
                "invalid online candidate buffer"
            }
            .into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let candidate =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(candidate, candidate_length) })
                .map_err(|_| "candidate is not UTF-8")?;
        with_session(handle, |session| {
            if source == 0 && !session.cloud_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            if source == 1 && !session.ai_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let applied = session
                .runtime
                .apply_online_candidate_owned(query, candidate, source)
                .map_err(|e| e.to_string());
            let applied = session.lifecycle_on_error(pending, applied)?;
            let view = session.runtime.view();
            session.lifecycle_view(pending, &view);
            Ok(json!({ "applied": applied, "view": view }))
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
        if query.is_null()
            || body.is_null()
            || query_length > msime_input_runtime::MAX_ONLINE_QUERY_BYTES
            || body_length > 262144
        {
            return Err("invalid cloud response buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let current_cloud_query = query.clone();
        let candidate = msime_input_runtime::cloud_candidate_from_response(query, unsafe {
            std::slice::from_raw_parts(body, body_length)
        });
        with_session(handle, |session| {
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let applied = if session.cloud_query_is_current(&current_cloud_query) {
                if let Some(candidate) = candidate {
                    let OnlineCandidate {
                        query,
                        text,
                        source,
                    } = candidate;
                    let applied = session
                        .runtime
                        .apply_online_candidate_owned(query, &text, source)
                        .map_err(|e| e.to_string());
                    session.lifecycle_on_error(pending, applied)?
                } else {
                    false
                }
            } else {
                false
            };
            let view = session.runtime.view();
            session.lifecycle_view(pending, &view);
            Ok(json!({ "applied": applied, "view": view }))
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
            || query_length > msime_input_runtime::MAX_ONLINE_QUERY_BYTES
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
            if source == 0 && !session.cloud_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            if source == 1 && !session.ai_query_is_current(&query) {
                return Ok(json!({"applied":false,"view":session.runtime.view()}));
            }
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let applied = session
                .runtime
                .apply_online_candidates_owned(query, &candidates, source)
                .map_err(|e| e.to_string());
            let applied = session.lifecycle_on_error(pending, applied)?;
            let view = session.runtime.view();
            session.lifecycle_view(pending, &view);
            Ok(json!({ "applied": applied, "view": view }))
        })
    })
}

/// AI 回答为空、失败，或 `msime_client_apply_online_candidates` 返回 `applied:false` 时宿主会调用它。原来用来把 AI 座位上的占位行换成「—」；共享内核不再放占位行（AI 回答到达时第一条直接插在第一页最后一格，#6888），这个入口为已发布的宿主留着：缓冲和来源照旧校验，合法的调用一律回答 `applied:false` 和当前视图，不改候选、不换代。
///
/// # Safety
/// `query` 必须指向 `query_length` 个可读字节；只在本次调用期间读取，不保留。
#[no_mangle]
pub unsafe extern "C" fn msime_client_resolve_online_candidates(
    handle: u64,
    query: *const u8,
    query_length: usize,
    source: u8,
) -> *mut c_char {
    response(|| {
        if query.is_null()
            || query_length > msime_input_runtime::MAX_ONLINE_QUERY_BYTES
            || source > 1
        {
            return Err("invalid online resolve buffer".into());
        }
        // 只校验文档是一份在线查询，内容不再用到。
        serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        with_session(handle, |session| {
            Ok(json!({"applied":false,"view":session.runtime.view()}))
        })
    })
}

/// Remove cached and visible rows for one online source (0 = cloud, 1 = AI).
#[no_mangle]
pub extern "C" fn msime_client_clear_online_candidates(handle: u64, source: u8) -> *mut c_char {
    response(|| {
        if source > 1 {
            return Err("invalid online candidate source".into());
        }
        with_session(handle, |session| {
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let cleared = session
                .runtime
                .clear_online_candidates(source)
                .map_err(|e| e.to_string());
            session.lifecycle_on_error(pending, cleared)?;
            let view = session.runtime.view();
            session.lifecycle_view(pending, &view);
            Ok(json!({ "applied": true, "view": view }))
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
