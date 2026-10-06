//! Android's account-backed AI keyboard-skin generator.
//!
//! The host owns cancellation and progress presentation. This module owns the
//! backend contract, response validation, account refreshes, and artwork-job
//! cleanup so that no UI or platform type enters client-core.

use crate::account::{
    AccountApi, AccountError, AccountSessionStorage, BackendAccountClient, BackendAccountSession,
};
use crate::preferences::{TouchKeyboardSkinDesign, TouchSkinKeyMaterial, TouchSkinKeyShape};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const MAX_PROMPT_CHARACTERS: usize = 500;
const MAX_CHAT_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_ARTWORK_RESPONSE_BYTES: usize = 12 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTWORK_SECONDS: u64 = 200;

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

#[derive(Clone, Debug)]
pub struct AiSkinJob {
    pub id: String,
    pub state: String,
    pub artwork: Option<AiSkinArtwork>,
}

pub trait AiSkinApi: Send + Sync + 'static {
    fn ai_models(&self, token: &str) -> Result<AiModelCatalog, AccountError>;
    fn ai_chat(
        &self,
        messages: &[AiChatMessage<'_>],
        model: &str,
        token: &str,
    ) -> Result<String, AccountError>;
    fn create_skin_artwork(&self, prompt: &str, token: &str) -> Result<AiSkinJob, AccountError>;
    fn get_skin_artwork(&self, id: &str, token: &str) -> Result<AiSkinJob, AccountError>;
    fn delete_skin_artwork(&self, id: &str, token: &str) -> Result<(), AccountError>;
}

impl AiSkinApi for BackendAccountClient {
    fn ai_models(&self, token: &str) -> Result<AiModelCatalog, AccountError> {
        let catalog = self.json_with_limit_timeout::<AiModelCatalog, ()>(
            Method::GET,
            "/v1/models",
            Some(token),
            None,
            128 * 1024,
            Duration::from_secs(30),
        )?;
        if !crate::account::valid_model_catalog(
            catalog.data.iter().map(|model| model.id.as_str()),
            &catalog.default_model,
            33,
            200,
        ) {
            return Err(AccountError::Unavailable);
        }
        Ok(catalog)
    }

    fn ai_chat(
        &self,
        messages: &[AiChatMessage<'_>],
        model: &str,
        token: &str,
    ) -> Result<String, AccountError> {
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
            return Err(AccountError::Invalid);
        }
        let body = Body {
            messages,
            model,
            max_tokens: 2048,
            stream: false,
        };
        let encoded = serde_json::to_vec(&body).map_err(|_| AccountError::Invalid)?;
        if encoded.len() > 64 * 1024 {
            return Err(AccountError::Invalid);
        }
        let result: Response = self.json_with_limit_timeout(
            Method::POST,
            "/v1/chat/completions",
            Some(token),
            Some(&body),
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

    fn create_skin_artwork(&self, prompt: &str, token: &str) -> Result<AiSkinJob, AccountError> {
        #[derive(Serialize)]
        struct Body<'a> {
            prompt: &'a str,
        }
        let job: RawSkinJob = self.json_with_limit_timeout(
            Method::POST,
            "/v1/skins/jobs",
            Some(token),
            Some(&Body { prompt }),
            128 * 1024,
            Duration::from_secs(30),
        )?;
        validate_job(job)
    }

    fn get_skin_artwork(&self, id: &str, token: &str) -> Result<AiSkinJob, AccountError> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/skins/jobs/{id}");
        let job = validate_job(self.json_with_limit_timeout(
            Method::GET,
            &path,
            Some(token),
            None::<&()>,
            MAX_ARTWORK_RESPONSE_BYTES,
            Duration::from_secs(30),
        )?)?;
        if job.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(job)
    }

    fn delete_skin_artwork(&self, id: &str, token: &str) -> Result<(), AccountError> {
        if !crate::text::is_lower_hex(id, 48) {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/skins/jobs/{id}");
        self.request_with_limit_timeout(
            Method::DELETE,
            &path,
            Some(token),
            None,
            16 * 1024,
            Duration::from_secs(5),
        )
        .map(|_| ())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkinJob {
    id: String,
    state: String,
    artwork: Option<AiSkinArtwork>,
}

fn validate_job(job: RawSkinJob) -> Result<AiSkinJob, AccountError> {
    if !crate::text::is_lower_hex(&job.id, 48)
        || !matches!(job.state.as_str(), "running" | "succeeded" | "failed")
    {
        return Err(AccountError::Unavailable);
    }
    if let Some(artwork) = &job.artwork {
        validate_artwork(artwork).map_err(|_| AccountError::Unavailable)?;
    }
    Ok(AiSkinJob {
        id: job.id,
        state: job.state,
        artwork: job.artwork,
    })
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
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAiSkinService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendAiSkinService<A, S>
where
    A: AccountApi + AiSkinApi,
    S: AccountSessionStorage,
{
    pub fn generate(
        &self,
        prompt: &str,
        cancelled: &AtomicBool,
        progress: impl Fn(usize) + Send + Sync,
    ) -> Result<Vec<AiSkinProposal>, AiSkinError> {
        if prompt.is_empty() || !crate::text::is_bounded_chars(prompt, MAX_PROMPT_CHARACTERS) {
            return Err(AiSkinError::InvalidResponse);
        }
        check_cancelled(cancelled)?;
        let (user_id, token, generation) = self
            .session
            .credentials_with_generation(None, None)
            .map_err(Self::account_error)?;
        let catalog = self.request_authenticated(&user_id, generation, token, |api, token| {
            api.ai_models(token)
        })?;
        check_cancelled(cancelled)?;
        let token = self.credentials_for_generation(&user_id, generation)?;
        let response = self.request_authenticated(&user_id, generation, token, |api, token| {
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
        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let user_id_ref = user_id.as_str();
        let progress_ref = &progress;
        let mut results = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(3);
            for (index, plan) in plans.into_iter().enumerate() {
                let completed = Arc::clone(&completed);
                let user_id = user_id_ref;
                let progress = progress_ref;
                handles.push(scope.spawn(move || {
                    let result = self.illustrate(user_id, generation, &plan, cancelled);
                    if result.is_ok() {
                        let count = completed.fetch_add(1, Ordering::AcqRel) + 1;
                        progress(count);
                    }
                    (index, result)
                }));
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
                            first_error = Some(AiSkinError::Account(AccountError::Unavailable));
                        }
                    }
                }
            }
            first_error.map_or_else(|| Ok(results), Err)
        })?;
        results.sort_by_key(|(index, _)| *index);
        let mut ordered = Vec::with_capacity(results.len());
        ordered.extend(results.into_iter().map(|(_, result)| result));
        self.ensure_identity(&user_id, generation)?;
        Ok(ordered)
    }

    fn request_authenticated<T>(
        &self,
        user_id: &str,
        generation: u64,
        token: String,
        operation: impl Fn(&A, &str) -> Result<T, AccountError>,
    ) -> Result<T, AiSkinError> {
        let result = match operation(&self.api, &token) {
            Err(AccountError::Unauthorized) => {
                let (_, replacement, replacement_generation) = self
                    .session
                    .credentials_with_generation(Some(&token), Some(user_id))
                    .map_err(Self::account_error)?;
                if replacement_generation != generation {
                    return Err(AiSkinError::Cancelled);
                }
                operation(&self.api, &replacement)
            }
            result => result,
        }?;
        self.ensure_identity(user_id, generation)?;
        Ok(result)
    }

    fn ensure_identity(&self, user_id: &str, generation: u64) -> Result<(), AiSkinError> {
        self.session
            .with_generation(generation, Some(user_id), || Ok(()))
            .map_err(Self::account_error)
    }

    fn account_error(error: AccountError) -> AiSkinError {
        if error == AccountError::Cancelled {
            AiSkinError::Cancelled
        } else {
            AiSkinError::Account(error)
        }
    }

    fn credentials_for_generation(
        &self,
        user_id: &str,
        generation: u64,
    ) -> Result<String, AiSkinError> {
        let (_, token, current_generation) = self
            .session
            .credentials_with_generation(None, Some(user_id))
            .map_err(Self::account_error)?;
        if current_generation != generation {
            return Err(AiSkinError::Cancelled);
        }
        Ok(token)
    }

    fn illustrate(
        &self,
        user_id: &str,
        generation: u64,
        plan: &AiSkinPlan,
        cancelled: &AtomicBool,
    ) -> Result<AiSkinProposal, AiSkinError> {
        check_cancelled(cancelled)?;
        let token = self.credentials_for_generation(user_id, generation)?;
        let job = match self.api.create_skin_artwork(&plan.artwork_prompt, &token) {
            Err(AccountError::Unauthorized) => {
                let (_, replacement, replacement_generation) = self
                    .session
                    .credentials_with_generation(Some(&token), Some(user_id))
                    .map_err(Self::account_error)?;
                if replacement_generation != generation {
                    return Err(AiSkinError::Cancelled);
                }
                self.api
                    .create_skin_artwork(&plan.artwork_prompt, &replacement)?
            }
            result => result?,
        };
        let id = job.id;
        let result = self.poll_artwork(user_id, generation, &id, cancelled);
        // Completed and failed jobs are both released; cancellation must not
        // leave an upstream task billable after this call returns.
        self.cleanup_job(user_id, generation, &id);
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
        user_id: &str,
        generation: u64,
        id: &str,
        cancelled: &AtomicBool,
    ) -> Result<AiSkinArtwork, AiSkinError> {
        let deadline = Instant::now() + Duration::from_secs(MAX_ARTWORK_SECONDS);
        loop {
            check_cancelled(cancelled)?;
            if Instant::now() >= deadline {
                return Err(AiSkinError::Account(AccountError::Unavailable));
            }
            let token = self.credentials_for_generation(user_id, generation)?;
            let job = match self.api.get_skin_artwork(id, &token) {
                Err(AccountError::Unauthorized) => {
                    let (_, replacement, replacement_generation) = self
                        .session
                        .credentials_with_generation(Some(&token), Some(user_id))
                        .map_err(Self::account_error)?;
                    if replacement_generation != generation {
                        return Err(AiSkinError::Cancelled);
                    }
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
                    let artwork = job.artwork.ok_or(AiSkinError::InvalidResponse)?;
                    validate_artwork(&artwork)?;
                    return Ok(artwork);
                }
                _ => return Err(AiSkinError::Account(AccountError::Unavailable)),
            }
        }
    }

    fn cleanup_job(&self, user_id: &str, generation: u64, id: &str) {
        let Ok(token) = self.credentials_for_generation(user_id, generation) else {
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

fn luminance(rgb: u32) -> f64 {
    let channel = |shift: u32| {
        let value = ((rgb >> shift) & 0xFF) as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn contrast(first: u32, second: u32) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

pub(crate) fn readable_text(background: u32) -> u32 {
    if luminance(background) > 0.179 {
        0
    } else {
        0xFFFFFF
    }
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
    struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

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

    #[derive(Clone, Default)]
    struct FakeApi;

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

    impl AiSkinApi for FakeApi {
        fn ai_models(&self, _: &str) -> Result<AiModelCatalog, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn ai_chat(
            &self,
            _: &[AiChatMessage<'_>],
            _: &str,
            _: &str,
        ) -> Result<String, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn create_skin_artwork(&self, _: &str, _: &str) -> Result<AiSkinJob, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn get_skin_artwork(&self, _: &str, _: &str) -> Result<AiSkinJob, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn delete_skin_artwork(&self, _: &str, _: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }
    }

    fn token(byte: u8) -> String {
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
        });
    }

    fn response() -> String {
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
        let session = Arc::new(BackendAccountSession::new(FakeApi, storage));
        let service = BackendAiSkinService::new(FakeApi, Arc::clone(&session));
        let (_, token, generation) = session
            .credentials_with_generation(None, Some("fixture-user"))
            .unwrap();
        let result = service.request_authenticated("fixture-user", generation, token, |_, _| {
            session.forget().unwrap();
            // The replacement session has the same user id, which the old check accepted.
            session.sign_in("synthetic-challenge", "123456").unwrap();
            Ok::<_, AccountError>("stale result")
        });
        assert_eq!(result, Err(AiSkinError::Cancelled));
    }
}
