use msime_client_core::is_bounded_text;
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "ios", test))]
use serde_json::json;
use serde_json::Value;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

#[cfg(any(target_os = "ios", target_os = "android"))]
use tauri::plugin::PluginHandle;
#[cfg(any(target_os = "ios", target_os = "android"))]
use tauri::Manager;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_msime_mobile_platform);

#[cfg(target_os = "android")]
#[derive(Clone)]
pub struct AndroidVoicePlatform<R: Runtime>(PluginHandle<R>);

#[cfg(target_os = "android")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidVoiceRequest<'a> {
    request_id: &'a str,
    language: &'a str,
    /// The configured transcription provider, when there is a usable one.
    ///
    /// Absent means the host should use the platform recognizer, which is what this host has
    /// always done and remains the right default here: Android ships a speech service that needs
    /// no account, no token and no network of the user's choosing. The provider is what a user
    /// gets by configuring one in settings, not something to be required of everyone.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<MobileVoiceTranscriptionRequest>,
    /// The optional rewrite that runs over whatever was transcribed, by either engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    polish: Option<AndroidVoicePolishRequest>,
}

/// The optional rewrite that runs over a transcript, when the user asked for one.
///
/// The prompt is not here: the host resolves the selected slot through the shared preset table so
/// there is one copy of the wording that marks the transcript as data rather than instructions.
#[cfg(any(target_os = "android", test))]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AndroidVoicePolishRequest {
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub prompt_id: String,
    pub prompt_custom_1: String,
    pub prompt_custom_2: String,
    pub prompt_custom_3: String,
}

#[cfg(any(target_os = "android", test))]
impl AndroidVoicePolishRequest {
    pub fn is_valid(&self) -> bool {
        msime_client_core::voice::provider::valid_mobile_voice_endpoint(&self.endpoint, false)
            && msime_client_core::voice::provider::bounded_voice_fields(
                &self.endpoint,
                &self.model,
                &self.token,
            )
            && !self.model.trim().is_empty()
            && !self.token.trim().is_empty()
            && is_bounded_text(&self.prompt_id, 64)
            && [
                &self.prompt_custom_1,
                &self.prompt_custom_2,
                &self.prompt_custom_3,
            ]
            .iter()
            .all(|slot| is_bounded_text(slot, 8192))
    }
}

#[cfg(target_os = "android")]
#[derive(Clone, Debug, Deserialize)]
struct AndroidVoiceResponse {
    text: String,
}

#[cfg(any(target_os = "android", test))]
fn valid_android_voice_request(request_id: &str, language: &str) -> bool {
    msime_client_core::voice::is_valid_request_id(request_id)
        && !language.is_empty()
        && is_bounded_text(language, 64)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn valid_mobile_voice_text(text: &str) -> bool {
    !text.trim().is_empty()
        && msime_client_core::is_bounded_chars_without_nul(text, MAX_MOBILE_VOICE_TEXT_CHARS)
}

#[cfg(target_os = "android")]
impl<R: Runtime> AndroidVoicePlatform<R> {
    /// `provider` is passed on only when it validates; an invalid one falls back to the platform
    /// recognizer rather than failing the request, because a misconfigured token should not take
    /// away the recognizer the user had before they configured anything.
    pub async fn recognize_voice(
        &self,
        request_id: &str,
        language: &str,
        provider: Option<MobileVoiceTranscriptionRequest>,
        polish: Option<AndroidVoicePolishRequest>,
    ) -> Result<String, ()> {
        if !valid_android_voice_request(request_id, language) {
            return Err(());
        }
        let provider = provider.filter(MobileVoiceTranscriptionRequest::is_valid);
        let polish = polish.filter(AndroidVoicePolishRequest::is_valid);
        let response = self
            .0
            .run_mobile_plugin_async::<AndroidVoiceResponse>(
                "recognizeVoice",
                AndroidVoiceRequest {
                    request_id,
                    language,
                    provider,
                    polish,
                },
            )
            .await
            .map_err(|_| ())?;
        if !msime_client_core::is_bounded_chars_without_nul(&response.text, 10_000) {
            return Err(());
        }
        Ok(response.text)
    }

    pub fn stop_voice(&self, request_id: &str) -> Result<(), ()> {
        if !valid_android_voice_request(request_id, "und") {
            return Err(());
        }
        self.0
            .run_mobile_plugin("stopVoice", serde_json::json!({ "requestId": request_id }))
            .map_err(|_| ())
    }

    pub fn cancel_voice(&self, request_id: Option<&str>) -> Result<(), ()> {
        if request_id.is_some_and(|value| !valid_android_voice_request(value, "und")) {
            return Err(());
        }
        self.0
            .run_mobile_plugin(
                "cancelVoice",
                serde_json::json!({ "requestId": request_id }),
            )
            .map_err(|_| ())
    }

    pub fn save_voice_text(&self, text: &str) -> Result<(), ()> {
        if !valid_mobile_voice_text(text) {
            return Err(());
        }
        self.0
            .run_mobile_plugin("saveVoiceText", serde_json::json!({ "text": text }))
            .map_err(|_| ())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppIconInfo {
    pub supported: bool,
    pub selected: String,
}

const MAX_IOS_CUSTOM_KEYBOARD_SKIN_BYTES: usize = 800_000;
#[cfg(any(target_os = "ios", test))]
const MAX_IOS_CLIPBOARD_TEXT_UTF16_UNITS: usize = 4_000;
#[cfg(any(target_os = "android", target_os = "ios", test))]
const MAX_MOBILE_VOICE_TEXT_CHARS: usize = 10_000;
const MAX_MOBILE_VOICE_HEADER_BYTES: usize = 8_192;
const MAX_MOBILE_VOICE_BOOSTING_TABLE_BYTES: usize = 4_096;
const MAX_MOBILE_VOICE_MODEL_PATH_BYTES: usize = 4_096;
/// Same ceiling `msime_client_voice_hotwords` applies to its `limit`.
const MAX_MOBILE_VOICE_HOTWORDS: usize = 1_000;
const MAX_MOBILE_VOICE_HOTWORD_TEXT_BYTES: usize = 256;
const MAX_MOBILE_VOICE_HOTWORD_PINYIN_BYTES: usize = 1_024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IosKeyboardPreferences {
    pub input_scheme: String,
    pub traditional_chinese_output: bool,
    pub sound_enabled: bool,
    pub haptics_enabled: bool,
    pub haptic_strength: String,
    pub english_suggestions: bool,
    /// The candidate strip draws the shared desktop candidate skin and colours instead of the keyboard skin's; the switch lives in the App Group because the keyboard reads it on every redraw.
    pub candidate_palette_follows_desktop: bool,
    /// 行内预编辑: the keyboard also writes the composition into the text field as marked text. Off by default and kept in the App Group, because the shared `tsf_preedit_style` defaults to raw in every document and would switch every existing iOS user over.
    pub inline_preedit: bool,
    /// Whether this device can vibrate for key presses: false on iPad, which has no Taptic Engine. Read-only; the plugin reports it and never stores it.
    pub haptics_available: bool,
    /// 数字行与 Tab 键 on the iPad full-width keyboard, kept in the App Group. The plugin reports it only on an iPad and writes it only when present, so a phone neither shows nor stores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet_full_keys: Option<bool>,
    /// 横屏分离式键盘：iPad 全宽键盘横屏时把键区分成左右两半，存在 App Group 的 `keyboard.tablet.split`，默认关。和 `tablet_full_keys` 一样只在 iPad 上报告、只在请求里带着时才写入。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet_split_keyboard: Option<bool>,
    pub dictionary_learning: bool,
    /// 全局主题 id（`Preferences::global_theme`），存在 App Group 的 `globalTheme` 下，键盘扩展不读偏好文档也能拿到。只有八个主题 id 有效。
    pub global_theme: String,
    /// The custom theme's keyboard design (`Preferences::custom_theme.keyboard`) as JSON; `None` when the custom theme has no design and draws its base theme's keyboard. The App Group keeps it under `customKeyboardSkin.v1`.
    pub custom_keyboard_skin: Option<String>,
}

/// The small, native-facing AI configuration shared by the Tauri settings app
/// and the keyboard extension. The extension deliberately receives only the
/// already-resolved token for the configured endpoint; the complete Rust
/// preferences document never crosses the plugin boundary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IosKeyboardAiPreferences {
    pub enabled: bool,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub prompt: String,
    pub token: String,
}

impl IosKeyboardAiPreferences {
    pub fn is_valid(&self) -> bool {
        is_bounded_text(&self.provider, 64)
            && is_bounded_text(&self.endpoint, 2_048)
            && is_bounded_text(&self.model, 512)
            && is_bounded_text(&self.prompt, 16 * 1_024)
            && is_bounded_text(&self.token, 16 * 1_024)
            && (!self.enabled
                || (!self.provider.is_empty()
                    && !self.endpoint.trim().is_empty()
                    && !self.model.trim().is_empty()
                    && !self.prompt.trim().is_empty()
                    && !self.token.trim().is_empty()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MobileVoiceRequestHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MobileVoiceTranscriptionRequest {
    pub request_id: String,
    pub provider: String,
    /// 请求格式（`msime_client_core::voice::provider::asr_request_format`）：`multipart`、`chat_audio`、`doubao_websocket` 或 `local`。原生插件按它挑请求构造，不按 provider 名字判断。
    pub request_format: String,
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub headers: Vec<MobileVoiceRequestHeader>,
    pub enable_itn: bool,
    pub enable_punctuation: bool,
    pub enable_ddc: bool,
    pub boosting_table_id: String,
    /// Provider `local` only: the absolute path of the installed model directory to run on the device.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_path: String,
    /// Provider `local` only: the user's own dictionary words, passed to a recognizer with native hotword support or applied after the final text by `msime_client_voice_hotword_correct` when the model's manifest says `"hotwords": "pinyin"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hotwords: Vec<MobileVoiceHotword>,
}

/// One user-dictionary word for on-device recognition, shaped like `msime_client_core::voice::hotwords::Hotword`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MobileVoiceHotword {
    pub text: String,
    /// Toneless, lowercase syllables separated by single spaces.
    pub pinyin: String,
}

fn valid_mobile_voice_hotwords(hotwords: &[MobileVoiceHotword]) -> bool {
    hotwords.len() <= MAX_MOBILE_VOICE_HOTWORDS
        && hotwords.iter().all(|hotword| {
            !hotword.text.trim().is_empty()
                && is_bounded_text(&hotword.text, MAX_MOBILE_VOICE_HOTWORD_TEXT_BYTES)
                && is_bounded_text(&hotword.pinyin, MAX_MOBILE_VOICE_HOTWORD_PINYIN_BYTES)
        })
}

impl MobileVoiceTranscriptionRequest {
    pub fn is_valid(&self) -> bool {
        let common = msime_client_core::voice::is_valid_request_id(&self.request_id)
            && msime_client_core::voice::provider::bounded_voice_fields(
                &self.endpoint,
                &self.model,
                &self.token,
            )
            && is_bounded_text(
                &self.boosting_table_id,
                MAX_MOBILE_VOICE_BOOSTING_TABLE_BYTES,
            )
            && is_bounded_text(&self.model_path, MAX_MOBILE_VOICE_MODEL_PATH_BYTES)
            && valid_mobile_voice_hotwords(&self.hotwords);
        // 格式必须就是这个 provider 的格式，插件只看格式，所以两者不一致的请求一律拒绝。
        if !common
            || msime_client_core::voice::provider::asr_request_format(&self.provider)
                != Some(self.request_format.as_str())
        {
            return false;
        }
        // On-device recognition: nothing is sent anywhere, so no endpoint, token, header or boosting table may ride along, and the model has to be an absolute path in the app's own storage.
        if self.provider == "local" {
            return self.model_path.starts_with('/')
                && self.endpoint.is_empty()
                && self.model.is_empty()
                && self.token.is_empty()
                && self.headers.is_empty()
                && self.boosting_table_id.is_empty();
        }
        if !self.model_path.is_empty() || !self.hotwords.is_empty() {
            return false;
        }
        // multipart 和 chat_audio 都是带 Bearer 密钥的 HTTPS 整句上传，形状检查相同；豆包是流式 WebSocket，在下面单独检查。
        if matches!(
            self.request_format.as_str(),
            msime_client_core::voice::provider::ASR_REQUEST_MULTIPART
                | msime_client_core::voice::provider::ASR_REQUEST_CHAT_AUDIO
        ) {
            return msime_client_core::voice::provider::valid_mobile_voice_endpoint(
                &self.endpoint,
                false,
            ) && !self.model.trim().is_empty()
                && self.headers.is_empty()
                && self.boosting_table_id.is_empty();
        }
        self.provider == "doubao"
            && msime_client_core::voice::provider::valid_mobile_voice_endpoint(&self.endpoint, true)
            && self.model.is_empty()
            && self.token.is_empty()
            && valid_doubao_headers(&self.headers)
    }
}

fn valid_doubao_headers(headers: &[MobileVoiceRequestHeader]) -> bool {
    if !(3..=4).contains(&headers.len())
        || headers.iter().any(|header| {
            !matches!(
                header.name.as_str(),
                "x-api-key"
                    | "x-api-app-key"
                    | "x-api-access-key"
                    | "x-api-resource-id"
                    | "x-api-request-id"
            ) || header.value.is_empty()
                || !is_bounded_text(&header.value, MAX_MOBILE_VOICE_HEADER_BYTES)
                || !header.value.is_ascii()
        })
    {
        return false;
    }
    let count = |name: &str| headers.iter().filter(|header| header.name == name).count();
    let shared = count("x-api-resource-id") == 1 && count("x-api-request-id") == 1;
    let api_key =
        count("x-api-key") == 1 && count("x-api-app-key") == 0 && count("x-api-access-key") == 0;
    let legacy =
        count("x-api-key") == 0 && count("x-api-app-key") == 1 && count("x-api-access-key") == 1;
    shared && (api_key || legacy)
}

#[cfg(any(target_os = "ios", test))]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MobileVoiceTranscriptionResponse {
    pub text: String,
}

#[cfg(any(target_os = "ios", test))]
impl MobileVoiceTranscriptionResponse {
    fn is_valid(&self) -> bool {
        msime_client_core::is_bounded_chars_without_nul(&self.text, MAX_MOBILE_VOICE_TEXT_CHARS)
    }
}

impl IosKeyboardPreferences {
    pub fn is_valid(&self) -> bool {
        matches!(
            self.input_scheme.as_str(),
            "quanpin"
                | "nineKey"
                | "shuangpin"
                | "ziranma"
                | "microsoft"
                | "shoudao"
                | "wubi"
                | "japaneseNineKey"
                | "japanese"
                | "korean"
                | "handwriting"
                | "cantonese"
                | "zhuyin"
                | "vietnamese"
                | "tibetan"
                | "stroke"
        ) && matches!(
            self.haptic_strength.as_str(),
            "light" | "medium" | "strong" | "system"
        )
            // The ids of msime_client_core::skin::theme::GlobalTheme, which this crate does not depend on; the desktop crate's iOS account tests hold the two lists together.
            && matches!(
                self.global_theme.as_str(),
                "system" | "native" | "shuishan" | "light" | "paper" | "night" | "ink" | "custom"
            )
            && self.custom_keyboard_skin.as_ref().is_none_or(|value| {
                value.len() <= MAX_IOS_CUSTOM_KEYBOARD_SKIN_BYTES
                    && serde_json::from_str::<Value>(value)
                        .is_ok_and(|document| document.is_object())
            })
    }
}

/// The installed families UIKit reports, sorted and without duplicates, or nil when the list is not something a font picker should show.
#[cfg(any(target_os = "ios", test))]
fn installed_font_families(families: Vec<String>) -> Option<Vec<String>> {
    const MAX_FAMILIES: usize = 16_384;
    const MAX_FAMILY_BYTES: usize = 128;
    if families.len() > MAX_FAMILIES
        || families
            .iter()
            .any(|family| family.trim().is_empty() || !is_bounded_text(family, MAX_FAMILY_BYTES))
    {
        return None;
    }
    let families: std::collections::BTreeSet<String> = families.into_iter().collect();
    let mut result = Vec::with_capacity(families.len());
    result.extend(families);
    Some(result)
}

#[cfg(any(target_os = "ios", test))]
fn is_valid_ios_clipboard_text(value: &str) -> bool {
    !value.is_empty()
        && msime_client_core::is_bounded_utf16(value, MAX_IOS_CLIPBOARD_TEXT_UTF16_UNITS)
        && !value.contains('\0')
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
struct AppIconRequest<'a> {
    style: &'a str,
}

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
struct AccountSessionResponse {
    value: Option<String>,
}

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
struct IosOnboardingStatusResponse {
    completed: bool,
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
struct AccountSessionRequest<'a> {
    value: &'a str,
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppleSignInRequest<'a> {
    challenge_id: &'a str,
    nonce: &'a str,
}

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppleSignInResponse {
    credential: String,
}

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
struct SkinFolderPickResponse {
    path: Option<String>,
}

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
struct FontFamiliesResponse {
    families: Vec<String>,
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
struct CopyTextRequest<'a> {
    text: &'a str,
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
struct SaveVoiceTextRequest<'a> {
    text: &'a str,
}

#[cfg(target_os = "ios")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoiceControlRequest<'a> {
    request_id: Option<&'a str>,
}

#[cfg(any(target_os = "ios", test))]
#[derive(Deserialize)]
struct LegacyAppleAccountSession {
    tokens: Value,
    #[serde(rename = "expiresAt")]
    expires_at: f64,
}

#[cfg(any(target_os = "ios", test))]
const MAX_ACCOUNT_SESSION_BYTES: usize = 16 * 1024;
#[cfg(any(target_os = "ios", test))]
const APPLE_REFERENCE_DATE_UNIX_OFFSET_SECONDS: f64 = 978_307_200.0;

pub fn is_supported_app_icon_style(style: &str) -> bool {
    matches!(style, "classic" | "forest" | "sky" | "dusk" | "vermilion")
}

#[cfg(any(target_os = "ios", test))]
fn is_valid_account_session_payload(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_ACCOUNT_SESSION_BYTES
}

#[cfg(any(target_os = "ios", test))]
fn apple_date_to_unix_millis(seconds: f64) -> Option<u64> {
    let milliseconds = (seconds + APPLE_REFERENCE_DATE_UNIX_OFFSET_SECONDS) * 1000.0;
    if !milliseconds.is_finite() || milliseconds < 0.0 || milliseconds > u64::MAX as f64 {
        return None;
    }
    Some(milliseconds.round() as u64)
}

#[cfg(any(target_os = "ios", test))]
fn migrated_account_session_payload(value: &str) -> Option<String> {
    let document: Value = serde_json::from_str(value).ok()?;
    if document.get("expires_at_unix_ms").is_some() {
        return None;
    }

    if document.get("expiresAt").is_some() {
        let legacy: LegacyAppleAccountSession = serde_json::from_value(document).ok()?;
        let expires_at_unix_ms = apple_date_to_unix_millis(legacy.expires_at)?;
        return serde_json::to_string(&json!({
            "tokens": legacy.tokens,
            "expires_at_unix_ms": expires_at_unix_ms,
        }))
        .ok();
    }

    None
}

#[cfg(target_os = "ios")]
pub struct MobilePlatform<R: Runtime>(PluginHandle<R>);

#[cfg(target_os = "ios")]
impl<R: Runtime> Clone for MobilePlatform<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(target_os = "ios")]
impl<R: Runtime> MobilePlatform<R> {
    pub fn open_system_keyboard_settings(&self) -> Result<(), ()> {
        self.0
            .run_mobile_plugin("openSystemKeyboardSettings", ())
            .map_err(|_| ())
    }

    pub fn onboarding_completed(&self) -> Result<bool, ()> {
        self.0
            .run_mobile_plugin::<IosOnboardingStatusResponse>("onboardingStatus", ())
            .map(|response| response.completed)
            .map_err(|_| ())
    }

    pub fn complete_onboarding(&self) -> Result<(), ()> {
        self.0
            .run_mobile_plugin("completeOnboarding", ())
            .map_err(|_| ())
    }

    pub fn app_icon_info(&self) -> Result<AppIconInfo, ()> {
        self.0.run_mobile_plugin("appIconInfo", ()).map_err(|_| ())
    }

    pub fn set_app_icon(&self, style: &str) -> Result<AppIconInfo, ()> {
        self.0
            .run_mobile_plugin("setAppIcon", AppIconRequest { style })
            .map_err(|_| ())
    }

    pub fn load_account_session(&self) -> Result<Option<String>, ()> {
        let response = self
            .0
            .run_mobile_plugin::<AccountSessionResponse>("loadSession", ())
            .map_err(|_| ())?;
        response
            .value
            .map(|value| {
                if !is_valid_account_session_payload(&value) {
                    return Err(());
                }
                if let Some(migrated) = migrated_account_session_payload(&value) {
                    self.save_account_session(&migrated)?;
                    Ok(migrated)
                } else {
                    Ok(value)
                }
            })
            .transpose()
    }

    pub fn save_account_session(&self, value: &str) -> Result<(), ()> {
        if !is_valid_account_session_payload(value) {
            return Err(());
        }
        self.0
            .run_mobile_plugin("saveSession", AccountSessionRequest { value })
            .map_err(|_| ())
    }

    pub fn clear_account_session(&self) -> Result<(), ()> {
        self.0.run_mobile_plugin("clearSession", ()).map_err(|_| ())
    }

    pub async fn sign_in_with_apple(&self, challenge_id: &str, nonce: &str) -> Result<String, ()> {
        if challenge_id.is_empty()
            || challenge_id.len() > 256
            || msime_client_core::has_disallowed_control_with_options(challenge_id, false)
            || nonce.is_empty()
            || nonce.len() > 4096
            || msime_client_core::has_disallowed_control_with_options(nonce, false)
        {
            return Err(());
        }
        let response = self
            .0
            .run_mobile_plugin_async::<AppleSignInResponse>(
                "signInWithApple",
                AppleSignInRequest {
                    challenge_id,
                    nonce,
                },
            )
            .await
            .map_err(|_| ())?;
        (!response.credential.is_empty()
            && response.credential.len() <= 16 * 1024
            && !msime_client_core::has_disallowed_control_with_options(&response.credential, false))
        .then_some(response.credential)
        .ok_or(())
    }

    /// Let the user pick a skin folder in Files. `None` when they dismissed the picker. The folder stays readable to this process until [`Self::end_skin_folder_access`], which the caller must invoke once it has copied the folder.
    pub async fn pick_skin_folder(&self) -> Result<Option<std::path::PathBuf>, ()> {
        let response = self
            .0
            .run_mobile_plugin_async::<SkinFolderPickResponse>("pickSkinFolder", ())
            .await
            .map_err(|_| ())?;
        match response.path {
            None => Ok(None),
            Some(path) if std::path::Path::new(&path).is_absolute() => Ok(Some(path.into())),
            Some(_) => Err(()),
        }
    }

    pub fn end_skin_folder_access(&self) -> Result<(), ()> {
        self.0
            .run_mobile_plugin("endSkinFolderAccess", ())
            .map_err(|_| ())
    }

    /// Family names only, as the desktop font catalog gives them; the keyboard draws candidates with the same UIKit families, so a name picked here is one it can find.
    pub fn list_font_families(&self) -> Result<Vec<String>, ()> {
        let response = self
            .0
            .run_mobile_plugin::<FontFamiliesResponse>("listFontFamilies", ())
            .map_err(|_| ())?;
        installed_font_families(response.families).ok_or(())
    }

    pub fn copy_text(&self, text: &str) -> Result<(), ()> {
        if !is_valid_ios_clipboard_text(text) {
            return Err(());
        }
        self.0
            .run_mobile_plugin("copyText", CopyTextRequest { text })
            .map_err(|_| ())
    }

    pub fn save_voice_text(&self, text: &str) -> Result<(), ()> {
        if !valid_mobile_voice_text(text) {
            return Err(());
        }
        self.0
            .run_mobile_plugin("saveVoiceText", SaveVoiceTextRequest { text })
            .map_err(|_| ())
    }

    pub async fn recognize_voice(
        &self,
        request: MobileVoiceTranscriptionRequest,
    ) -> Result<MobileVoiceTranscriptionResponse, ()> {
        if !request.is_valid() {
            return Err(());
        }
        let response = self
            .0
            .run_mobile_plugin_async::<MobileVoiceTranscriptionResponse>("recognizeVoice", request)
            .await
            .map_err(|_| ())?;
        response.is_valid().then_some(response).ok_or(())
    }

    pub fn stop_voice(&self, request_id: &str) -> Result<(), ()> {
        if !msime_client_core::voice::is_valid_request_id(request_id) {
            return Err(());
        }
        self.0
            .run_mobile_plugin(
                "stopVoice",
                VoiceControlRequest {
                    request_id: Some(request_id),
                },
            )
            .map_err(|_| ())
    }

    pub fn cancel_voice(&self, request_id: Option<&str>) -> Result<(), ()> {
        if request_id.is_some_and(|value| !msime_client_core::voice::is_valid_request_id(value)) {
            return Err(());
        }
        self.0
            .run_mobile_plugin("cancelVoice", VoiceControlRequest { request_id })
            .map_err(|_| ())
    }

    pub fn load_keyboard_preferences(&self) -> Result<IosKeyboardPreferences, ()> {
        let preferences = self
            .0
            .run_mobile_plugin::<IosKeyboardPreferences>("loadKeyboardPreferences", ())
            .map_err(|_| ())?;
        preferences.is_valid().then_some(preferences).ok_or(())
    }

    pub fn save_keyboard_preferences(
        &self,
        preferences: &IosKeyboardPreferences,
    ) -> Result<IosKeyboardPreferences, ()> {
        if !preferences.is_valid() {
            return Err(());
        }
        let saved = self
            .0
            .run_mobile_plugin::<IosKeyboardPreferences>("saveKeyboardPreferences", preferences)
            .map_err(|_| ())?;
        saved.is_valid().then_some(saved).ok_or(())
    }

    pub fn save_keyboard_ai(&self, preferences: &IosKeyboardAiPreferences) -> Result<(), ()> {
        if !preferences.is_valid() {
            return Err(());
        }
        self.0
            .run_mobile_plugin("saveKeyboardAI", preferences)
            .map_err(|_| ())
    }

    pub fn preview_keyboard_haptics(&self, strength: &str) -> Result<(), ()> {
        if !matches!(strength, "light" | "medium" | "strong" | "system") {
            return Err(());
        }
        self.0
            .run_mobile_plugin("previewKeyboardHaptics", json!({ "strength": strength }))
            .map_err(|_| ())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("msime-mobile-platform")
        .setup(|app, api| {
            #[cfg(target_os = "ios")]
            {
                let handle = api.register_ios_plugin(init_plugin_msime_mobile_platform)?;
                app.manage(MobilePlatform(handle));
            }
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin("app.msime.android", "VoicePlugin")?;
                app.manage(AndroidVoicePlatform(handle));
            }
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::{
        installed_font_families, is_supported_app_icon_style, is_valid_account_session_payload,
        is_valid_ios_clipboard_text, migrated_account_session_payload, valid_android_voice_request,
        IosKeyboardAiPreferences, IosKeyboardPreferences, MobileVoiceHotword,
        MobileVoiceRequestHeader, MobileVoiceTranscriptionRequest,
        MobileVoiceTranscriptionResponse, MAX_ACCOUNT_SESSION_BYTES,
        MAX_IOS_CLIPBOARD_TEXT_UTF16_UNITS, MAX_MOBILE_VOICE_TEXT_CHARS,
    };
    use serde_json::Value;

    #[test]
    fn app_icon_styles_are_an_explicit_allowlist() {
        for style in ["classic", "forest", "sky", "dusk", "vermilion"] {
            assert!(is_supported_app_icon_style(style));
        }
        for style in ["", "Classic", "unknown", "../AppIcon"] {
            assert!(!is_supported_app_icon_style(style));
        }
    }

    #[test]
    fn ios_keyboard_ai_preferences_require_a_complete_enabled_configuration() {
        let valid = IosKeyboardAiPreferences {
            enabled: true,
            provider: "deepSeek".into(),
            endpoint: "https://api.example.invalid/v1/chat/completions".into(),
            model: "fixture-model".into(),
            prompt: "只返回结果".into(),
            token: "fixture-token".into(),
        };
        assert!(valid.is_valid());
        assert!(!IosKeyboardAiPreferences {
            token: String::new(),
            ..valid.clone()
        }
        .is_valid());
        assert!(IosKeyboardAiPreferences {
            enabled: false,
            endpoint: String::new(),
            model: String::new(),
            prompt: String::new(),
            token: String::new(),
            ..valid
        }
        .is_valid());
    }

    #[test]
    fn android_voice_polish_requests_reject_malformed_secure_endpoints() {
        let request = super::AndroidVoicePolishRequest {
            endpoint: "https://fixture.invalid/v1/chat/completions".into(),
            model: "fixture-model".into(),
            token: "synthetic-token".into(),
            prompt_id: "polish".into(),
            prompt_custom_1: String::new(),
            prompt_custom_2: String::new(),
            prompt_custom_3: String::new(),
        };
        assert!(request.is_valid());
        for endpoint in [
            "https:///v1/chat/completions",
            "https://user:pass@fixture.invalid/v1/chat/completions",
            "https://fixture.invalid/v1/chat/completions#fragment",
        ] {
            assert!(!super::AndroidVoicePolishRequest {
                endpoint: endpoint.into(),
                ..request.clone()
            }
            .is_valid());
        }
    }

    #[test]
    fn android_voice_requests_use_bounded_platform_arguments() {
        assert!(valid_android_voice_request("fixture-1", "zh-CN"));
        let long_value = "x".repeat(65);
        for request_id in ["", "request_id", long_value.as_str()] {
            assert!(!valid_android_voice_request(request_id, "zh-CN"));
        }
        for language in ["", "zh\nCN", long_value.as_str()] {
            assert!(!valid_android_voice_request("fixture-1", language));
        }
    }

    #[test]
    fn ios_voice_requests_accept_only_bounded_batch_providers() {
        let request = MobileVoiceTranscriptionRequest {
            request_id: "fixture-request-1".into(),
            provider: "openai".into(),
            request_format: "multipart".into(),
            endpoint: "https://fixture.invalid/v1/audio/transcriptions".into(),
            model: "fixture-model".into(),
            token: "synthetic-token".into(),
            headers: Vec::new(),
            enable_itn: true,
            enable_punctuation: true,
            enable_ddc: false,
            boosting_table_id: String::new(),
            model_path: String::new(),
            hotwords: Vec::new(),
        };
        assert!(request.is_valid());
        for (provider, format) in [
            ("openai", "multipart"),
            ("siliconflow", "multipart"),
            ("groq", "multipart"),
            ("everyapi", "multipart"),
            ("mistral", "multipart"),
            ("bailian", "chat_audio"),
        ] {
            assert!(MobileVoiceTranscriptionRequest {
                provider: provider.into(),
                request_format: format.into(),
                ..request.clone()
            }
            .is_valid());
            // 格式和 provider 对不上时拒绝，插件不会按错的格式发请求。
            assert!(!MobileVoiceTranscriptionRequest {
                provider: provider.into(),
                request_format: if format == "multipart" {
                    "chat_audio"
                } else {
                    "multipart"
                }
                .into(),
                ..request.clone()
            }
            .is_valid());
        }
        for provider in ["system", "custom", ""] {
            assert!(!MobileVoiceTranscriptionRequest {
                provider: provider.into(),
                ..request.clone()
            }
            .is_valid());
        }
        assert!(!MobileVoiceTranscriptionRequest {
            endpoint: "http://fixture.invalid/transcriptions".into(),
            ..request.clone()
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionRequest {
            model: "fixture\nmodel".into(),
            ..request.clone()
        }
        .is_valid());

        for endpoint in [
            "https:///v1/audio/transcriptions",
            "https://user:pass@fixture.invalid/v1/audio/transcriptions",
            "https://fixture.invalid/v1/audio/transcriptions#fragment",
        ] {
            assert!(!MobileVoiceTranscriptionRequest {
                endpoint: endpoint.into(),
                ..request.clone()
            }
            .is_valid());
        }
    }

    #[test]
    fn local_voice_requests_carry_only_a_model_path_and_hotwords() {
        let request = MobileVoiceTranscriptionRequest {
            request_id: "fixture-request-1".into(),
            provider: "local".into(),
            request_format: "local".into(),
            endpoint: String::new(),
            model: String::new(),
            token: String::new(),
            headers: Vec::new(),
            enable_itn: true,
            enable_punctuation: true,
            enable_ddc: false,
            boosting_table_id: String::new(),
            model_path: "/data/user/0/fixture/voice-models/x-asr-zh-en-streaming".into(),
            hotwords: vec![MobileVoiceHotword {
                text: "水杉".into(),
                pinyin: "shui shan".into(),
            }],
        };
        assert!(request.is_valid());
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(
            value["modelPath"],
            "/data/user/0/fixture/voice-models/x-asr-zh-en-streaming"
        );
        assert_eq!(value["hotwords"][0]["pinyin"], "shui shan");

        for invalid in [
            MobileVoiceTranscriptionRequest {
                model_path: String::new(),
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                model_path: "relative/model".into(),
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                model_path: "/fixture/\nmodel".into(),
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                endpoint: "https://fixture.invalid/asr".into(),
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                token: "synthetic-token".into(),
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                hotwords: vec![
                    MobileVoiceHotword {
                        text: "水杉".into(),
                        pinyin: "shui shan".into(),
                    };
                    1_001
                ],
                ..request.clone()
            },
            MobileVoiceTranscriptionRequest {
                hotwords: vec![MobileVoiceHotword {
                    text: " ".into(),
                    pinyin: String::new(),
                }],
                ..request.clone()
            },
        ] {
            assert!(!invalid.is_valid());
        }

        // A network provider never carries the on-device fields, and an older serialisation without them still reads.
        let network = MobileVoiceTranscriptionRequest {
            provider: "openai".into(),
            request_format: "multipart".into(),
            endpoint: "https://fixture.invalid/v1/audio/transcriptions".into(),
            model: "fixture-model".into(),
            token: "synthetic-token".into(),
            model_path: String::new(),
            hotwords: Vec::new(),
            ..request.clone()
        };
        assert!(network.is_valid());
        let value = serde_json::to_value(&network).unwrap();
        assert!(value.get("modelPath").is_none());
        assert!(value.get("hotwords").is_none());
        let decoded: MobileVoiceTranscriptionRequest = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, network);
        assert!(!MobileVoiceTranscriptionRequest {
            model_path: "/fixture/model".into(),
            ..network.clone()
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionRequest {
            hotwords: request.hotwords.clone(),
            ..network
        }
        .is_valid());
    }

    #[test]
    fn ios_voice_requests_accept_only_provider_bound_doubao_headers() {
        let headers = vec![
            MobileVoiceRequestHeader {
                name: "x-api-key".into(),
                value: "synthetic-key".into(),
            },
            MobileVoiceRequestHeader {
                name: "x-api-resource-id".into(),
                value: "fixture-resource".into(),
            },
            MobileVoiceRequestHeader {
                name: "x-api-request-id".into(),
                value: "00000000-0000-4000-8000-000000000000".into(),
            },
        ];
        let request = MobileVoiceTranscriptionRequest {
            request_id: "fixture-request-1".into(),
            provider: "doubao".into(),
            request_format: "doubao_websocket".into(),
            endpoint: "wss://fixture.invalid/asr".into(),
            model: String::new(),
            token: String::new(),
            headers,
            enable_itn: true,
            enable_punctuation: true,
            enable_ddc: false,
            boosting_table_id: "fixture-table".into(),
            model_path: String::new(),
            hotwords: Vec::new(),
        };
        assert!(request.is_valid());
        assert!(!MobileVoiceTranscriptionRequest {
            endpoint: "https://fixture.invalid/asr".into(),
            ..request.clone()
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionRequest {
            token: "synthetic-duplicate".into(),
            ..request.clone()
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionRequest {
            headers: vec![
                MobileVoiceRequestHeader {
                    name: "authorization".into(),
                    value: "synthetic-key".into(),
                },
                MobileVoiceRequestHeader {
                    name: "x-api-resource-id".into(),
                    value: "fixture-resource".into(),
                },
                MobileVoiceRequestHeader {
                    name: "x-api-request-id".into(),
                    value: "fixture-request".into(),
                },
            ],
            ..request
        }
        .is_valid());
    }

    #[test]
    fn doubao_headers_reject_non_ascii_values_before_mobile_transport() {
        let headers = vec![
            MobileVoiceRequestHeader {
                name: "x-api-key".into(),
                value: "密钥".into(),
            },
            MobileVoiceRequestHeader {
                name: "x-api-resource-id".into(),
                value: "fixture-resource".into(),
            },
            MobileVoiceRequestHeader {
                name: "x-api-request-id".into(),
                value: "00000000-0000-4000-8000-000000000000".into(),
            },
        ];
        let request = MobileVoiceTranscriptionRequest {
            request_id: "fixture-request-1".into(),
            provider: "doubao".into(),
            request_format: "doubao_websocket".into(),
            endpoint: "wss://fixture.invalid/asr".into(),
            model: String::new(),
            token: String::new(),
            headers,
            enable_itn: true,
            enable_punctuation: true,
            enable_ddc: false,
            boosting_table_id: String::new(),
            model_path: String::new(),
            hotwords: Vec::new(),
        };
        assert!(!request.is_valid());
    }

    #[test]
    fn ios_voice_responses_reject_unbounded_or_nul_text() {
        assert!(MobileVoiceTranscriptionResponse {
            text: "fixture result".into()
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionResponse {
            text: "x".repeat(MAX_MOBILE_VOICE_TEXT_CHARS + 1)
        }
        .is_valid());
        assert!(!MobileVoiceTranscriptionResponse {
            text: "fixture\0result".into()
        }
        .is_valid());
    }

    #[test]
    fn account_session_payloads_use_utf8_byte_limits() {
        assert!(!is_valid_account_session_payload(""));
        assert!(is_valid_account_session_payload(
            &"x".repeat(MAX_ACCOUNT_SESSION_BYTES)
        ));
        assert!(!is_valid_account_session_payload(
            &"x".repeat(MAX_ACCOUNT_SESSION_BYTES + 1)
        ));
        assert!(!is_valid_account_session_payload(
            &"界".repeat(MAX_ACCOUNT_SESSION_BYTES / 3 + 1)
        ));
    }

    #[test]
    fn current_account_session_payload_is_left_unchanged() {
        let payload = r#"{"tokens":{},"expires_at_unix_ms":123}"#;
        assert_eq!(migrated_account_session_payload(payload), None);
    }

    #[test]
    fn native_apple_account_session_is_migrated_to_unix_milliseconds() {
        let payload = r#"{"tokens":{"access_token":"synthetic"},"expiresAt":0}"#;
        let migrated = migrated_account_session_payload(payload).unwrap();
        let document: Value = serde_json::from_str(&migrated).unwrap();
        assert_eq!(document["tokens"]["access_token"], "synthetic");
        assert_eq!(document["expires_at_unix_ms"], 978_307_200_000_u64);
        assert!(document.get("expiresAt").is_none());
    }

    fn keyboard_preferences() -> IosKeyboardPreferences {
        IosKeyboardPreferences {
            input_scheme: "japaneseNineKey".into(),
            traditional_chinese_output: true,
            sound_enabled: true,
            haptics_enabled: true,
            haptic_strength: "strong".into(),
            english_suggestions: true,
            candidate_palette_follows_desktop: true,
            inline_preedit: true,
            haptics_available: true,
            tablet_full_keys: None,
            tablet_split_keyboard: None,
            dictionary_learning: false,
            global_theme: "custom".into(),
            custom_keyboard_skin: Some(r#"{"background":15269867}"#.into()),
        }
    }

    #[test]
    fn ios_keyboard_preferences_accept_the_cantonese_zhuyin_vietnamese_and_stroke_touch_schemes() {
        for scheme in ["cantonese", "zhuyin", "vietnamese", "stroke"] {
            let mut preferences = keyboard_preferences();
            preferences.input_scheme = scheme.into();
            assert!(preferences.is_valid(), "{scheme}");
        }
        for scheme in [
            "Cantonese",
            "jyutping",
            "bopomofo",
            "telex",
            "Stroke",
            "bihua",
        ] {
            let mut preferences = keyboard_preferences();
            preferences.input_scheme = scheme.into();
            assert!(!preferences.is_valid(), "{scheme}");
        }
    }

    // 藏文触屏方案以 `tibetan` 存进 App Group；大小写不同的写法或转写法的名字都不接受。
    #[test]
    fn ios_keyboard_preferences_accept_the_tibetan_touch_scheme() {
        let mut preferences = keyboard_preferences();
        preferences.input_scheme = "tibetan".into();
        assert!(preferences.is_valid());
        for scheme in ["Tibetan", "wylie", "ewts"] {
            let mut preferences = keyboard_preferences();
            preferences.input_scheme = scheme.into();
            assert!(!preferences.is_valid(), "{scheme}");
        }
    }

    #[test]
    fn ios_keyboard_preferences_use_bounded_allowlisted_values() {
        assert!(keyboard_preferences().is_valid());

        let mut invalid = keyboard_preferences();
        invalid.input_scheme = "future".into();
        assert!(!invalid.is_valid());

        let mut invalid = keyboard_preferences();
        invalid.haptic_strength = "maximum".into();
        assert!(!invalid.is_valid());

        let mut system = keyboard_preferences();
        system.haptic_strength = "system".into();
        assert!(system.is_valid(), "跟随系统");

        let mut invalid = keyboard_preferences();
        invalid.global_theme = "../skin".into();
        assert!(!invalid.is_valid());

        // The removed keyboard skins are not themes.
        let mut invalid = keyboard_preferences();
        invalid.global_theme = "ocean".into();
        assert!(!invalid.is_valid());

        let mut invalid = keyboard_preferences();
        invalid.custom_keyboard_skin = Some("[]".into());
        assert!(!invalid.is_valid());
    }

    #[test]
    fn ios_keyboard_preferences_round_trip_the_candidate_palette_switch() {
        let encoded = serde_json::to_value(keyboard_preferences()).unwrap();
        assert_eq!(encoded["candidatePaletteFollowsDesktop"], true);
        let decoded: IosKeyboardPreferences = serde_json::from_value(encoded).unwrap();
        assert!(decoded.candidate_palette_follows_desktop);
    }

    #[test]
    fn ios_keyboard_preferences_round_trip_the_inline_preedit_switch() {
        let encoded = serde_json::to_value(keyboard_preferences()).unwrap();
        assert_eq!(encoded["inlinePreedit"], true);
        let decoded: IosKeyboardPreferences = serde_json::from_value(encoded).unwrap();
        assert!(decoded.inline_preedit);
    }

    #[test]
    fn ios_keyboard_preferences_report_whether_the_device_can_vibrate() {
        let mut encoded = serde_json::to_value(keyboard_preferences()).unwrap();
        encoded["hapticsAvailable"] = false.into();
        let decoded: IosKeyboardPreferences = serde_json::from_value(encoded).unwrap();
        assert!(!decoded.haptics_available);
    }

    #[test]
    fn ios_keyboard_preferences_carry_the_ipad_digit_row_only_when_reported() {
        // A phone snapshot has no switch, and saving it back must not write one.
        let phone = serde_json::to_value(keyboard_preferences()).unwrap();
        assert!(phone.get("tabletFullKeys").is_none());
        let decoded: IosKeyboardPreferences = serde_json::from_value(phone).unwrap();
        assert_eq!(decoded.tablet_full_keys, None);

        let mut ipad = keyboard_preferences();
        ipad.tablet_full_keys = Some(false);
        let encoded = serde_json::to_value(&ipad).unwrap();
        assert_eq!(encoded["tabletFullKeys"], false);
        let decoded: IosKeyboardPreferences = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.tablet_full_keys, Some(false));
    }

    // 横屏分离式键盘同样只在 iPad 上报告：手机的快照里没有这个键，存回去也不会写出它。
    #[test]
    fn ios_keyboard_preferences_carry_the_ipad_split_keyboard_only_when_reported() {
        let phone = serde_json::to_value(keyboard_preferences()).unwrap();
        assert!(phone.get("tabletSplitKeyboard").is_none());
        let decoded: IosKeyboardPreferences = serde_json::from_value(phone).unwrap();
        assert_eq!(decoded.tablet_split_keyboard, None);

        let mut ipad = keyboard_preferences();
        ipad.tablet_split_keyboard = Some(true);
        let encoded = serde_json::to_value(&ipad).unwrap();
        assert_eq!(encoded["tabletSplitKeyboard"], true);
        let decoded: IosKeyboardPreferences = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.tablet_split_keyboard, Some(true));
    }

    #[test]
    fn ios_font_families_are_sorted_unique_and_bounded() {
        assert_eq!(
            installed_font_families(vec![
                "PingFang SC".into(),
                "Helvetica".into(),
                "PingFang SC".into()
            ]),
            Some(vec!["Helvetica".to_string(), "PingFang SC".to_string()])
        );
        assert_eq!(installed_font_families(vec![]), Some(vec![]));
        assert_eq!(installed_font_families(vec![" ".into()]), None);
        assert_eq!(installed_font_families(vec!["a\nb".into()]), None);
        assert_eq!(installed_font_families(vec!["x".repeat(129)]), None);
        assert_eq!(installed_font_families(vec!["f".into(); 16_385]), None);
    }

    #[test]
    fn ios_clipboard_text_uses_the_apple_utf16_boundary() {
        assert!(!is_valid_ios_clipboard_text(""));
        assert!(is_valid_ios_clipboard_text(
            &"a".repeat(MAX_IOS_CLIPBOARD_TEXT_UTF16_UNITS)
        ));
        assert!(!is_valid_ios_clipboard_text(
            &"😀".repeat(MAX_IOS_CLIPBOARD_TEXT_UTF16_UNITS / 2 + 1)
        ));
        assert!(!is_valid_ios_clipboard_text("safe\0hidden"));
    }
}
