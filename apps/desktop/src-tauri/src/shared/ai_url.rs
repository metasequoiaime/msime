#[cfg(any(not(target_os = "linux"), test))]
use reqwest::Url;

/// Convert an endpoint that may point at a chat or transcription resource to
/// its sibling model-list resource. Callers choose whether endpoint query
/// parameters belong to the model request.
#[cfg(any(not(target_os = "linux"), test))]
pub(crate) fn models_url(mut url: Url, clear_query: bool) -> Url {
    let mut path = url.path().trim_end_matches('/').to_owned();
    for suffix in ["/chat/completions", "/audio/transcriptions"] {
        if let Some(prefix) = path.strip_suffix(suffix) {
            path = prefix.to_owned();
            break;
        }
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str("models");
    url.set_path(&path);
    if clear_query {
        url.set_query(None);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::models_url;

    #[test]
    fn replaces_known_endpoint_suffix_and_preserves_requested_query_policy() {
        let endpoint =
            reqwest::Url::parse("https://fixture.invalid/v1/chat/completions?tenant=synthetic")
                .unwrap();
        assert_eq!(
            models_url(endpoint.clone(), true).as_str(),
            "https://fixture.invalid/v1/models"
        );
        assert_eq!(
            models_url(endpoint, false).as_str(),
            "https://fixture.invalid/v1/models?tenant=synthetic"
        );
    }
}
