//! 学习密钥管理复用账号认证与持久存储；登录编号不含凭据，不能跨重新登录复用。

use super::*;
use msime_client_core::cloud::learning_e2e::Operation;
use msime_client_services::account::{AccountApi, AccountLoginBinding, AccountSessionStorage};

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Request {
    /// 只读本机登录；没有学习授权也不会登记设备或创建密钥。
    LearningBinding {},
    /// 宿主显式操作，不启动学习数据上传或自动调度。
    LearningOperation {
        login: AccountLoginBinding,
        device_name: String,
        #[serde(deserialize_with = "strict_operation")]
        request: Operation,
    },
}

/// 驱动兼容自己的旧请求；此新增边界要求嵌套请求字段也精确匹配，不能漏掉确认字段。
fn strict_operation<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Operation, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let operation: Operation =
        serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;
    if serde_json::to_value(&operation).map_err(serde::de::Error::custom)? != value {
        return Err(serde::de::Error::custom("invalid learning operation"));
    }
    Ok(operation)
}

pub(super) fn run<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    request: Request,
) -> Result<Value, Failure> {
    match request {
        Request::LearningBinding {} => session
            .login_binding()
            .map(|login| json!({ "login": login }))
            .map_err(|error| error.code().to_owned().into()),
        Request::LearningOperation {
            login,
            device_name,
            request,
        } => session
            .learning_operation(request, &device_name, Some(&login))
            .map_err(|error| error.code().to_owned().into()),
    }
}

#[cfg(test)]
mod tests;
