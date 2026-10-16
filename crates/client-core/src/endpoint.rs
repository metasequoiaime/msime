//! Shared URL shape checks for endpoints that receive credentials.

/// Require an explicit host, the selected secure scheme, and no URL credentials or fragment.
/// Callers retain their own length and field-specific validation.
pub fn valid_endpoint_shape(endpoint: &str, url: &url::Url, expected_scheme: &str) -> bool {
    url.scheme() == expected_scheme
        && endpoint.split_once("://").is_some_and(|(_, authority)| {
            authority
                .as_bytes()
                .first()
                .is_some_and(|byte| *byte != b'/')
        })
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

#[cfg(test)]
mod tests {
    use super::valid_endpoint_shape;

    #[test]
    fn secure_endpoints_require_explicit_hosts_and_no_embedded_credentials() {
        for (endpoint, scheme, expected) in [
            ("https://api.example.invalid/v1", "https", true),
            ("wss://asr.example.invalid/stream", "wss", true),
            ("https:///v1", "https", false),
            ("https://user:secret@api.example.invalid/v1", "https", false),
            ("https://api.example.invalid/v1#secret", "https", false),
            ("http://api.example.invalid/v1", "https", false),
            ("https://api.example.invalid/v1", "wss", false),
        ] {
            let url = url::Url::parse(endpoint).unwrap();
            assert_eq!(
                valid_endpoint_shape(endpoint, &url, scheme),
                expected,
                "{endpoint}"
            );
        }
    }
}
