//! `BackendAccountClient`: one HTTPS call per account operation, over the
//! transport the host injected.

use super::validate::*;
use super::*;

#[derive(Clone)]
pub struct BackendAccountClient {
    client: Client,
    origin: Url,
}

impl BackendAccountClient {
    pub fn new() -> Result<Self, AccountError> {
        Self::with_origin(ACCOUNT_ORIGIN, false)
    }

    fn with_origin(origin: &str, allow_http_loopback: bool) -> Result<Self, AccountError> {
        let origin = Url::parse(origin).map_err(|_| AccountError::Invalid)?;
        let valid_scheme = origin.scheme() == "https"
            || (allow_http_loopback
                && origin.scheme() == "http"
                && origin.host_str() == Some("127.0.0.1"));
        if !valid_scheme
            || origin.cannot_be_a_base()
            || origin.username() != ""
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(AccountError::Invalid);
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .user_agent("MSIME/Android")
            .build()
            .map_err(|_| AccountError::Unavailable)?;
        Ok(Self { client, origin })
    }

    #[cfg(test)]
    pub(crate) fn loopback(origin: &str) -> Result<Self, AccountError> {
        Self::with_origin(origin, true)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit(method, path, token, body, MAX_JSON_BYTES)
    }

    pub(crate) fn request_with_limit(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_response_bytes: usize,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout(
            method,
            path,
            token,
            body,
            maximum_response_bytes,
            Duration::from_secs(30),
        )
    }

    pub(crate) fn request_with_limit_timeout(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout_accept(
            method,
            path,
            token,
            body,
            MAX_JSON_BYTES,
            maximum_response_bytes,
            timeout,
            "application/json",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn request_with_limit_timeout_accept(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
        accept: &str,
    ) -> Result<Vec<u8>, AccountError> {
        if !path.starts_with("/v1/") || path.contains('\\') {
            return Err(AccountError::Invalid);
        }
        if body
            .as_ref()
            .is_some_and(|value| value.len() > maximum_request_bytes)
            || token.is_some_and(|value| value.is_empty() || value.chars().any(char::is_whitespace))
        {
            return Err(AccountError::Invalid);
        }
        let url = self.origin.join(path).map_err(|_| AccountError::Invalid)?;
        if url.scheme() != self.origin.scheme()
            || url.host_str() != self.origin.host_str()
            || url.port_or_known_default() != self.origin.port_or_known_default()
            || url.username() != ""
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(AccountError::Invalid);
        }
        let mut request = self
            .client
            .request(method, url)
            .header(reqwest::header::ACCEPT, accept);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        let response = request
            .timeout(timeout)
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        read_bounded_response(response, maximum_response_bytes)
    }

    pub(crate) fn json<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request(method, path, token, body)?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    pub(crate) fn json_with_limit<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_response_bytes: usize,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit(method, path, token, body, maximum_response_bytes)?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    pub(crate) fn json_with_limit_timeout<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit_timeout(
            method,
            path,
            token,
            body,
            maximum_response_bytes,
            timeout,
        )?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    /// Like [`Self::json_with_limit_timeout`], for the few requests whose body may exceed the 1 MiB every other account request is held to. The caller names the request bound explicitly, so nothing else inherits it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn json_with_limits_timeout<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit_timeout_accept(
            method,
            path,
            token,
            body,
            maximum_request_bytes,
            maximum_response_bytes,
            timeout,
            "application/json",
        )?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    fn empty<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
    ) -> Result<(), AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        self.request(method, path, token, body).map(|_| ())
    }

    pub fn clipboard(
        &self,
        search: &str,
        access_token: &str,
    ) -> Result<AccountClipboardPage, AccountError> {
        validate_clipboard_search(search)?;
        let encoded = crate::cloud::dictionary::percent_encode(search);
        let page = self.json::<AccountClipboardPage, ()>(
            Method::GET,
            &format!("/v1/users/me/clipboard?q={encoded}"),
            Some(access_token),
            None,
        )?;
        validate_clipboard_page(&page)?;
        Ok(page)
    }

    pub fn set_clipboard_enabled(
        &self,
        enabled: bool,
        access_token: &str,
    ) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            enabled: bool,
        }
        self.empty(
            Method::PUT,
            "/v1/users/me/clipboard/settings",
            Some(access_token),
            Some(&Body { enabled }),
        )
    }

    pub fn add_clipboard(
        &self,
        text: &str,
        access_token: &str,
    ) -> Result<AccountClipboardItem, AccountError> {
        validate_clipboard_text(text)?;
        #[derive(Serialize)]
        struct Body<'a> {
            text: &'a str,
        }
        let item = self.json(
            Method::POST,
            "/v1/users/me/clipboard",
            Some(access_token),
            Some(&Body { text }),
        )?;
        validate_clipboard_item(&item)?;
        Ok(item)
    }

    pub fn delete_clipboard(
        &self,
        id: Option<&str>,
        access_token: &str,
    ) -> Result<(), AccountError> {
        if let Some(id) = id {
            validate_clipboard_id(id)?;
        }
        let path = id
            .map(|value| format!("/v1/users/me/clipboard/{value}"))
            .unwrap_or_else(|| "/v1/users/me/clipboard".to_owned());
        self.empty::<()>(Method::DELETE, &path, Some(access_token), None)
    }

    pub fn dictionary(
        &self,
        kind: DictionaryKind,
        search: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryPage, AccountError> {
        let path = crate::cloud::dictionary::dictionary_path(kind, offset, search)
            .ok_or(AccountError::Invalid)?;
        let page =
            self.json::<AccountDictionaryPage, ()>(Method::GET, &path, Some(access_token), None)?;
        validate_dictionary_page(&page, kind)?;
        Ok(page)
    }

    pub fn dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        offset: usize,
        scheme: &str,
        profile: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        validate_dictionary_catalog_query(code, offset, scheme, profile)?;
        let path = dictionary_catalog_path(kind, code, offset, scheme, profile)?;
        let page = self.json::<AccountDictionaryCatalogPage, ()>(
            Method::GET,
            &path,
            Some(access_token),
            None,
        )?;
        validate_dictionary_catalog_page(&page, kind)?;
        Ok(page)
    }

    pub fn dictionary_changes(
        &self,
        after: i64,
        limit: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        if after < 0 || !(1..=100).contains(&limit) {
            return Err(AccountError::Invalid);
        }
        let page = self.json::<AccountDictionaryChangePage, ()>(
            Method::GET,
            &format!("/v1/users/me/dictionary/changes?after={after}&limit={limit}"),
            Some(access_token),
            None,
        )?;
        if page.changes.len() > limit {
            return Err(AccountError::Unavailable);
        }
        let mut cursor = after;
        for change in &page.changes {
            if change.revision <= cursor {
                return Err(AccountError::Unavailable);
            }
            if change.previous.as_ref().is_some_and(|entry| {
                validate_dictionary_entry(entry, entry.kind).is_err()
                    || entry.revision > change.revision
            }) || change.replacement.as_ref().is_some_and(|entry| {
                validate_dictionary_entry(entry, entry.kind).is_err()
                    || entry.revision > change.revision
            }) {
                return Err(AccountError::Unavailable);
            }
            cursor = change.revision;
        }
        if page.next != cursor || (page.has_more && page.changes.is_empty()) {
            return Err(AccountError::Unavailable);
        }
        Ok(page)
    }

    pub fn dictionary_snapshot(&self, access_token: &str) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout_accept(
            Method::GET,
            "/v1/users/me/dictionary/snapshot",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_DICTIONARY_SNAPSHOT_BYTES,
            Duration::from_secs(120),
            "application/x-ndjson",
        )
    }

    pub fn dictionary_snapshot_to_file(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        if !destination.is_absolute() || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join("/v1/users/me/dictionary/snapshot")
            .map_err(|_| AccountError::Invalid)?;
        let mut response = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(120))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        if !response.status().is_success() {
            return Err(AccountError::from_status(response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_DICTIONARY_SNAPSHOT_BYTES as u64)
        {
            return Err(AccountError::Unavailable);
        }
        let parent = destination.parent().ok_or(AccountError::Invalid)?;
        if !parent.is_absolute() {
            return Err(AccountError::Invalid);
        }
        let mut temporary = tempfile::Builder::new()
            .prefix("msime-snapshot-")
            .tempfile_in(parent)
            .map_err(|_| AccountError::Unavailable)?;
        let bytes = std::io::copy(
            &mut response
                .by_ref()
                .take((MAX_DICTIONARY_SNAPSHOT_BYTES + 1) as u64),
            temporary.as_file_mut(),
        )
        .map_err(|_| AccountError::Unavailable)?;
        if bytes == 0 || bytes > MAX_DICTIONARY_SNAPSHOT_BYTES as u64 {
            return Err(AccountError::Unavailable);
        }
        temporary
            .as_file_mut()
            .flush()
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| AccountError::Unavailable)?;
        temporary
            .persist(destination)
            .map_err(|_| AccountError::Unavailable)?;
        Ok(bytes)
    }

    pub fn restore_dictionary_snapshot(
        &self,
        snapshot: &[u8],
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        if revision < 0
            || snapshot.is_empty()
            || snapshot.len() > MAX_DICTIONARY_SNAPSHOT_BYTES
            || snapshot.contains(&0)
            || !crate::text::is_lower_hex(access_token, 64)
        {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join(&format!(
                "/v1/users/me/dictionary/snapshot?revision={revision}"
            ))
            .map_err(|_| AccountError::Invalid)?;
        let response = self
            .client
            .put(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(130))
            .body(snapshot.to_vec())
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        Self::decode_snapshot_restore(response, revision)
    }

    /// Stream a validated private snapshot file as the request body without loading it in memory.
    pub fn restore_dictionary_snapshot_file(
        &self,
        snapshot: &Path,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        if revision < 0 || !snapshot.is_absolute() || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let metadata = std::fs::symlink_metadata(snapshot).map_err(|_| AccountError::Invalid)?;
        if !metadata.file_type().is_file() {
            return Err(AccountError::Invalid);
        }
        let file = std::fs::File::open(snapshot).map_err(|_| AccountError::Invalid)?;
        let bytes = file.metadata().map_err(|_| AccountError::Invalid)?.len();
        if bytes == 0 || bytes > MAX_DICTIONARY_SNAPSHOT_BYTES as u64 {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join(&format!(
                "/v1/users/me/dictionary/snapshot?revision={revision}"
            ))
            .map_err(|_| AccountError::Invalid)?;
        let response = self
            .client
            .put(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(600))
            .body(reqwest::blocking::Body::sized(file, bytes))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        Self::decode_snapshot_restore(response, revision)
    }

    fn decode_snapshot_restore(
        response: Response,
        revision: i64,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if !media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/json")) {
            return Err(AccountError::Unavailable);
        }
        let bytes = read_bounded_response(response, MAX_JSON_BYTES)?;
        let result: AccountDictionarySnapshotRestore =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        if !result.reset || result.revision <= revision {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn edit_dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        revision: i64,
        replacement: Option<(&str, &str, i64)>,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_catalog_identity(kind, code, word)?;
        if revision < 0 {
            return Err(AccountError::Invalid);
        }
        if let Some((replacement_code, replacement_word, replacement_weight)) = replacement {
            validate_new_dictionary_value(
                kind,
                replacement_code,
                replacement_word,
                replacement_weight,
            )?;
        }
        #[derive(Serialize)]
        struct Identity<'a> {
            code: &'a str,
            word: &'a str,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            previous: Identity<'a>,
            replacement: Option<Replacement<'a>>,
        }
        #[derive(Serialize)]
        struct Replacement<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
        }
        let replacement_body =
            replacement.map(|(replacement_code, replacement_word, weight)| Replacement {
                code: replacement_code,
                word: replacement_word,
                weight,
            });
        let change = self.json(
            Method::POST,
            &format!(
                "/v1/users/me/dictionaries/{}/edit",
                crate::cloud::dictionary::kind_path(kind)
            ),
            Some(access_token),
            Some(&Body {
                revision,
                previous: Identity { code, word },
                replacement: replacement_body,
            }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn personal_candidates(
        &self,
        query: &AccountCandidateQuery,
        access_token: &str,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        validate_candidate_query(query)?;
        let result: AccountPersonalCandidates = self.json(
            Method::POST,
            "/v1/users/me/dictionary/candidates",
            Some(access_token),
            Some(query),
        )?;
        validate_personal_candidates(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rank_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        mode: &str,
        linear_step: i64,
        trigger_count: i64,
        force_top: bool,
        access_token: &str,
    ) -> Result<AccountRankingResult, AccountError> {
        validate_candidate_query(query)?;
        validate_candidate_value(query, code, word)?;
        validate_ranking_arguments(query, revision, mode, linear_step, trigger_count)?;
        #[derive(Serialize)]
        struct Action<'a> {
            code: &'a str,
            word: &'a str,
            mode: &'a str,
            linear_step: i64,
            trigger_count: i64,
            force_top: bool,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            query: &'a AccountCandidateQuery,
            action: Action<'a>,
        }
        let result = self.json(
            Method::POST,
            "/v1/users/me/dictionary/ranking",
            Some(access_token),
            Some(&Body {
                revision,
                query,
                action: Action {
                    code,
                    word,
                    mode,
                    linear_step,
                    trigger_count,
                    force_top,
                },
            }),
        )?;
        validate_ranking_result(&result)?;
        Ok(result)
    }

    pub fn remove_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_candidate_query(query)?;
        validate_candidate_value(query, code, word)?;
        if revision < 0 || query.kind == "quick" {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            query: &'a AccountCandidateQuery,
            code: &'a str,
            word: &'a str,
        }
        let change = self.json(
            Method::DELETE,
            "/v1/users/me/dictionary/candidates",
            Some(access_token),
            Some(&Body {
                revision,
                query,
                code,
                word,
            }),
        )?;
        validate_dictionary_change(&change, dictionary_kind_for_candidate(query)?)?;
        Ok(change)
    }

    pub fn fixed_positions(
        &self,
        context: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountFixedPositions, AccountError> {
        validate_bounded_text(context, 1024)?;
        if offset > 1_000_000 {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/users/me/dictionary/positions?context={}&offset={offset}&limit=100",
            crate::cloud::dictionary::percent_encode(context)
        );
        let result =
            self.json::<AccountFixedPositions, ()>(Method::GET, &path, Some(access_token), None)?;
        validate_fixed_positions(&result)?;
        Ok(result)
    }

    pub fn set_fixed_position(
        &self,
        context: &str,
        code: &str,
        word: &str,
        position: Option<i64>,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        validate_bounded_text(context, 1024)?;
        validate_bounded_text(code, 256)?;
        validate_bounded_text(word, 1024)?;
        if revision < 0 || position.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            context: &'a str,
            code: &'a str,
            word: &'a str,
            position: Option<i64>,
            revision: i64,
        }
        let result: AccountDictionaryRevision = self.json(
            if position.is_some() {
                Method::PUT
            } else {
                Method::DELETE
            },
            "/v1/users/me/dictionary/positions",
            Some(access_token),
            Some(&Body {
                context,
                code,
                word,
                position,
                revision,
            }),
        )?;
        if result.revision < 0 {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    pub fn add_dictionary(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        weight: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_new_dictionary_value(kind, code, word, weight)?;
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
        }
        let path =
            crate::cloud::dictionary::mutation_path(kind, "add").ok_or(AccountError::Invalid)?;
        let change = self.json(
            Method::POST,
            &path,
            Some(access_token),
            Some(&Body { code, word, weight }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        code: &str,
        word: &str,
        weight: i64,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_id(id)?;
        validate_new_dictionary_value(kind, code, word, weight)?;
        if revision <= 0 {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
            revision: i64,
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/{}",
            crate::cloud::dictionary::kind_path(kind),
            id
        );
        let change = self.json(
            Method::PUT,
            &path,
            Some(access_token),
            Some(&Body {
                code,
                word,
                weight,
                revision,
            }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn delete_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_id(id)?;
        if revision <= 0 {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body {
            revision: i64,
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/{}",
            crate::cloud::dictionary::kind_path(kind),
            id
        );
        let change = self.json(
            Method::DELETE,
            &path,
            Some(access_token),
            Some(&Body { revision }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn import_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        text: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        validate_dictionary_import(kind, format, text)?;
        let (path, body) = if format == "hans" {
            (
                crate::cloud::dictionary::mutation_path(kind, "import-hans")
                    .ok_or(AccountError::Invalid)?,
                serde_json::json!({ "text": text, "weight": 100000_i64 }),
            )
        } else {
            (
                crate::cloud::dictionary::mutation_path(kind, "import")
                    .ok_or(AccountError::Invalid)?,
                serde_json::json!({ "text": text, "format": format }),
            )
        };
        let result = self.json(Method::POST, &path, Some(access_token), Some(&body))?;
        validate_dictionary_import_result(&result)?;
        Ok(result)
    }

    pub fn export_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        if !matches!(format, "standard" | "windows") {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/export?format={format}",
            crate::cloud::dictionary::kind_path(kind)
        );
        let bytes = self.request_with_limit_timeout_accept(
            Method::GET,
            &path,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_DICTIONARY_EXPORT_BYTES,
            Duration::from_secs(600),
            "text/plain",
        )?;
        let text = String::from_utf8(bytes).map_err(|_| AccountError::Unavailable)?;
        if text.is_empty() || text.contains('\0') {
            return Err(AccountError::Unavailable);
        }
        Ok(AccountDictionaryExport {
            text,
            filename: format!(
                "dictionary-{}.tsv",
                crate::cloud::dictionary::kind_path(kind)
            ),
        })
    }
}

impl AccountApi for BackendAccountClient {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        #[derive(Deserialize)]
        struct Providers {
            providers: std::collections::HashMap<String, bool>,
        }
        self.json::<Providers, ()>(Method::GET, "/v1/auth/providers", None, None)
            .map(|value| value.providers)
    }

    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        validate_provider_target(provider, target)?;
        #[derive(Serialize)]
        struct Body<'a> {
            provider: &'a str,
            target: &'a str,
            purpose: &'static str,
        }
        let challenge = self.json(
            Method::POST,
            "/v1/auth/challenges",
            None,
            Some(&Body {
                provider,
                target,
                purpose: "login",
            }),
        )?;
        validate_challenge(&challenge)?;
        Ok(challenge)
    }

    fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
        validate_login(challenge, credential)?;
        #[derive(Serialize)]
        struct Body<'a> {
            challenge_id: &'a str,
            credential: &'a str,
        }
        let tokens = self.json(
            Method::POST,
            "/v1/auth/login",
            None,
            Some(&Body {
                challenge_id: challenge,
                credential,
            }),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        if !crate::text::is_lower_hex(refresh_token, 64) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            refresh_token: &'a str,
        }
        let tokens = self.json(
            Method::POST,
            "/v1/auth/refresh",
            None,
            Some(&Body { refresh_token }),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn profile(&self, access_token: &str) -> Result<AccountProfile, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let profile =
            self.json::<AccountProfile, ()>(Method::GET, "/v1/users/me", Some(access_token), None)?;
        validate_profile(&profile)?;
        Ok(profile)
    }

    fn rename(&self, display_name: &str, access_token: &str) -> Result<(), AccountError> {
        validate_display_name(display_name)?;
        #[derive(Serialize)]
        struct Body<'a> {
            display_name: &'a str,
        }
        self.empty(
            Method::PATCH,
            "/v1/users/me",
            Some(access_token),
            Some(&Body { display_name }),
        )
    }

    fn logout(&self, access_token: &str, all: bool) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            all: bool,
        }
        self.empty(
            Method::POST,
            "/v1/auth/logout",
            Some(access_token),
            Some(&Body { all }),
        )
    }

    fn delete_account(&self, access_token: &str) -> Result<(), AccountError> {
        self.empty::<()>(Method::DELETE, "/v1/users/me", Some(access_token), None)
    }

    fn chat_models(&self, access_token: &str) -> Result<AccountChatModels, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let models = self.json::<AccountChatModels, ()>(
            Method::GET,
            "/v1/models",
            Some(access_token),
            None,
        )?;
        validate_chat_models(&models)?;
        Ok(models)
    }

    fn chat(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
        access_token: &str,
    ) -> Result<String, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        validate_chat_request(messages, model)?;
        #[derive(Serialize)]
        struct Body<'a> {
            messages: &'a [AccountChatMessage],
            model: &'a str,
            max_tokens: u16,
            stream: bool,
        }
        #[derive(Deserialize)]
        struct Response {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: AccountChatMessage,
        }
        let body = Body {
            messages,
            model,
            max_tokens: 2048,
            stream: false,
        };
        let body_bytes = serde_json::to_vec(&body).map_err(|_| AccountError::Invalid)?;
        if body_bytes.len() > MAX_CHAT_REQUEST_BYTES {
            return Err(AccountError::Invalid);
        }
        let response = self.json_with_limit_timeout::<Response, _>(
            Method::POST,
            "/v1/chat/completions",
            Some(access_token),
            Some(&body),
            MAX_CHAT_RESPONSE_BYTES,
            Duration::from_secs(125),
        )?;
        let reply = response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or(AccountError::Unavailable)?;
        if reply.role != "assistant"
            || reply.content.trim().is_empty()
            || reply.content.len() > MAX_CHAT_RESPONSE_BYTES
            || crate::text::has_disallowed_control(&reply.content)
        {
            return Err(AccountError::Unavailable);
        }
        Ok(reply.content)
    }

    fn preference_schema(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let schema = self.json::<AccountPreferenceSchema, ()>(
            Method::GET,
            "/v1/users/me/preferences/schema",
            Some(access_token),
            None,
        )?;
        validate_preference_schema(&schema)?;
        Ok(schema)
    }

    fn preferences(&self, access_token: &str) -> Result<AccountPreferences, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let preferences = self.json::<AccountPreferences, ()>(
            Method::GET,
            "/v1/users/me/preferences",
            Some(access_token),
            None,
        )?;
        validate_account_preferences(&preferences)?;
        Ok(preferences)
    }

    fn put_preferences(
        &self,
        preferences: &AccountPreferences,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        validate_account_preferences(preferences)?;
        let updated = self.json(
            Method::PUT,
            "/v1/users/me/preferences",
            Some(access_token),
            Some(preferences),
        )?;
        validate_account_preferences(&updated)?;
        Ok(updated)
    }

    fn clipboard(
        &self,
        search: &str,
        access_token: &str,
    ) -> Result<AccountClipboardPage, AccountError> {
        self.clipboard(search, access_token)
    }

    fn set_clipboard_enabled(&self, enabled: bool, access_token: &str) -> Result<(), AccountError> {
        self.set_clipboard_enabled(enabled, access_token)
    }

    fn add_clipboard(
        &self,
        text: &str,
        access_token: &str,
    ) -> Result<AccountClipboardItem, AccountError> {
        self.add_clipboard(text, access_token)
    }

    fn delete_clipboard(&self, id: Option<&str>, access_token: &str) -> Result<(), AccountError> {
        self.delete_clipboard(id, access_token)
    }

    fn dictionary(
        &self,
        kind: DictionaryKind,
        search: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryPage, AccountError> {
        self.dictionary(kind, search, offset, access_token)
    }

    fn dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        offset: usize,
        scheme: &str,
        profile: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        self.dictionary_catalog(kind, code, offset, scheme, profile, access_token)
    }

    #[allow(clippy::too_many_arguments)]
    fn edit_dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        revision: i64,
        replacement: Option<(&str, &str, i64)>,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.edit_dictionary_catalog(kind, code, word, revision, replacement, access_token)
    }

    fn personal_candidates(
        &self,
        query: &AccountCandidateQuery,
        access_token: &str,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        self.personal_candidates(query, access_token)
    }

    #[allow(clippy::too_many_arguments)]
    fn rank_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        mode: &str,
        linear_step: i64,
        trigger_count: i64,
        force_top: bool,
        access_token: &str,
    ) -> Result<AccountRankingResult, AccountError> {
        self.rank_candidate(
            query,
            code,
            word,
            revision,
            mode,
            linear_step,
            trigger_count,
            force_top,
            access_token,
        )
    }

    fn remove_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.remove_candidate(query, code, word, revision, access_token)
    }

    fn fixed_positions(
        &self,
        context: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountFixedPositions, AccountError> {
        self.fixed_positions(context, offset, access_token)
    }

    fn set_fixed_position(
        &self,
        context: &str,
        code: &str,
        word: &str,
        position: Option<i64>,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        self.set_fixed_position(context, code, word, position, revision, access_token)
    }

    fn add_dictionary(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        weight: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.add_dictionary(kind, code, word, weight, access_token)
    }

    fn update_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        code: &str,
        word: &str,
        weight: i64,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.update_dictionary(kind, id, code, word, weight, revision, access_token)
    }

    fn delete_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.delete_dictionary(kind, id, revision, access_token)
    }

    fn import_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        text: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        self.import_dictionary(kind, format, text, access_token)
    }

    fn export_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        self.export_dictionary(kind, format, access_token)
    }

    fn dictionary_changes(
        &self,
        after: i64,
        limit: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        self.dictionary_changes(after, limit, access_token)
    }

    fn dictionary_snapshot(&self, access_token: &str) -> Result<Vec<u8>, AccountError> {
        self.dictionary_snapshot(access_token)
    }

    fn dictionary_snapshot_to_file(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        self.dictionary_snapshot_to_file(destination, access_token)
    }

    fn restore_dictionary_snapshot(
        &self,
        snapshot: &[u8],
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        self.restore_dictionary_snapshot(snapshot, revision, access_token)
    }
}
