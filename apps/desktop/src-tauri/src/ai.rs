//! The bring-your-own-endpoint AI assistant's two settings-page requests: the
//! model listing and the one-shot polish test.
//!
//! The validation, the requests and the response parsing live in client-core's
//! `ai::settings`, which the native Windows settings window reaches through
//! `msime_client_ai_models` and `msime_client_ai_test`; these commands only move
//! the blocking call off the async runtime and turn its error into a code.
//!
//! Only the two desktop hosts that expose it build this. Android and iOS answer
//! the same two commands from their own platform modules, because those requests
//! go out through the platform's HTTP stack rather than reqwest.

#[cfg(not(target_os = "linux"))]
use crate::CommandError;
#[cfg(not(target_os = "linux"))]
use msime_client_core::ai::settings::AiSettingsError;

#[cfg(not(target_os = "linux"))]
fn command_error(error: AiSettingsError) -> CommandError {
    CommandError { code: error.code() }
}

// Linux routes these two through the provider socket instead - see the
// same-named commands in lib.rs - and `#[tauri::command]` declares a crate-level
// `macro_rules! __cmd__<name>`, so two commands sharing a name collide however
// separate their modules are. Gate the definitions the way their registration is
// already gated rather than leaving both to exist on Linux.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_models(
    endpoint: String,
    token: String,
) -> Result<Vec<String>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        msime_client_core::ai::settings::fetch_models(&endpoint, &token).map_err(command_error)
    })
    .await
    .map_err(|_| command_error(AiSettingsError::ModelsUnavailable))?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_test(
    endpoint: String,
    model: String,
    prompt: String,
    token: String,
    text: String,
) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        msime_client_core::ai::settings::test_polish(&endpoint, &model, &prompt, &token, &text)
            .map_err(command_error)
    })
    .await
    .map_err(|_| command_error(AiSettingsError::TestUnavailable))?
}
