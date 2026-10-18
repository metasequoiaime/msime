//! `BackendAccountSession`: token refresh, single-flight, and the saved session
//! the host persists.

use super::google::*;
use super::validate::*;
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

mod adopt;
pub use adopt::*;

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
    pub(super) api: A,
    pub(super) storage: S,
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
            let stored = self.stored()?;
            if state.loaded && !same_login_option(state.saved.as_ref(), stored.as_ref()) {
                Self::next_generation(state)?;
            }
            state.saved = stored;
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
        self.sign_in_with_cleanup(challenge, credential, |_| {})
    }

    /// Saves a new session and cleans up the previous account while the session is held stable.
    /// `cleanup` is called only when the account id changes and must not call this session again.
    pub fn sign_in_with_cleanup<F>(
        &self,
        challenge: &str,
        credential: &str,
        cleanup: F,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str),
    {
        validate_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential, cleanup)
    }

    /// Signs in the device's anonymous account: the subject is presented as the challenge target and the secret answers it. Only `anonymous::ensure_anonymous_account` holds those values.
    pub(super) fn sign_in_anonymous(
        &self,
        subject: &str,
        secret: &str,
    ) -> Result<AccountUser, AccountError> {
        self.sign_in_anonymous_if(subject, secret, || Ok(true))
    }

    /// 与 [`Self::sign_in_anonymous`] 相同，只是在刷新锁里、保存会话之前再问一次 `still_current`：返回假时不保存，结果是 [`AccountError::Cancelled`]。登录要联网，期间 `session_adopt` 可能已经在同一把锁里把身份连同会话换成了另一个匿名账号，这时把这一次登录的会话写上去，会话和身份就不再属于同一个账号。
    pub(super) fn sign_in_anonymous_if(
        &self,
        subject: &str,
        secret: &str,
        still_current: impl FnOnce() -> Result<bool, AccountError>,
    ) -> Result<AccountUser, AccountError> {
        let challenge = self.request_code("anonymous", subject)?;
        let challenge = challenge.challenge_id;
        self.sign_in_exchanging_if(|| self.api.login(&challenge, secret), still_current, |_| {})
    }

    /// Completes an Apple challenge using the identity token returned by the
    /// native AuthenticationServices flow. The token never crosses the UI
    /// boundary; platform hosts pass it directly into the session.
    pub fn sign_in_apple(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        self.sign_in_apple_with_cleanup(challenge, credential, |_| {})
    }

    pub fn sign_in_apple_with_cleanup<F>(
        &self,
        challenge: &str,
        credential: &str,
        cleanup: F,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str),
    {
        validate_apple_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential, cleanup)
    }

    /// Completes a Google challenge with the authorization code the system browser delivered to the loopback redirect. The backend holds the PKCE verifier and the client secret and performs the exchange; this process only forwards the code.
    pub fn sign_in_google(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        validate_google_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential, |_| {})
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

    /// 申请一个 Apple challenge，给宿主自己弹的 `ASAuthorizationAppleIDRequest` 用：`nonce` 原样放进请求的 `nonce`，`challenge_id` 放进 `state`，拿到的 identity token 交给 [`Self::sign_in_apple`]。
    pub fn apple_challenge(&self) -> Result<AccountChallenge, AccountError> {
        self.nonce_challenge("apple")
    }

    /// 申请一个 Google ID token 流程的 challenge（不带回跳地址），给 iOS 的 GoogleSignIn 和安卓的 Credential Manager 用：`nonce` 原样交给 Google，拿到的 ID token 交给 [`Self::sign_in_google_id_token`]。
    pub fn google_id_token_challenge(&self) -> Result<AccountChallenge, AccountError> {
        self.nonce_challenge("google")
    }

    /// 这两种流程的 identity token 里必须带着服务端生成的 nonce，响应里没有 nonce 时登录不可能成功，按服务端不可用处理。
    fn nonce_challenge(&self, provider: &str) -> Result<AccountChallenge, AccountError> {
        let challenge = self.api.challenge(provider, "")?;
        validate_challenge(&challenge)?;
        if challenge.nonce.as_deref().is_none_or(str::is_empty) {
            return Err(AccountError::Unavailable);
        }
        Ok(challenge)
    }

    /// 申请一个 Google 回环流程的 challenge，给自己监听回环端口的宿主（鸿蒙）用。`target` 是 [`google_loopback_target`] 给的回跳地址；授权链接必须是指向这个回跳地址的 Google 授权链接，否则是 [`AccountError::Unavailable`]，不要打开它。浏览器回跳带来的授权码交给 [`Self::sign_in_google`]。
    pub fn google_loopback_challenge(
        &self,
        target: &str,
    ) -> Result<AccountChallenge, AccountError> {
        let challenge = self.request_code("google", target)?;
        validate_challenge(&challenge)?;
        let url = challenge
            .authorization_url
            .as_deref()
            .ok_or(AccountError::Unavailable)?;
        google_authorization_state(url, target)?;
        Ok(challenge)
    }

    /// 用 Google 的 ID token 完成 [`Self::google_id_token_challenge`] 申请的 challenge。
    pub fn sign_in_google_id_token(
        &self,
        challenge: &str,
        credential: &str,
    ) -> Result<AccountUser, AccountError> {
        validate_google_id_token_login(challenge, credential)?;
        self.sign_in_validated(challenge, credential, |_| {})
    }

    /// 开始一次 Apple 网页登录（安卓）：生成 verifier，把它的 S256 摘要交给服务端，返回要在浏览器里打开的 Apple 授权链接和 verifier。`app` 是发起登录的安卓 applicationId。
    pub fn apple_web_start(&self, app: &str) -> Result<AppleWebSignIn, AccountError> {
        if !super::apple_web::valid_apple_web_app(app) {
            return Err(AccountError::Invalid);
        }
        let code_verifier = super::apple_web::new_code_verifier()?;
        let challenge = self
            .api
            .apple_web_challenge(&super::apple_web::code_challenge(&code_verifier), app)?;
        super::apple_web::validate_apple_web_challenge(&challenge)?;
        Ok(AppleWebSignIn {
            authorization_url: challenge.authorization_url,
            expires_in: challenge.expires_in,
            code_verifier,
        })
    }

    /// 用回调交回来的 `grant` 和 [`Self::apple_web_start`] 给的 verifier 换会话并保存。
    pub fn sign_in_apple_web(
        &self,
        grant: &str,
        code_verifier: &str,
    ) -> Result<AccountUser, AccountError> {
        if !super::apple_web::valid_apple_web_grant(grant)
            || !super::apple_web::valid_code_verifier(code_verifier)
        {
            return Err(AccountError::Invalid);
        }
        self.sign_in_exchanging(|| self.api.apple_web_login(grant, code_verifier), |_| {})
    }

    /// 给已登录的账号添加邮箱登录（`purpose:"link"`，Android 的「添加登录方式」）：请服务端给 `target` 发验证码，验证码交给 [`Self::link_email`]。只能是 `email`。要用登录账号的令牌，没有登录时是 `account_unauthorized`，不联网；会话不是 10 分钟内登录的是 `account_recent_login_required`，宿主让用户重新登录同一个账号后再来。令牌被拒时与其他操作一样刷新一次、在同一个账号里重试。
    pub fn link_request_code(
        &self,
        provider: &str,
        target: &str,
    ) -> Result<AccountChallenge, AccountFailure> {
        if provider != "email" {
            return Err(AccountError::Invalid.into());
        }
        validate_provider_target(provider, target)?;
        let challenge =
            self.authenticated_detailed(|api, token| api.link_challenge(provider, target, token))?;
        validate_challenge(&challenge)?;
        Ok(challenge)
    }

    /// 给已登录的账号添加 Google 登录：与 [`Self::google_id_token_challenge`] 相同的 ID token 流程，只是 `purpose:"link"`，要求同 [`Self::link_request_code`]。拿到的 ID token 交给 [`Self::link_google_id_token`]。
    pub fn link_google_id_token_challenge(&self) -> Result<AccountChallenge, AccountFailure> {
        let challenge =
            self.authenticated_detailed(|api, token| api.link_challenge("google", "", token))?;
        validate_challenge(&challenge)?;
        if challenge.nonce.as_deref().is_none_or(str::is_empty) {
            return Err(AccountError::Unavailable.into());
        }
        Ok(challenge)
    }

    /// 给已登录的账号添加 Apple 登录（网页流程）：与 [`Self::apple_web_start`] 相同，只是 `purpose:"link"`，要求同 [`Self::link_request_code`]。回调的 `grant` 交给 [`Self::link_apple_web`]。
    pub fn link_apple_web_start(&self, app: &str) -> Result<AppleWebSignIn, AccountFailure> {
        if !super::apple_web::valid_apple_web_app(app) {
            return Err(AccountError::Invalid.into());
        }
        let code_verifier = super::apple_web::new_code_verifier()?;
        let code_challenge = super::apple_web::code_challenge(&code_verifier);
        let challenge = self.authenticated_detailed(|api, token| {
            api.apple_web_link_challenge(&code_challenge, app, token)
        })?;
        super::apple_web::validate_apple_web_challenge(&challenge)?;
        Ok(AppleWebSignIn {
            authorization_url: challenge.authorization_url,
            expires_in: challenge.expires_in,
            code_verifier,
        })
    }

    /// 用邮件里的 6 位验证码完成 [`Self::link_request_code`] 的绑定。
    pub fn link_email(&self, challenge: &str, code: &str) -> Result<AccountUser, AccountFailure> {
        validate_login(challenge, code)?;
        self.link_exchanging(|api, token| api.link_login(challenge, code, token))
    }

    /// 用 Google 的 ID token 完成 [`Self::link_google_id_token_challenge`] 的绑定。
    pub fn link_google_id_token(
        &self,
        challenge: &str,
        id_token: &str,
    ) -> Result<AccountUser, AccountFailure> {
        validate_google_id_token_login(challenge, id_token)?;
        self.link_exchanging(|api, token| api.link_login(challenge, id_token, token))
    }

    /// 用回调交回来的 `grant` 和 [`Self::link_apple_web_start`] 给的 verifier 完成绑定。
    pub fn link_apple_web(
        &self,
        grant: &str,
        code_verifier: &str,
    ) -> Result<AccountUser, AccountFailure> {
        if !super::apple_web::valid_apple_web_grant(grant)
            || !super::apple_web::valid_code_verifier(code_verifier)
        {
            return Err(AccountError::Invalid.into());
        }
        self.link_exchanging(|api, token| api.apple_web_link_login(grant, code_verifier, token))
    }

    /// 绑定的完成一步：用登录账号的令牌（快过期时先刷新）把凭据交给服务端。服务端把登录方式绑到这个账号上，并为同一个账号签发一个新会话（10 分钟内登录的那种），这里用它替换本地会话，与 Android 的 `keepSession` 相同；旧会话在服务端留到自己过期。
    ///
    /// 服务端以 401 拒绝时不刷新重试：验证码不对和令牌失效都是 401 `invalid_credentials`，分不开，重试会白白耗掉 challenge 的一次尝试。期间本地退出、换了账号或重新登录（会话代次变了）时是 `account_cancelled`，新会话丢掉、不写入，绑定在服务端已经生效。
    fn link_exchanging<E>(&self, exchange: E) -> Result<AccountUser, AccountFailure>
    where
        E: FnOnce(&A, &str) -> Result<AccountTokens, AccountFailure>,
    {
        let (user_id, token, generation) = self.credentials_with_generation(None, None)?;
        let tokens = exchange(&self.api, &token)?;
        validate_tokens(&tokens)?;
        // 服务端保证绑定得到的是发起绑定的那个账号；不是的话不能把它当成本机的登录。
        if tokens.user.id != user_id {
            return Err(AccountError::Unavailable.into());
        }
        let value = saved_session(tokens, Some(uuid::Uuid::new_v4()))?;
        let user = value.tokens.user.clone();
        self.storage
            .with_refresh_lock(|| {
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
                // 换了一个会话（新的登录 ID），与登录相同地推进代次：拿着旧会话令牌的进行中的操作和刷新不会再把旧会话写回来。
                Self::next_generation(&mut state)?;
                state.refresh = None;
                self.storage.save(&value)?;
                state.saved = Some(value);
                state.loaded = true;
                Ok(user)
            })
            .map_err(Into::into)
    }

    fn sign_in_validated<F>(
        &self,
        challenge: &str,
        credential: &str,
        cleanup: F,
    ) -> Result<AccountUser, AccountError>
    where
        F: FnOnce(&str),
    {
        self.sign_in_exchanging(|| self.api.login(challenge, credential), cleanup)
    }

    /// 登录的公共部分：`exchange` 向服务端换来令牌，然后在刷新锁里保存新会话。换令牌之前先推进会话代次，所以期间开始的另一次登录或退出会让这一次以 [`AccountError::Cancelled`] 结束，不会覆盖它。
    fn sign_in_exchanging<E, F>(&self, exchange: E, cleanup: F) -> Result<AccountUser, AccountError>
    where
        E: FnOnce() -> Result<AccountTokens, AccountError>,
        F: FnOnce(&str),
    {
        self.sign_in_exchanging_if(exchange, || Ok(true), cleanup)
    }

    /// 与 [`Self::sign_in_exchanging`] 相同，只是在刷新锁里、保存之前调 `still_current`，返回假时不保存，结果是 [`AccountError::Cancelled`]。
    fn sign_in_exchanging_if<E, G, F>(
        &self,
        exchange: E,
        still_current: G,
        cleanup: F,
    ) -> Result<AccountUser, AccountError>
    where
        E: FnOnce() -> Result<AccountTokens, AccountError>,
        G: FnOnce() -> Result<bool, AccountError>,
        F: FnOnce(&str),
    {
        let version = {
            let mut state = self.lock()?;
            let version = Self::next_generation(&mut state)?;
            state.refresh = None;
            version
        };
        let tokens = exchange()?;
        validate_tokens(&tokens)?;
        let value = saved_session(tokens, Some(uuid::Uuid::new_v4()))?;
        let user = value.tokens.user.clone();
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            if state.generation != version {
                return Err(AccountError::Cancelled);
            }
            let previous = if self.load_locked(&mut state).is_ok() {
                state
                    .saved
                    .as_ref()
                    .map(|saved| saved.tokens.user.id.clone())
            } else {
                None
            };
            if state.generation != version || !still_current()? {
                return Err(AccountError::Cancelled);
            }
            self.storage.save(&value)?;
            state.saved = Some(value);
            state.loaded = true;
            if let Some(previous) = previous.as_deref().filter(|id| *id != user.id) {
                cleanup(previous);
            }
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
                    let changed = !same_login_option(state.saved.as_ref(), Some(&stored));
                    if changed {
                        state.saved = Some(stored);
                        state.loaded = true;
                        Self::next_generation(&mut state)?;
                        return Err(AccountError::Cancelled);
                    }
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

        let session_id = self
            .lock()?
            .saved
            .as_ref()
            .and_then(|saved| saved.session_id);
        let api_result = self.api.refresh(&refresh_token);
        let mut state = self.lock()?;
        if state.generation != version {
            return Err(AccountError::Cancelled);
        }
        match api_result {
            Ok(tokens) => {
                validate_tokens(&tokens)?;
                let value = saved_session(tokens, session_id)?;
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
                                || current.tokens.user.id != value.tokens.user.id
                                || current.session_id != session_id =>
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
                            let changed = !same_login_option(state.saved.as_ref(), Some(&stored));
                            if changed {
                                state.saved = Some(stored);
                                state.loaded = true;
                                Self::next_generation(&mut state)?;
                                return Err(AccountError::Cancelled);
                            }
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
        self.ensure_session_id()?;
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

    /// 请求开始前在共享写锁内迁移旧会话，让两个进程使用同一个 ID，并保留旧文件的正常刷新行为。
    fn ensure_session_id(&self) -> Result<(), AccountError> {
        let missing = {
            let mut state = self.lock()?;
            self.load_locked(&mut state)?;
            state
                .saved
                .as_ref()
                .is_some_and(|saved| saved.session_id.is_none())
        };
        if missing {
            self.storage.with_refresh_lock(|| {
                let mut state = self.lock()?;
                self.load_locked(&mut state)?;
                if let Some(saved) = state
                    .saved
                    .as_ref()
                    .filter(|saved| saved.session_id.is_none())
                {
                    let mut migrated = saved.clone();
                    migrated.session_id = Some(uuid::Uuid::new_v4());
                    self.storage.save(&migrated)?;
                    state.saved = Some(migrated);
                }
                Ok(())
            })?;
        }
        Ok(())
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

    /// 在账户锁内读取当前身份并执行本地操作，供宿主清理失效的账户数据。
    /// 闭包不得再次调用此会话。
    pub fn with_current_identity<T, F>(&self, operation: F) -> Result<T, AccountError>
    where
        F: FnOnce(Option<(&str, u64)>) -> Result<T, AccountError>,
    {
        let mut state = self.lock()?;
        self.load_locked(&mut state)?;
        let identity = state
            .saved
            .as_ref()
            .map(|saved| (saved.tokens.user.id.as_str(), state.generation));
        operation(identity)
    }

    pub fn profile(&self) -> Result<AccountProfile, AccountError> {
        self.profile_for_login(None)
    }

    pub(super) fn profile_for_login(
        &self,
        expected: Option<(&str, u64)>,
    ) -> Result<AccountProfile, AccountError> {
        let (user_id, profile, generation) =
            self.authenticated_with_user_at(expected, |api, token| api.profile(token))?;
        validate_profile(&profile)?;
        if profile.user.id != user_id {
            return Err(AccountError::Cancelled);
        }
        self.update_user(profile.user.clone(), generation)?;
        Ok(profile)
    }

    pub fn rename(&self, display_name: &str) -> Result<AccountProfile, AccountError> {
        validate_display_name(display_name)?;
        let (user_id, _, generation) =
            self.authenticated_with_user(|api, token| api.rename(display_name, token))?;
        self.profile_for_login(Some((&user_id, generation)))
    }

    /// Uploads the PNG or JPEG at `path` as the user's avatar and returns the refreshed profile, whose `avatar_url` now names it.
    pub fn upload_avatar(&self, path: &Path) -> Result<AccountProfile, AccountError> {
        let image = read_account_avatar_upload(path)?;
        let (user_id, _, generation) =
            self.authenticated_with_user(|api, token| api.upload_avatar(&image, token))?;
        self.profile_for_login(Some((&user_id, generation)))
    }

    /// Removes the user's uploaded avatar and returns the refreshed profile.
    pub fn remove_avatar(&self) -> Result<AccountProfile, AccountError> {
        let (user_id, _, generation) =
            self.authenticated_with_user(|api, token| api.delete_avatar(token))?;
        self.profile_for_login(Some((&user_id, generation)))
    }

    /// The signed-in user's avatar, or `None` when they are signed out or have none. Reads the saved user, so it needs no backend round trip beyond the image itself.
    pub fn avatar(&self) -> Result<Option<AccountAvatarImage>, AccountError> {
        match self.status()?.and_then(|user| user.avatar_url) {
            Some(url) => fetch_account_avatar(&url).map(Some),
            None => Ok(None),
        }
    }

    pub fn logout(&self, all: bool) -> Result<(), AccountError> {
        self.logout_with_cleanup(all, |_| {})
    }

    /// Invalidates the local session and runs a host cleanup before another login can use it.
    pub fn logout_with_cleanup<F>(&self, all: bool, cleanup: F) -> Result<(), AccountError>
    where
        F: FnOnce(Option<&str>),
    {
        let (owner, generation) = self.cleanup_ticket()?;
        let token = match self.access_token(None) {
            Ok(token) => token,
            Err(error) => {
                self.forget_with_cleanup_at_generation(generation, owner.as_deref(), cleanup)?;
                return Err(error);
            }
        };
        self.forget_with_cleanup_at_generation(generation, owner.as_deref(), cleanup)?;
        self.api.logout(&token, all)
    }

    pub fn delete_account(&self) -> Result<(), AccountError> {
        self.delete_account_with_cleanup(|_| {})
    }

    /// Runs host cleanup only after the remote account deletion succeeds.
    pub fn delete_account_with_cleanup<F>(&self, cleanup: F) -> Result<(), AccountError>
    where
        F: FnOnce(Option<&str>),
    {
        let (user_id, _, generation) =
            self.authenticated_with_user(|api, token| api.delete_account(token))?;
        self.forget_with_cleanup_at_generation(generation, Some(&user_id), cleanup)
    }

    /// 与 [`Self::delete_account`] 相同，失败时是 [`AccountFailure`]：会话不是 10 分钟内登录的是 `account_recent_login_required`，本地会话留着，宿主让用户重新登录后再注销。注销成功后清掉本地会话。
    pub fn delete_account_detailed(&self) -> Result<(), AccountFailure> {
        let (user_id, _, generation) =
            self.authenticated_with_user_at(None, |api, token| api.delete_account_detailed(token))?;
        self.forget_with_cleanup_at_generation(generation, Some(&user_id), |_| {})
            .map_err(Into::into)
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
        self.authenticated_with_user_at(None, operation)
    }

    /// 返回 [`AccountFailure`] 的新接口用的 [`Self::authenticated`]：401 时同样带着被拒的令牌刷新一次、在同一个账号里重试，结束时同样核对账号没有变。
    fn authenticated_detailed<T, F>(&self, operation: F) -> Result<T, AccountFailure>
    where
        F: Fn(&A, &str) -> Result<T, AccountFailure>,
    {
        self.authenticated_with_user_at(None, operation)
            .map(|(_, result, _)| result)
    }

    fn authenticated_with_user_at<T, E, F>(
        &self,
        expected: Option<(&str, u64)>,
        operation: F,
    ) -> Result<(String, T, u64), E>
    where
        E: AuthenticationError,
        F: Fn(&A, &str) -> Result<T, E>,
    {
        let (user_id, token, generation) =
            self.credentials_with_generation(None, expected.map(|(user, _)| user))?;
        if expected.is_some_and(|(_, prior)| prior != generation) {
            return Err(AccountError::Cancelled.into());
        }
        let result = match operation(&self.api, &token) {
            Err(error) if error.unauthorized() => {
                let (_, replacement, replacement_generation) =
                    self.credentials_with_generation(Some(&token), Some(&user_id))?;
                if replacement_generation != generation {
                    return Err(AccountError::Cancelled.into());
                }
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
            return Err(AccountError::Cancelled.into());
        }
        Ok((user_id, result, generation))
    }

    /// 与 [`Self::chat`] 相同，失败时是 [`AccountFailure`]：序列化后的请求体超过服务端的 64 KiB 整包上限时在本地报 `account_too_large`，不发出去；限流带 `retry_after_seconds`，503 是 `account_unavailable`，没拿到响应是 `account_network`。
    pub fn chat_checked(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
    ) -> Result<String, AccountFailure> {
        prepare_chat(messages, model)?;
        self.authenticated_detailed(|api, token| api.chat_detailed(messages, model, token))
    }

    /// 与 [`Self::chat_models`] 相同，失败时是 [`AccountFailure`]。
    pub fn chat_models_detailed(&self) -> Result<AccountChatModels, AccountFailure> {
        self.authenticated_detailed(|api, token| api.chat_models_detailed(token))
    }

    /// 云端的常用语列表。只用这个会话的账号。
    pub fn phrases(&self) -> Result<AccountPhrases, AccountFailure> {
        self.authenticated_detailed(|api, token| api.phrases(token))
    }

    /// 按 `phrases.revision` 比较并交换，整份替换云端的常用语；发送前完整校验，冲突是 `account_conflict`，不重试。
    pub fn put_phrases(&self, phrases: &AccountPhrases) -> Result<AccountPhrases, AccountFailure> {
        phrases_request_body(phrases)?;
        self.authenticated_detailed(|api, token| api.put_phrases(phrases, token))
    }

    /// 「我的设备」：本人未撤销、未过期的登录会话。
    pub fn devices(&self) -> Result<AccountDevices, AccountFailure> {
        self.authenticated_detailed(|api, token| api.devices(token))
    }

    /// 撤销一个登录会话，返回本机是否因此退出了登录。本地保存的会话 id 是客户端生成的 UUID，不是服务端的会话 id，所以先列一次：要撤销的是当前会话时改走 [`Self::logout`]（服务端效果相同，本地会话同时清掉）；否则 `DELETE`。列不出来时直接 `DELETE`：万一撤销的是当前会话，下一次请求 401、刷新被拒时本地会话会被清掉。
    pub fn revoke_device(&self, id: &str) -> Result<bool, AccountFailure> {
        if !valid_device_session_id(id) {
            return Err(AccountError::Invalid.into());
        }
        let current = self.devices().is_ok_and(|devices| {
            devices
                .sessions
                .iter()
                .any(|device| device.id == id && device.current)
        });
        if current {
            self.logout(false)?;
            return Ok(true);
        }
        self.authenticated_detailed(|api, token| api.revoke_device(id, token))
            .map(|()| false)
    }

    /// 云端数据各分区的汇总。
    pub fn data_summary(&self) -> Result<AccountDataSummary, AccountFailure> {
        self.authenticated_detailed(|api, token| api.data_summary(token))
    }

    /// 把云端数据导出成 zip 写到 `destination`（绝对路径）。服务端每天只允许 3 次，超了是 `account_rate_limited`，带 `retry_after_seconds`。
    pub fn data_export(&self, destination: &Path) -> Result<AccountDataExport, AccountFailure> {
        if !destination.is_absolute() {
            return Err(AccountError::Invalid.into());
        }
        self.authenticated_detailed(|api, token| api.data_export(destination, token))
    }

    /// 清空所选分区。服务端要求会话在 10 分钟内建立，否则是 `account_recent_login_required`。
    pub fn data_delete(&self, sections: &[String]) -> Result<(), AccountFailure> {
        if !valid_data_sections(sections) {
            return Err(AccountError::Invalid.into());
        }
        self.authenticated_detailed(|api, token| api.data_delete(sections, token))
    }

    /// 用这个会话的账号上传一段语音贡献。语音贡献只用本机匿名账号（#8201 的决定 12），`msime_client_account` 在匿名会话上调用它。
    pub fn voice_contribution(
        &self,
        contribution: &AccountVoiceContribution,
    ) -> Result<AccountVoiceContributionReceipt, AccountFailure> {
        self.authenticated_detailed(|api, token| api.voice_contribution(contribution, token))
    }

    /// 把 `platform`（[`DOWNLOAD_LINK_PLATFORMS`] 之一）的下载链接发到这个账号已验证的邮箱。只用这个会话的账号；没有已验证邮箱是 `account_no_verified_email`。
    pub fn download_link(&self, platform: &str) -> Result<AccountDownloadLink, AccountFailure> {
        if !DOWNLOAD_LINK_PLATFORMS.contains(&platform) {
            return Err(AccountError::Invalid.into());
        }
        self.authenticated_detailed(|api, token| api.download_link(platform, token))
    }

    /// 上传打字统计的最近几天，`upload` 由 [`crate::typing_statistics::upload::build_upload`] 构建。只用这个会话的账号；没有登录是 `account_unauthorized`。
    pub fn put_typing_statistics(
        &self,
        upload: &crate::typing_statistics::upload::TypingStatisticsUpload,
    ) -> Result<(), AccountFailure> {
        self.authenticated_detailed(|api, token| api.put_typing_statistics(upload, token))
    }

    /// 清空本人全部设备在云端的打字统计。
    pub fn delete_typing_statistics(&self) -> Result<(), AccountFailure> {
        self.authenticated_detailed(|api, token| api.delete_typing_statistics(token))
    }

    /// 本人绑定的 QQ 群友。
    pub fn qq_links(&self) -> Result<AccountQqLinks, AccountFailure> {
        self.authenticated_detailed(|api, token| api.qq_links(token))
    }

    /// 用机器人给的绑定码绑定一个 QQ 群友；码前后的空白和中间的空格、连字符先去掉，其余不是 8 位数字时在本地拒绝。
    pub fn redeem_qq_link(&self, code: &str) -> Result<AccountQqLink, AccountFailure> {
        let code: String = code
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        if !valid_qq_link_code(&code) {
            return Err(AccountError::Invalid.into());
        }
        self.authenticated_detailed(|api, token| api.redeem_qq_link(&code, token))
    }

    /// 解除一条绑定。
    pub fn delete_qq_link(&self, id: &str) -> Result<(), AccountFailure> {
        if !valid_qq_link_id(id) {
            return Err(AccountError::Invalid.into());
        }
        self.authenticated_detailed(|api, token| api.delete_qq_link(id, token))
    }

    /// 以指定的那一次登录发一个请求：`user_id` 和 `generation` 是调用方先前从 [`Self::with_current_identity`] 读到的。会话换了账号、退出后重新登录（代次变了）时是 `account_cancelled`，不会用别的登录去发；401 时带着被拒的令牌刷新一次，在同一次登录里重试。给云同步这种「读、合并、写」要横跨好几个请求的调用方用，每一步都落在开始那一轮的登录上。
    pub fn authenticated_as<T, F>(
        &self,
        user_id: &str,
        generation: u64,
        operation: F,
    ) -> Result<T, AccountFailure>
    where
        F: Fn(&A, &str) -> Result<T, AccountFailure>,
    {
        self.authenticated_with_user_at(Some((user_id, generation)), operation)
            .map(|(_, result, _)| result)
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

    /// 按 `preferences.revision` 比较并交换，整份替换云端的设置文档（`PUT /v1/users/me/preferences`：服务端只在云端的修订号仍是它时写入，写入后修订号加 1），失败时是 [`AccountFailure`]（#8201 的 P4）。发送前校验：修订号为负、键或值不合规是 `account_invalid`，请求体超过服务端的 1 MiB 是 `account_too_large`；冲突（409 `revision_conflict`）是 `account_conflict`，不重试。给了 `expected_user_id` 时只写这个账号：会话已经退出、换成别的账号或请求期间重新登录时是 `account_cancelled`，不会写进另一个账号。
    pub fn put_preferences_checked(
        &self,
        preferences: &AccountPreferences,
        expected_user_id: Option<&str>,
    ) -> Result<AccountPreferences, AccountFailure> {
        if validate_account_preferences(preferences).is_err() {
            return Err(AccountError::Invalid.into());
        }
        let body = serde_json::to_vec(preferences).map_err(|_| AccountError::Invalid)?;
        if body.len() > MAX_JSON_BYTES {
            return Err(AccountFailure::too_large());
        }
        let expected = match expected_user_id {
            Some(user_id) => {
                let (_, _, generation) = self.credentials_with_generation(None, Some(user_id))?;
                Some((user_id, generation))
            }
            None => None,
        };
        self.authenticated_with_user_at(expected, |api, token| {
            api.put_preferences_detailed(preferences, token)
        })
        .map(|(_, updated, _)| updated)
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

    pub fn set_clipboard_enabled(&self, enabled: bool) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.set_clipboard_enabled(enabled, token))
    }

    pub fn delete_clipboard(&self, id: Option<&str>) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.delete_clipboard(id, token))
    }

    pub fn pin_clipboard(&self, id: &str, pinned: bool) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.pin_clipboard(id, pinned, token))
    }

    pub fn set_clipboard_retention(&self, days: u32) -> Result<(), AccountError> {
        self.authenticated(|api, token| api.set_clipboard_retention(days, token))
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

    /// 与 [`Self::export_dictionary`] 相同，正文直接写进 `destination`、不放进内存（见 [`BackendAccountClient::export_dictionary_to_file`]），返回写入的字节数。
    pub fn export_dictionary_to_file(
        &self,
        kind: DictionaryKind,
        format: &str,
        destination: &Path,
    ) -> Result<u64, AccountError> {
        self.authenticated(|api, token| {
            api.export_dictionary_to_file(kind, format, destination, token)
        })
    }

    pub fn forget(&self) -> Result<(), AccountError> {
        self.forget_with_cleanup(|_| {})
    }

    /// Invalidates the account and cleans up host state while the old identity is held stable.
    /// The callback must not call this session again.
    pub fn forget_with_cleanup<F>(&self, cleanup: F) -> Result<(), AccountError>
    where
        F: FnOnce(Option<&str>),
    {
        let (owner, generation) = self.cleanup_ticket()?;
        self.forget_with_cleanup_at_generation(generation, owner.as_deref(), cleanup)
    }

    fn cleanup_ticket(&self) -> Result<(Option<String>, u64), AccountError> {
        let mut state = self.lock()?;
        let owner = if self.load_locked(&mut state).is_ok() {
            state
                .saved
                .as_ref()
                .map(|saved| saved.tokens.user.id.clone())
        } else {
            None
        };
        Ok((owner, state.generation))
    }

    fn forget_with_cleanup_at_generation<F>(
        &self,
        generation: u64,
        expected_user_id: Option<&str>,
        cleanup: F,
    ) -> Result<(), AccountError>
    where
        F: FnOnce(Option<&str>),
    {
        // Match login and refresh lock ordering: storage first, then the session mutex.
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            if state.generation != generation {
                return Err(AccountError::Cancelled);
            }
            let owner = if self.load_locked(&mut state).is_ok() {
                state
                    .saved
                    .as_ref()
                    .map(|saved| saved.tokens.user.id.clone())
            } else {
                None
            };
            if state.generation != generation {
                return Err(AccountError::Cancelled);
            }
            if let (Some(expected), Some(current)) = (expected_user_id, owner.as_deref()) {
                if current != expected {
                    return Err(AccountError::Cancelled);
                }
            }
            let owner = owner.or_else(|| expected_user_id.map(str::to_owned));
            // No asynchronous operation can be running at the terminal value:
            // next_generation refuses to issue it there. Keep the value stable
            // while still clearing the account state.
            state.generation = state.generation.saturating_add(1);
            state.refresh = None;
            state.saved = None;
            state.loaded = true;
            self.storage.clear()?;
            cleanup(owner.as_deref());
            Ok(())
        })
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

/// 认证调用的错误类型：已有接口用 [`AccountError`]，新接口用 [`AccountFailure`]；两者都要能从会话错误转换过来，也都要认得出 401。
trait AuthenticationError: From<AccountError> {
    fn unauthorized(&self) -> bool;
}

impl AuthenticationError for AccountError {
    fn unauthorized(&self) -> bool {
        *self == AccountError::Unauthorized
    }
}

impl AuthenticationError for AccountFailure {
    fn unauthorized(&self) -> bool {
        self.is_unauthorized()
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
            let (_, replacement, replacement_generation) =
                session.credentials_with_generation(active_token.as_deref(), expected)?;
            if identity
                .as_ref()
                .is_some_and(|value| replacement_generation != value.2)
            {
                return Err(AccountError::Cancelled);
            }
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

fn saved_session(
    tokens: AccountTokens,
    session_id: Option<uuid::Uuid>,
) -> Result<SavedAccountSession, AccountError> {
    let now = unix_ms()?;
    let duration = tokens
        .expires_in
        .checked_mul(1000)
        .ok_or(AccountError::Unavailable)?;
    let expires_at_unix_ms = now.checked_add(duration).ok_or(AccountError::Unavailable)?;
    Ok(SavedAccountSession {
        tokens,
        expires_at_unix_ms,
        session_id,
    })
}

fn same_login_option(a: Option<&SavedAccountSession>, b: Option<&SavedAccountSession>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) if a.tokens.user.id == b.tokens.user.id => {
            match (a.session_id, b.session_id) {
                (Some(a), Some(b)) => a == b,
                // 旧文件没有登录 ID；认证请求会先迁移，直接刷新仍可接收另一进程的正常轮换。
                (None, None) => true,
                _ => false,
            }
        }
        _ => false,
    }
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
