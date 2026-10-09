//! `BackendAccountSession`: token refresh, single-flight, and the saved session
//! the host persists.

use super::google::*;
use super::validate::*;
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct SessionState {
    loaded: bool,
    saved: Option<SavedAccountSession>,
    generation: u64,
    refresh: Option<Arc<RefreshFlight>>,
}

struct RefreshFlight {
    result: Mutex<Option<Result<String, AccountError>>>,
    ready: Condvar,
}

impl RefreshFlight {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    fn finish(&self, result: Result<String, AccountError>) {
        if let Ok(mut slot) = self.result.lock() {
            *slot = Some(result);
            self.ready.notify_all();
        }
    }

    fn wait(&self) -> Result<String, AccountError> {
        let mut slot = self.result.lock().map_err(|_| AccountError::Unavailable)?;
        while slot.is_none() {
            slot = self
                .ready
                .wait(slot)
                .map_err(|_| AccountError::Unavailable)?;
        }
        slot.clone().ok_or(AccountError::Unavailable)?
    }
}

pub struct BackendAccountSession<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    storage: S,
    state: Mutex<SessionState>,
    /// Cancel flag of the Google browser sign-in in progress, if any. It lives outside `state` because the sign-in waits on the browser for minutes without holding the session lock.
    google_sign_in: Mutex<Option<Arc<AtomicBool>>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    pub fn new(api: A, storage: S) -> Self {
        Self {
            api,
            storage,
            state: Mutex::new(SessionState {
                loaded: false,
                saved: None,
                generation: 0,
                refresh: None,
            }),
            google_sign_in: Mutex::new(None),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, SessionState>, AccountError> {
        self.state.lock().map_err(|_| AccountError::Unavailable)
    }

    #[cfg(test)]
    pub(crate) fn set_generation_for_test(&self, generation: u64) {
        self.state.lock().unwrap().generation = generation;
    }

    /// Reserve an identity for an operation that may complete asynchronously.
    /// The terminal value is never handed to such an operation: once it is
    /// reached, there is no later value available to invalidate it on logout.
    fn next_generation(state: &mut SessionState) -> Result<u64, AccountError> {
        let next = state
            .generation
            .checked_add(1)
            .filter(|&generation| generation < u64::MAX)
            .ok_or(AccountError::Unavailable)?;
        state.generation = next;
        Ok(next)
    }

    /// A store other processes share is read every time: another process may have refreshed, signed out or switched accounts, and the copy held here would then be stale. A refresh from a stale copy presents a refresh token the backend has already rotated, and the backend answers that by revoking the session for every process.
    fn load_locked(&self, state: &mut SessionState) -> Result<(), AccountError> {
        if !state.loaded || self.storage.shared_across_processes() {
            state.saved = self.stored()?;
            state.loaded = true;
        }
        Ok(())
    }

    fn stored(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        let saved = self.storage.load()?;
        if let Some(value) = &saved {
            validate_saved_session(value)?;
        }
        Ok(saved)
    }

    pub fn status(&self) -> Result<Option<AccountUser>, AccountError> {
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        Ok(state.saved.as_ref().map(|saved| saved.tokens.user.clone()))
    }

    pub fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        self.api.providers()
    }

    pub fn request_code(
        &self,
        provider: &str,
        target: &str,
    ) -> Result<AccountChallenge, AccountError> {
        validate_provider_target(provider, target)?;
        self.api.challenge(provider, target)
    }

    pub fn sign_in(&self, challenge: &str, credential: &str) -> Result<AccountUser, AccountError> {
        validate_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential)
    }

    /// Signs in the device's anonymous account: the subject is presented as the challenge target and the secret answers it. Only `anonymous::ensure_anonymous_account` holds those values.
    pub(super) fn sign_in_anonymous(
        &self,
        subject: &str,
        secret: &str,
    ) -> Result<AccountUser, AccountError> {
        let challenge = self.request_code("anonymous", subject)?;
        self.sign_in_validated(&challenge.challenge_id, secret)
    }

    /// Completes an Apple challenge using the identity token returned by the
    /// native AuthenticationServices flow. The token never crosses the UI
    /// boundary; platform hosts pass it directly into the session.
    pub fn sign_in_apple(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        validate_apple_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential)
    }

    /// Completes a Google challenge with the authorization code the system browser delivered to the loopback redirect. The backend holds the PKCE verifier and the client secret and performs the exchange; this process only forwards the code.
    pub fn sign_in_google(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        validate_google_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential)
    }

    /// Runs the desktop Google sign-in (RFC 8252 loopback redirect): binds a loopback listener, requests a challenge for its redirect URI, hands the backend's authorization URL to `open_browser`, waits for the redirect, and signs in with the returned code. `open_browser` receives a URL already checked to be a Google authorization URL for this listener. The wait lasts at most [`GOOGLE_SIGN_IN_TIMEOUT`] and ends early enough for the code to reach the backend before the challenge expires; [`Self::cancel_google_sign_in`] or starting another Google sign-in ends it with [`AccountError::Cancelled`].
    pub fn sign_in_google_with_browser<F>(
        &self,
        open_browser: F,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str) -> Result<(), AccountError>,
    {
        self.sign_in_google_with_timeout(open_browser, GOOGLE_SIGN_IN_TIMEOUT)
    }

    pub(super) fn sign_in_google_with_timeout<F>(
        &self,
        open_browser: F,
        timeout: Duration,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str) -> Result<(), AccountError>,
    {
        let cancelled = Arc::new(AtomicBool::new(false));
        let previous = self
            .google_sign_in
            .lock()
            .map_err(|_| AccountError::Unavailable)?
            .replace(Arc::clone(&cancelled));
        if let Some(previous) = previous {
            previous.store(true, Ordering::SeqCst);
        }
        let result = self.run_google_sign_in(open_browser, timeout, &cancelled);
        if let Ok(mut current) = self.google_sign_in.lock() {
            if current
                .as_ref()
                .is_some_and(|flag| Arc::ptr_eq(flag, &cancelled))
            {
                *current = None;
            }
        }
        result
    }

    /// Ends the Google browser sign-in in progress, which then returns [`AccountError::Cancelled`]. The browser tab cannot report that the user closed it, so the page offers this instead of leaving the user waiting for the timeout. Does nothing when no Google sign-in is running.
    pub fn cancel_google_sign_in(&self) {
        if let Ok(current) = self.google_sign_in.lock() {
            if let Some(flag) = current.as_ref() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    fn run_google_sign_in<F>(
        &self,
        open_browser: F,
        timeout: Duration,
        cancelled: &AtomicBool,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str) -> Result<(), AccountError>,
    {
        let requested = std::time::Instant::now();
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| AccountError::Unavailable)?;
        let port = listener
            .local_addr()
            .map_err(|_| AccountError::Unavailable)?
            .port();
        let target = google_loopback_target(port);
        let challenge = self.request_code("google", &target)?;
        let url = challenge
            .authorization_url
            .as_deref()
            .ok_or(AccountError::Unavailable)?;
        let state = google_authorization_state(url, &target)?;
        // The challenge clock started when the backend created it, so the wait is measured from before the request.
        let window = google_callback_window(challenge.expires_in, timeout)
            .ok_or(AccountError::Unavailable)?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(AccountError::Cancelled);
        }
        open_browser(url)?;
        let code = receive_google_callback(&listener, &state, requested + window, cancelled)?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(AccountError::Cancelled);
        }
        self.sign_in_google(&challenge.challenge_id, &code)
    }

    fn sign_in_validated(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        let version = {
            let mut state = self.lock()?;
            let version = Self::next_generation(&mut state)?;
            state.refresh = None;
            version
        };
        let tokens = self.api.login(challenge, credential)?;
        validate_tokens(&tokens)?;
        let value = saved_session(tokens)?;
        let user = value.tokens.user.clone();
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            if state.generation != version {
                return Err(AccountError::Cancelled);
            }
            self.storage.save(&value)?;
            state.saved = Some(value);
            state.loaded = true;
            Ok(user)
        })
    }

    pub fn access_token(&self, rejected_token: Option<&str>) -> Result<String, AccountError> {
        let (flight, version, refresh_token) = {
            let mut state = self.lock()?;
            self.load_locked(&mut state)?;
            let current = state.saved.as_ref().ok_or(AccountError::Unauthorized)?;
            if current.expires_at_unix_ms > refresh_deadline_ms()
                && rejected_token != Some(current.tokens.access_token.as_str())
            {
                return Ok(current.tokens.access_token.clone());
            }
            if let Some(flight) = &state.refresh {
                let flight = Arc::clone(flight);
                drop(state);
                return flight.wait();
            }
            if state.generation == u64::MAX {
                return Err(AccountError::Unavailable);
            }
            let version = state.generation;
            let refresh_token = current.tokens.refresh_token.clone();
            let flight = Arc::new(RefreshFlight::new());
            state.refresh = Some(Arc::clone(&flight));
            (flight, version, refresh_token)
        };

        let result = self.storage.with_refresh_lock(|| {
            self.refresh_holding_lock(&refresh_token, rejected_token, version)
        });
        if let Ok(mut state) = self.lock() {
            if state
                .refresh
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &flight))
            {
                state.refresh = None;
            }
        }
        flight.finish(result.clone());
        result
    }

    /// Runs with the store's refresh lock held, so no other process sharing the store can rotate the session between the read below and the save after the refresh.
    fn refresh_holding_lock(
        &self,
        expected: &str,
        rejected_token: Option<&str>,
        version: u64,
    ) -> Result<String, AccountError> {
        let shared = self.storage.shared_across_processes();
        let mut refresh_token = expected.to_owned();
        if shared {
            let stored = self.stored()?;
            let mut state = self.lock()?;
            if state.generation != version {
                return Err(AccountError::Cancelled);
            }
            match stored {
                // Another process signed out while this one waited for the lock.
                None => {
                    state.saved = None;
                    state.loaded = true;
                    return Err(AccountError::Unauthorized);
                }
                // Another process refreshed while this one waited; refreshing from the token it already spent would revoke the session.
                Some(stored) if stored.tokens.refresh_token != expected => {
                    let usable = usable_session(&stored, rejected_token);
                    let access_token = stored.tokens.access_token.clone();
                    refresh_token = stored.tokens.refresh_token.clone();
                    state.saved = Some(stored);
                    state.loaded = true;
                    if usable {
                        return Ok(access_token);
                    }
                }
                Some(_) => {}
            }
        }

        let api_result = self.api.refresh(&refresh_token);
        let mut state = self.lock()?;
        if state.generation != version {
            return Err(AccountError::Cancelled);
        }
        match api_result {
            Ok(tokens) => {
                validate_tokens(&tokens)?;
                let value = saved_session(tokens)?;
                if shared {
                    // A writer that does not take the lock can still change the store. Tokens for a session that is no longer the stored one are discarded rather than resurrecting it.
                    match self.stored()? {
                        None => {
                            state.saved = None;
                            state.loaded = true;
                            return Err(AccountError::Unauthorized);
                        }
                        Some(current)
                            if current.tokens.refresh_token != refresh_token
                                || current.tokens.user.id != value.tokens.user.id =>
                        {
                            state.saved = Some(current);
                            state.loaded = true;
                            return Err(AccountError::Cancelled);
                        }
                        Some(_) => {}
                    }
                }
                self.storage.save(&value)?;
                let token = value.tokens.access_token.clone();
                state.saved = Some(value);
                state.loaded = true;
                Ok(token)
            }
            Err(AccountError::Unauthorized) => {
                // Clear only the session that was rejected; one another process saved meanwhile is adopted instead.
                if shared {
                    if let Ok(Some(stored)) = self.stored() {
                        if stored.tokens.refresh_token != refresh_token {
                            let usable = usable_session(&stored, rejected_token);
                            let access_token = stored.tokens.access_token.clone();
                            state.saved = Some(stored);
                            state.loaded = true;
                            return if usable {
                                Ok(access_token)
                            } else {
                                Err(AccountError::Unauthorized)
                            };
                        }
                    }
                }
                state.saved = None;
                state.loaded = true;
                self.storage.clear().and(Err(AccountError::Unauthorized))
            }
            Err(error) => Err(error),
        }
    }

    pub fn credentials(
        &self,
        rejected_token: Option<&str>,
        expected_user_id: Option<&str>,
    ) -> Result<(String, String), AccountError> {
        self.credentials_with_generation(rejected_token, expected_user_id)
            .map(|(user_id, token, _)| (user_id, token))
    }

    /// Returns credentials together with the session generation that authorized them.
    ///
    /// Hosts that perform a local write after reading account data use the generation to reject a
    /// sign-out and re-login of the same user. The user id alone cannot distinguish those two
    /// sessions.
    pub fn credentials_with_generation(
        &self,
        rejected_token: Option<&str>,
        expected_user_id: Option<&str>,
    ) -> Result<(String, String, u64), AccountError> {
        {
            let mut state = self.lock()?;
            self.load_locked(&mut state)?;
            if expected_user_id.is_some_and(|expected| {
                state
                    .saved
                    .as_ref()
                    .map(|saved| saved.tokens.user.id.as_str())
                    != Some(expected)
            }) {
                return Err(AccountError::Cancelled);
            }
        }
        let token = self.access_token(rejected_token)?;
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        let saved = state.saved.as_ref().ok_or(AccountError::Cancelled)?;
        if saved.tokens.access_token != token
            || expected_user_id.is_some_and(|expected| saved.tokens.user.id != expected)
        {
            return Err(AccountError::Cancelled);
        }
        Ok((saved.tokens.user.id.clone(), token, state.generation))
    }

    /// Runs a local side effect while the account generation is held stable.
    ///
    /// The session mutex remains held for `operation`, so logout or a new login cannot invalidate
    /// the check between validation and the host's write. The closure must not call this session
    /// again; it is intended for platform preference stores and other local account state.
    pub fn with_generation<T, F>(
        &self,
        generation: u64,
        expected_user_id: Option<&str>,
        operation: F,
    ) -> Result<T, AccountError>
    where
        F: FnOnce() -> Result<T, AccountError>,
    {
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        if state.generation != generation
            || expected_user_id.is_some_and(|expected| {
                state
                    .saved
                    .as_ref()
                    .map(|saved| saved.tokens.user.id.as_str())
                    != Some(expected)
            })
        {
            return Err(AccountError::Cancelled);
        }
        operation()
    }

    pub fn profile(&self) -> Result<AccountProfile, AccountError> {
        let (user_id, profile, generation) =
            self.authenticated_with_user(|api, token| api.profile(token))?;
        validate_profile(&profile)?;
        if profile.user.id != user_id {
            return Err(AccountError::Cancelled);
        }
        self.update_user(profile.user.clone(), generation)?;
        Ok(profile)
    }

    pub fn rename(&self, display_name: &str) -> Result<AccountProfile, AccountError> {
        validate_display_name(display_name)?;
        self.authenticated(|api, token| api.rename(display_name, token))?;
        self.profile()
    }

    /// Uploads the PNG or JPEG at `path` as the user's avatar and returns the refreshed profile, whose `avatar_url` now names it.
    pub fn upload_avatar(&self, path: &Path) -> Result<AccountProfile, AccountError> {
        let image = read_account_avatar_upload(path)?;
        self.authenticated(|api, token| api.upload_avatar(&image, token))?;
        self.profile()
    }

    /// Removes the user's uploaded avatar and returns the refreshed profile.
    pub fn remove_avatar(&self) -> Result<AccountProfile, AccountError> {
        self.authenticated(|api, token| api.delete_avatar(token))?;
        self.profile()
    }

    /// The signed-in user's avatar, or `None` when they are signed out or have none. Reads the saved user, so it needs no backend round trip beyond the image itself.
    pub fn avatar(&self) -> Result<Option<AccountAvatarImage>, AccountError> {
        match self.status()?.and_then(|user| user.avatar_url) {
            Some(url) => fetch_account_avatar(&url).map(Some),
            None => Ok(None),
        }
    }

    pub fn logout(&self, all: bool) -> Result<(), AccountError> {
        let token = match self.access_token(None) {
            Ok(token) => token,
            Err(error) => {
                self.forget()?;
                return Err(error);
            }
        };
        self.forget()?;
        self.api.logout(&token, all)
    }

    pub fn delete_account(&self) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.delete_account(token))?;
        self.forget()
    }

    pub fn chat_models(&self) -> Result<AccountChatModels, AccountError> {
        self.authenticated(|api, token| api.chat_models(token))
    }

    pub fn chat(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
    ) -> Result<String, AccountError> {
        self.authenticated(|api, token| api.chat(messages, model, token))
    }

    fn authenticated<T, F>(&self, operation: F) -> Result<T, AccountError>
    where
        F: Fn(&A, &str) -> Result<T, AccountError>,
    {
        self.authenticated_with_user(operation)
            .map(|(_, result, _)| result)
    }

    fn authenticated_with_user<T, F>(&self, operation: F) -> Result<(String, T, u64), AccountError>
    where
        F: Fn(&A, &str) -> Result<T, AccountError>,
    {
        let (user_id, token, generation) = self.credentials_with_generation(None, None)?;
        let result = match operation(&self.api, &token) {
            Err(AccountError::Unauthorized) => {
                let (_, replacement) = self.credentials(Some(&token), Some(&user_id))?;
                operation(&self.api, &replacement)
            }
            result => result,
        }?;
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        if state.generation != generation
            || state
                .saved
                .as_ref()
                .map(|saved| saved.tokens.user.id.as_str())
                != Some(user_id.as_str())
        {
            return Err(AccountError::Cancelled);
        }
        Ok((user_id, result, generation))
    }

    pub fn preference_schema(&self) -> Result<AccountPreferenceSchema, AccountError> {
        let schema = self.authenticated(|api, token| api.preference_schema(token))?;
        validate_preference_schema(&schema)?;
        Ok(schema)
    }

    pub fn preferences(&self) -> Result<AccountPreferences, AccountError> {
        let preferences = self.authenticated(|api, token| api.preferences(token))?;
        validate_account_preferences(&preferences)?;
        Ok(preferences)
    }

    pub fn put_preferences(
        &self,
        preferences: &AccountPreferences,
    ) -> Result<AccountPreferences, AccountError> {
        validate_account_preferences(preferences)?;
        let updated = self.authenticated(|api, token| api.put_preferences(preferences, token))?;
        validate_account_preferences(&updated)?;
        Ok(updated)
    }

    /// Uploads preferences only for the generation that read them.
    ///
    /// A same-user re-login has a different generation even though its user id is unchanged. The
    /// ordinary method is still correct for a fresh request, while this variant protects a
    /// read/merge/write sequence owned by a settings page.
    pub fn put_preferences_with_generation(
        &self,
        preferences: &AccountPreferences,
        expected_generation: u64,
        expected_user_id: &str,
    ) -> Result<AccountPreferences, AccountError> {
        validate_account_preferences(preferences)?;
        let (user_id, token, generation) =
            self.credentials_with_generation(None, Some(expected_user_id))?;
        if generation != expected_generation || user_id != expected_user_id {
            return Err(AccountError::Cancelled);
        }
        let updated = match self.api.put_preferences(preferences, &token) {
            Err(AccountError::Unauthorized) => {
                let (_, replacement, replacement_generation) =
                    self.credentials_with_generation(Some(&token), Some(expected_user_id))?;
                if replacement_generation != expected_generation {
                    return Err(AccountError::Cancelled);
                }
                self.api.put_preferences(preferences, &replacement)?
            }
            result => result?,
        };
        validate_account_preferences(&updated)?;
        let mut state = self.lock()?;
        // 移动端会话文件与键盘进程共享，接受云端写入前重新读取，避免另一进程退出账号被本地缓存遮住。
        self.load_locked(&mut state)?;
        if state.generation != expected_generation
            || state
                .saved
                .as_ref()
                .map(|saved| saved.tokens.user.id.as_str())
                != Some(expected_user_id)
        {
            return Err(AccountError::Cancelled);
        }
        Ok(updated)
    }

    pub fn clipboard(&self, search: &str) -> Result<AccountClipboardPage, AccountError> {
        self.authenticated(|api, token| api.clipboard(search, token))
    }

    pub fn set_clipboard_enabled(&self, enabled: bool) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.set_clipboard_enabled(enabled, token))
    }

    pub fn add_clipboard(&self, text: &str) -> Result<AccountClipboardItem, AccountError> {
        self.authenticated(|api, token| api.add_clipboard(text, token))
    }

    pub fn delete_clipboard(&self, id: Option<&str>) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.delete_clipboard(id, token))
    }

    pub fn dictionary(
        &self,
        kind: DictionaryKind,
        search: &str,
        offset: usize,
    ) -> Result<AccountDictionaryPage, AccountError> {
        self.authenticated(|api, token| api.dictionary(kind, search, offset, token))
    }

    pub fn dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        offset: usize,
        scheme: &str,
        profile: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        self.authenticated(|api, token| {
            api.dictionary_catalog(kind, code, offset, scheme, profile, token)
        })
    }

    pub fn dictionary_changes(
        &self,
        after: i64,
        limit: usize,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        self.authenticated(|api, token| api.dictionary_changes(after, limit, token))
    }

    pub fn dictionary_snapshot(&self) -> Result<Vec<u8>, AccountError> {
        self.authenticated(|api, token| api.dictionary_snapshot(token))
    }

    pub fn dictionary_snapshot_to_file(&self, destination: &Path) -> Result<u64, AccountError> {
        self.authenticated(|api, token| api.dictionary_snapshot_to_file(destination, token))
    }

    pub fn restore_dictionary_snapshot(
        &self,
        snapshot: &[u8],
        revision: i64,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        self.authenticated(|api, token| api.restore_dictionary_snapshot(snapshot, revision, token))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn edit_dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        revision: i64,
        replacement: Option<(&str, &str, i64)>,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.authenticated(|api, token| {
            api.edit_dictionary_catalog(kind, code, word, revision, replacement, token)
        })
    }

    pub fn personal_candidates(
        &self,
        query: &AccountCandidateQuery,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        self.authenticated(|api, token| api.personal_candidates(query, token))
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
    ) -> Result<AccountRankingResult, AccountError> {
        self.authenticated(|api, token| {
            api.rank_candidate(
                query,
                code,
                word,
                revision,
                mode,
                linear_step,
                trigger_count,
                force_top,
                token,
            )
        })
    }

    pub fn remove_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.authenticated(|api, token| api.remove_candidate(query, code, word, revision, token))
    }

    pub fn fixed_positions(
        &self,
        context: &str,
        offset: usize,
    ) -> Result<AccountFixedPositions, AccountError> {
        self.authenticated(|api, token| api.fixed_positions(context, offset, token))
    }

    pub fn set_fixed_position(
        &self,
        context: &str,
        code: &str,
        word: &str,
        position: Option<i64>,
        revision: i64,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        self.authenticated(|api, token| {
            api.set_fixed_position(context, code, word, position, revision, token)
        })
    }

    pub fn add_dictionary(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        weight: i64,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.authenticated(|api, token| api.add_dictionary(kind, code, word, weight, token))
    }

    pub fn update_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        code: &str,
        word: &str,
        weight: i64,
        revision: i64,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.authenticated(|api, token| {
            api.update_dictionary(kind, id, code, word, weight, revision, token)
        })
    }

    pub fn delete_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        revision: i64,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.authenticated(|api, token| api.delete_dictionary(kind, id, revision, token))
    }

    pub fn import_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        text: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        self.authenticated(|api, token| api.import_dictionary(kind, format, text, token))
    }

    pub fn export_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        self.authenticated(|api, token| api.export_dictionary(kind, format, token))
    }

    pub fn forget(&self) -> Result<(), AccountError> {
        {
            let mut state = self.lock()?;
            // No asynchronous operation can be running at the terminal value:
            // next_generation refuses to issue it there. Keep the value stable
            // while still clearing the account state.
            state.generation = state.generation.saturating_add(1);
            state.refresh = None;
            state.saved = None;
            state.loaded = true;
        }
        // 必须在刷新锁内清理，避免另一个进程的刷新在退出后写回 token。拿不到锁时保持存储不变；无锁清理会与进行中的刷新竞争并恢复会话。
        self.storage.with_refresh_lock(|| self.storage.clear())
    }

    fn update_user(&self, user: AccountUser, generation: u64) -> Result<(), AccountError> {
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            self.load_locked(&mut state)?;
            if state.generation != generation {
                return Err(AccountError::Cancelled);
            }
            let current = state.saved.as_mut().ok_or(AccountError::Cancelled)?;
            if current.tokens.user.id != user.id {
                return Err(AccountError::Cancelled);
            }
            current.tokens.user = user;
            self.storage.save(current)
        })
    }
}

pub(crate) fn request_with_account_session<A, S, T>(
    api: &A,
    session: &BackendAccountSession<A, S>,
    authenticated: bool,
    operation: impl Fn(&A, Option<&str>) -> Result<T, AccountError>,
) -> Result<T, AccountError>
where
    A: AccountApi,
    S: AccountSessionStorage,
{
    let identity = if session.status()?.is_some() {
        Some(session.credentials_with_generation(None, None)?)
    } else {
        None
    };
    if authenticated && identity.is_none() {
        return Err(AccountError::Unauthorized);
    }
    let mut active_token = identity.as_ref().map(|value| value.1.clone());
    let result = match operation(api, active_token.as_deref()) {
        Err(AccountError::Unauthorized) if identity.is_some() => {
            let expected = identity.as_ref().map(|value| value.0.as_str());
            let (_, replacement) = session.credentials(active_token.as_deref(), expected)?;
            active_token = Some(replacement);
            operation(api, active_token.as_deref())
        }
        result => result,
    }?;
    let expected = identity.as_ref().map(|value| value.0.as_str());
    let current = session.status()?.map(|user| user.id);
    let generation_matches = identity.as_ref().is_none_or(|(_, _, generation)| {
        session
            .lock()
            .map(|state| state.generation == *generation)
            .unwrap_or(false)
    });
    if current.as_deref() != expected || !generation_matches {
        return Err(AccountError::Cancelled);
    }
    Ok(result)
}

fn saved_session(tokens: AccountTokens) -> Result<SavedAccountSession, AccountError> {
    let now = unix_ms()?;
    let duration = tokens
        .expires_in
        .checked_mul(1000)
        .ok_or(AccountError::Unavailable)?;
    let expires_at_unix_ms = now.checked_add(duration).ok_or(AccountError::Unavailable)?;
    Ok(SavedAccountSession {
        tokens,
        expires_at_unix_ms,
    })
}

pub(super) fn validate_saved_session(session: &SavedAccountSession) -> Result<(), AccountError> {
    validate_tokens(&session.tokens).map_err(|_| AccountError::Storage)?;
    let maximum = unix_ms()?
        .checked_add(MAX_SESSION_SECONDS * 1000)
        .ok_or(AccountError::Storage)?;
    if session.expires_at_unix_ms > maximum {
        return Err(AccountError::Storage);
    }
    Ok(())
}

fn unix_ms() -> Result<u64, AccountError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AccountError::Unavailable)?
        .as_millis();
    u64::try_from(millis).map_err(|_| AccountError::Unavailable)
}

/// Whether `saved` can be used as is: not about to expire, and not the access token the caller was just refused with.
fn usable_session(saved: &SavedAccountSession, rejected_token: Option<&str>) -> bool {
    saved.expires_at_unix_ms > refresh_deadline_ms()
        && rejected_token != Some(saved.tokens.access_token.as_str())
}

fn refresh_deadline_ms() -> u64 {
    unix_ms()
        .unwrap_or(u64::MAX)
        .saturating_add(REFRESH_EARLY_SECONDS * 1000)
}

pub trait AccountSession {
    type Error;
    fn identity(&self) -> impl Future<Output = Result<AccountIdentity, Self::Error>> + Send;
    fn bearer_token(&self) -> impl Future<Output = Result<String, Self::Error>> + Send;
    fn refresh(&self) -> impl Future<Output = Result<String, Self::Error>> + Send;
}

impl<A: AccountApi, S: AccountSessionStorage> AccountSession for BackendAccountSession<A, S> {
    type Error = AccountError;

    async fn identity(&self) -> Result<AccountIdentity, Self::Error> {
        self.status()?
            .map(|user| AccountIdentity { user_id: user.id })
            .ok_or(AccountError::Unauthorized)
    }

    async fn bearer_token(&self) -> Result<String, Self::Error> {
        self.access_token(None)
    }

    async fn refresh(&self) -> Result<String, Self::Error> {
        let current = self.access_token(None)?;
        self.access_token(Some(&current))
    }
}
