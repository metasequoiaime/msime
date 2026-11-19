//! 桌面清除先绑定本机登录；账号读取或屏障失败不能降级成无屏障清除。

use crate::learning_context::Context;
use msime_client_services::account::learning_sync::round::RoundError;
use msime_client_services::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use serde_json::Value;

/// 使用设置宿主已持有的账号会话，不另建身份或发起网络请求。
/// 非重置动作与明确未登录时继续使用原词库入口和原返回值。
pub fn request_with_account<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    options: &Value,
    action: &Value,
    announce: impl FnMut(),
) -> Result<Value, &'static str> {
    if action.get("operation").and_then(Value::as_str) != Some("reset") {
        return super::request(options, action, announce);
    }
    // 重置动作只接受操作名，避免把其他动作的字段带入清除边界。
    if action.as_object().is_none_or(|object| object.len() != 1) {
        return Err("dictionary_reset_rejected");
    }
    let login = session
        .login_binding()
        .map_err(|_| "dictionary_reset_rejected")?;
    let Some(login) = login else {
        return super::request(options, action, announce);
    };
    let context = Context::parse(options.clone()).map_err(|_| "dictionary_reset_rejected")?;
    let reset = || {
        session
            .with_learning_reset(&login, &context.borrowed(), || context.reset())
            .map_err(|error| match error {
                RoundError::Busy => "dictionary maintenance busy".to_owned(),
                _ => "learned-data reset rejected".to_owned(),
            })
    };
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    let result = {
        let mut hosts = msime_client_services::dictionary::quiesce::QuiescedHosts::new(
            options["user_data"].as_str(),
            announce,
        );
        hosts.run(reset)
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let result = {
        let _ = announce;
        reset()
    };
    result.map_err(|reason| super::error_code(&reason))
}
