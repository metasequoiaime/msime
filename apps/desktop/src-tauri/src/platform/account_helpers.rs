use msime_client_core::account::AccountError;
use std::sync::Arc;

pub(crate) fn account_command_error(error: AccountError) -> crate::CommandError {
    crate::CommandError { code: error.code() }
}

pub(crate) async fn call_session<S, T, F>(
    session: &Arc<S>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    S: Send + Sync + 'static,
    T: Send + 'static,
    F: FnOnce(&S) -> Result<T, AccountError> + Send + 'static,
{
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || operation(&session))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(account_command_error)
}
