//! Every value crossing the account boundary is bounded and checked here.
//! The backend is not trusted to keep its own limits, and neither is the caller.

use super::google::valid_google_loopback_target;
use super::*;

pub(super) fn validate_clipboard_search(value: &str) -> Result<(), AccountError> {
    validate_bounded_text(value, 1024)
}

pub(super) fn validate_providers(
    value: &std::collections::HashMap<String, bool>,
) -> Result<(), AccountError> {
    if value.len() > 16
        || value.keys().any(|key| {
            key.is_empty()
                || key.len() > 32
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_' || byte == b'-')
        })
    {
        Err(AccountError::Unavailable)
    } else {
        Ok(())
    }
}

pub(super) fn validate_clipboard_text(value: &str) -> Result<(), AccountError> {
    if value.trim().is_empty()
        || !crate::is_bounded_utf16(value, 4000)
        || value.contains('\0')
        || crate::text::has_disallowed_control(value)
    {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_clipboard_id(value: &str) -> Result<(), AccountError> {
    if !crate::text::is_lower_hex(value, 64) {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_clipboard_item(value: &AccountClipboardItem) -> Result<(), AccountError> {
    validate_clipboard_id(&value.id)?;
    validate_clipboard_text(&value.text)?;
    if value.updated_at.is_empty() || !crate::text::is_bounded_text(&value.updated_at, 128) {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

pub(super) fn validate_clipboard_page(value: &AccountClipboardPage) -> Result<(), AccountError> {
    if value.items.len() > 50 {
        return Err(AccountError::Unavailable);
    }
    for item in &value.items {
        validate_clipboard_item(item)?;
    }
    Ok(())
}

fn dictionary_code_is_well_formed(kind: DictionaryKind, code: &str) -> bool {
    match kind {
        DictionaryKind::Pinyin => crate::dictionary::pinyin_code_is_well_formed(code, true),
        DictionaryKind::Wubi | DictionaryKind::Wubi98 => {
            crate::dictionary::wubi_code_is_well_formed(code)
        }
        DictionaryKind::Quick => {
            crate::dictionary::quick_phrase_transport_code_is_well_formed(code)
        }
        DictionaryKind::English => crate::dictionary::english_code_is_well_formed(code),
    }
}

fn validate_dictionary_fields(
    kind: DictionaryKind,
    code: &str,
    word: &str,
) -> Result<(), AccountError> {
    if !dictionary_code_is_well_formed(kind, code)
        || code.is_empty()
        || !crate::text::is_bounded_text(code, 256)
        || word.is_empty()
        || !crate::text::is_bounded_text(word, 1024)
    {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_dictionary_catalog_query(
    code: &str,
    offset: usize,
    scheme: &str,
    profile: &str,
) -> Result<(), AccountError> {
    if offset > 1_000_000
        || !crate::text::is_bounded_text(code, 256)
        || scheme.is_empty()
        || !crate::text::is_bounded_text(scheme, 64)
        || profile.is_empty()
        || !crate::text::is_bounded_text(profile, 64)
    {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn dictionary_catalog_path(
    kind: DictionaryKind,
    code: &str,
    offset: usize,
    scheme: &str,
    profile: &str,
) -> Result<String, AccountError> {
    validate_dictionary_catalog_query(code, offset, scheme, profile)?;
    Ok(format!(
        "/v1/users/me/dictionaries/{}/catalog?q={}&offset={offset}&limit=100&scheme={}&profile={}",
        crate::cloud::dictionary::kind_path(kind),
        crate::cloud::dictionary::percent_encode(code),
        crate::cloud::dictionary::percent_encode(scheme),
        crate::cloud::dictionary::percent_encode(profile)
    ))
}

pub(super) fn validate_dictionary_catalog_identity(
    kind: DictionaryKind,
    code: &str,
    word: &str,
) -> Result<(), AccountError> {
    validate_dictionary_fields(kind, code, word)
}

pub(super) fn validate_dictionary_catalog_entry(
    entry: &AccountDictionaryCatalogEntry,
    expected_kind: DictionaryKind,
) -> Result<(), AccountError> {
    if entry.kind != expected_kind {
        return Err(AccountError::Unavailable);
    }
    validate_dictionary_value(expected_kind, &entry.code, &entry.word, entry.weight)
        .map_err(|_| AccountError::Unavailable)
}

pub(super) fn validate_dictionary_catalog_page(
    page: &AccountDictionaryCatalogPage,
    expected_kind: DictionaryKind,
) -> Result<(), AccountError> {
    if page.entries.len() > MAX_DICTIONARY_PAGE_ENTRIES
        || page.offset > 1_000_000
        || page.revision < 0
        || !crate::text::is_bounded_text(&page.normalized, 256)
    {
        return Err(AccountError::Unavailable);
    }
    for entry in &page.entries {
        validate_dictionary_catalog_entry(entry, expected_kind)?;
    }
    Ok(())
}

pub(super) fn validate_bounded_text(value: &str, maximum_bytes: usize) -> Result<(), AccountError> {
    if !crate::text::is_bounded_text(value, maximum_bytes) {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_candidate_query(query: &AccountCandidateQuery) -> Result<(), AccountError> {
    if !crate::cloud::dictionary::valid_candidate_query(
        &query.text,
        &query.kind,
        &query.scheme,
        &query.profile,
        query.limit,
    ) {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn dictionary_kind_for_candidate(
    query: &AccountCandidateQuery,
) -> Result<DictionaryKind, AccountError> {
    match query.kind.as_str() {
        "pinyin" | "jianpin" => Ok(DictionaryKind::Pinyin),
        "wubi" => Ok(DictionaryKind::Wubi),
        "wubi98" => Ok(DictionaryKind::Wubi98),
        "quick" => Ok(DictionaryKind::Quick),
        "english" => Ok(DictionaryKind::English),
        _ => Err(AccountError::Invalid),
    }
}

pub(super) fn validate_candidate_value(
    query: &AccountCandidateQuery,
    code: &str,
    word: &str,
) -> Result<(), AccountError> {
    if !crate::cloud::dictionary::valid_candidate_value(code, word) {
        return Err(AccountError::Invalid);
    }
    if query.kind == "quick"
        && !crate::is_bounded_utf16(word, crate::dictionary::import::MAX_QUICK_PHRASE_UTF16)
    {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_ranking_arguments(
    query: &AccountCandidateQuery,
    revision: i64,
    mode: &str,
    linear_step: i64,
    trigger_count: i64,
) -> Result<(), AccountError> {
    if revision < 0
        || query.kind == "quick"
        || !matches!(mode, "disabled" | "pin" | "halve" | "linear" | "promote")
        || !(1..=100).contains(&linear_step)
        || !(1..=10).contains(&trigger_count)
    {
        Err(AccountError::Invalid)
    } else {
        Ok(())
    }
}

pub(super) fn validate_personal_candidates(
    result: &AccountPersonalCandidates,
) -> Result<(), AccountError> {
    if result.candidates.len() > 100 || result.revision < 0 {
        return Err(AccountError::Unavailable);
    }
    validate_bounded_text(&result.context, 1024).map_err(|_| AccountError::Unavailable)?;
    for candidate in &result.candidates {
        validate_bounded_text(&candidate.code, 256).map_err(|_| AccountError::Unavailable)?;
        validate_bounded_text(&candidate.word, 1024).map_err(|_| AccountError::Unavailable)?;
        if candidate.code.is_empty() || candidate.word.is_empty() || candidate.weight < 0 {
            return Err(AccountError::Unavailable);
        }
        if let Some(canonical) = candidate.canonical_pinyin.as_deref() {
            validate_bounded_text(canonical, 256).map_err(|_| AccountError::Unavailable)?;
        }
    }
    Ok(())
}

pub(super) fn validate_ranking_result(result: &AccountRankingResult) -> Result<(), AccountError> {
    if result.revision < 0 || result.selection.count < 0 {
        Err(AccountError::Unavailable)
    } else {
        Ok(())
    }
}

pub(super) fn validate_fixed_positions(result: &AccountFixedPositions) -> Result<(), AccountError> {
    if result.positions.len() > 100 || result.offset > 1_000_000 {
        return Err(AccountError::Unavailable);
    }
    for item in &result.positions {
        validate_bounded_text(&item.context, 1024).map_err(|_| AccountError::Unavailable)?;
        validate_bounded_text(&item.code, 256).map_err(|_| AccountError::Unavailable)?;
        validate_bounded_text(&item.word, 1024).map_err(|_| AccountError::Unavailable)?;
        if item.position <= 0 || item.position > 5 {
            return Err(AccountError::Unavailable);
        }
    }
    Ok(())
}

pub(super) fn validate_dictionary_id(value: &str) -> Result<(), AccountError> {
    if crate::text::is_lower_hex(value, 64) {
        Ok(())
    } else {
        Err(AccountError::Invalid)
    }
}

pub(super) fn validate_dictionary_value(
    kind: DictionaryKind,
    code: &str,
    word: &str,
    weight: i64,
) -> Result<(), AccountError> {
    // Keep writes and inbound responses on the same shared contract. The
    // shared field check runs first; this layer adds only the per-dictionary
    // syntax and legacy inbound compatibility rules.
    validate_dictionary_fields(kind, code, word)?;
    let code_limit = match kind {
        DictionaryKind::Pinyin => 256,
        DictionaryKind::Wubi | DictionaryKind::Wubi98 => 4,
        DictionaryKind::Quick => 32,
        DictionaryKind::English => 64,
    };
    if code.len() > code_limit
        || weight < 0
        || (kind == DictionaryKind::Quick
            && !crate::is_bounded_utf16(word, crate::dictionary::import::MAX_QUICK_PHRASE_UTF16))
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

/// The check for a value this client is about to write: `validate_dictionary_value` plus the stricter rules new input follows. A quick phrase code must be letters only, as in the reference; inbound rows keep the lenient check so a stored code with a digit still syncs.
pub(super) fn validate_new_dictionary_value(
    kind: DictionaryKind,
    code: &str,
    word: &str,
    weight: i64,
) -> Result<(), AccountError> {
    validate_dictionary_value(kind, code, word, weight)?;
    if kind == DictionaryKind::Quick && !crate::dictionary::quick_phrase_code_is_well_formed(code) {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_dictionary_entry(
    entry: &AccountDictionaryEntry,
    expected_kind: DictionaryKind,
) -> Result<(), AccountError> {
    if entry.kind != expected_kind || entry.revision <= 0 {
        return Err(AccountError::Unavailable);
    }
    validate_dictionary_id(&entry.id).map_err(|_| AccountError::Unavailable)?;
    validate_dictionary_value(expected_kind, &entry.code, &entry.word, entry.weight)
        .map_err(|_| AccountError::Unavailable)
}

pub(super) fn validate_dictionary_page(
    page: &AccountDictionaryPage,
    expected_kind: DictionaryKind,
) -> Result<(), AccountError> {
    if page.entries.len() > MAX_DICTIONARY_PAGE_ENTRIES || page.offset > 1_000_000 {
        return Err(AccountError::Unavailable);
    }
    for entry in &page.entries {
        validate_dictionary_entry(entry, expected_kind)?;
    }
    Ok(())
}

pub(super) fn validate_dictionary_change(
    change: &AccountDictionaryChange,
    expected_kind: DictionaryKind,
) -> Result<(), AccountError> {
    if change.revision <= 0
        || change
            .previous
            .as_ref()
            .is_some_and(|entry| validate_dictionary_entry(entry, expected_kind).is_err())
        || change
            .replacement
            .as_ref()
            .is_some_and(|entry| validate_dictionary_entry(entry, expected_kind).is_err())
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

pub(super) fn validate_dictionary_import(
    kind: DictionaryKind,
    format: &str,
    text: &str,
) -> Result<(), AccountError> {
    if !matches!(format, "standard" | "windows" | "hans")
        || (format == "hans" && kind != DictionaryKind::Pinyin)
        || crate::cloud::dictionary::validate_import(text).is_err()
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_dictionary_import_result(
    result: &AccountDictionaryImportResult,
) -> Result<(), AccountError> {
    if result.imported > 1_000_000 || result.revision < 0 {
        Err(AccountError::Unavailable)
    } else {
        Ok(())
    }
}

pub(super) fn read_bounded_response(
    response: Response,
    maximum_response_bytes: usize,
) -> Result<Vec<u8>, AccountError> {
    if !response.status().is_success() {
        return Err(error_from_response(response));
    }
    if response
        .content_length()
        .is_some_and(|length| length > maximum_response_bytes as u64)
    {
        return Err(AccountError::Unavailable);
    }
    crate::bounded_io::read_bounded(response, maximum_response_bytes as u64)
        .map_err(|_| AccountError::Unavailable)
}

/// Largest error body read to find the server's error code; the backend's error documents are a few dozen bytes.
const MAX_ERROR_BODY_BYTES: u64 = 4096;

pub(super) fn error_from_response(response: Response) -> AccountError {
    let status = response.status();
    let body = crate::bounded_io::read_bounded(response, MAX_ERROR_BODY_BYTES).unwrap_or_default();
    AccountError::from_response(status, &body)
}

pub fn validate_account_preferences(value: &AccountPreferences) -> Result<(), AccountError> {
    if value.revision < 0 || value.settings.len() > MAX_ACCOUNT_PREFERENCE_FIELDS {
        return Err(AccountError::Unavailable);
    }
    for (key, value) in &value.settings {
        if !valid_preference_key(key) {
            return Err(AccountError::Unavailable);
        }
        match value {
            AccountPreferenceValue::Number(number) if !number.is_finite() => {
                return Err(AccountError::Unavailable);
            }
            AccountPreferenceValue::String(string)
                if !crate::text::is_bounded_text(string, MAX_ACCOUNT_PREFERENCE_STRING_BYTES) =>
            {
                return Err(AccountError::Unavailable);
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_preference_schema(value: &AccountPreferenceSchema) -> Result<(), AccountError> {
    if value.fields.len() > MAX_ACCOUNT_PREFERENCE_FIELDS
        || !(1..=MAX_JSON_BYTES).contains(&value.maximum_bytes)
        || value.update_mode != "replace"
        || !value.revision_required
    {
        return Err(AccountError::Unavailable);
    }
    for (key, field) in &value.fields {
        if !valid_preference_key(key)
            || !matches!(
                field.value_type.as_str(),
                "boolean" | "integer" | "number" | "string"
            )
        {
            return Err(AccountError::Unavailable);
        }
    }
    Ok(())
}

/// Merge a host's supported values into a previously downloaded cloud
/// snapshot. Unknown platform fields are intentionally retained, while a
/// caller that tries to write an unknown or incorrectly typed field is rejected.
pub fn merge_account_preferences(
    base: &AccountPreferences,
    replacing: &BTreeMap<String, AccountPreferenceValue>,
    schema: &AccountPreferenceSchema,
) -> Result<AccountPreferences, AccountError> {
    validate_account_preferences(base)?;
    validate_preference_schema(schema)?;
    if base.revision < 0 {
        return Err(AccountError::Invalid);
    }
    let mut settings = base.settings.clone();
    for (key, value) in replacing {
        let field = schema.fields.get(key).ok_or(AccountError::Invalid)?;
        if field.value_type != value.kind()
            && !(field.value_type == "number" && value.kind() == "integer")
        {
            return Err(AccountError::Invalid);
        }
        settings.insert(key.clone(), value.clone());
    }
    let merged = AccountPreferences {
        revision: base.revision,
        settings,
    };
    validate_account_preferences(&merged)?;
    let bytes = serde_json::to_vec(&merged).map_err(|_| AccountError::Invalid)?;
    if bytes.len() > schema.maximum_bytes.min(MAX_JSON_BYTES) {
        return Err(AccountError::Invalid);
    }
    Ok(merged)
}

pub(super) fn valid_preference_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ACCOUNT_PREFERENCE_KEY_BYTES
        && crate::is_ascii_identifier_with_dots(value)
}

pub(super) fn validate_provider_target(provider: &str, target: &str) -> Result<(), AccountError> {
    if provider == "apple" {
        return target.is_empty().then_some(()).ok_or(AccountError::Invalid);
    }
    if provider == "google" {
        return valid_google_loopback_target(target)
            .then_some(())
            .ok_or(AccountError::Invalid);
    }
    if provider == "anonymous" {
        return super::anonymous::valid_anonymous_subject(target)
            .then_some(())
            .ok_or(AccountError::Invalid);
    }
    if !matches!(provider, "email" | "phone")
        || target.is_empty()
        || !crate::text::is_bounded_text(target, 320)
        || target.trim() != target
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_challenge(challenge: &AccountChallenge) -> Result<(), AccountError> {
    if challenge.challenge_id.is_empty()
        || !crate::text::is_bounded_text(&challenge.challenge_id, 256)
        || challenge.expires_in == 0
        || challenge
            .nonce
            .as_ref()
            .is_some_and(|value| !crate::text::is_bounded_text(value, 4096))
        || challenge
            .authorization_url
            .as_ref()
            .is_some_and(|value| !crate::text::is_bounded_text(value, 4096))
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

pub(super) fn validate_login(challenge: &str, credential: &str) -> Result<(), AccountError> {
    if challenge.is_empty()
        || !crate::text::is_bounded_text(challenge, 256)
        || credential.len() != 6
        || !crate::is_ascii_digits(credential)
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_apple_login(challenge: &str, credential: &str) -> Result<(), AccountError> {
    if challenge.is_empty()
        || !crate::text::is_bounded_text(challenge, 256)
        || credential.is_empty()
        || !crate::text::is_bounded_text(credential, 16 * 1024)
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

/// A Google authorization code arrives on the loopback redirect and goes straight to the backend, which exchanges it with the desktop client secret. Codes are opaque printable ASCII, so anything else is refused before it leaves the process.
pub(super) fn validate_google_login(challenge: &str, credential: &str) -> Result<(), AccountError> {
    if challenge.is_empty()
        || !crate::text::is_bounded_text(challenge, 256)
        || credential.is_empty()
        || credential.len() > 2048
        || !credential.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

/// The transport check for `/v1/auth/login`. The session has already applied the provider's own rule (six digits, an Apple identity token or a Google authorization code), so the client accepts a credential that satisfies any of them.
pub(super) fn validate_login_request(
    challenge: &str,
    credential: &str,
) -> Result<(), AccountError> {
    validate_login(challenge, credential)
        .or_else(|_| validate_apple_login(challenge, credential))
        .or_else(|_| validate_google_login(challenge, credential))
}

pub(super) fn validate_display_name(value: &str) -> Result<(), AccountError> {
    if value.is_empty() || value.trim() != value || !crate::text::is_bounded_chars(value, 64) {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_chat_models(value: &AccountChatModels) -> Result<(), AccountError> {
    if !crate::account::valid_model_catalog(
        value.data.iter().map(|model| model.id.as_str()),
        &value.default_model,
        MAX_CHAT_MODELS,
        MAX_CHAT_MODEL_ID_BYTES,
    ) {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

pub(super) fn validate_chat_request(
    messages: &[AccountChatMessage],
    model: &str,
) -> Result<(), AccountError> {
    if model.is_empty()
        || !crate::text::is_bounded_text(model, MAX_CHAT_MODEL_ID_BYTES)
        || messages.is_empty()
        || messages.len() > MAX_CHAT_MESSAGES
        || messages.iter().any(|message| {
            !matches!(message.role.as_str(), "user" | "assistant" | "system")
                || message.content.trim().is_empty()
                || message.content.len() > MAX_CHAT_MESSAGE_BYTES
                || crate::text::has_disallowed_control(&message.content)
        })
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_tokens(tokens: &AccountTokens) -> Result<(), AccountError> {
    if tokens.token_type != "Bearer"
        || tokens.expires_in == 0
        || tokens.expires_in > MAX_SESSION_SECONDS
        || !crate::text::is_lower_hex(&tokens.access_token, 64)
        || !crate::text::is_lower_hex(&tokens.refresh_token, 64)
    {
        return Err(AccountError::Unavailable);
    }
    validate_user(&tokens.user).map_err(|_| AccountError::Unavailable)
}

pub(super) fn validate_user(user: &AccountUser) -> Result<(), AccountError> {
    validate_identity(&AccountIdentity {
        user_id: user.id.clone(),
    })
    .map_err(|_| AccountError::Invalid)?;
    if !crate::text::is_bounded_chars(&user.display_name, 64)
        || !crate::text::is_bounded_text(&user.created_at, 128)
        || user
            .email
            .as_ref()
            .is_some_and(|email| !crate::text::is_bounded_text(email, 320))
        || user
            .avatar_url
            .as_ref()
            .is_some_and(|url| !crate::text::is_bounded_text(url, 2048))
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

pub(super) fn validate_profile(profile: &AccountProfile) -> Result<(), AccountError> {
    validate_user(&profile.user)?;
    if profile.identities.len() > 16
        || profile.identities.iter().any(|identity| {
            identity.provider.is_empty()
                || identity.provider.len() > 32
                || !identity
                    .provider
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_' || byte == b'-')
                || !crate::text::is_bounded_text(&identity.subject, 512)
        })
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

pub fn validate_identity(identity: &AccountIdentity) -> Result<(), &'static str> {
    if identity.user_id.is_empty() || !crate::text::is_bounded_text(&identity.user_id, 256) {
        return Err("invalid account identity");
    }
    Ok(())
}
