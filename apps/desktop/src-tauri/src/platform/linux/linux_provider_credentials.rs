//! Linux provider credential files.
//!
//! Windows and macOS keep AI tokens, translation secrets and speech keys in the shared preferences document, because the shell itself sends those requests. On Linux the network requests belong to the user's provider services: `msime-linux-online-provider` reads `ai-provider.json` and `tencent-provider.json`, and `msime-linux-voice-provider` reads `voice-provider.json`, all from `$XDG_CONFIG_HOME/msime-client` and on every request. This module lets the settings page write those files instead of asking the user to hand-edit JSON: the provider picks the change up on the next request, without a restart. The voice service starts without the file, because on-device recognition (`local`) reads no credential and polishing credentials alone are a file it accepts; saving a voice credential still enables its socket unit, so a socket left disabled by an earlier setup or version comes back once the user configures voice input.
//!
//! The files follow the provider's own reader (`load_private_config`, `load_ai_config`, `load_tencent_config`): a regular file owned by this user with no group or other bits, at most 16 KiB, published by rename so the provider never reads a half-written document. Validation mirrors the provider's, so a document this module writes is one the provider accepts - the AI file is validated as a whole, and a single bad profile would disable every provider in it.
//!
//! The React surface only learns which providers have a credential and the endpoint and model each one is bound to. Secrets travel from the webview into this process and never back.

use super::config_home;
use msime_client_core::{has_disallowed_control_with_options, is_ascii_graphic, is_bounded_chars};
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The provider's `load_private_config` reads at most this many bytes.
const MAX_PROVIDER_CONFIG_BYTES: usize = 16 * 1024;
/// The provider's `load_ai_config` rejects more profiles than this.
const MAX_AI_PROFILES: usize = 16;
const AI_FILE: &str = "ai-provider.json";
const TENCENT_FILE: &str = "tencent-provider.json";
const VOICE_FILE: &str = "voice-provider.json";
/// The voice provider's `LOCAL_PROVIDER`: on-device recognition, whose file entry, when a user writes one, carries only the provider name.
const LOCAL_ASR_PROVIDER: &str = "local";
/// The voice provider's `ASR_PROVIDERS` and `POLISH_PROVIDERS`.
const ASR_PROVIDERS: [&str; 6] = [
    "openai",
    "groq",
    "siliconflow",
    "everyapi",
    "mistral",
    "doubao",
];
const POLISH_PROVIDERS: [&str; 4] = ["openai", "groq", "siliconflow", "deepseek"];
/// The voice provider's default Doubao resource when an entry names none.
const DOUBAO_DEFAULT_RESOURCE: &str = "volc.seedasr.sauc.duration";
/// The voice provider's bound on each Doubao field.
const MAX_DOUBAO_FIELD: usize = 512;

/// Serialises read-modify-write of the two files within this process.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CredentialError {
    /// No usable `$XDG_CONFIG_HOME` or `$HOME`.
    Location,
    /// The existing file is not one this host or the provider would accept: wrong owner or mode, a symlink, too large, or not the expected JSON.
    Existing,
    Storage,
    InvalidProvider,
    InvalidEndpoint,
    InvalidModel,
    InvalidToken,
    TokenRequired,
    TooManyProfiles,
    InvalidSecret,
    InvalidRegion,
}

impl CredentialError {
    fn code(&self) -> &'static str {
        match self {
            Self::Location => "provider_credentials_location",
            Self::Existing => "provider_credentials_existing_invalid",
            Self::Storage => "provider_credentials_storage",
            Self::InvalidProvider => "provider_credentials_invalid_provider",
            Self::InvalidEndpoint => "provider_credentials_invalid_endpoint",
            Self::InvalidModel => "provider_credentials_invalid_model",
            Self::InvalidToken => "provider_credentials_invalid_token",
            Self::TokenRequired => "provider_credentials_token_required",
            Self::TooManyProfiles => "provider_credentials_too_many_profiles",
            Self::InvalidSecret => "provider_credentials_invalid_secret",
            Self::InvalidRegion => "provider_credentials_invalid_region",
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiCredentialStatus {
    provider: String,
    endpoint: String,
    model: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TencentCredentialStatus {
    region: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialStatus {
    ai: Vec<AiCredentialStatus>,
    /// The AI file exists but the provider would refuse it, so no AI provider works until it is saved again or removed.
    ai_invalid: bool,
    tencent: Option<TencentCredentialStatus>,
    tencent_invalid: bool,
    voice_asr: Vec<VoiceCredentialStatus>,
    voice_polish: Vec<VoiceCredentialStatus>,
    /// The voice file exists but the provider would refuse it, so voice input does not start until it is saved again or removed.
    voice_invalid: bool,
}

/// A stored voice entry as the settings page may see it. `model` and `endpoint` are what the file holds, empty when the entry leaves them to the provider's defaults.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCredentialStatus {
    provider: String,
    model: String,
    endpoint: String,
    /// Doubao recognition only.
    resource_id: Option<String>,
    auth_mode: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSaveResponse {
    status: ProviderCredentialStatus,
    /// Whether `systemctl --user` accepted enabling the voice socket unit after a save; always true after a removal, which leaves the unit alone because on-device recognition needs the service without any file. False on a system without the user manager or without the unit installed; the file is saved either way.
    service_updated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VoiceKind {
    Asr,
    Polish,
}

impl VoiceKind {
    fn parse(value: &str) -> Result<Self, CredentialError> {
        match value {
            "asr" => Ok(Self::Asr),
            "polish" => Ok(Self::Polish),
            _ => Err(CredentialError::InvalidProvider),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Asr => "asr",
            Self::Polish => "polish",
        }
    }

    fn providers(self) -> &'static [&'static str] {
        match self {
            Self::Asr => &ASR_PROVIDERS,
            Self::Polish => &POLISH_PROVIDERS,
        }
    }

    /// The most entries of this kind the provider accepts: one per credential provider, plus the `local` recognition entry a user may have written by hand.
    fn max_entries(self) -> usize {
        self.providers().len() + usize::from(self == Self::Asr)
    }
}

/// One voice credential as the settings page submits it. `None` secrets keep the stored value.
pub(crate) struct VoiceCredential<'a> {
    pub kind: VoiceKind,
    pub provider: &'a str,
    pub endpoint: &'a str,
    pub model: &'a str,
    pub token: Option<&'a str>,
    /// Doubao legacy console authentication only.
    pub app_key: Option<&'a str>,
    pub resource_id: &'a str,
    pub auth_mode: &'a str,
}

/// `$XDG_CONFIG_HOME/msime-client`, resolved the way `msime-linux-provider-session` resolves it: a relative `XDG_CONFIG_HOME` is an error, not a fallback.
fn config_directory() -> Result<PathBuf, CredentialError> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty());
    if xdg
        .as_deref()
        .is_some_and(|value| !Path::new(value).is_absolute())
    {
        return Err(CredentialError::Location);
    }
    config_home(xdg.as_deref(), std::env::var_os("HOME").as_deref())
        .map(|base| {
            base.join(
                &msime_client_core::edition::Edition::linux_package_identity_or_full()
                    .client_directory,
            )
        })
        .ok_or(CredentialError::Location)
}

/// The document at `path`, `None` when there is none. A file the provider's reader would refuse is an error rather than something to overwrite: the user may have put it there by hand.
fn read_private(path: &Path) -> Result<Option<Map<String, Value>>, CredentialError> {
    if let Some(parent) = path.parent() {
        super::reject_symlink_ancestors(parent).map_err(|_| CredentialError::Storage)?;
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CredentialError::Storage),
    };
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.len() > MAX_PROVIDER_CONFIG_BYTES as u64
    {
        return Err(CredentialError::Existing);
    }
    // The file can grow after symlink_metadata returns. Read through a bounded handle so a
    // concurrent replacement cannot turn the size check into an unbounded allocation.
    let bytes = super::read_bounded_file(path, MAX_PROVIDER_CONFIG_BYTES as u64)
        .map_err(|_| CredentialError::Storage)?;
    if bytes.len() > MAX_PROVIDER_CONFIG_BYTES {
        return Err(CredentialError::Existing);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| CredentialError::Storage)?;
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(Some(map)),
        _ => Err(CredentialError::Existing),
    }
}

/// Publish `document` at `path` owner-only, or remove the file when there is nothing left to store.
fn write_private(path: &Path, document: Option<&Value>) -> Result<(), CredentialError> {
    let parent = path.parent().ok_or(CredentialError::Storage)?;
    super::reject_symlink_ancestors(parent).map_err(|_| CredentialError::Storage)?;
    let Some(document) = document else {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CredentialError::Storage),
        };
    };
    let mut value = serde_json::to_vec_pretty(document).map_err(|_| CredentialError::Storage)?;
    value.push(b'\n');
    if value.len() > MAX_PROVIDER_CONFIG_BYTES {
        return Err(CredentialError::TooManyProfiles);
    }
    if !super::create_directory_and_check(parent).map_err(|_| CredentialError::Storage)? {
        return Err(CredentialError::Storage);
    }
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| CredentialError::Storage)?;
    // Created 0600 from the start: between a create and a chmod the secret would be readable.
    let temporary = path.with_extension("json.new");
    let _ = std::fs::remove_file(&temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| CredentialError::Storage)?;
    let written = file
        .write_all(&value)
        .and_then(|()| file.sync_all())
        .map_err(|_| CredentialError::Storage);
    drop(file);
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(CredentialError::Storage);
    }
    Ok(())
}

/// Surrounding whitespace the provider strips before it validates.
fn trim_pasted(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\r', '\n'])
}

/// A secret as the provider accepts it: printable ASCII, and not an obvious placeholder.
fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && is_ascii_graphic(value)
        && !value.starts_with('<')
        && !value.starts_with("FAKESECRET_")
}

fn valid_endpoint(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some_and(|host| !host.is_empty())
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}

type AiEntries = BTreeMap<String, Map<String, Value>>;

/// Every AI entry in the document keyed by provider, whether it sits at the top level or under `profiles`. Extra keys in an entry are kept so a hand-written field survives a save.
fn ai_entries(document: &Map<String, Value>) -> Result<AiEntries, CredentialError> {
    let mut entries = AiEntries::new();
    if document.contains_key("provider") {
        let provider = document
            .get("provider")
            .and_then(Value::as_str)
            .ok_or(CredentialError::Existing)?;
        let mut entry = document.clone();
        entry.remove("profiles");
        entries.insert(provider.to_owned(), entry);
    }
    match document.get("profiles") {
        None => {}
        Some(Value::Object(profiles)) => {
            for (provider, entry) in profiles {
                let Value::Object(entry) = entry else {
                    return Err(CredentialError::Existing);
                };
                if entries.contains_key(provider) {
                    return Err(CredentialError::Existing);
                }
                let mut entry = entry.clone();
                entry.insert("provider".into(), Value::String(provider.clone()));
                entries.insert(provider.clone(), entry);
            }
        }
        Some(_) => return Err(CredentialError::Existing),
    }
    Ok(entries)
}

fn entry_text<'a>(entry: &'a Map<String, Value>, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The checks `load_ai_config` applies to each entry.
fn validate_ai_entry(entry: &Map<String, Value>) -> Result<(), CredentialError> {
    let provider = trim_pasted(entry_text(entry, "provider"));
    if provider.is_empty() || !is_bounded_chars(provider, 64) {
        return Err(CredentialError::InvalidProvider);
    }
    let endpoint = trim_pasted(entry_text(entry, "endpoint"));
    if has_disallowed_control_with_options(endpoint, false) || !valid_endpoint(endpoint) {
        return Err(CredentialError::InvalidEndpoint);
    }
    let model = trim_pasted(entry_text(entry, "model"));
    if model.is_empty() || has_disallowed_control_with_options(model, false) {
        return Err(CredentialError::InvalidModel);
    }
    if !valid_secret(trim_pasted(entry_text(entry, "token"))) {
        return Err(CredentialError::InvalidToken);
    }
    Ok(())
}

/// Written as `{"profiles": {...}}`: one shape for any number of providers, which `load_ai_config` reads the same as a top-level entry.
fn ai_document(entries: AiEntries) -> Result<Option<Value>, CredentialError> {
    if entries.is_empty() {
        return Ok(None);
    }
    if entries.len() > MAX_AI_PROFILES {
        return Err(CredentialError::TooManyProfiles);
    }
    let mut profiles = Map::new();
    for (provider, mut entry) in entries {
        validate_ai_entry(&entry)?;
        entry.remove("provider");
        profiles.insert(provider, Value::Object(entry));
    }
    let mut document = Map::new();
    document.insert("profiles".into(), Value::Object(profiles));
    Ok(Some(Value::Object(document)))
}

fn ai_status(document: &Map<String, Value>) -> Result<Vec<AiCredentialStatus>, CredentialError> {
    let entries = ai_entries(document)?;
    if entries.len() > MAX_AI_PROFILES {
        return Err(CredentialError::Existing);
    }
    let mut status = Vec::with_capacity(entries.len());
    for (provider, entry) in &entries {
        validate_ai_entry(entry).map_err(|_| CredentialError::Existing)?;
        status.push(AiCredentialStatus {
            provider: provider.clone(),
            endpoint: trim_pasted(entry_text(entry, "endpoint")).to_owned(),
            model: trim_pasted(entry_text(entry, "model")).to_owned(),
        });
    }
    Ok(status)
}

fn tencent_status(
    document: &Map<String, Value>,
) -> Result<TencentCredentialStatus, CredentialError> {
    for key in ["secret_id", "secret_key"] {
        if !valid_secret(trim_pasted(entry_text(document, key))) {
            return Err(CredentialError::Existing);
        }
    }
    let region = match document.get("region") {
        None => "",
        Some(Value::String(region)) if valid_region(region) => region,
        Some(_) => return Err(CredentialError::Existing),
    };
    Ok(TencentCredentialStatus {
        region: if region.is_empty() {
            "ap-guangzhou"
        } else {
            region
        }
        .to_owned(),
    })
}

fn valid_region(region: &str) -> bool {
    region.len() <= 64
        && region
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub(crate) fn status_in(directory: &Path) -> Result<ProviderCredentialStatus, CredentialError> {
    let (ai, ai_invalid) = match read_private(&directory.join(AI_FILE)) {
        Ok(None) => (Vec::new(), false),
        Ok(Some(document)) => match ai_status(&document) {
            Ok(ai) => (ai, false),
            Err(_) => (Vec::new(), true),
        },
        Err(CredentialError::Existing) => (Vec::new(), true),
        Err(error) => return Err(error),
    };
    let (tencent, tencent_invalid) = match read_private(&directory.join(TENCENT_FILE)) {
        Ok(None) => (None, false),
        Ok(Some(document)) => match tencent_status(&document) {
            Ok(tencent) => (Some(tencent), false),
            Err(_) => (None, true),
        },
        Err(CredentialError::Existing) => (None, true),
        Err(error) => return Err(error),
    };
    let (voice_asr, voice_polish, voice_invalid) = match read_private(&directory.join(VOICE_FILE)) {
        Ok(None) => (Vec::new(), Vec::new(), false),
        Ok(Some(document)) => match voice_status(&document) {
            Ok((asr, polish)) => (asr, polish, false),
            Err(_) => (Vec::new(), Vec::new(), true),
        },
        Err(CredentialError::Existing) => (Vec::new(), Vec::new(), true),
        Err(error) => return Err(error),
    };
    Ok(ProviderCredentialStatus {
        ai,
        ai_invalid,
        tencent,
        tencent_invalid,
        voice_asr,
        voice_polish,
        voice_invalid,
    })
}

/// The voice provider's `provider_id`: ASCII, compared case-insensitively.
fn voice_provider_id(value: &str) -> Option<String> {
    value.is_ascii().then(|| value.to_ascii_lowercase())
}

/// Voice entries of one kind keyed by provider, and which of them sits in the `asr` / `polish` slot rather than under `<kind>_profiles`.
struct VoiceEntries {
    default: Option<String>,
    entries: BTreeMap<String, Map<String, Value>>,
}

fn voice_entries(
    document: &Map<String, Value>,
    kind: VoiceKind,
) -> Result<VoiceEntries, CredentialError> {
    let mut entries = BTreeMap::new();
    let mut default = None;
    match document.get(kind.key()) {
        None => {}
        Some(Value::Object(entry)) => {
            let provider = entry
                .get("provider")
                .and_then(Value::as_str)
                .and_then(voice_provider_id)
                .ok_or(CredentialError::Existing)?;
            let mut entry = entry.clone();
            entry.insert("provider".into(), Value::String(provider.clone()));
            entries.insert(provider.clone(), entry);
            default = Some(provider);
        }
        Some(_) => return Err(CredentialError::Existing),
    }
    match document.get(&format!("{}_profiles", kind.key())) {
        None => {}
        Some(Value::Object(profiles)) => {
            for (provider, entry) in profiles {
                let provider = voice_provider_id(provider).ok_or(CredentialError::Existing)?;
                let Value::Object(entry) = entry else {
                    return Err(CredentialError::Existing);
                };
                if entries.contains_key(&provider) {
                    return Err(CredentialError::Existing);
                }
                let mut entry = entry.clone();
                entry.insert("provider".into(), Value::String(provider.clone()));
                entries.insert(provider, entry);
            }
        }
        Some(_) => return Err(CredentialError::Existing),
    }
    Ok(VoiceEntries { default, entries })
}

/// The voice provider's `normalize_doubao_auth_mode`.
fn doubao_auth_mode(entry: &Map<String, Value>) -> &'static str {
    match entry
        .get("doubao_auth_mode")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("api_key") => "api_key",
        Some("legacy") => "legacy",
        // An absent or empty mode is the single API key, whatever else the entry carries.
        _ => "api_key",
    }
}

fn doubao_field_ok(value: &str) -> bool {
    value.len() <= MAX_DOUBAO_FIELD && is_ascii_graphic(value)
}

/// Whether `entry` is the provider's on-device recognition entry, which `load_config` accepts without a credential.
fn is_local_asr_entry(kind: VoiceKind, entry: &Map<String, Value>) -> bool {
    kind == VoiceKind::Asr && entry_text(entry, "provider") == LOCAL_ASR_PROVIDER
}

/// The checks the voice provider's `load_config` applies to each entry, with its defaults for an absent endpoint or model.
fn validate_voice_entry(
    kind: VoiceKind,
    entry: &Map<String, Value>,
) -> Result<(), CredentialError> {
    if is_local_asr_entry(kind, entry) {
        // Nothing to authenticate and no endpoint: the provider takes the model from each request.
        return Ok(());
    }
    let provider = entry_text(entry, "provider");
    if !kind.providers().contains(&provider) {
        return Err(CredentialError::InvalidProvider);
    }
    for key in ["endpoint", "model", "app_key", "resource_id"] {
        if entry.get(key).is_some_and(|value| !value.is_string()) {
            return Err(CredentialError::Existing);
        }
    }
    let endpoint = trim_pasted(entry_text(entry, "endpoint"));
    let model = trim_pasted(entry_text(entry, "model"));
    if has_disallowed_control_with_options(endpoint, false)
        || has_disallowed_control_with_options(model, false)
    {
        return Err(CredentialError::InvalidEndpoint);
    }
    let doubao = kind == VoiceKind::Asr && provider == "doubao";
    if !endpoint.is_empty() {
        let scheme = if doubao { "wss" } else { "https" };
        let valid = Url::parse(endpoint).is_ok_and(|url| {
            url.scheme() == scheme
                && url.host_str().is_some_and(|host| !host.is_empty())
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        });
        if !valid {
            return Err(CredentialError::InvalidEndpoint);
        }
    }
    if doubao {
        let app_key = trim_pasted(entry_text(entry, "app_key"));
        let resource_id = match entry.get("resource_id") {
            Some(_) => trim_pasted(entry_text(entry, "resource_id")),
            None => DOUBAO_DEFAULT_RESOURCE,
        };
        if !doubao_field_ok(model) || !doubao_field_ok(resource_id) || resource_id.is_empty() {
            return Err(CredentialError::InvalidModel);
        }
        if !doubao_field_ok(app_key)
            || app_key.starts_with('<')
            || app_key.starts_with("FAKESECRET_")
        {
            return Err(CredentialError::InvalidSecret);
        }
        if doubao_auth_mode(entry) == "legacy" && app_key.is_empty() {
            return Err(CredentialError::TokenRequired);
        }
    }
    if !valid_secret(trim_pasted(entry_text(entry, "token"))) {
        return Err(CredentialError::InvalidToken);
    }
    Ok(())
}

fn voice_status_of(
    kind: VoiceKind,
    entries: &VoiceEntries,
) -> Result<Vec<VoiceCredentialStatus>, CredentialError> {
    if entries.entries.len() > kind.max_entries() {
        return Err(CredentialError::Existing);
    }
    let mut status = Vec::with_capacity(entries.entries.len());
    for (provider, entry) in &entries.entries {
        validate_voice_entry(kind, entry).map_err(|_| CredentialError::Existing)?;
        // Not a credential: the settings page has nothing to show or clear for it.
        if is_local_asr_entry(kind, entry) {
            continue;
        }
        let doubao = kind == VoiceKind::Asr && provider == "doubao";
        status.push(VoiceCredentialStatus {
            provider: provider.clone(),
            model: trim_pasted(entry_text(entry, "model")).to_owned(),
            endpoint: trim_pasted(entry_text(entry, "endpoint")).to_owned(),
            resource_id: doubao.then(|| match entry.get("resource_id") {
                Some(_) => trim_pasted(entry_text(entry, "resource_id")).to_owned(),
                None => DOUBAO_DEFAULT_RESOURCE.to_owned(),
            }),
            auth_mode: doubao.then(|| doubao_auth_mode(entry).to_owned()),
        });
    }
    Ok(status)
}

type VoiceStatusPair = (Vec<VoiceCredentialStatus>, Vec<VoiceCredentialStatus>);

fn voice_status(document: &Map<String, Value>) -> Result<VoiceStatusPair, CredentialError> {
    let asr = voice_entries(document, VoiceKind::Asr)?;
    let polish = voice_entries(document, VoiceKind::Polish)?;
    // `load_config` needs a recognition or a polishing entry; either alone is a file it accepts.
    if asr.default.is_none() && polish.default.is_none() {
        return Err(CredentialError::Existing);
    }
    Ok((
        voice_status_of(VoiceKind::Asr, &asr)?,
        voice_status_of(VoiceKind::Polish, &polish)?,
    ))
}

/// Lay one kind's entries out the way `load_config` requires: one in the `asr` / `polish` slot, keeping the one that was there, and the rest under `<kind>_profiles`.
fn place_voice_entries(
    document: &mut Map<String, Value>,
    kind: VoiceKind,
    voice: VoiceEntries,
) -> Result<(), CredentialError> {
    let profiles_key = format!("{}_profiles", kind.key());
    document.remove(kind.key());
    document.remove(&profiles_key);
    if voice.entries.len() > kind.max_entries() {
        return Err(CredentialError::TooManyProfiles);
    }
    let default = voice
        .default
        .filter(|provider| voice.entries.contains_key(provider))
        .or_else(|| voice.entries.keys().next().cloned());
    let mut profiles = Map::new();
    for (provider, mut entry) in voice.entries {
        validate_voice_entry(kind, &entry)?;
        if Some(&provider) == default.as_ref() {
            document.insert(kind.key().into(), Value::Object(entry));
        } else {
            entry.remove("provider");
            profiles.insert(provider, Value::Object(entry));
        }
    }
    if !profiles.is_empty() {
        document.insert(profiles_key, Value::Object(profiles));
    }
    Ok(())
}

/// Store one voice credential, keyed by provider and bound to the model the settings page shows: the provider only uses an entry whose model matches the request's.
pub(crate) fn save_voice_in(
    directory: &Path,
    credential: &VoiceCredential<'_>,
) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(VOICE_FILE);
    let mut document = read_private(&path)?.unwrap_or_default();
    let kind = credential.kind;
    let mut voice = voice_entries(&document, kind)?;
    let provider = voice_provider_id(trim_pasted(credential.provider))
        .ok_or(CredentialError::InvalidProvider)?;
    // On-device recognition has no credential to store.
    if !kind.providers().contains(&provider.as_str()) {
        return Err(CredentialError::InvalidProvider);
    }
    let mut entry = voice.entries.remove(&provider).unwrap_or_default();
    let token = match credential.token.map(trim_pasted) {
        Some(token) => token.to_owned(),
        None => trim_pasted(entry_text(&entry, "token")).to_owned(),
    };
    if token.is_empty() {
        return Err(CredentialError::TokenRequired);
    }
    entry.insert("provider".into(), Value::String(provider.clone()));
    entry.insert("token".into(), Value::String(token));
    for (key, value) in [
        ("endpoint", credential.endpoint),
        ("model", credential.model),
    ] {
        let value = trim_pasted(value);
        if value.is_empty() {
            entry.remove(key);
        } else {
            entry.insert(key.into(), Value::String(value.to_owned()));
        }
    }
    if kind == VoiceKind::Asr && provider == "doubao" {
        let mode = if credential.auth_mode == "legacy" {
            "legacy"
        } else {
            "api_key"
        };
        entry.insert("doubao_auth_mode".into(), Value::String(mode.into()));
        if mode == "legacy" {
            if let Some(app_key) = credential.app_key.map(trim_pasted) {
                entry.insert("app_key".into(), Value::String(app_key.to_owned()));
            }
        } else {
            entry.remove("app_key");
        }
        let resource_id = trim_pasted(credential.resource_id);
        if resource_id.is_empty() {
            entry.remove("resource_id");
        } else {
            entry.insert("resource_id".into(), Value::String(resource_id.to_owned()));
        }
    } else {
        for key in ["app_key", "resource_id", "doubao_auth_mode"] {
            entry.remove(key);
        }
    }
    validate_voice_entry(kind, &entry)?;
    voice.entries.insert(provider, entry);
    place_voice_entries(&mut document, kind, voice).map_err(|error| match error {
        CredentialError::TooManyProfiles => error,
        _ => CredentialError::Existing,
    })?;
    // The other kind is carried over untouched, but it still has to pass: the provider rejects the whole file for one bad entry.
    let other = match kind {
        VoiceKind::Asr => VoiceKind::Polish,
        VoiceKind::Polish => VoiceKind::Asr,
    };
    voice_status_of(other, &voice_entries(&document, other)?)
        .map_err(|_| CredentialError::Existing)?;
    write_private(&path, Some(&Value::Object(document)))
}

/// Remove one voice credential. Either kind may go on its own, since the provider accepts a file with only recognition or only polishing entries; removing the last entry of all removes the file.
pub(crate) fn clear_voice_in(
    directory: &Path,
    kind: VoiceKind,
    provider: &str,
) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(VOICE_FILE);
    let Some(mut document) = read_private(&path)? else {
        return Ok(());
    };
    let mut voice = voice_entries(&document, kind)?;
    let provider =
        voice_provider_id(trim_pasted(provider)).ok_or(CredentialError::InvalidProvider)?;
    if voice.entries.remove(&provider).is_none() {
        return Ok(());
    }
    place_voice_entries(&mut document, kind, voice).map_err(|_| CredentialError::Existing)?;
    if !document.contains_key("asr") && !document.contains_key("polish") {
        return write_private(&path, None);
    }
    write_private(&path, Some(&Value::Object(document)))
}

/// Enable the voice socket unit, clearing a failed earlier start so the next connection tries again. The unit is never disabled from here: without any credential the service still serves on-device recognition, and `msime-linux-setup` enables it for that reason.
pub(crate) fn enable_voice_service() -> bool {
    let systemctl = |arguments: &[&str]| {
        std::process::Command::new("systemctl")
            .arg("--user")
            .args(arguments)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    // 单元名随本安装包所属的版本（full 是 msime-linux-voice.*），只启用本版本的语音服务。
    let identity = msime_client_core::edition::Edition::linux_package_identity_or_full();
    let _ = systemctl(&["reset-failed", identity.user_unit("voice.service").as_str()]);
    systemctl(&[
        "enable",
        "--now",
        identity.user_unit("voice.socket").as_str(),
    ])
}

/// Store the credential for `provider`, bound to `endpoint` and `model`. A `None` token keeps the stored one, so the user can rebind an endpoint or model without pasting the key again.
pub(crate) fn save_ai_in(
    directory: &Path,
    provider: &str,
    endpoint: &str,
    model: &str,
    token: Option<&str>,
) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(AI_FILE);
    let mut entries = match read_private(&path)? {
        Some(document) => ai_entries(&document)?,
        None => AiEntries::new(),
    };
    let provider = trim_pasted(provider);
    let token = match token.map(trim_pasted) {
        Some(token) => token.to_owned(),
        None => entries
            .get(provider)
            .map(|entry| trim_pasted(entry_text(entry, "token")).to_owned())
            .filter(|token| !token.is_empty())
            .ok_or(CredentialError::TokenRequired)?,
    };
    let mut entry = entries.remove(provider).unwrap_or_default();
    entry.insert("provider".into(), Value::String(provider.to_owned()));
    entry.insert(
        "endpoint".into(),
        Value::String(trim_pasted(endpoint).to_owned()),
    );
    entry.insert("model".into(), Value::String(trim_pasted(model).to_owned()));
    entry.insert("token".into(), Value::String(token));
    // Validate the new entry on its own first so the error names what the user just typed rather than an older profile.
    validate_ai_entry(&entry)?;
    entries.insert(provider.to_owned(), entry);
    let document = ai_document(entries).map_err(|error| match error {
        CredentialError::TooManyProfiles => error,
        _ => CredentialError::Existing,
    })?;
    write_private(&path, document.as_ref())
}

pub(crate) fn clear_ai_in(directory: &Path, provider: &str) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(AI_FILE);
    let Some(document) = read_private(&path)? else {
        return Ok(());
    };
    let mut entries = ai_entries(&document)?;
    if entries.remove(trim_pasted(provider)).is_none() {
        return Ok(());
    }
    let document = ai_document(entries).map_err(|_| CredentialError::Existing)?;
    write_private(&path, document.as_ref())
}

/// Store the Tencent Cloud credential. `None` for either secret keeps the stored value, so the region can change without re-entering both.
pub(crate) fn save_tencent_in(
    directory: &Path,
    secret_id: Option<&str>,
    secret_key: Option<&str>,
    region: &str,
) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(TENCENT_FILE);
    let mut document = read_private(&path)?.unwrap_or_default();
    for (key, value) in [("secret_id", secret_id), ("secret_key", secret_key)] {
        let value = match value.map(trim_pasted) {
            Some(value) => value.to_owned(),
            None => trim_pasted(entry_text(&document, key)).to_owned(),
        };
        if value.is_empty() {
            return Err(CredentialError::TokenRequired);
        }
        if !valid_secret(&value) {
            return Err(CredentialError::InvalidSecret);
        }
        document.insert(key.into(), Value::String(value));
    }
    let region = region.trim();
    if !valid_region(region) {
        return Err(CredentialError::InvalidRegion);
    }
    if region.is_empty() {
        document.remove("region");
    } else {
        document.insert("region".into(), Value::String(region.to_owned()));
    }
    write_private(&path, Some(&Value::Object(document)))
}

pub(crate) fn clear_tencent_in(directory: &Path) -> Result<(), CredentialError> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = directory.join(TENCENT_FILE);
    // Refuse to delete a file this host would not have written; the user put it there.
    read_private(&path)?;
    write_private(&path, None)
}

async fn run<T, F>(operation: F) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce(&Path) -> Result<T, CredentialError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let directory = config_directory()?;
        operation(&directory)
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "provider_credentials_storage",
    })?
    .map_err(|error| crate::CommandError { code: error.code() })
}

#[tauri::command]
pub async fn provider_credentials_status() -> Result<ProviderCredentialStatus, crate::CommandError>
{
    run(status_in).await
}

#[tauri::command]
pub async fn save_ai_provider_credential(
    provider: String,
    endpoint: String,
    model: String,
    token: Option<String>,
) -> Result<ProviderCredentialStatus, crate::CommandError> {
    run(move |directory| {
        save_ai_in(directory, &provider, &endpoint, &model, token.as_deref())?;
        status_in(directory)
    })
    .await
}

#[tauri::command]
pub async fn clear_ai_provider_credential(
    provider: String,
) -> Result<ProviderCredentialStatus, crate::CommandError> {
    run(move |directory| {
        clear_ai_in(directory, &provider)?;
        status_in(directory)
    })
    .await
}

#[tauri::command]
pub async fn save_tencent_provider_credential(
    secret_id: Option<String>,
    secret_key: Option<String>,
    region: String,
) -> Result<ProviderCredentialStatus, crate::CommandError> {
    run(move |directory| {
        save_tencent_in(
            directory,
            secret_id.as_deref(),
            secret_key.as_deref(),
            &region,
        )?;
        status_in(directory)
    })
    .await
}

#[tauri::command]
pub async fn clear_tencent_provider_credential(
) -> Result<ProviderCredentialStatus, crate::CommandError> {
    run(|directory| {
        clear_tencent_in(directory)?;
        status_in(directory)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn save_voice_provider_credential(
    kind: String,
    provider: String,
    endpoint: String,
    model: String,
    token: Option<String>,
    app_key: Option<String>,
    resource_id: String,
    auth_mode: String,
) -> Result<VoiceSaveResponse, crate::CommandError> {
    run(move |directory| {
        save_voice_in(
            directory,
            &VoiceCredential {
                kind: VoiceKind::parse(&kind)?,
                provider: &provider,
                endpoint: &endpoint,
                model: &model,
                token: token.as_deref(),
                app_key: app_key.as_deref(),
                resource_id: &resource_id,
                auth_mode: &auth_mode,
            },
        )?;
        let service_updated = enable_voice_service();
        Ok(VoiceSaveResponse {
            status: status_in(directory)?,
            service_updated,
        })
    })
    .await
}

#[tauri::command]
pub async fn clear_voice_provider_credential(
    kind: String,
    provider: String,
) -> Result<VoiceSaveResponse, crate::CommandError> {
    run(move |directory| {
        clear_voice_in(directory, VoiceKind::parse(&kind)?, &provider)?;
        Ok(VoiceSaveResponse {
            status: status_in(directory)?,
            service_updated: true,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn directory() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn saves_an_owner_only_ai_profile_the_provider_reads() {
        let temp = directory();
        let root = temp.path().join("msime-client");
        save_ai_in(
            &root,
            "deepseek",
            " https://api.deepseek.com/chat/completions ",
            "deepseek-chat",
            Some(" sk-abc\n"),
        )
        .unwrap();
        let path = root.join(AI_FILE);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            read(&path),
            serde_json::json!({"profiles": {"deepseek": {
                "endpoint": "https://api.deepseek.com/chat/completions",
                "model": "deepseek-chat",
                "token": "sk-abc"
            }}})
        );
        let status = status_in(&root).unwrap();
        assert_eq!(
            status.ai,
            vec![AiCredentialStatus {
                provider: "deepseek".into(),
                endpoint: "https://api.deepseek.com/chat/completions".into(),
                model: "deepseek-chat".into(),
            }]
        );
        assert!(!status.ai_invalid && status.tencent.is_none());
    }

    #[test]
    fn keeps_the_token_when_rebinding_and_folds_a_top_level_entry() {
        let temp = directory();
        let root = temp.path();
        let path = root.join(AI_FILE);
        std::fs::write(
            &path,
            r#"{"provider":"openai","endpoint":"https://api.openai.com/v1/chat/completions","model":"gpt-4o","token":"sk-old","note":"mine"}"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        save_ai_in(
            root,
            "openai",
            "https://api.openai.com/v1/chat/completions",
            "gpt-4.1",
            None,
        )
        .unwrap();
        save_ai_in(
            root,
            "kimi",
            "https://api.moonshot.cn/v1/chat/completions",
            "kimi-k2",
            Some("sk-kimi"),
        )
        .unwrap();
        let document = read(&path);
        assert_eq!(document["profiles"]["openai"]["token"], "sk-old");
        assert_eq!(document["profiles"]["openai"]["model"], "gpt-4.1");
        assert_eq!(document["profiles"]["openai"]["note"], "mine");
        assert_eq!(document["profiles"]["kimi"]["token"], "sk-kimi");
        assert!(document.get("provider").is_none());
        clear_ai_in(root, "openai").unwrap();
        clear_ai_in(root, "kimi").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn rejects_what_the_provider_would_refuse() {
        let temp = directory();
        let root = temp.path();
        let save = |endpoint: &str, model: &str, token: Option<&str>| {
            save_ai_in(root, "openai", endpoint, model, token)
        };
        let endpoint = "https://api.openai.com/v1/chat/completions";
        assert_eq!(
            save("http://api.openai.com/v1", "m", Some("sk")),
            Err(CredentialError::InvalidEndpoint)
        );
        assert_eq!(
            save("https://user:pass@api.openai.com/v1", "m", Some("sk")),
            Err(CredentialError::InvalidEndpoint)
        );
        assert_eq!(
            save("https://api.openai.com/v1#x", "m", Some("sk")),
            Err(CredentialError::InvalidEndpoint)
        );
        assert_eq!(
            save(endpoint, " ", Some("sk")),
            Err(CredentialError::InvalidModel)
        );
        assert_eq!(
            save(endpoint, "m", Some("sk key")),
            Err(CredentialError::InvalidToken)
        );
        assert_eq!(
            save(endpoint, "m", Some("<token>")),
            Err(CredentialError::InvalidToken)
        );
        assert_eq!(
            save(endpoint, "m", Some("FAKESECRET_x")),
            Err(CredentialError::InvalidToken)
        );
        assert_eq!(
            save(endpoint, "m", Some("密钥")),
            Err(CredentialError::InvalidToken)
        );
        assert_eq!(
            save(endpoint, "m", None),
            Err(CredentialError::TokenRequired)
        );
        assert_eq!(
            save_ai_in(root, "", endpoint, "m", Some("sk")),
            Err(CredentialError::InvalidProvider)
        );
        assert!(!root.join(AI_FILE).exists());
    }

    #[test]
    fn leaves_a_file_it_did_not_write_alone() {
        let temp = directory();
        let root = temp.path();
        let path = root.join(AI_FILE);
        std::fs::write(&path, r#"{"provider":"openai"}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            save_ai_in(root, "openai", "https://a.example/v1", "m", Some("sk")),
            Err(CredentialError::Existing)
        );
        assert!(status_in(root).unwrap().ai_invalid);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        // Owner-only now, but the entry has no token: the provider refuses the whole file, and so does the status.
        assert!(status_in(root).unwrap().ai_invalid);
        assert_eq!(
            save_ai_in(root, "kimi", "https://a.example/v1", "m", Some("sk")),
            Err(CredentialError::Existing)
        );
        // Saving the broken provider itself repairs it.
        save_ai_in(root, "openai", "https://a.example/v1", "m", Some("sk")).unwrap();
        assert!(!status_in(root).unwrap().ai_invalid);

        let link = root.join(TENCENT_FILE);
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(status_in(root).unwrap().tencent_invalid);
        assert_eq!(clear_tencent_in(root), Err(CredentialError::Existing));
        assert!(link.exists());
    }

    #[test]
    fn rejects_a_symlinked_storage_ancestor_before_saving() {
        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let real = parent.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let linked = real.join("linked");
        msime_path_trust::untrusted_symlink(outside.path(), &linked).unwrap();

        assert_eq!(
            save_ai_in(
                &linked.join("state"),
                "openai",
                "https://a.example/v1",
                "model",
                Some("synthetic-token"),
            ),
            Err(CredentialError::Storage)
        );
        assert!(!outside.path().join("state").exists());
    }

    #[test]
    fn saves_and_clears_the_tencent_credential() {
        let temp = directory();
        let root = temp.path();
        assert_eq!(
            save_tencent_in(root, None, Some("key"), ""),
            Err(CredentialError::TokenRequired)
        );
        assert_eq!(
            save_tencent_in(root, Some("id"), Some("key"), "AP-Beijing"),
            Err(CredentialError::InvalidRegion)
        );
        assert_eq!(
            save_tencent_in(root, Some("id with space"), Some("key"), ""),
            Err(CredentialError::InvalidSecret)
        );
        save_tencent_in(root, Some(" AKIDexample "), Some("secret"), "").unwrap();
        assert_eq!(
            status_in(root).unwrap().tencent,
            Some(TencentCredentialStatus {
                region: "ap-guangzhou".into()
            })
        );
        save_tencent_in(root, None, None, "ap-shanghai").unwrap();
        let path = root.join(TENCENT_FILE);
        assert_eq!(
            read(&path),
            serde_json::json!({"secret_id": "AKIDexample", "secret_key": "secret", "region": "ap-shanghai"})
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        clear_tencent_in(root).unwrap();
        assert!(!path.exists());
        assert_eq!(status_in(root).unwrap().tencent, None);
    }

    fn voice<'a>(
        kind: VoiceKind,
        provider: &'a str,
        model: &'a str,
        token: Option<&'a str>,
    ) -> VoiceCredential<'a> {
        VoiceCredential {
            kind,
            provider,
            endpoint: "",
            model,
            token,
            app_key: None,
            resource_id: "",
            auth_mode: "",
        }
    }

    #[test]
    fn saves_voice_entries_in_the_layout_the_voice_provider_requires() {
        let temp = directory();
        let root = temp.path();
        // On-device recognition has no credential to save.
        assert_eq!(
            save_voice_in(root, &voice(VoiceKind::Asr, "local", "", Some("sk"))),
            Err(CredentialError::InvalidProvider)
        );
        assert!(!root.join(VOICE_FILE).exists());
        save_voice_in(
            root,
            &voice(
                VoiceKind::Asr,
                "SiliconFlow",
                "FunAudioLLM/SenseVoiceSmall",
                Some(" sk-s "),
            ),
        )
        .unwrap();
        save_voice_in(root, &voice(VoiceKind::Asr, "openai", "", Some("sk-o"))).unwrap();
        save_voice_in(
            root,
            &voice(
                VoiceKind::Polish,
                "deepseek",
                "deepseek-v4-flash",
                Some("sk-p"),
            ),
        )
        .unwrap();
        let path = root.join(VOICE_FILE);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            read(&path),
            serde_json::json!({
                "asr": {"provider": "siliconflow", "token": "sk-s", "model": "FunAudioLLM/SenseVoiceSmall"},
                "asr_profiles": {"openai": {"token": "sk-o"}},
                "polish": {"provider": "deepseek", "token": "sk-p", "model": "deepseek-v4-flash"}
            })
        );
        let status = status_in(root).unwrap();
        assert_eq!(status.voice_asr.len(), 2);
        assert_eq!(status.voice_polish[0].model, "deepseek-v4-flash");
        assert!(!status.voice_invalid);

        // Rebinding the model keeps the token.
        save_voice_in(
            root,
            &voice(VoiceKind::Asr, "siliconflow", "TeleAI/TeleSpeechASR", None),
        )
        .unwrap();
        assert_eq!(read(&path)["asr"]["token"], "sk-s");

        // Either kind can go on its own, as the provider accepts a file with only one; the last entry of all takes the file.
        clear_voice_in(root, VoiceKind::Asr, "siliconflow").unwrap();
        assert_eq!(read(&path)["asr"]["provider"], "openai");
        clear_voice_in(root, VoiceKind::Asr, "openai").unwrap();
        assert_eq!(
            read(&path),
            serde_json::json!({
                "polish": {"provider": "deepseek", "token": "sk-p", "model": "deepseek-v4-flash"}
            })
        );
        let status = status_in(root).unwrap();
        assert!(status.voice_asr.is_empty() && !status.voice_invalid);
        clear_voice_in(root, VoiceKind::Polish, "deepseek").unwrap();
        assert!(!path.exists());
        assert!(!status_in(root).unwrap().voice_invalid);
    }

    #[test]
    fn polishing_needs_no_recognition_credential() {
        let temp = directory();
        let root = temp.path();
        // A user on on-device recognition only has polishing to store.
        save_voice_in(
            root,
            &voice(VoiceKind::Polish, "deepseek", "", Some("sk-p")),
        )
        .unwrap();
        let path = root.join(VOICE_FILE);
        assert_eq!(
            read(&path),
            serde_json::json!({"polish": {"provider": "deepseek", "token": "sk-p"}})
        );
        let status = status_in(root).unwrap();
        assert!(status.voice_asr.is_empty() && !status.voice_invalid);
        assert_eq!(status.voice_polish[0].provider, "deepseek");

        // A hand-written `local` recognition entry is one the provider accepts: it is carried over, not reported as a credential.
        let mut document = read(&path);
        document["asr"] = serde_json::json!({"provider": "local"});
        write_private(&path, Some(&document)).unwrap();
        let status = status_in(root).unwrap();
        assert!(!status.voice_invalid && status.voice_asr.is_empty());
        save_voice_in(root, &voice(VoiceKind::Asr, "openai", "", Some("sk-o"))).unwrap();
        assert_eq!(read(&path)["asr"], serde_json::json!({"provider": "local"}));
        assert_eq!(read(&path)["asr_profiles"]["openai"]["token"], "sk-o");
        assert_eq!(status_in(root).unwrap().voice_asr.len(), 1);

        // A file with neither kind is not one the provider starts with.
        write_private(&path, Some(&serde_json::json!({}))).unwrap();
        assert!(status_in(root).unwrap().voice_invalid);
    }

    #[test]
    fn follows_the_voice_providers_doubao_and_endpoint_rules() {
        let temp = directory();
        let root = temp.path();
        let mut doubao = voice(VoiceKind::Asr, "doubao", "", Some("api-key"));
        doubao.auth_mode = "legacy";
        assert_eq!(
            save_voice_in(root, &doubao),
            Err(CredentialError::TokenRequired)
        );
        doubao.app_key = Some("<app key>");
        assert_eq!(
            save_voice_in(root, &doubao),
            Err(CredentialError::InvalidSecret)
        );
        doubao.app_key = Some("app-1");
        doubao.endpoint = "https://openspeech.bytedance.com/api/v3/sauc/bigmodel";
        assert_eq!(
            save_voice_in(root, &doubao),
            Err(CredentialError::InvalidEndpoint)
        );
        doubao.endpoint = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel";
        doubao.resource_id = "volc.bigasr.sauc.duration";
        save_voice_in(root, &doubao).unwrap();
        let document = read(&root.join(VOICE_FILE));
        assert_eq!(document["asr"]["app_key"], "app-1");
        assert_eq!(document["asr"]["doubao_auth_mode"], "legacy");
        let status = status_in(root).unwrap();
        assert_eq!(
            status.voice_asr[0].resource_id.as_deref(),
            Some("volc.bigasr.sauc.duration")
        );
        assert_eq!(status.voice_asr[0].auth_mode.as_deref(), Some("legacy"));
        let mut stored = document.clone();
        stored["asr"]["doubao_auth_mode"] = Value::String(String::new());
        write_private(&root.join(VOICE_FILE), Some(&stored)).unwrap();
        assert_eq!(
            status_in(root).unwrap().voice_asr[0].auth_mode.as_deref(),
            Some("api_key")
        );

        // Switching to the single API key drops the stored App Key.
        let mut api_key = voice(VoiceKind::Asr, "doubao", "", None);
        api_key.auth_mode = "api_key";
        save_voice_in(root, &api_key).unwrap();
        let document = read(&root.join(VOICE_FILE));
        assert!(document["asr"].get("app_key").is_none());
        assert!(document["asr"].get("resource_id").is_none());
        assert_eq!(
            status_in(root).unwrap().voice_asr[0].resource_id.as_deref(),
            Some(DOUBAO_DEFAULT_RESOURCE)
        );

        assert_eq!(
            save_voice_in(root, &voice(VoiceKind::Asr, "whisper", "", Some("sk"))),
            Err(CredentialError::InvalidProvider)
        );
        assert_eq!(
            save_voice_in(root, &voice(VoiceKind::Polish, "mistral", "", Some("sk"))),
            Err(CredentialError::InvalidProvider)
        );
        let mut http = voice(VoiceKind::Polish, "openai", "", Some("sk"));
        http.endpoint = "http://api.openai.com/v1/chat/completions";
        assert_eq!(
            save_voice_in(root, &http),
            Err(CredentialError::InvalidEndpoint)
        );
    }

    #[test]
    fn caps_the_profile_count_at_the_providers_limit() {
        let temp = directory();
        let root = temp.path();
        for index in 0..MAX_AI_PROFILES {
            save_ai_in(
                root,
                &format!("p{index}"),
                "https://a.example/v1",
                "m",
                Some("sk"),
            )
            .unwrap();
        }
        assert_eq!(
            save_ai_in(root, "one-more", "https://a.example/v1", "m", Some("sk")),
            Err(CredentialError::TooManyProfiles)
        );
        assert_eq!(status_in(root).unwrap().ai.len(), MAX_AI_PROFILES);
    }

    #[test]
    fn oversized_private_document_is_rejected_after_a_bounded_read() {
        let temp = directory();
        let path = temp.path().join(AI_FILE);
        std::fs::write(&path, vec![b' '; MAX_PROVIDER_CONFIG_BYTES + 1]).unwrap();
        assert!(matches!(
            read_private(&path),
            Err(CredentialError::Existing)
        ));
    }
}
