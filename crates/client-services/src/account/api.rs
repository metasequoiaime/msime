//! The two traits the platform host implements: the backend calls themselves
//! and somewhere to keep a signed-in session.

use super::*;

pub trait AccountApi: Send + Sync + 'static {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError>;
    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError>;
    fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError>;
    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError>;
    fn profile(&self, access_token: &str) -> Result<AccountProfile, AccountError>;
    fn rename(&self, display_name: &str, access_token: &str) -> Result<(), AccountError>;
    fn logout(&self, access_token: &str, all: bool) -> Result<(), AccountError>;
    fn delete_account(&self, access_token: &str) -> Result<(), AccountError>;

    /// Replaces the user's uploaded avatar with `image`, a PNG or JPEG already read by [`read_account_avatar_upload`].
    fn upload_avatar(
        &self,
        _image: &AccountAvatarImage,
        _access_token: &str,
    ) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    /// Removes the user's uploaded avatar; the Google picture, if any, shows again.
    fn delete_avatar(&self, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn chat_models(&self, _access_token: &str) -> Result<AccountChatModels, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn chat(
        &self,
        _messages: &[AccountChatMessage],
        _model: &str,
        _access_token: &str,
    ) -> Result<String, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn preference_schema(
        &self,
        _access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn preferences(&self, _access_token: &str) -> Result<AccountPreferences, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn put_preferences(
        &self,
        _preferences: &AccountPreferences,
        _access_token: &str,
    ) -> Result<AccountPreferences, AccountError> {
        Err(AccountError::Unavailable)
    }

    /// 发出云剪贴板驱动要求的一个请求（`docs/cloud-clipboard-e2e.md` 的「客户端驱动」），带上登录凭据。服务端的任何响应，包括非 2xx，都原样交回状态码和正文，由驱动解读；只有没拿到响应（断网、超时、超长）才是错误。
    fn clipboard_e2e(
        &self,
        _request: &crate::cloud::clipboard_e2e::HttpRequest,
        _access_token: &str,
    ) -> Result<crate::cloud::clipboard_e2e::HttpResponse, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn set_clipboard_enabled(
        &self,
        _enabled: bool,
        _access_token: &str,
    ) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn delete_clipboard(&self, _id: Option<&str>, _access_token: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn dictionary(
        &self,
        _kind: DictionaryKind,
        _search: &str,
        _offset: usize,
        _access_token: &str,
    ) -> Result<AccountDictionaryPage, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn dictionary_catalog(
        &self,
        _kind: DictionaryKind,
        _code: &str,
        _offset: usize,
        _scheme: &str,
        _profile: &str,
        _access_token: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        Err(AccountError::Unavailable)
    }

    #[allow(clippy::too_many_arguments)]
    fn edit_dictionary_catalog(
        &self,
        _kind: DictionaryKind,
        _code: &str,
        _word: &str,
        _revision: i64,
        _replacement: Option<(&str, &str, i64)>,
        _access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn personal_candidates(
        &self,
        _query: &AccountCandidateQuery,
        _access_token: &str,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        Err(AccountError::Unavailable)
    }

    #[allow(clippy::too_many_arguments)]
    fn rank_candidate(
        &self,
        _query: &AccountCandidateQuery,
        _code: &str,
        _word: &str,
        _revision: i64,
        _mode: &str,
        _linear_step: i64,
        _trigger_count: i64,
        _force_top: bool,
        _access_token: &str,
    ) -> Result<AccountRankingResult, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn remove_candidate(
        &self,
        _query: &AccountCandidateQuery,
        _code: &str,
        _word: &str,
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn fixed_positions(
        &self,
        _context: &str,
        _offset: usize,
        _access_token: &str,
    ) -> Result<AccountFixedPositions, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn set_fixed_position(
        &self,
        _context: &str,
        _code: &str,
        _word: &str,
        _position: Option<i64>,
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn add_dictionary(
        &self,
        _kind: DictionaryKind,
        _code: &str,
        _word: &str,
        _weight: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        Err(AccountError::Unavailable)
    }

    #[allow(clippy::too_many_arguments)]
    fn update_dictionary(
        &self,
        _kind: DictionaryKind,
        _id: &str,
        _code: &str,
        _word: &str,
        _weight: i64,
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn delete_dictionary(
        &self,
        _kind: DictionaryKind,
        _id: &str,
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn import_dictionary(
        &self,
        _kind: DictionaryKind,
        _format: &str,
        _text: &str,
        _access_token: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn export_dictionary(
        &self,
        _kind: DictionaryKind,
        _format: &str,
        _access_token: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn dictionary_changes(
        &self,
        _after: i64,
        _limit: usize,
        _access_token: &str,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn dictionary_snapshot(&self, _access_token: &str) -> Result<Vec<u8>, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn dictionary_snapshot_to_file(
        &self,
        _destination: &Path,
        _access_token: &str,
    ) -> Result<u64, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn restore_dictionary_snapshot(
        &self,
        _snapshot: &[u8],
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        Err(AccountError::Unavailable)
    }

    // 以下是 #8201 补上的接口。它们返回带状态码、服务端错误码和 `Retry-After` 的 [`AccountFailure`]，默认实现都是 `account_unavailable`，不影响已有的实现者。

    /// `POST /v1/translate`，见 [`translate_request_body`] 和 [`parse_translate_response`]。
    fn translate(
        &self,
        _request: &AccountTranslateRequest,
        _timeout: Duration,
        _access_token: &str,
    ) -> Result<AccountTranslations, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `GET /v1/users/me/phrases`。
    fn phrases(&self, _access_token: &str) -> Result<AccountPhrases, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `PUT /v1/users/me/phrases`：按 `phrases.revision` 比较并交换，整份替换。
    fn put_phrases(
        &self,
        _phrases: &AccountPhrases,
        _access_token: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `GET /v1/users/me/sessions`。
    fn devices(&self, _access_token: &str) -> Result<AccountDevices, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `DELETE /v1/users/me/sessions/{id}`。
    fn revoke_device(&self, _id: &str, _access_token: &str) -> Result<(), AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `GET /v1/users/me/data`。
    fn data_summary(&self, _access_token: &str) -> Result<AccountDataSummary, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `GET /v1/users/me/data/export`，流式写到 `destination`。
    fn data_export(
        &self,
        _destination: &Path,
        _access_token: &str,
    ) -> Result<AccountDataExport, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `DELETE /v1/users/me/data`。
    fn data_delete(&self, _sections: &[String], _access_token: &str) -> Result<(), AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `POST /v1/feedback`，`feedback` 由 [`prepare_feedback`] 校验过。
    fn feedback(
        &self,
        _feedback: &AccountFeedback,
        _access_token: &str,
    ) -> Result<AccountFeedbackReceipt, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `POST /v1/users/me/diagnostics`，`upload` 由 [`crate::diagnostics::upload::prepare_upload`] 重建过。
    fn diagnostics_upload(
        &self,
        _upload: &crate::diagnostics::upload::DiagnosticUpload,
        _access_token: &str,
    ) -> Result<crate::diagnostics::upload::DiagnosticUploadReceipt, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `GET /v1/users/me/diagnostics`。
    fn diagnostics_state(
        &self,
        _access_token: &str,
    ) -> Result<crate::diagnostics::upload::DiagnosticState, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `POST /v1/users/me/diagnostics/token`：旧令牌立即失效。
    fn diagnostics_regenerate_token(&self, _access_token: &str) -> Result<String, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// `DELETE /v1/users/me/diagnostics`。
    fn diagnostics_delete(&self, _access_token: &str) -> Result<(), AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    // 以下几项给云同步（`cloud_sync`）用：与上面同名的接口相同，失败时带着状态码、服务端错误码和 `Retry-After`，同步据此区分 409 `revision_conflict` 和别的 409，并尊重限流。默认实现转调不带细节的那个，只实现了旧接口的假后端照样能用。

    /// 与 [`Self::preference_schema`] 相同，失败时是 [`AccountFailure`]。
    fn preference_schema_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountFailure> {
        self.preference_schema(access_token).map_err(Into::into)
    }

    /// 与 [`Self::preferences`] 相同，失败时是 [`AccountFailure`]。
    fn preferences_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountFailure> {
        self.preferences(access_token).map_err(Into::into)
    }

    /// 与 [`Self::put_preferences`] 相同，失败时是 [`AccountFailure`]。
    fn put_preferences_detailed(
        &self,
        preferences: &AccountPreferences,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountFailure> {
        self.put_preferences(preferences, access_token)
            .map_err(Into::into)
    }

    /// `GET /v1/users/me/dictionary/changes?after=&limit=1`：云端词库自 `after` 以来有没有改动，以及第一条改动是不是 `reset`（整份替换或清空）。
    fn dictionary_probe(
        &self,
        _after: i64,
        _access_token: &str,
    ) -> Result<AccountDictionaryProbe, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }

    /// 与 [`Self::dictionary_snapshot_to_file`] 相同，失败时是 [`AccountFailure`]。
    fn dictionary_snapshot_download(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountFailure> {
        self.dictionary_snapshot_to_file(destination, access_token)
            .map_err(Into::into)
    }

    /// `PUT /v1/users/me/dictionary/snapshot?revision=N`，正文从 `snapshot`（宿主私有的绝对路径）流式读出；按 `revision` 比较并交换。
    fn dictionary_snapshot_upload(
        &self,
        _snapshot: &Path,
        _revision: i64,
        _access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountFailure> {
        Err(AccountError::Unavailable.into())
    }
}

pub trait AccountSessionStorage: Send + Sync + 'static {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError>;
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError>;
    fn clear(&self) -> Result<(), AccountError>;

    /// 本机保存的云剪贴板密钥串（`cloud::clipboard_e2e::Keystore` 的编码，对宿主不透明），还没有时为 `None`。每台设备只有一份，与会话放在同一个安全存储里，[`Self::clear`] 时一起删除；位置见 `docs/cloud-clipboard-e2e.md` 的「密钥串的存放位置」。不提供它的存储用不了云剪贴板。
    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        Err(AccountError::Storage)
    }

    /// 原子替换本机的云剪贴板密钥串。驱动每交回一个变化了的值就调用一次，在发出下一个请求之前。
    fn save_clipboard_keystore(&self, _value: &str) -> Result<(), AccountError> {
        Err(AccountError::Storage)
    }

    /// Whether other processes read and write this store. The backend rotates the refresh token on every refresh and revokes the whole session when a used one is presented again, so a session that holds a shared store in memory would refresh from a token another process already spent. A shared store is therefore read on every access, and refreshed only under [`Self::with_refresh_lock`].
    fn shared_across_processes(&self) -> bool {
        false
    }

    /// Runs `body` holding the lock every process sharing this store takes around a refresh, a sign-in and a sign-out, so none of them can rotate the session between another's read and its save. A store no other process opens needs no lock.
    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError>
    where
        Self: Sized,
    {
        body()
    }
}
