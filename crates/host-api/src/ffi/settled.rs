//! 在会话线程外运行停顿模型。
//!
//! 桌面 24M 模型的实测 p95 为 153ms；macOS 按键和停顿定时器共用主线程，
//! 同步评分会让随后到达的空格等按键排队。工作线程独占模型和候选副本，
//! 不调用 `with_session`。

use crate::*;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};

struct SettledJob {
    session: u64,
    inner: Mutex<SettledJobInner>,
}

struct SettledJobInner {
    abandoned: bool,
    phase: SettledPhase,
}

enum SettledPhase {
    Running,
    Ready(Box<msime_input_runtime::SettledRerankScore>),
    Done,
}

struct Registry {
    jobs: HashMap<u64, Arc<SettledJob>>,
    by_session: HashMap<u64, u64>,
}

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY
        .get_or_init(|| {
            Mutex::new(Registry {
                jobs: HashMap::new(),
                by_session: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn job_inner(job: &SettledJob) -> std::sync::MutexGuard<'_, SettledJobInner> {
    job.inner.lock().unwrap_or_else(|error| error.into_inner())
}

/// 会话销毁时放弃在途评分；工作线程完成后发现 `abandoned` 就释放模型，不再发布。
pub(crate) fn abandon_settled_job(handle: u64) {
    let job = {
        let mut registry = registry();
        let Some(token) = registry.by_session.remove(&handle) else {
            return;
        };
        registry.jobs.remove(&token)
    };
    let Some(job) = job else {
        return;
    };
    let mut inner = job_inner(&job);
    inner.abandoned = true;
    if let SettledPhase::Ready(score) = std::mem::replace(&mut inner.phase, SettledPhase::Done) {
        drop(score);
    }
}

fn open_token(handle: u64) -> Option<u64> {
    let mut registry = registry();
    let token = *registry.by_session.get(&handle)?;
    let finished = registry
        .jobs
        .get(&token)
        .is_some_and(|job| matches!(job_inner(job).phase, SettledPhase::Done));
    if finished {
        registry.jobs.remove(&token);
        registry.by_session.remove(&handle);
        return None;
    }
    Some(token)
}

fn remove_job(handle: u64, token: u64) {
    let mut registry = registry();
    registry.jobs.remove(&token);
    if registry.by_session.get(&handle).copied() == Some(token) {
        registry.by_session.remove(&handle);
    }
}

fn publish(
    session: &mut HostSession,
    score: msime_input_runtime::SettledRerankScore,
) -> Result<Value, String> {
    let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
    let applied = session.runtime.apply_settled_rerank(score);
    if applied.moved {
        let view = session.runtime.view();
        session.lifecycle_view(pending, &view);
        Ok(json!({"status": "done", "moved": true, "current": true, "view": view}))
    } else {
        Ok(json!({"status": "done", "moved": false, "current": applied.current}))
    }
}

fn next_token() -> u64 {
    loop {
        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        if token != 0 {
            return token;
        }
    }
}

fn spawn_score(
    handle: u64,
    request: msime_input_runtime::SettledRerankRequest,
) -> Result<u64, Box<msime_input_runtime::SettledRerankRequest>> {
    let token = next_token();
    let job = Arc::new(SettledJob {
        session: handle,
        inner: Mutex::new(SettledJobInner {
            abandoned: false,
            phase: SettledPhase::Running,
        }),
    });
    let (sender, receiver) = mpsc::channel();
    let worker = Arc::clone(&job);
    let spawned = std::thread::Builder::new()
        .name("msime-settled-rerank".into())
        .spawn(move || {
            let Ok(request) = receiver.recv() else {
                return;
            };
            let scored = catch_unwind(AssertUnwindSafe(|| {
                msime_input_runtime::score_settled_rerank(request)
            }));
            let mut inner = job_inner(&worker);
            match scored {
                Ok(score) if !inner.abandoned => inner.phase = SettledPhase::Ready(Box::new(score)),
                Ok(score) => {
                    drop(score);
                    inner.phase = SettledPhase::Done;
                }
                Err(_) => {
                    // 展开栈时已释放请求及其模型。
                    inner.abandoned = true;
                    inner.phase = SettledPhase::Done;
                }
            }
        });
    if spawned.is_err() {
        return Err(Box::new(request));
    }
    if let Err(sent) = sender.send(request) {
        return Err(Box::new(sent.0));
    }
    let mut registry = registry();
    registry.jobs.insert(token, job);
    registry.by_session.insert(handle, token);
    Ok(token)
}

/// 停顿重解每一片最多花的模型时间，见 [`deferred_on_session`]。按键最多等一片：畅享 90 Plus 上逐键模式的整键 p95 是 19.5 ms，一片连同前后两次不带模型的解码不超过它。
const PAUSE_SLICE: std::time::Duration = std::time::Duration::from_millis(8);

/// 跑不进逐键预算、延后到停顿的逐键模型（`Runtime::set_deferred_scorer`）：在会话线程上带着它重解当前组字，一次一片（[`PAUSE_SLICE`]）。
///
/// - 解完了：回 `status: done`，`moved` 表示候选内容是否改变；代次前进时附带新 `view`，文字不变也要更新候选身份。
/// - 这一片没解完：回 `status: continue`，宿主让排队的输入先走（鸿蒙用 `setTimeout(0)`），没有新按键就再调 `msime_client_start_settled_rerank`；有新按键就不再调，按键照常处理，下次停顿接着用已经打过的分。代次前进时同样附带 `view`。
///
/// 没有延后的模型时是 `None`，照常走落定重排。不放到工作线程：重解要用引擎，会话只在它自己的线程上。
fn deferred_on_session(session: &mut HostSession) -> Option<Result<Value, String>> {
    if !session.runtime.has_deferred_scorer() {
        return None;
    }
    let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
    let generation = session.runtime.generation();
    let (status, moved) = match session.runtime.settle_deferred_scorer(PAUSE_SLICE) {
        Ok(msime_input_runtime::DeferredSettle::Done { moved }) => ("done", moved),
        Ok(msime_input_runtime::DeferredSettle::Continue { moved }) => ("continue", moved),
        Err(error) => return Some(Err(error.to_string())),
    };
    // 重解会使旧候选身份过期；文字没换位置时也要把新代次交回宿主。
    Some(if session.runtime.generation() != generation {
        let view = session.runtime.view();
        session.lifecycle_view(pending, &view);
        Ok(json!({"status": status, "moved": moved, "current": true, "view": view}))
    } else {
        Ok(json!({"status": status, "moved": false, "current": true}))
    })
}

fn settled_on_session(session: &mut HostSession) -> Result<Value, String> {
    let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
    let moved = session.runtime.rerank_settled();
    if moved {
        let view = session.runtime.view();
        session.lifecycle_view(pending, &view);
        Ok(json!({"status": "done", "moved": true, "current": true, "view": view}))
    } else {
        Ok(json!({"status": "done", "moved": false, "current": true}))
    }
}

/// 在后台启动停顿重排。挂着延后到停顿的逐键模型时，当场带着它重解当前组字的一片，没解完回 `status: continue`（见 `deferred_on_session`）。无需模型或只做呈现整理时返回 `status: done` 和 `current: true`，
/// 顺序变化时附带 `moved: true` 与 `view`；后台评分返回 `status: pending` 和 `token`。
/// 每会话只允许一个任务，重复启动返回相同的令牌。
#[no_mangle]
pub extern "C" fn msime_client_start_settled_rerank(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            if let Some(settled) = deferred_on_session(session) {
                return settled;
            }
            if let Some(token) = open_token(handle) {
                return Ok(json!({"status": "pending", "token": token}));
            }
            let Some(request) = session.runtime.begin_settled_rerank() else {
                return settled_on_session(session);
            };
            match spawn_score(handle, request) {
                Ok(token) => Ok(json!({"status": "pending", "token": token})),
                Err(request) => {
                    let score = msime_input_runtime::score_settled_rerank(*request);
                    publish(session, score)
                }
            }
        })
    })
}

/// 收集后台评分，未完成时返回 `pending`。宿主已离开原组字时传 `accept: false`，
/// 只归还仍适用的模型，不重排候选。必须在会话线程调用。
#[no_mangle]
pub extern "C" fn msime_client_poll_settled_rerank(
    handle: u64,
    token: u64,
    accept: bool,
) -> *mut c_char {
    response(|| {
        if token == 0 {
            return Ok(json!({"status": "done", "moved": false, "current": false}));
        }
        with_session(handle, |session| {
            let job = registry().jobs.get(&token).cloned();
            let Some(job) = job else {
                return Ok(json!({"status": "done", "moved": false, "current": false}));
            };
            if job.session != handle {
                return Ok(json!({"status": "done", "moved": false, "current": false}));
            }
            let take = {
                let mut inner = job_inner(&job);
                match std::mem::replace(&mut inner.phase, SettledPhase::Done) {
                    SettledPhase::Running => {
                        inner.phase = SettledPhase::Running;
                        Take::Pending
                    }
                    SettledPhase::Ready(score) => Take::Ready(score),
                    SettledPhase::Done => Take::Absent,
                }
            };
            match take {
                Take::Pending => Ok(json!({"status": "pending", "token": token})),
                Take::Absent => {
                    remove_job(handle, token);
                    Ok(json!({"status": "done", "moved": false, "current": false}))
                }
                Take::Ready(score) => {
                    remove_job(handle, token);
                    if !accept {
                        session.runtime.discard_settled_rerank(*score);
                        Ok(json!({"status": "done", "moved": false, "current": false}))
                    } else {
                        publish(session, *score)
                    }
                }
            }
        })
    })
}

/// 当场跑一次停顿重排，等它评完再返回，结果与 `msime_client_start_settled_rerank` 加上随后的 `msime_client_poll_settled_rerank(accept = true)` 相同。宿主在停顿 150 ms 后才这样做，`candidate_preview` 代表的是「打完这串键停下来」，所以直接在会话线程上评分，不起工作线程。
pub(crate) fn settle_now(session: &mut HostSession) -> Result<Value, String> {
    // 每一片都至少跑一次模型、记下它的分，所以一片片做下去一定会解完。
    while let Some(settled) = deferred_on_session(session) {
        match settled {
            Ok(value) if value["status"] == "continue" => {}
            settled => return settled,
        }
    }
    match session.runtime.begin_settled_rerank() {
        Some(request) => publish(session, msime_input_runtime::score_settled_rerank(request)),
        None => settled_on_session(session),
    }
}

enum Take {
    Pending,
    Ready(Box<msime_input_runtime::SettledRerankScore>),
    Absent,
}

#[cfg(test)]
pub(crate) fn test_hold_settled_job(handle: u64) -> u64 {
    let token = next_token();
    let job = Arc::new(SettledJob {
        session: handle,
        inner: Mutex::new(SettledJobInner {
            abandoned: false,
            phase: SettledPhase::Running,
        }),
    });
    let mut registry = registry();
    registry.jobs.insert(token, job);
    registry.by_session.insert(handle, token);
    token
}

#[cfg(test)]
pub(crate) fn test_settled_job_count() -> usize {
    registry().jobs.len()
}
