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

    fn clipboard(
        &self,
        _search: &str,
        _access_token: &str,
    ) -> Result<AccountClipboardPage, AccountError> {
        Err(AccountError::Unavailable)
    }

    fn set_clipboard_enabled(
        &self,
        _enabled: bool,
        _access_token: &str,
    ) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }

    fn add_clipboard(
        &self,
        _text: &str,
        _access_token: &str,
    ) -> Result<AccountClipboardItem, AccountError> {
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
}

pub trait AccountSessionStorage: Send + Sync + 'static {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError>;
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError>;
    fn clear(&self) -> Result<(), AccountError>;

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
