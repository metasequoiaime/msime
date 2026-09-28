use msime_client_core::cloud::dictionary::DictionaryKind;
use serde_json::Value;

pub(crate) fn dictionary_kind(value: &str) -> Result<DictionaryKind, crate::CommandError> {
    match value {
        "pinyin" => Ok(DictionaryKind::Pinyin),
        "wubi" => Ok(DictionaryKind::Wubi),
        "quick" => Ok(DictionaryKind::Quick),
        "english" => Ok(DictionaryKind::English),
        _ => Err(crate::CommandError {
            code: "invalid_cloud_dictionary",
        }),
    }
}

pub(crate) fn snapshot_command_error() -> crate::CommandError {
    crate::CommandError {
        code: "snapshot_unavailable",
    }
}

pub(crate) fn snapshot_response_without_account(
    mut value: Value,
) -> Result<Value, crate::CommandError> {
    let object = value.as_object_mut().ok_or_else(snapshot_command_error)?;
    if let Some(request) = object.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("accountId");
    }
    Ok(value)
}
