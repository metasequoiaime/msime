//! Shared authentication policy for credential probes and native recognition.

/// Produces sensitive request headers; callers must not log or persist them. An empty mode means API-key auth, which always ignores stale App IDs, including masked placeholders.
pub fn headers(
    mode: &str,
    app_id: &str,
    token: &str,
    resource_id: &str,
) -> Option<Vec<(&'static str, String)>> {
    let (app_id, token, resource_id) = (app_id.trim(), token.trim(), resource_id.trim());
    let legacy = match mode.trim() {
        "api_key" | "" => false,
        "legacy" => true,
        _ => return None,
    };
    if !crate::credential::usable_token(token)
        || !crate::credential::usable_token(resource_id)
        || (legacy && !crate::credential::usable_token(app_id))
        || !token.is_ascii()
        || !resource_id.is_ascii()
        || (legacy && !app_id.is_ascii())
    {
        return None;
    }
    let mut headers = vec![
        ("x-api-resource-id", resource_id.to_owned()),
        ("x-api-request-id", uuid::Uuid::new_v4().to_string()),
    ];
    if legacy {
        headers.push(("x-api-app-key", app_id.to_owned()));
        headers.push(("x-api-access-key", token.to_owned()));
    } else {
        headers.push(("x-api-key", token.to_owned()));
    }
    Some(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_mode_wins_and_empty_mode_means_api_key() {
        for (mode, app, legacy) in [
            ("api_key", "stale-app", false),
            ("api_key", "<stored>", false),
            ("legacy", "synthetic-app", true),
            ("", "synthetic-app", false),
            ("", "", false),
        ] {
            let result = headers(mode, app, " synthetic-token ", " fixture-resource ").unwrap();
            let get = |name| {
                result
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.as_str())
            };
            assert_eq!(get("x-api-key").is_some(), !legacy);
            assert_eq!(get("x-api-app-key").is_some(), legacy);
            assert_eq!(get("x-api-access-key").is_some(), legacy);
            assert_eq!(get("x-api-resource-id"), Some("fixture-resource"));
            assert!(uuid::Uuid::parse_str(get("x-api-request-id").unwrap()).is_ok());
        }
    }

    #[test]
    fn malformed_or_missing_auth_is_rejected_without_echoing_secrets() {
        assert!(headers("unknown", "app", "synthetic-token", "resource").is_none());
        for value in ["", "***", "<stored>", "injected\r\nheader"] {
            assert!(headers("legacy", value, "synthetic-token", "resource").is_none());
            assert!(headers("api_key", "ignored", value, "resource").is_none());
            assert!(headers("api_key", "ignored", "synthetic-token", value).is_none());
        }
        assert!(headers("api_key", "", &"x".repeat(8193), "resource").is_none());
    }

    #[test]
    fn non_ascii_header_credentials_are_rejected_before_ascii_handshake() {
        assert!(headers("api_key", "ignored", "密钥", "resource").is_none());
        assert!(headers("api_key", "ignored", "synthetic-token", "资源").is_none());
        assert!(headers("legacy", "应用", "synthetic-token", "resource").is_none());
    }
}
