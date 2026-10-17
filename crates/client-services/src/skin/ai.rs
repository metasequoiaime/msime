//! Android 上依托账号的 AI 键盘皮肤生成器。
//!
//! 取消和进度展示归宿主负责。本模块负责后端契约、响应校验、账号刷新以及插画任务的清理，这样任何 UI 或平台类型都不会进入 client-services。

use crate::account::{
    AccountApi, AccountError, AccountFailure, AccountSessionStorage, BackendAccountClient,
    BackendAccountSession,
};
use crate::preferences::{TouchKeyboardSkinDesign, TouchSkinKeyMaterial, TouchSkinKeyShape};
use crate::theme::{luminance, readable_text};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_PROMPT_CHARACTERS: usize = 500;
/// 插画任务的提示词上限（服务端 `createSkinArtworkJob`：去掉首尾空白后 1–1200 个码点，不含 NUL）。
const MAX_JOB_PROMPT_CHARACTERS: usize = 1200;
/// 失败任务 `reason` 的字节上限；超过或含控制字符时丢掉这个字段。
const MAX_JOB_REASON_BYTES: usize = 256;
const MAX_CHAT_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_ARTWORK_RESPONSE_BYTES: usize = 12 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTWORK_SECONDS: u64 = 200;
/// 请求体的上限，与其他账号请求的 1 MiB 相同；对话的请求体另外按服务端的 64 KiB 整包上限先查。
const MAX_REQUEST_BYTES: usize = 1024 * 1024;

/// 一张插画。拒绝未知字段，因为 `msime_client_ai_skin_plan` 的 `artwork` 用它解析宿主交来的插画；服务端任务响应里的插画先按不拒绝未知字段的 `RawArtwork` 解析再换成它。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkinArtwork {
    pub b64_json: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSkinProposal {
    pub name: String,
    pub description: String,
    pub design: TouchKeyboardSkinDesign,
    pub artwork_prompt: String,
    pub artwork: AiSkinArtwork,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AiSkinError {
    #[error("AI skin generation was cancelled")]
    Cancelled,
    #[error("AI skin response was invalid")]
    InvalidResponse,
    #[error(transparent)]
    Account(#[from] AccountError),
}

/// 与 [`AiSkinError`] 相同，只是账号一侧的失败是带 HTTP 状态码、服务端错误码、`Retry-After` 和有没有拿到响应的 [`AccountFailure`]：C 接口（`msime_client_ai_skin_generate`、`msime_client_ai_skin_job`）据此报 `retry_after_seconds`，并把 503 和断网分开。Tauri 的 `ai_skin_generate` 经 [`BackendAiSkinService::generate`] 拿到的仍是 [`AiSkinError`]，码不变。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiSkinFailure {
    Cancelled,
    InvalidResponse,
    Account(AccountFailure),
}

impl From<AccountFailure> for AiSkinFailure {
    fn from(failure: AccountFailure) -> Self {
        Self::Account(failure)
    }
}

impl From<AccountError> for AiSkinFailure {
    fn from(error: AccountError) -> Self {
        Self::Account(error.into())
    }
}

impl From<AiSkinError> for AiSkinFailure {
    fn from(error: AiSkinError) -> Self {
        match error {
            AiSkinError::Cancelled => Self::Cancelled,
            AiSkinError::InvalidResponse => Self::InvalidResponse,
            AiSkinError::Account(error) => error.into(),
        }
    }
}

/// 只留 [`AccountFailure::error`]，与这些路径改成 [`AccountFailure`] 之前给出的 [`AiSkinError`] 相同。
impl From<AiSkinFailure> for AiSkinError {
    fn from(failure: AiSkinFailure) -> Self {
        match failure {
            AiSkinFailure::Cancelled => Self::Cancelled,
            AiSkinFailure::InvalidResponse => Self::InvalidResponse,
            AiSkinFailure::Account(failure) => Self::Account(failure.error),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiModelCatalog {
    pub data: Vec<AiModel>,
    pub default_model: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiModel {
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AiChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

/// 一个插画任务。`state` 是 `running`、`succeeded` 或 `failed`；`reason` 是失败的原因码，`expires_at` 是任务过期的时间（RFC 3339），服务端没给或不合法时没有。
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AiSkinJob {
    pub id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artwork: Option<AiSkinArtwork>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// 进行中的生成按宿主给的请求 id 登记，好让另一次调用取消它。同一个 id 正在生成时再次开始是 `ai_skin_busy`，原任务的取消标记不变。
#[derive(Default)]
pub struct AiSkinRequests(Mutex<HashMap<String, Arc<AtomicBool>>>);

impl AiSkinRequests {
    /// 登记一次生成，返回它的取消标记。
    pub fn begin(&self, id: &str) -> Result<Arc<AtomicBool>, &'static str> {
        let mut requests = self.0.lock().map_err(|_| "ai_skin_unavailable")?;
        // 拒绝重复 id 时保留原任务的取消标记。
        if requests.contains_key(id) {
            return Err("ai_skin_busy");
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        requests.insert(id.to_owned(), Arc::clone(&cancelled));
        Ok(cancelled)
    }

    /// 取消 `id` 的生成，返回是否有一个在跑。只碰这张表，立即返回。
    pub fn cancel(&self, id: &str) -> Result<bool, &'static str> {
        let requests = self.0.lock().map_err(|_| "ai_skin_unavailable")?;
        Ok(requests
            .get(id)
            .map(|cancelled| cancelled.store(true, Ordering::Release))
            .is_some())
    }

    /// 生成结束（成功、失败或取消）后注销，同一个 id 之后可以再用。
    pub fn finish(&self, id: &str) {
        if let Ok(mut requests) = self.0.lock() {
            requests.remove(id);
        }
    }
}

/// AI 皮肤用到的四条路由。失败都是 [`AccountFailure`]，保留状态码、`Retry-After` 和有没有拿到响应；[`BackendAiSkinService::generate`] 交给 Tauri 时只留其中的 [`AccountError`]。
pub trait AiSkinApi: Send + Sync + 'static {
    fn ai_models(&self, token: &str) -> Result<AiModelCatalog, AccountFailure>;
    fn ai_chat(
        &self,
        messages: &[AiChatMessage<'_>],
        model: &str,
        token: &str,
    ) -> Result<String, AccountFailure>;
    fn create_skin_artwork(&self, prompt: &str, token: &str) -> Result<AiSkinJob, AccountFailure>;
    fn get_skin_artwork(&self, id: &str, token: &str) -> Result<AiSkinJob, AccountFailure>;
    fn delete_skin_artwork(&self, id: &str, token: &str) -> Result<(), AccountFailure>;
}

impl AiSkinApi for BackendAccountClient {
    fn ai_models(&self, token: &str) -> Result<AiModelCatalog, AccountFailure> {
        let catalog = self.json_detailed::<AiModelCatalog, ()>(
            Method::GET,
            "/v1/models",
            Some(token),
            None,
            MAX_REQUEST_BYTES,
            128 * 1024,
            Duration::from_secs(30),
        )?;
        if !crate::account::valid_model_catalog(
            catalog.data.iter().map(|model| model.id.as_str()),
            &catalog.default_model,
            33,
            200,
        ) {
            return Err(AccountError::Unavailable.into());
        }
        Ok(catalog)
    }

    fn ai_chat(
        &self,
        messages: &[AiChatMessage<'_>],
        model: &str,
        token: &str,
    ) -> Result<String, AccountFailure> {
        #[derive(Serialize)]
        struct Body<'a> {
            messages: &'a [AiChatMessage<'a>],
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
            message: Message,
        }
        #[derive(Deserialize)]
        struct Message {
            role: String,
            content: String,
        }
        if model.is_empty()
            || messages.is_empty()
            || messages.len() > 16
            || messages.iter().any(|message| {
                !matches!(message.role, "system" | "user" | "assistant")
                    || message.content.is_empty()
                    || !crate::text::is_bounded_text(message.content, 16 * 1024)
            })
        {
            return Err(AccountError::Invalid.into());
        }
        let body = Body {
            messages,
            model,
            max_tokens: 2048,
            stream: false,
        };
        let encoded = serde_json::to_vec(&body).map_err(|_| AccountError::Invalid)?;
        if encoded.len() > 64 * 1024 {
            return Err(AccountError::Invalid.into());
        }
        let result: Response = self.json_detailed(
            Method::POST,
            "/v1/chat/completions",
            Some(token),
            Some(&body),
            MAX_REQUEST_BYTES,
            MAX_CHAT_RESPONSE_BYTES,
            Duration::from_secs(125),
        )?;
        let message = result
            .choices
            .first()
            .map(|choice| &choice.message)
            .filter(|message| {
                message.role == "assistant"
                    && !message.content.trim().is_empty()
                    && message.content.len() <= MAX_CHAT_RESPONSE_BYTES
            })
            .ok_or(AccountError::Unavailable)?;
        Ok(message.content.clone())
    }

    fn create_skin_artwork(&self, prompt: &str, token: &str) -> Result<AiSkinJob, AccountFailure> {
        #[derive(Serialize)]
        struct Body<'a> {
            prompt: &'a str,
        }
        let job: RawSkinJob = self.json_detailed(
            Method::POST,
            "/v1/skins/jobs",
            Some(token),
            Some(&Body { prompt }),
            MAX_REQUEST_BYTES,
            128 * 1024,
            Duration::from_secs(30),
        )?;
        Ok(validate_job(job)?)
    }

    fn get_skin_artwork(&self, id: &str, token: &str) -> Result<AiSkinJob, AccountFailure> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AccountError::Invalid.into());
        }
        let path = format!("/v1/skins/jobs/{id}");
        let job = validate_job(self.json_detailed(
            Method::GET,
            &path,
            Some(token),
            None::<&()>,
            MAX_REQUEST_BYTES,
            MAX_ARTWORK_RESPONSE_BYTES,
            Duration::from_secs(30),
        )?)?;
        if job.id != id {
            return Err(AccountError::Unavailable.into());
        }
        Ok(job)
    }

    fn delete_skin_artwork(&self, id: &str, token: &str) -> Result<(), AccountFailure> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AccountError::Invalid.into());
        }
        let path = format!("/v1/skins/jobs/{id}");
        self.request_detailed(
            Method::DELETE,
            &path,
            Some(token),
            None,
            MAX_REQUEST_BYTES,
            16 * 1024,
            Duration::from_secs(5),
            "application/json",
        )
        .map(|_| ())
    }
}

/// 服务端的任务响应。不拒绝未知字段：这是服务端的响应，本地测试回放的只是当前的形状，服务端以后再加字段时测试抓不到，生产上就会再坏一次（2026-10-01 加了 `expires_at` 以后，带 `deny_unknown_fields` 的旧版本每次创建任务都解析失败）。
#[derive(Clone, Debug, Deserialize)]
struct RawSkinJob {
    id: String,
    state: String,
    #[serde(default)]
    artwork: Option<RawArtwork>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

/// 任务响应里的插画，字段与 [`AiSkinArtwork`] 相同；同 [`RawSkinJob`]，服务端以后在 `artwork` 里加字段时不能让每个成功的任务都解析失败。
#[derive(Clone, Debug, Deserialize)]
struct RawArtwork {
    b64_json: String,
    mime_type: String,
    width: u32,
    height: u32,
}

impl From<RawArtwork> for AiSkinArtwork {
    fn from(raw: RawArtwork) -> Self {
        Self {
            b64_json: raw.b64_json,
            mime_type: raw.mime_type,
            width: raw.width,
            height: raw.height,
        }
    }
}

fn validate_job(job: RawSkinJob) -> Result<AiSkinJob, AccountError> {
    if !crate::text::is_lower_hex(&job.id, 48)
        || !matches!(job.state.as_str(), "running" | "succeeded" | "failed")
    {
        return Err(AccountError::Unavailable);
    }
    let artwork = job.artwork.map(AiSkinArtwork::from);
    if let Some(artwork) = &artwork {
        validate_artwork(artwork).map_err(|_| AccountError::Unavailable)?;
    }
    Ok(AiSkinJob {
        id: job.id,
        state: job.state,
        artwork,
        reason: job
            .reason
            .filter(|reason| crate::text::is_bounded_text(reason, MAX_JOB_REASON_BYTES)),
        expires_at: job
            .expires_at
            .filter(|at| crate::account::parse_rfc3339(at).is_some()),
    })
}

/// 插画任务的提示词按服务端的规则检查：去掉首尾空白后 1–1200 个码点，不含 NUL。返回去掉首尾空白的提示词。
fn job_prompt(prompt: &str) -> Result<&str, AiSkinError> {
    let prompt = prompt.trim();
    if prompt.is_empty()
        || !crate::text::is_bounded_chars_without_nul(prompt, MAX_JOB_PROMPT_CHARACTERS)
    {
        return Err(AiSkinError::InvalidResponse);
    }
    Ok(prompt)
}

fn validate_artwork(artwork: &AiSkinArtwork) -> Result<Vec<u8>, AiSkinError> {
    if !matches!(artwork.mime_type.as_str(), "image/png" | "image/jpeg")
        || !(1..=2048).contains(&artwork.width)
        || !(1..=2048).contains(&artwork.height)
        || artwork.b64_json.len() > 11 * 1024 * 1024
    {
        return Err(AiSkinError::InvalidResponse);
    }
    let bytes = BASE64
        .decode(&artwork.b64_json)
        .map_err(|_| AiSkinError::InvalidResponse)?;
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
        return Err(AiSkinError::InvalidResponse);
    }
    let valid_header = match artwork.mime_type.as_str() {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1A\n"),
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        _ => false,
    };
    valid_header
        .then_some(bytes)
        .ok_or(AiSkinError::InvalidResponse)
}

pub struct BackendAiSkinService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
    /// 本机匿名账号的会话。为 `Some` 时没有登录也能生成（#8201 的决定 16），为 `None` 时只用登录账号。
    anonymous: Option<Arc<BackendAccountSession<A, S>>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAiSkinService<A, S> {
    /// 只用登录账号的服务，Tauri 的 `ai_skin_generate` 用它。
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self {
            api,
            session,
            anonymous: None,
        }
    }

    /// 登录账号优先、没有登录时用本机匿名账号 `anonymous` 的服务，与释义的取令牌方式相同；C 的 `msime_client_ai_skin_generate`、`msime_client_ai_skin_job` 用它。一次生成从头到尾留在开始时选中的那个账号上：插画任务归创建它的账号，换了账号就读不到。
    pub fn with_anonymous(
        api: A,
        session: Arc<BackendAccountSession<A, S>>,
        anonymous: Arc<BackendAccountSession<A, S>>,
    ) -> Self {
        Self {
            api,
            session,
            anonymous: Some(anonymous),
        }
    }
}

/// 一次生成或任务请求所用的那一次登录：哪个会话、哪个账号、会话代次。
struct Login<'a, A: AccountApi, S: AccountSessionStorage> {
    session: &'a BackendAccountSession<A, S>,
    user_id: String,
    generation: u64,
}

impl<A, S> BackendAiSkinService<A, S>
where
    A: AccountApi + AiSkinApi,
    S: AccountSessionStorage,
{
    /// 完整的生成，给 Tauri 的 `ai_skin_generate`：与 [`Self::generate_detailed`] 相同，账号一侧的失败只留 [`AccountError`]。
    pub fn generate(
        &self,
        prompt: &str,
        cancelled: &AtomicBool,
        progress: impl FnMut(usize),
    ) -> Result<Vec<AiSkinProposal>, AiSkinError> {
        self.generate_detailed(prompt, cancelled, progress)
            .map_err(Into::into)
    }

    /// 完整的生成：模型目录、对话出三套设计、三张插画，失败时账号一侧是 [`AccountFailure`]。
    pub fn generate_detailed(
        &self,
        prompt: &str,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(usize),
    ) -> Result<Vec<AiSkinProposal>, AiSkinFailure> {
        if prompt.is_empty() || !crate::text::is_bounded_chars(prompt, MAX_PROMPT_CHARACTERS) {
            return Err(AiSkinFailure::InvalidResponse);
        }
        check_cancelled(cancelled)?;
        let (login, token) = self.login()?;
        let catalog =
            self.request_authenticated(&login, token, |api, token| api.ai_models(token))?;
        check_cancelled(cancelled)?;
        let token = self.credentials_for_generation(&login)?;
        let response = self.request_authenticated(&login, token, |api, token| {
            api.ai_chat(
                &[
                    AiChatMessage {
                        role: "system",
                        content: AI_SKIN_SYSTEM_PROMPT,
                    },
                    AiChatMessage {
                        role: "user",
                        content: prompt,
                    },
                ],
                &catalog.default_model,
                token,
            )
        })?;
        let plans = plan_ai_skins(&response)?;
        check_cancelled(cancelled)?;
        let login_ref = &login;
        let mut results = thread::scope(|scope| {
            // 出图线程只经通道报告完成，进度回调在调用 `generate` 的线程上依次调用，与 C 接口其他进度回调的约定相同：JNI 和 NAPI 胶水可以直接用调用线程的 env，回调也不会并发。
            let (finished, completions) = mpsc::channel::<()>();
            let mut handles = Vec::with_capacity(3);
            for (index, plan) in plans.into_iter().enumerate() {
                let finished = finished.clone();
                handles.push(scope.spawn(move || {
                    let result = self.illustrate(login_ref, &plan, cancelled);
                    if result.is_ok() {
                        let _ = finished.send(());
                    }
                    (index, result)
                }));
            }
            drop(finished);
            let mut completed = 0;
            // 所有出图线程结束、发送端都放下以后 `recv` 才返回错误，之后再按原来的顺序 join。
            while completions.recv().is_ok() {
                completed += 1;
                progress(completed);
            }
            let mut results = Vec::with_capacity(3);
            let mut first_error = None;
            for handle in handles {
                match handle.join() {
                    Ok((index, Ok(result))) => results.push((index, result)),
                    Ok((_, Err(error))) => {
                        cancelled.store(true, Ordering::Release);
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                    Err(_) => {
                        cancelled.store(true, Ordering::Release);
                        if first_error.is_none() {
                            first_error = Some(AccountError::Unavailable.into());
                        }
                    }
                }
            }
            first_error.map_or_else(|| Ok(results), Err)
        })?;
        results.sort_by_key(|(index, _)| *index);
        let mut ordered = Vec::with_capacity(results.len());
        ordered.extend(results.into_iter().map(|(_, result)| result));
        self.ensure_identity(&login)?;
        Ok(ordered)
    }

    /// 新建一个插画任务（`POST /v1/skins/jobs`），立即返回 `running` 的任务。提示词去掉首尾空白后 1–1200 个码点、不含 NUL，否则是 [`AiSkinFailure::InvalidResponse`]（`ai_skin_invalid`）。
    pub fn create_artwork_job(&self, prompt: &str) -> Result<AiSkinJob, AiSkinFailure> {
        let prompt = job_prompt(prompt)?;
        self.job_request(|api, token| api.create_skin_artwork(prompt, token))
    }

    /// 读一个插画任务（`GET /v1/skins/jobs/{id}`）。`id` 是 48 位小写十六进制。
    pub fn artwork_job(&self, id: &str) -> Result<AiSkinJob, AiSkinFailure> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AiSkinFailure::InvalidResponse);
        }
        self.job_request(|api, token| api.get_skin_artwork(id, token))
    }

    /// 删除一个插画任务（`DELETE /v1/skins/jobs/{id}`），进行中的任务同时取消。
    pub fn delete_artwork_job(&self, id: &str) -> Result<(), AiSkinFailure> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AiSkinFailure::InvalidResponse);
        }
        self.job_request(|api, token| api.delete_skin_artwork(id, token))
    }

    /// 这次请求用哪一次登录：登录账号的；没有登录（或登录的会话已被服务端吊销并清掉）而有匿名会话时用匿名账号的。
    fn login(&self) -> Result<(Login<'_, A, S>, String), AiSkinFailure> {
        let (session, credentials) = match (
            self.session.credentials_with_generation(None, None),
            &self.anonymous,
        ) {
            (Err(AccountError::Unauthorized), Some(anonymous)) => (
                &**anonymous,
                anonymous.credentials_with_generation(None, None),
            ),
            (credentials, _) => (&*self.session, credentials),
        };
        let (user_id, token, generation) = credentials.map_err(Self::account_error)?;
        Ok((
            Login {
                session,
                user_id,
                generation,
            },
            token,
        ))
    }

    /// 用选中账号的令牌做一次任务请求：401 时刷新一次、在同一个账号里重试，结束时核对账号和会话代次没有变。
    fn job_request<T>(
        &self,
        operation: impl Fn(&A, &str) -> Result<T, AccountFailure>,
    ) -> Result<T, AiSkinFailure> {
        let (login, token) = self.login()?;
        self.request_authenticated(&login, token, operation)
    }

    fn request_authenticated<T>(
        &self,
        login: &Login<'_, A, S>,
        token: String,
        operation: impl Fn(&A, &str) -> Result<T, AccountFailure>,
    ) -> Result<T, AiSkinFailure> {
        let result = match operation(&self.api, &token) {
            Err(failure) if failure.is_unauthorized() => {
                let replacement = self.replacement_token(login, &token)?;
                operation(&self.api, &replacement)
            }
            result => result,
        }?;
        self.ensure_identity(login)?;
        Ok(result)
    }

    /// 服务端以 401 拒绝了 `rejected` 以后，同一个账号、同一次登录刷新出来的令牌；换了登录是 [`AiSkinFailure::Cancelled`]。
    fn replacement_token(
        &self,
        login: &Login<'_, A, S>,
        rejected: &str,
    ) -> Result<String, AiSkinFailure> {
        let (_, replacement, replacement_generation) = login
            .session
            .credentials_with_generation(Some(rejected), Some(&login.user_id))
            .map_err(Self::account_error)?;
        if replacement_generation != login.generation {
            return Err(AiSkinFailure::Cancelled);
        }
        Ok(replacement)
    }

    fn ensure_identity(&self, login: &Login<'_, A, S>) -> Result<(), AiSkinFailure> {
        login
            .session
            .with_generation(login.generation, Some(&login.user_id), || Ok(()))
            .map_err(Self::account_error)
    }

    fn account_error(error: AccountError) -> AiSkinFailure {
        if error == AccountError::Cancelled {
            AiSkinFailure::Cancelled
        } else {
            error.into()
        }
    }

    fn credentials_for_generation(&self, login: &Login<'_, A, S>) -> Result<String, AiSkinFailure> {
        let (_, token, current_generation) = login
            .session
            .credentials_with_generation(None, Some(&login.user_id))
            .map_err(Self::account_error)?;
        if current_generation != login.generation {
            return Err(AiSkinFailure::Cancelled);
        }
        Ok(token)
    }

    fn illustrate(
        &self,
        login: &Login<'_, A, S>,
        plan: &AiSkinPlan,
        cancelled: &AtomicBool,
    ) -> Result<AiSkinProposal, AiSkinFailure> {
        check_cancelled(cancelled)?;
        let token = self.credentials_for_generation(login)?;
        let job = match self.api.create_skin_artwork(&plan.artwork_prompt, &token) {
            Err(failure) if failure.is_unauthorized() => {
                let replacement = self.replacement_token(login, &token)?;
                self.api
                    .create_skin_artwork(&plan.artwork_prompt, &replacement)?
            }
            result => result?,
        };
        let id = job.id;
        let result = self.poll_artwork(login, &id, cancelled);
        // Completed and failed jobs are both released; cancellation must not
        // leave an upstream task billable after this call returns.
        self.cleanup_job(login, &id);
        let artwork = result?;
        Ok(AiSkinProposal {
            name: plan.name.clone(),
            description: plan.description.clone(),
            design: plan.design.clone(),
            artwork_prompt: plan.artwork_prompt.clone(),
            artwork,
        })
    }

    fn poll_artwork(
        &self,
        login: &Login<'_, A, S>,
        id: &str,
        cancelled: &AtomicBool,
    ) -> Result<AiSkinArtwork, AiSkinFailure> {
        let deadline = Instant::now() + Duration::from_secs(MAX_ARTWORK_SECONDS);
        loop {
            check_cancelled(cancelled)?;
            if Instant::now() >= deadline {
                return Err(AccountError::Unavailable.into());
            }
            let token = self.credentials_for_generation(login)?;
            let job = match self.api.get_skin_artwork(id, &token) {
                Err(failure) if failure.is_unauthorized() => {
                    let replacement = self.replacement_token(login, &token)?;
                    self.api.get_skin_artwork(id, &replacement)?
                }
                result => result?,
            };
            match job.state.as_str() {
                "running" => {
                    let until = Instant::now() + Duration::from_secs(5);
                    while Instant::now() < until {
                        check_cancelled(cancelled)?;
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                "succeeded" => {
                    let artwork = job.artwork.ok_or(AiSkinFailure::InvalidResponse)?;
                    validate_artwork(&artwork)?;
                    return Ok(artwork);
                }
                _ => return Err(AccountError::Unavailable.into()),
            }
        }
    }

    fn cleanup_job(&self, login: &Login<'_, A, S>, id: &str) {
        let Ok(token) = self.credentials_for_generation(login) else {
            return;
        };
        let _ = self.api.delete_skin_artwork(id, &token);
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), AiSkinError> {
    if cancelled.load(Ordering::Acquire) {
        Err(AiSkinError::Cancelled)
    } else {
        Ok(())
    }
}

/// The contract with the model, which is shared rather than per host.
///
/// It names the exact JSON shape `plan_ai_skins` then refuses anything else for, so the two have to
/// travel together: a host that composed its own instruction would be asking for a document the
/// parser was not written against. Hosts that cannot run the pipeline themselves - the ones whose
/// HTTP has to go through the surrounding platform - send this verbatim and hand the answer back.
pub const AI_SKIN_SYSTEM_PROMPT: &str = "你是输入法皮肤设计师。根据用户描述生成恰好三套明显不同、精致且文字清晰的键盘皮肤。只返回 JSON 对象，不要 Markdown。格式：{\"skins\":[{\"name\":\"中文名称\",\"description\":\"中文设计说明\",\"artworkPrompt\":\"原创背景场景、角色、画风和装饰，主体位于画面边缘，中央留白\",\"background\":\"#E8F0EB\",\"keyBackground\":\"#FFFFFF\",\"keyForeground\":\"#17251D\",\"accent\":\"#185C47\",\"actionBackground\":\"#185C47\",\"gradientEnd\":null,\"gradientHorizontal\":false,\"keyShape\":\"pebble\",\"keyMaterial\":\"raised\",\"cornerRadius\":8,\"borderWidth\":0,\"shadow\":0.1,\"pattern\":0,\"monospaced\":false}]}。每套必须包含所有字段。name 为 1–32 字，description 为 1–280 字，artworkPrompt 为 40–100 字。颜色为 #RRGGBB；keyShape 只能为 rounded、capsule、ticket、pebble；keyMaterial 只能为 flat、raised、glass、paper；三套造型与材质都必须不同。不要生成照片、URL、代码或外部资源；artworkPrompt 不得描述键盘、键帽、按钮、按键、布局、界面或文字设计。";

/// One of the three designs the model proposed, after validation and contrast repair.
///
/// Published because a host that cannot run `generate` itself still has to get here: it performs
/// the chat over its own HTTP stack and needs this crate to say whether the answer is usable, and
/// what it means. The artwork is not part of it - that is fetched per plan afterwards.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSkinPlan {
    pub name: String,
    pub description: String,
    pub artwork_prompt: String,
    pub design: TouchKeyboardSkinDesign,
}

/// The model's answer, as three usable designs or a refusal.
///
/// Parsing and the "three visibly different skins" rule are one step because they are one promise:
/// the system prompt asked for three distinct shapes, materials and scene descriptions, and two
/// proposals that differ only in a hex value are not what the user asked to choose between. Keeping
/// the two apart let `generate` be the only caller that applied the second half.
pub fn plan_ai_skins(text: &str) -> Result<Vec<AiSkinPlan>, AiSkinError> {
    let plans = parse(text)?;
    if distinct(plans.iter().filter_map(|plan| plan.design.key_shape)) != 3
        || distinct(plans.iter().filter_map(|plan| plan.design.key_material)) != 3
        || plans
            .iter()
            .map(|plan| plan.artwork_prompt.as_str())
            .collect::<HashSet<_>>()
            .len()
            != 3
    {
        return Err(AiSkinError::InvalidResponse);
    }
    Ok(plans)
}

fn distinct<T: PartialEq>(values: impl Iterator<Item = T>) -> usize {
    values
        .fold(Vec::with_capacity(3), |mut seen, value| {
            if !seen.contains(&value) {
                seen.push(value);
            }
            seen
        })
        .len()
}

/// Whether a returned image is one this client will show, rather than merely bytes.
pub fn validate_ai_skin_artwork(artwork: &AiSkinArtwork) -> Result<(), AiSkinError> {
    validate_artwork(artwork).map(|_| ())
}

fn parse(text: &str) -> Result<Vec<AiSkinPlan>, AiSkinError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Response {
        skins: Vec<RawDesign>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawDesign {
        name: String,
        description: String,
        #[serde(rename = "artworkPrompt")]
        artwork_prompt: String,
        background: String,
        #[serde(rename = "keyBackground")]
        key_background: String,
        #[serde(rename = "keyForeground")]
        key_foreground: String,
        accent: String,
        #[serde(rename = "actionBackground")]
        action_background: String,
        #[serde(rename = "gradientEnd")]
        gradient_end: Option<String>,
        #[serde(rename = "gradientHorizontal")]
        gradient_horizontal: bool,
        #[serde(rename = "keyShape")]
        key_shape: TouchSkinKeyShape,
        #[serde(rename = "keyMaterial")]
        key_material: TouchSkinKeyMaterial,
        #[serde(rename = "cornerRadius")]
        corner_radius: f64,
        #[serde(rename = "borderWidth")]
        border_width: f64,
        shadow: f64,
        pattern: u8,
        monospaced: bool,
    }
    fn color(value: &str) -> Result<u32, AiSkinError> {
        if value.len() != 7 || !value.starts_with('#') {
            return Err(AiSkinError::InvalidResponse);
        }
        u32::from_str_radix(&value[1..], 16).map_err(|_| AiSkinError::InvalidResponse)
    }
    if text.len() > MAX_CHAT_RESPONSE_BYTES {
        return Err(AiSkinError::InvalidResponse);
    }
    let response: Response =
        serde_json::from_str(text).map_err(|_| AiSkinError::InvalidResponse)?;
    if response.skins.len() != 3 {
        return Err(AiSkinError::InvalidResponse);
    }
    let mut result = Vec::with_capacity(3);
    for source in response.skins {
        if !(1..=32).contains(&source.name.trim().chars().count())
            || !(1..=280).contains(&source.description.chars().count())
            || !(40..=100).contains(&source.artwork_prompt.trim().chars().count())
            || crate::has_disallowed_control_with_options(&source.name, false)
            || crate::has_disallowed_control_with_options(&source.description, false)
            || crate::has_disallowed_control_with_options(&source.artwork_prompt, false)
            || !source.corner_radius.is_finite()
            || !(0.0..=20.0).contains(&source.corner_radius)
            || !source.border_width.is_finite()
            || !(0.0..=2.0).contains(&source.border_width)
            || !source.shadow.is_finite()
            || !(0.0..=0.4).contains(&source.shadow)
            || source.pattern > 3
        {
            return Err(AiSkinError::InvalidResponse);
        }
        let mut design = TouchKeyboardSkinDesign {
            background: color(&source.background)?,
            key_background: color(&source.key_background)?,
            key_foreground: color(&source.key_foreground)?,
            accent: color(&source.accent)?,
            action_background: color(&source.action_background)?,
            corner_radius: source.corner_radius,
            border_width: source.border_width,
            shadow: source.shadow,
            pattern: source.pattern,
            monospaced: source.monospaced,
            key_shape: Some(source.key_shape),
            key_material: Some(source.key_material),
            key_opacity: None,
            gradient_end: source.gradient_end.map(|value| color(&value)).transpose()?,
            gradient_horizontal: Some(source.gradient_horizontal),
            pattern_opacity: None,
            custom_border_color: None,
            photo: None,
            photo_shade: None,
            photo_position: None,
        };
        if contrast(design.key_foreground, design.key_background) < 4.5 {
            design.key_foreground = readable_text(design.key_background);
        }
        if contrast(design.accent, design.background) < 4.5
            || contrast(design.accent, design.key_background) < 4.5
            || design
                .gradient_end
                .is_some_and(|color| contrast(design.accent, color) < 4.5)
        {
            let surfaces = [
                design.background,
                design.key_background,
                design.gradient_end.unwrap_or(design.background),
            ];
            let black = surfaces
                .iter()
                .map(|color| contrast(0, *color))
                .fold(f64::INFINITY, f64::min);
            let white = surfaces
                .iter()
                .map(|color| contrast(0xFFFFFF, *color))
                .fold(f64::INFINITY, f64::min);
            design.accent = if black >= white { 0 } else { 0xFFFFFF };
        }
        if !design.validate() || !has_readable_text(&design) {
            return Err(AiSkinError::InvalidResponse);
        }
        result.push(AiSkinPlan {
            name: source.name,
            description: source.description,
            artwork_prompt: source.artwork_prompt,
            design,
        });
    }
    if result.iter().enumerate().any(|(index, plan)| {
        result[..index]
            .iter()
            .any(|previous| previous.design == plan.design)
    }) {
        return Err(AiSkinError::InvalidResponse);
    }
    Ok(result)
}

fn contrast(first: u32, second: u32) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn has_readable_text(design: &TouchKeyboardSkinDesign) -> bool {
    contrast(design.key_foreground, design.key_background) >= 4.5
        && contrast(design.accent, design.background) >= 4.5
        && contrast(design.accent, design.key_background) >= 4.5
        && design
            .gradient_end
            .is_none_or(|color| contrast(design.accent, color) >= 4.5)
}

#[cfg(test)]
mod anonymous_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        AccountChallenge, AccountProfile, AccountSessionStorage, AccountTokens, AccountUser,
        SavedAccountSession,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;

    #[derive(Clone, Default)]
    pub(super) struct MemoryStorage(pub(super) Arc<Mutex<Option<SavedAccountSession>>>);

    impl AccountSessionStorage for MemoryStorage {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
            *self.0.lock().unwrap() = Some(session.clone());
            Ok(())
        }

        fn clear(&self) -> Result<(), AccountError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    /// `working` 为假时 AI 接口一律失败；为真时按合成数据走完一次生成：模型目录、三套设计、每张插画第一次读就已完成。
    #[derive(Clone, Default)]
    struct FakeApi {
        working: bool,
    }

    impl AccountApi for FakeApi {
        fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
            Ok(HashMap::new())
        }
        fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
            Ok(AccountTokens {
                access_token: token(b'c'),
                refresh_token: token(b'd'),
                token_type: "Bearer".into(),
                expires_in: 900,
                user: AccountUser {
                    id: "fixture-user".into(),
                    display_name: "Fixture".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            })
        }
        fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }
        fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
            Ok(())
        }
        fn delete_account(&self, _: &str) -> Result<(), AccountError> {
            Ok(())
        }
    }

    pub(super) fn artwork() -> AiSkinArtwork {
        AiSkinArtwork {
            b64_json: BASE64.encode(b"\x89PNG\r\n\x1A\nfixture"),
            mime_type: "image/png".into(),
            width: 64,
            height: 64,
        }
    }

    pub(super) fn job(id: &str, state: &str, artwork: Option<AiSkinArtwork>) -> AiSkinJob {
        AiSkinJob {
            id: id.into(),
            state: state.into(),
            artwork,
            reason: None,
            expires_at: None,
        }
    }

    impl AiSkinApi for FakeApi {
        fn ai_models(&self, _: &str) -> Result<AiModelCatalog, AccountFailure> {
            if !self.working {
                return Err(AccountError::Unavailable.into());
            }
            Ok(AiModelCatalog {
                data: vec![AiModel {
                    id: "synthetic-model".into(),
                }],
                default_model: "synthetic-model".into(),
            })
        }
        fn ai_chat(
            &self,
            _: &[AiChatMessage<'_>],
            _: &str,
            _: &str,
        ) -> Result<String, AccountFailure> {
            if !self.working {
                return Err(AccountError::Unavailable.into());
            }
            Ok(response())
        }
        fn create_skin_artwork(&self, prompt: &str, _: &str) -> Result<AiSkinJob, AccountFailure> {
            if !self.working {
                return Err(AccountError::Unavailable.into());
            }
            // 三套设计的插画提示词只差一个数字，按字节和区分出三个任务 id。
            let seed = prompt
                .bytes()
                .fold(0_u8, |sum, byte| sum.wrapping_add(byte));
            let id = format!("{seed:02x}").repeat(24);
            Ok(job(&id, "running", None))
        }
        fn get_skin_artwork(&self, id: &str, _: &str) -> Result<AiSkinJob, AccountFailure> {
            if !self.working {
                return Err(AccountError::Unavailable.into());
            }
            Ok(job(id, "succeeded", Some(artwork())))
        }
        fn delete_skin_artwork(&self, _: &str, _: &str) -> Result<(), AccountFailure> {
            if !self.working {
                return Err(AccountError::Unavailable.into());
            }
            Ok(())
        }
    }

    pub(super) fn token(byte: u8) -> String {
        std::iter::repeat_n(char::from(byte), 64).collect()
    }

    fn installed(storage: &MemoryStorage) {
        *storage.0.lock().unwrap() = Some(SavedAccountSession {
            tokens: AccountTokens {
                access_token: token(b'a'),
                refresh_token: token(b'b'),
                token_type: "Bearer".into(),
                expires_in: 900,
                user: AccountUser {
                    id: "fixture-user".into(),
                    display_name: "Fixture".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 60_000,
            session_id: None,
        });
    }

    pub(super) fn response() -> String {
        let designs = [
            ("rounded", "flat", "#FFFFFF", "#17251D", "#185C47"),
            ("capsule", "raised", "#303E4F", "#EFF5FC", "#CEE0F3"),
            ("ticket", "paper", "#FFF5DF", "#382A1C", "#53391F"),
        ];
        let skins = designs
            .iter()
            .enumerate()
            .map(|(index, (shape, material, key_background, key_foreground, accent))| {
                serde_json::json!({
                    "name": format!("测试设计 {index}"),
                    "description": "合成测试方案",
                    "artworkPrompt": format!("原创背景场景 {index}：原创角色在画面边缘，柔和水彩，自然装饰，中央安静留白，独特氛围"),
                    "background": if index == 1 { "#181F2B" } else { "#E8F0EB" },
                    "keyBackground": key_background,
                    "keyForeground": key_foreground,
                    "accent": accent,
                    "actionBackground": accent,
                    "gradientEnd": null,
                    "gradientHorizontal": false,
                    "keyShape": shape,
                    "keyMaterial": material,
                    "cornerRadius": index * 5,
                    "borderWidth": 0,
                    "shadow": 0.1,
                    "pattern": index,
                    "monospaced": false,
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({ "skins": skins }).to_string()
    }

    #[test]
    fn parses_three_distinct_designs_and_repairs_text_contrast() {
        let plans = parse(&response()).expect("valid fixture");
        assert_eq!(plans.len(), 3);
        assert_eq!(
            plans
                .iter()
                .filter_map(|plan| plan.design.key_shape)
                .collect::<Vec<_>>()
                .len(),
            3
        );
        assert!(plans.iter().all(|plan| has_readable_text(&plan.design)));
    }

    #[test]
    fn rejects_malformed_or_unsafe_designs() {
        let mut value: serde_json::Value = serde_json::from_str(&response()).unwrap();
        value["skins"][0]["background"] = serde_json::json!("https://example.invalid/a.png");
        assert!(matches!(
            parse(&value.to_string()),
            Err(AiSkinError::InvalidResponse)
        ));
        let mut value: serde_json::Value = serde_json::from_str(&response()).unwrap();
        value["skins"][0]["artworkPrompt"] = serde_json::json!("太短");
        assert!(matches!(
            parse(&value.to_string()),
            Err(AiSkinError::InvalidResponse)
        ));
    }

    #[test]
    fn validates_png_and_jpeg_artwork_boundaries() {
        let png = AiSkinArtwork {
            b64_json: BASE64.encode(b"\x89PNG\r\n\x1A\nfixture"),
            mime_type: "image/png".into(),
            width: 64,
            height: 64,
        };
        assert!(validate_artwork(&png).is_ok());
        let mut bad = png.clone();
        bad.mime_type = "image/jpeg".into();
        assert!(validate_artwork(&bad).is_err());
    }

    #[test]
    fn authenticated_ai_result_is_cancelled_after_same_user_relogin() {
        let storage = MemoryStorage::default();
        installed(&storage);
        let session = Arc::new(BackendAccountSession::new(FakeApi::default(), storage));
        let service = BackendAiSkinService::new(FakeApi::default(), Arc::clone(&session));
        let (login, token) = service.login().unwrap();
        assert_eq!(login.user_id, "fixture-user");
        let result = service.request_authenticated(&login, token, |_, _| {
            session.forget().unwrap();
            // The replacement session has the same user id, which the old check accepted.
            session.sign_in("synthetic-challenge", "123456").unwrap();
            Ok::<_, AccountFailure>("stale result")
        });
        assert_eq!(result, Err(AiSkinFailure::Cancelled));
    }

    #[test]
    fn progress_is_reported_on_the_calling_thread_in_order() {
        let storage = MemoryStorage::default();
        installed(&storage);
        let api = FakeApi { working: true };
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
        let service = BackendAiSkinService::new(api, session);
        let caller = std::thread::current().id();
        let mut seen = Vec::new();
        let cancelled = AtomicBool::new(false);
        let proposals = service
            .generate("合成的提示词", &cancelled, |completed| {
                assert_eq!(std::thread::current().id(), caller);
                seen.push(completed);
            })
            .unwrap();
        assert_eq!(proposals.len(), 3);
        assert_eq!(seen, [1, 2, 3]);
    }

    #[test]
    fn job_requests_validate_prompts_and_ids_before_the_network() {
        let storage = MemoryStorage::default();
        installed(&storage);
        let session = Arc::new(BackendAccountSession::new(FakeApi::default(), storage));
        let service = BackendAiSkinService::new(FakeApi::default(), session);
        assert_eq!(
            service.create_artwork_job("   ").unwrap_err(),
            AiSkinFailure::InvalidResponse
        );
        assert_eq!(
            service.create_artwork_job(&"字".repeat(1201)).unwrap_err(),
            AiSkinFailure::InvalidResponse
        );
        assert_eq!(
            service.create_artwork_job("nul\0").unwrap_err(),
            AiSkinFailure::InvalidResponse
        );
        assert_eq!(
            service.artwork_job("not-a-job").unwrap_err(),
            AiSkinFailure::InvalidResponse
        );
        assert_eq!(
            service.delete_artwork_job(&"A".repeat(48)).unwrap_err(),
            AiSkinFailure::InvalidResponse
        );
        // 合法的请求到了 API 这一层，FakeApi 不工作时是账号错误。
        assert_eq!(
            service.artwork_job(&"a".repeat(48)).unwrap_err(),
            AiSkinFailure::Account(AccountError::Unavailable.into())
        );
    }

    #[test]
    fn signed_out_job_requests_are_unauthorized() {
        let session = Arc::new(BackendAccountSession::new(
            FakeApi::default(),
            MemoryStorage::default(),
        ));
        let service = BackendAiSkinService::new(FakeApi::default(), session);
        assert_eq!(
            service.create_artwork_job("合成提示词").unwrap_err(),
            AiSkinFailure::Account(AccountError::Unauthorized.into())
        );
    }

    fn serve(status: &'static str, body: String) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        origin
    }

    /// 回放 msime-cloud `origin/main` 的真实响应（`internal/server/skin_jobs.go` 的 `respondSkinJobCreated` 与 `respondSkinJob`）。修复前 `RawSkinJob` 拒绝未知字段，这两种响应都解析失败。
    #[test]
    fn production_job_responses_parse() {
        let id = "0123456789abcdef".repeat(3);
        let client = BackendAccountClient::loopback(&serve(
            "202 Accepted",
            serde_json::json!({ "id": id, "state": "running", "expires_at": "2026-10-16T09:40:00Z" })
                .to_string(),
        ))
        .unwrap();
        let created = client
            .create_skin_artwork("合成的插画提示词", &token(b'a'))
            .unwrap();
        assert_eq!(created.state, "running");
        assert_eq!(created.expires_at.as_deref(), Some("2026-10-16T09:40:00Z"));

        let client = BackendAccountClient::loopback(&serve(
            "200 OK",
            serde_json::json!({ "id": id, "state": "failed", "artwork": null, "reason": "upstream_timeout" })
                .to_string(),
        ))
        .unwrap();
        let failed = client.get_skin_artwork(&id, &token(b'a')).unwrap();
        assert_eq!(failed.state, "failed");
        assert_eq!(failed.reason.as_deref(), Some("upstream_timeout"));
        assert_eq!(failed.artwork, None);
        assert_eq!(
            serde_json::to_value(&failed).unwrap(),
            serde_json::json!({ "id": id, "state": "failed", "reason": "upstream_timeout" })
        );

        // 不合规的 reason 和 expires_at 只丢掉那个字段，任务本身照常可用。
        let client = BackendAccountClient::loopback(&serve(
            "200 OK",
            serde_json::json!({ "id": id, "state": "running", "artwork": null, "reason": "bad\u{7}", "expires_at": "soon", "added_later": 1 })
                .to_string(),
        ))
        .unwrap();
        let running = client.get_skin_artwork(&id, &token(b'a')).unwrap();
        assert_eq!((running.reason, running.expires_at), (None, None));

        // 成功的任务带插画；服务端以后在 artwork 里加的字段不让任务解析失败。
        let mut served = serde_json::to_value(artwork()).unwrap();
        served["revised_prompt"] = serde_json::json!("合成的改写提示词");
        let client = BackendAccountClient::loopback(&serve(
            "200 OK",
            serde_json::json!({ "id": id, "state": "succeeded", "artwork": served }).to_string(),
        ))
        .unwrap();
        let succeeded = client.get_skin_artwork(&id, &token(b'a')).unwrap();
        assert_eq!(succeeded.state, "succeeded");
        assert_eq!(succeeded.artwork, Some(artwork()));
        // 宿主交来的插画仍然拒绝未知字段。
        assert!(serde_json::from_value::<AiSkinArtwork>(served).is_err());
    }

    #[test]
    fn duplicate_request_keeps_original_generation_cancellable() {
        let requests = AiSkinRequests::default();
        let original = requests.begin("synthetic-request").unwrap();
        assert_eq!(
            requests.begin("synthetic-request").unwrap_err(),
            "ai_skin_busy"
        );
        assert_eq!(requests.cancel("synthetic-request"), Ok(true));
        assert!(original.load(Ordering::Acquire));
    }

    #[test]
    fn cancelling_one_request_does_not_cancel_another() {
        let requests = AiSkinRequests::default();
        let first = requests.begin("synthetic-first").unwrap();
        let second = requests.begin("synthetic-second").unwrap();
        assert_eq!(requests.cancel("synthetic-missing"), Ok(false));
        assert_eq!(requests.cancel("synthetic-first"), Ok(true));
        assert!(first.load(Ordering::Acquire));
        assert!(!second.load(Ordering::Acquire));
    }

    #[test]
    fn finished_request_id_can_be_reused_with_a_new_cancel_flag() {
        let requests = AiSkinRequests::default();
        let first = requests.begin("synthetic-request").unwrap();
        requests.finish("synthetic-request");
        let second = requests.begin("synthetic-request").unwrap();
        requests.cancel("synthetic-request").unwrap();
        assert!(!first.load(Ordering::Acquire));
        assert!(second.load(Ordering::Acquire));
    }
}
