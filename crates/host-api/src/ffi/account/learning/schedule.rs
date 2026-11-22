//! 原生宿主共用学习开关和持久调度策略；平台只负责生命周期与工作线程。

use super::*;
use crate::learning_context::ControlContext;

pub(super) fn status<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    login: &AccountLoginBinding,
    options: Value,
) -> Result<Value, Failure> {
    let context = ControlContext::parse(options)?;
    session
        .learning_schedule_status(login, context.state_directory())
        .map(|status| json!(status))
        .map_err(data::failure)
}

pub(super) fn enable<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    login: &AccountLoginBinding,
    options: Value,
    enabled: bool,
) -> Result<Value, Failure> {
    let context = ControlContext::parse(options)?;
    if enabled {
        context.require_learning()?;
    }
    session
        .learning_set_sync_enabled(login, context.state_directory(), enabled)
        .map(|status| json!(status))
        .map_err(data::failure)
}

pub(super) fn resume<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    login: &AccountLoginBinding,
    options: Value,
) -> Result<Value, Failure> {
    let context = ControlContext::parse(options)?;
    context.require_learning()?;
    session
        .learning_resume_upload(login, context.state_directory())
        .map(|status| json!(status))
        .map_err(data::failure)
}

pub(super) fn sync<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    login: &AccountLoginBinding,
    device_name: &str,
    options: Value,
    automatic: bool,
) -> Result<Value, Failure> {
    let context = data::Context::parse(options)?;
    context.require_learning()?;
    session
        .learning_sync_scheduled(login, device_name, &context.borrowed(), automatic)
        .map(|report| {
            json!({
                "skipped": report.skipped,
                "status": report.status,
                "round": report.round.map(data::round_report),
            })
        })
        .map_err(data::failure)
}

pub(super) fn cancel<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    login: &AccountLoginBinding,
    options: Value,
) -> Result<Value, Failure> {
    let context = ControlContext::parse(options)?;
    session
        .learning_cancel_scheduled(login, context.state_directory())
        .map(|status| json!(status))
        .map_err(data::failure)
}
