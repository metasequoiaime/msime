use msime_client_core::{has_disallowed_control_with_options, is_bounded_text};
use serde_json::Value;

pub fn validate_request(request: &Value) -> Result<(), &'static str> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("invalid cloud clipboard request")?;
    match operation {
        "list" => {
            let search = request.get("search").and_then(Value::as_str).unwrap_or("");
            if !is_bounded_text(search, 1024) {
                return Err("invalid cloud clipboard request");
            }
        }
        "add" => {
            let text = request
                .get("text")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if text.is_empty()
                || text.encode_utf16().count() > 4000
                || has_disallowed_control_with_options(text, true)
            {
                return Err("invalid cloud clipboard request");
            }
        }
        "delete" => {
            let id = request
                .get("id")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if id.is_empty() || id.len() > 256 || !msime_client_core::is_ascii_identifier(id) {
                return Err("invalid cloud clipboard request");
            }
        }
        "set_enabled" => {
            if !request.get("enabled").is_some_and(Value::is_boolean) {
                return Err("invalid cloud clipboard request");
            }
        }
        _ => return Err("invalid cloud clipboard request"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_list_add_delete_and_toggle_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"水"})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"line\nfeed"})).is_ok());
        assert!(validate_request(&json!({"operation":"delete","id":"entry-1"})).is_ok());
        assert!(validate_request(&json!({"operation":"set_enabled","enabled":true})).is_ok());
    }

    #[test]
    fn enforces_utf16_clipboard_bound() {
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4000)})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4001)})).is_err());
    }

    #[test]
    fn rejects_unsafe_or_unknown_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"bad\n"})).is_err());
        assert!(validate_request(&json!({"operation":"add","text":"bad\u{0007}"})).is_err());
        assert!(validate_request(&json!({"operation":"delete","id":"bad/id"})).is_err());
        assert!(validate_request(&json!({"operation":"unknown"})).is_err());
    }
}
