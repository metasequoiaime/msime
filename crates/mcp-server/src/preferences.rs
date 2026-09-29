//! The preferences an agent may see and change.
//!
//! An allowlist rather than the whole document: the document also holds API keys, account tokens and endpoints, none of which an agent should read, and most of its fields are choices only the settings page can present properly. The enums are mirrored here so the tool schema names exactly the values the store accepts; `the_mirrors_serialize_as_the_store_does` keeps the two in step.

use msime_client_core::preferences::{
    CandidateLayout, CharacterWidthPreference, ChineseScheme, DefaultImeMode, InputScheme,
    Preferences, PreferencesSnapshot, PreferencesStore, ShuangpinProfile,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Quanpin,
    Shuangpin,
    Wubi,
    Japanese,
}

/// The schemes an agent may switch to. Japanese is left to the user: it needs its own dictionary and a way back that the agent cannot see.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum ChineseSchemeChoice {
    Quanpin,
    Shuangpin,
    Wubi,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Xiaohe,
    Ziranma,
    Shoudao,
    Microsoft,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum StartMode {
    Chinese,
    English,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Width {
    Halfwidth,
    Fullwidth,
}

impl From<InputScheme> for Scheme {
    fn from(value: InputScheme) -> Self {
        match value {
            InputScheme::Quanpin => Self::Quanpin,
            InputScheme::Shuangpin => Self::Shuangpin,
            InputScheme::Wubi => Self::Wubi,
            InputScheme::Japanese => Self::Japanese,
        }
    }
}

impl From<ChineseSchemeChoice> for (InputScheme, ChineseScheme) {
    fn from(value: ChineseSchemeChoice) -> Self {
        match value {
            ChineseSchemeChoice::Quanpin => (InputScheme::Quanpin, ChineseScheme::Quanpin),
            ChineseSchemeChoice::Shuangpin => (InputScheme::Shuangpin, ChineseScheme::Shuangpin),
            ChineseSchemeChoice::Wubi => (InputScheme::Wubi, ChineseScheme::Wubi),
        }
    }
}

impl From<ShuangpinProfile> for Profile {
    fn from(value: ShuangpinProfile) -> Self {
        match value {
            ShuangpinProfile::Xiaohe => Self::Xiaohe,
            ShuangpinProfile::Ziranma => Self::Ziranma,
            ShuangpinProfile::Shoudao => Self::Shoudao,
            ShuangpinProfile::Microsoft => Self::Microsoft,
        }
    }
}

impl From<Profile> for ShuangpinProfile {
    fn from(value: Profile) -> Self {
        match value {
            Profile::Xiaohe => Self::Xiaohe,
            Profile::Ziranma => Self::Ziranma,
            Profile::Shoudao => Self::Shoudao,
            Profile::Microsoft => Self::Microsoft,
        }
    }
}

impl From<DefaultImeMode> for StartMode {
    fn from(value: DefaultImeMode) -> Self {
        match value {
            DefaultImeMode::Chinese => Self::Chinese,
            DefaultImeMode::English => Self::English,
        }
    }
}

impl From<StartMode> for DefaultImeMode {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::Chinese => Self::Chinese,
            StartMode::English => Self::English,
        }
    }
}

impl From<CharacterWidthPreference> for Width {
    fn from(value: CharacterWidthPreference) -> Self {
        match value {
            CharacterWidthPreference::Halfwidth => Self::Halfwidth,
            CharacterWidthPreference::Fullwidth => Self::Fullwidth,
        }
    }
}

impl From<Width> for CharacterWidthPreference {
    fn from(value: Width) -> Self {
        match value {
            Width::Halfwidth => Self::Halfwidth,
            Width::Fullwidth => Self::Fullwidth,
        }
    }
}

impl From<CandidateLayout> for Layout {
    fn from(value: CandidateLayout) -> Self {
        match value {
            CandidateLayout::Horizontal => Self::Horizontal,
            CandidateLayout::Vertical => Self::Vertical,
        }
    }
}

impl From<Layout> for CandidateLayout {
    fn from(value: Layout) -> Self {
        match value {
            Layout::Horizontal => Self::Horizontal,
            Layout::Vertical => Self::Vertical,
        }
    }
}

/// What `get_preferences` shows.
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PreferencesView {
    /// Pass back as `expected_revision` so a change made meanwhile is not overwritten.
    pub revision: u64,
    pub scheme: Scheme,
    pub shuangpin_profile: Profile,
    /// Candidates per page, 1 to 9.
    pub candidate_page_size: u8,
    /// Candidate font size in points, 12 to 32.
    pub candidate_font_size: u8,
    pub candidate_layout: Layout,
    pub candidate_follow_cursor: bool,
    /// Select candidates with the number row.
    pub number_row_selection: bool,
    /// Show a badge when switching between Chinese and English (macOS).
    pub input_mode_hud: bool,
    pub fuzzy_pinyin: bool,
    /// The mode a newly focused text field starts in.
    pub default_ime_mode: StartMode,
    /// The width of the ASCII letters, digits and punctuation the input method outputs.
    pub character_width: Width,
    /// Type Chinese punctuation in Chinese mode.
    pub chinese_punctuation: bool,
    /// With Chinese punctuation on, type , . : after a letter or digit as English punctuation.
    pub smart_punctuation: bool,
    /// Output Traditional Chinese.
    pub traditional_chinese_output: bool,
    /// In Wubi, answer a code with no match with candidates from the same pinyin spelling.
    pub wubi_mixed_pinyin: bool,
    /// In Wubi, show the rest of each candidate's code after the typed keys.
    pub wubi_code_hint: bool,
    /// Whether the input method writes its diagnostic log: focus changes, slow requests, candidate window and dictionary events and failures, never what is typed.
    pub diagnostic_log_server: bool,
    /// Windows only: whether the TIP adds its composition and key-latency records to the diagnostic log.
    pub diagnostic_log_tsf: bool,
    /// Whether the dictionary learns from typing. Read-only here: turning it off is the user's decision.
    pub learning: bool,
}

impl From<&PreferencesSnapshot> for PreferencesView {
    fn from(snapshot: &PreferencesSnapshot) -> Self {
        let preferences = &snapshot.preferences;
        Self {
            revision: snapshot.revision,
            scheme: preferences.scheme.into(),
            shuangpin_profile: preferences.shuangpin_profile.into(),
            candidate_page_size: preferences.candidate_page_size,
            candidate_font_size: preferences.candidate_font_size,
            candidate_layout: preferences.candidate_layout.into(),
            candidate_follow_cursor: preferences.candidate_follow_cursor,
            number_row_selection: preferences.number_row_selection,
            input_mode_hud: preferences.input_mode_hud,
            fuzzy_pinyin: preferences.fuzzy_pinyin.enabled,
            default_ime_mode: preferences.default_ime_mode.into(),
            character_width: preferences.character_width.into(),
            chinese_punctuation: preferences.chinese_punctuation,
            smart_punctuation: preferences.smart_punctuation,
            traditional_chinese_output: preferences.traditional_chinese_output,
            wubi_mixed_pinyin: preferences.wubi_mixed_pinyin,
            // Absent means on: documents written before the switch existed keep the hint.
            wubi_code_hint: preferences.wubi_code_hint.unwrap_or(true),
            diagnostic_log_server: preferences.diagnostic_log.server,
            diagnostic_log_tsf: preferences.diagnostic_log.tsf,
            learning: preferences.learning,
        }
    }
}

/// What `update_preferences` accepts. Absent fields are left as they are.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PreferencesChange {
    /// The revision `get_preferences` returned. The change is refused if the preferences have changed since; read them again and retry.
    pub expected_revision: u64,
    pub scheme: Option<ChineseSchemeChoice>,
    pub shuangpin_profile: Option<Profile>,
    /// 1 to 9.
    pub candidate_page_size: Option<u8>,
    /// 12 to 32.
    pub candidate_font_size: Option<u8>,
    pub candidate_layout: Option<Layout>,
    pub candidate_follow_cursor: Option<bool>,
    pub number_row_selection: Option<bool>,
    pub input_mode_hud: Option<bool>,
    /// Turning fuzzy pinyin on for the first time also turns on every fuzzy rule, as the settings page does.
    pub fuzzy_pinyin: Option<bool>,
    pub default_ime_mode: Option<StartMode>,
    pub character_width: Option<Width>,
    pub chinese_punctuation: Option<bool>,
    pub smart_punctuation: Option<bool>,
    pub traditional_chinese_output: Option<bool>,
    pub wubi_mixed_pinyin: Option<bool>,
    pub wubi_code_hint: Option<bool>,
    /// Turn on to look into a problem the user reports, and off again once it is understood.
    pub diagnostic_log_server: Option<bool>,
    /// Windows only; the other platforms keep it for the Windows settings to find.
    pub diagnostic_log_tsf: Option<bool>,
}

impl PreferencesChange {
    pub(crate) fn is_empty(&self) -> bool {
        self.scheme.is_none()
            && self.shuangpin_profile.is_none()
            && self.candidate_page_size.is_none()
            && self.candidate_font_size.is_none()
            && self.candidate_layout.is_none()
            && self.candidate_follow_cursor.is_none()
            && self.number_row_selection.is_none()
            && self.input_mode_hud.is_none()
            && self.fuzzy_pinyin.is_none()
            && self.default_ime_mode.is_none()
            && self.character_width.is_none()
            && self.chinese_punctuation.is_none()
            && self.smart_punctuation.is_none()
            && self.traditional_chinese_output.is_none()
            && self.wubi_mixed_pinyin.is_none()
            && self.wubi_code_hint.is_none()
            && self.diagnostic_log_server.is_none()
            && self.diagnostic_log_tsf.is_none()
    }

    fn apply(&self, preferences: &mut Preferences) {
        if let Some(choice) = self.scheme {
            let (scheme, chinese) = choice.into();
            preferences.scheme = scheme;
            // The scheme Japanese returns to; left pointing elsewhere, leaving Japanese would undo this change.
            preferences.last_chinese_scheme = Some(chinese);
        }
        if let Some(profile) = self.shuangpin_profile {
            preferences.shuangpin_profile = profile.into();
        }
        if let Some(size) = self.candidate_page_size {
            preferences.candidate_page_size = size;
        }
        if let Some(size) = self.candidate_font_size {
            preferences.candidate_font_size = size;
        }
        if let Some(layout) = self.candidate_layout {
            preferences.candidate_layout = layout.into();
        }
        if let Some(value) = self.candidate_follow_cursor {
            preferences.candidate_follow_cursor = value;
        }
        if let Some(value) = self.number_row_selection {
            preferences.number_row_selection = value;
        }
        if let Some(value) = self.input_mode_hud {
            preferences.input_mode_hud = value;
        }
        if let Some(value) = self.fuzzy_pinyin {
            preferences.fuzzy_pinyin.enabled = value;
        }
        if let Some(mode) = self.default_ime_mode {
            preferences.default_ime_mode = mode.into();
        }
        if let Some(width) = self.character_width {
            preferences.character_width = width.into();
        }
        if let Some(value) = self.chinese_punctuation {
            preferences.chinese_punctuation = value;
        }
        if let Some(value) = self.smart_punctuation {
            preferences.smart_punctuation = value;
        }
        if let Some(value) = self.traditional_chinese_output {
            preferences.traditional_chinese_output = value;
        }
        if let Some(value) = self.wubi_mixed_pinyin {
            preferences.wubi_mixed_pinyin = value;
        }
        if let Some(value) = self.wubi_code_hint {
            preferences.wubi_code_hint = Some(value);
        }
        if let Some(value) = self.diagnostic_log_server {
            preferences.diagnostic_log.server = value;
        }
        if let Some(value) = self.diagnostic_log_tsf {
            preferences.diagnostic_log.tsf = value;
        }
    }
}

pub fn load(state_dir: &Path) -> Result<PreferencesView, String> {
    let snapshot = PreferencesStore::new(state_dir)
        .load()
        .map_err(|error| error.to_string())?;
    Ok(PreferencesView::from(&snapshot))
}

/// Save `change` with compare-and-swap, and on Linux publish the result into the runtime-options document the IBus and Fcitx5 hosts read their preferences from, as the settings page does.
pub fn update(
    state_dir: &Path,
    options: &Path,
    change: &PreferencesChange,
) -> Result<PreferencesView, String> {
    if change.is_empty() {
        return Err("no preference to change".into());
    }
    let store = PreferencesStore::new(state_dir);
    let previous = store.load().map_err(|error| error.to_string())?;
    if previous.revision != change.expected_revision {
        return Err("the preferences changed since they were read; read them again".into());
    }
    let mut preferences = previous.preferences.clone();
    change.apply(&mut preferences);
    let snapshot = store
        .save(change.expected_revision, preferences)
        .map_err(|error| error.to_string())?;
    if cfg!(target_os = "linux") {
        if let Err(error) = publish_to_runtime_options(options, &snapshot.preferences) {
            // A document the hosts could not read is refused along with the save that produced it, so the store and the hosts do not disagree. A concurrent writer wins over the rollback.
            if error == TOO_LARGE {
                let _ = store.save(snapshot.revision, previous.preferences);
            }
            return Err(error);
        }
    }
    Ok(PreferencesView::from(&snapshot))
}

/// The most runtime-options.json may hold on Linux: the IBus launcher, the Fcitx5 addon and the upgrade refresh all read at most 16 KiB (`LINUX_RUNTIME_OPTIONS_LIMIT` in the desktop app).
const LINUX_RUNTIME_OPTIONS_LIMIT: usize = 16384;
const TOO_LARGE: &str = "the runtime options would be too large for the input method to read";

/// Replace the preferences in the runtime-options document, the way `sync_runtime_options` in the desktop app does. Everything else in the document, the skin catalog included, is kept as it is.
fn publish_to_runtime_options(path: &Path, preferences: &Preferences) -> Result<(), String> {
    use std::io::Write;
    let file = std::fs::File::open(path).map_err(|_| "cannot read the runtime options")?;
    let bytes = crate::bounded::read(file, LINUX_RUNTIME_OPTIONS_LIMIT as u64).map_err(
        |error| match error {
            crate::bounded::ReadError::TooLarge => TOO_LARGE,
            crate::bounded::ReadError::Io => "cannot read the runtime options",
        },
    )?;
    let mut document: Value =
        serde_json::from_slice(&bytes).map_err(|_| "cannot parse the runtime options")?;
    if !document.is_object() {
        return Err("cannot parse the runtime options".into());
    }
    let mut host = serde_json::to_value(preferences).map_err(|error| error.to_string())?;
    // The IBus and Fcitx5 hosts never draw the screen keyboard, so its photo stays out of their copy.
    if let Some(design) = host
        .pointer_mut("/custom_theme/keyboard")
        .and_then(Value::as_object_mut)
    {
        design.remove("photo");
    }
    document["preferences"] = host;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if bytes.len() > LINUX_RUNTIME_OPTIONS_LIMIT {
        return Err(TOO_LARGE.into());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "cannot write the runtime options")?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| "cannot write the runtime options")?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|_| "cannot write the runtime options".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_mirrors_serialize_as_the_store_does() {
        let same = |mirror: Value, store: Value| assert_eq!(mirror, store);
        for scheme in [
            InputScheme::Quanpin,
            InputScheme::Shuangpin,
            InputScheme::Wubi,
            InputScheme::Japanese,
        ] {
            same(json!(Scheme::from(scheme)), json!(scheme));
        }
        for choice in [
            ChineseSchemeChoice::Quanpin,
            ChineseSchemeChoice::Shuangpin,
            ChineseSchemeChoice::Wubi,
        ] {
            let (scheme, chinese): (InputScheme, ChineseScheme) = choice.into();
            same(json!(choice), json!(scheme));
            same(json!(choice), json!(chinese));
        }
        for profile in [
            ShuangpinProfile::Xiaohe,
            ShuangpinProfile::Ziranma,
            ShuangpinProfile::Shoudao,
            ShuangpinProfile::Microsoft,
        ] {
            same(json!(Profile::from(profile)), json!(profile));
            assert_eq!(ShuangpinProfile::from(Profile::from(profile)), profile);
        }
        for layout in [CandidateLayout::Horizontal, CandidateLayout::Vertical] {
            same(json!(Layout::from(layout)), json!(layout));
            assert_eq!(CandidateLayout::from(Layout::from(layout)), layout);
        }
        for mode in [DefaultImeMode::Chinese, DefaultImeMode::English] {
            same(json!(StartMode::from(mode)), json!(mode));
            assert_eq!(DefaultImeMode::from(StartMode::from(mode)), mode);
        }
        for width in [
            CharacterWidthPreference::Halfwidth,
            CharacterWidthPreference::Fullwidth,
        ] {
            same(json!(Width::from(width)), json!(width));
            assert_eq!(CharacterWidthPreference::from(Width::from(width)), width);
        }
    }

    fn change(revision: u64) -> PreferencesChange {
        PreferencesChange {
            expected_revision: revision,
            ..PreferencesChange::default()
        }
    }

    #[test]
    fn an_update_changes_only_what_it_names_and_refuses_a_stale_revision() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(
            &options,
            br#"{"api_version":1,"candidate_skin_catalog":[{"id":"kept"}]}"#,
        )
        .unwrap();
        let before = load(directory.path()).unwrap();
        let untouched = PreferencesStore::new(directory.path()).load().unwrap();

        assert_eq!(
            update(directory.path(), &options, &change(before.revision)).unwrap_err(),
            "no preference to change"
        );
        let updated = update(
            directory.path(),
            &options,
            &PreferencesChange {
                scheme: Some(ChineseSchemeChoice::Shuangpin),
                shuangpin_profile: Some(Profile::Ziranma),
                candidate_page_size: Some(7),
                fuzzy_pinyin: Some(true),
                character_width: Some(Width::Fullwidth),
                traditional_chinese_output: Some(true),
                wubi_code_hint: Some(false),
                diagnostic_log_server: Some(true),
                ..change(before.revision)
            },
        )
        .unwrap();
        assert_eq!(updated.scheme, Scheme::Shuangpin);
        assert_eq!(updated.shuangpin_profile, Profile::Ziranma);
        assert_eq!(updated.candidate_page_size, 7);
        assert!(updated.fuzzy_pinyin);
        assert_eq!(updated.character_width, Width::Fullwidth);
        assert!(updated.traditional_chinese_output);
        assert!(!updated.wubi_code_hint);
        assert!(updated.diagnostic_log_server && !updated.diagnostic_log_tsf);
        assert_eq!(updated.revision, before.revision + 1);

        let stored = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(
            stored.preferences.last_chinese_scheme,
            Some(ChineseScheme::Shuangpin)
        );
        // Turning fuzzy pinyin on for the first time seeds the rules, as the settings page does.
        assert!(!stored.preferences.fuzzy_pinyin.rules.is_empty());
        // Nothing outside the allowlist moved.
        let mut expected = untouched.preferences.clone();
        expected.scheme = InputScheme::Shuangpin;
        expected.last_chinese_scheme = Some(ChineseScheme::Shuangpin);
        expected.shuangpin_profile = ShuangpinProfile::Ziranma;
        expected.candidate_page_size = 7;
        expected.character_width = CharacterWidthPreference::Fullwidth;
        expected.traditional_chinese_output = true;
        expected.wubi_code_hint = Some(false);
        expected.diagnostic_log.server = true;
        expected.fuzzy_pinyin = stored.preferences.fuzzy_pinyin.clone();
        assert_eq!(json!(stored.preferences), json!(expected));

        // A stale revision is refused, and so is a value the store refuses.
        let stale = PreferencesChange {
            candidate_page_size: Some(5),
            ..change(before.revision)
        };
        assert!(update(directory.path(), &options, &stale).is_err());
        let invalid = PreferencesChange {
            candidate_page_size: Some(10),
            ..change(updated.revision)
        };
        assert!(update(directory.path(), &options, &invalid).is_err());
        assert_eq!(load(directory.path()).unwrap(), updated);

        // Linux hosts read their preferences from the runtime options; the rest of that document is kept.
        let document: Value = serde_json::from_slice(&std::fs::read(&options).unwrap()).unwrap();
        if cfg!(target_os = "linux") {
            assert_eq!(document["preferences"]["candidate_page_size"], 7);
        } else {
            assert!(document.get("preferences").is_none());
        }
        assert_eq!(document["candidate_skin_catalog"][0]["id"], "kept");
    }

    #[test]
    fn the_published_document_keeps_the_photo_out_and_refuses_to_outgrow_the_hosts() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let mut preferences = Preferences::default();
        preferences.custom_theme.keyboard = Some(msime_client_core::preferences::TouchKeyboardSkinDesign {
            photo: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".into()),
            ..Default::default()
        });
        preferences.validate().unwrap();
        publish_to_runtime_options(&options, &preferences).unwrap();
        let document: Value = serde_json::from_slice(&std::fs::read(&options).unwrap()).unwrap();
        assert_eq!(document["api_version"], 1);
        assert!(document["preferences"]["custom_theme"]["keyboard"].is_object());
        assert!(document["preferences"]["custom_theme"]["keyboard"]
            .get("photo")
            .is_none());

        std::fs::write(
            &options,
            serde_json::to_vec(&json!({ "padding": "x".repeat(LINUX_RUNTIME_OPTIONS_LIMIT) }))
                .unwrap(),
        )
        .unwrap();
        preferences.candidate_page_size = 5;
        assert_eq!(
            publish_to_runtime_options(&options, &preferences).unwrap_err(),
            TOO_LARGE
        );
    }

    #[test]
    fn an_oversized_source_is_rejected_before_json_decoding() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, vec![b'x'; LINUX_RUNTIME_OPTIONS_LIMIT + 1]).unwrap();
        assert_eq!(
            publish_to_runtime_options(&options, &Preferences::default()).unwrap_err(),
            TOO_LARGE
        );
    }
}
