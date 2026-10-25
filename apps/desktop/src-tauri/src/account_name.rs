use msime_client_services::account::name::{present_name, NamePresentation};

/// 昵称提示与保存预检复用共享纯函数，不创建会话或访问存储。
#[tauri::command]
pub(crate) fn account_name(
    name: String,
    id: String,
) -> Result<NamePresentation, crate::CommandError> {
    let request = serde_json::json!({"name": name, "id": id});
    if request.to_string().len() > 16 * 1024 {
        return Err(crate::CommandError {
            code: "account_invalid",
        });
    }
    Ok(present_name(&name, &id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_name_uses_the_shared_presentation_without_a_session() {
        let value = account_name("\u{feff} 合成🌲 \u{feff}".into(), "synthetic".into()).unwrap();
        assert_eq!(value.normalized, "合成🌲");
        assert!(value.valid);
        assert!(
            !account_name("合成\u{85}昵称".into(), String::new())
                .unwrap()
                .valid
        );
        assert!(account_name("🌲".repeat(64), String::new()).unwrap().valid);
        assert!(!account_name("🌲".repeat(65), String::new()).unwrap().valid);
    }

    #[test]
    fn account_name_bounds_the_serialized_request() {
        assert!(account_name("林".repeat(6000), String::new()).is_err());
        assert!(account_name("\0".repeat(3000), String::new()).is_err());
        assert!(account_name("合成".into(), "x".repeat(16 * 1024)).is_err());
    }
}
