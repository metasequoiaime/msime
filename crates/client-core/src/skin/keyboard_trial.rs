//! Crash-recoverable touch-keyboard skin trials independent of any UI host.
//! Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
//! (`KeyboardSkinTrial.swift`, `KeyboardSkinTrialTests.swift`).

use crate::file_lock;
use crate::preferences::{
    PreferencesError, PreferencesSnapshot, PreferencesStore, TouchKeyboardSkinDesign,
};
use crate::skin::theme::GlobalTheme;
use serde::{Deserialize, Serialize};
use std::fs::File;
#[cfg(not(unix))]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

const MAXIMUM_RECORD_BYTES: u64 = 2_000_000;
const MAXIMUM_NAME_GRAPHEMES: usize = 32;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KeyboardSkinTrial {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrialRecord {
    id: Uuid,
    name: String,
    previous_theme: GlobalTheme,
    /// The custom theme's base and package before the trial. When another theme was on screen the trial rebases the custom theme onto it and drops the package, so declining puts both back.
    previous_base: GlobalTheme,
    previous_candidate_skin: Option<String>,
    previous_design: Option<TouchKeyboardSkinDesign>,
    design: TouchKeyboardSkinDesign,
}

#[derive(Debug, Error)]
pub enum KeyboardSkinTrialError {
    #[error("keyboard skin trial storage failed")]
    Io(#[from] std::io::Error),
    #[error("keyboard skin trial is malformed")]
    Json(#[from] serde_json::Error),
    #[error("keyboard skin trial is invalid")]
    Invalid,
    #[error("keyboard skin trial preferences failed")]
    Preferences(#[from] PreferencesError),
}

#[derive(Clone)]
pub struct KeyboardSkinTrialStore {
    directory: PathBuf,
    preferences: Arc<PreferencesStore>,
}

impl KeyboardSkinTrialStore {
    pub fn new(directory: impl AsRef<Path>, preferences: Arc<PreferencesStore>) -> Self {
        Self {
            directory: directory.as_ref().to_owned(),
            preferences,
        }
    }

    pub fn begin(
        &self,
        name: &str,
        design: TouchKeyboardSkinDesign,
    ) -> Result<(KeyboardSkinTrial, PreferencesSnapshot), KeyboardSkinTrialError> {
        let _lock = self.lock()?;
        self.restore_locked()?;
        let name = normalized_name(name)?;
        if !design.validate() {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        let design = design.normalized();
        let snapshot = self.preferences.load()?;
        let record = TrialRecord {
            id: Uuid::new_v4(),
            name: name.clone(),
            previous_theme: snapshot.preferences.global_theme,
            previous_base: snapshot.preferences.custom_theme.base,
            previous_candidate_skin: snapshot.preferences.custom_theme.candidate_skin.clone(),
            previous_design: snapshot.preferences.custom_theme.keyboard.clone(),
            design: design.clone(),
        };
        self.write_record(&record)?;
        let mut preferences = snapshot.preferences;
        // Applying a keyboard design from another theme keeps that theme under the candidate window: it becomes the custom theme's base and any package left in the custom theme is dropped. While `custom` is already selected only the keyboard changes.
        if preferences.global_theme != GlobalTheme::Custom {
            preferences.custom_theme.base = preferences.global_theme;
            preferences.custom_theme.candidate_skin = None;
        }
        preferences.global_theme = GlobalTheme::Custom;
        preferences.custom_theme.keyboard = Some(design);
        let applied = match self.preferences.save(snapshot.revision, preferences) {
            Ok(applied) => applied,
            Err(error) => {
                let _ = self.remove_record();
                return Err(error.into());
            }
        };
        Ok((
            KeyboardSkinTrial {
                id: record.id,
                name,
            },
            applied,
        ))
    }

    pub fn finish(
        &self,
        id: Uuid,
        keep: bool,
    ) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        let _lock = self.lock()?;
        let Some(record) = self.pending()? else {
            return Ok(self.preferences.load()?);
        };
        if record.id != id {
            return Ok(self.preferences.load()?);
        }
        if keep {
            self.remove_record()?;
            return Ok(self.preferences.load()?);
        }
        self.restore_record(record)
    }

    pub fn restore_pending(&self) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        let _lock = self.lock()?;
        self.restore_locked()
    }

    fn restore_locked(&self) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        match self.pending()? {
            Some(record) => self.restore_record(record),
            None => Ok(self.preferences.load()?),
        }
    }

    fn restore_record(
        &self,
        record: TrialRecord,
    ) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        let snapshot = self.preferences.load()?;
        if snapshot.preferences.global_theme != GlobalTheme::Custom
            || snapshot.preferences.custom_theme.keyboard.as_ref() != Some(&record.design)
        {
            self.remove_record()?;
            return Ok(snapshot);
        }
        let mut preferences = snapshot.preferences;
        if record.previous_theme != GlobalTheme::Custom {
            preferences.custom_theme.base = record.previous_base;
            preferences.custom_theme.candidate_skin = record.previous_candidate_skin;
        }
        preferences.global_theme = record.previous_theme;
        preferences.custom_theme.keyboard = record.previous_design;
        let restored = self.preferences.save(snapshot.revision, preferences)?;
        self.remove_record()?;
        Ok(restored)
    }

    fn lock(&self) -> Result<File, KeyboardSkinTrialError> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        let lock = file_lock::open_lock_file(self.directory.join("KeyboardSkinTrial.lock"))?;
        file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn path(&self) -> PathBuf {
        self.directory.join("KeyboardSkinTrial.json")
    }

    fn pending(&self) -> Result<Option<TrialRecord>, KeyboardSkinTrialError> {
        let file = match crate::storage::open_private_file_in(&self.path()) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(KeyboardSkinTrialError::Invalid),
        };
        if file.metadata()?.len() > MAXIMUM_RECORD_BYTES {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        let bytes = crate::bounded_io::read_bounded_file(file, MAXIMUM_RECORD_BYTES, || {
            KeyboardSkinTrialError::Invalid
        })?;
        let record: TrialRecord = serde_json::from_slice(&bytes)?;
        if record.id.is_nil()
            || record.previous_base == GlobalTheme::Custom
            || normalized_name(&record.name)? != record.name
            || !record.design.validate()
            || !record
                .previous_design
                .as_ref()
                .is_none_or(TouchKeyboardSkinDesign::validate)
        {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        Ok(Some(record))
    }

    fn write_record(&self, record: &TrialRecord) -> Result<(), KeyboardSkinTrialError> {
        let bytes = serde_json::to_vec(record)?;
        if bytes.len() as u64 > MAXIMUM_RECORD_BYTES {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(&self.directory)?;
            crate::storage::write_private_file_at(
                &directory,
                std::ffi::OsStr::new("KeyboardSkinTrial.json"),
                &bytes,
            )?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().sync_all()?;
            temporary
                .persist(self.path())
                .map_err(|error| error.error)?;
            Ok(())
        }
    }

    fn remove_record(&self) -> Result<(), KeyboardSkinTrialError> {
        match crate::storage::remove_private_file(&self.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn normalized_name(name: &str) -> Result<String, KeyboardSkinTrialError> {
    let name = name.trim();
    if name.is_empty()
        || name.graphemes(true).count() > MAXIMUM_NAME_GRAPHEMES
        || crate::has_disallowed_control_with_options(name, false)
    {
        return Err(KeyboardSkinTrialError::Invalid);
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn stores() -> (
        tempfile::TempDir,
        Arc<PreferencesStore>,
        KeyboardSkinTrialStore,
    ) {
        let root = tempfile::tempdir().unwrap();
        let preferences = Arc::new(PreferencesStore::new(root.path()));
        let trials = KeyboardSkinTrialStore::new(root.path(), Arc::clone(&preferences));
        (root, preferences, trials)
    }

    #[test]
    fn trial_restores_exact_previous_design_or_keeps_download() {
        let (_root, preferences, trials) = stores();
        let original = preferences.load().unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x123456,
            ..TouchKeyboardSkinDesign::default()
        };
        let (trial, applied) = trials.begin("社区皮肤", design.clone()).unwrap();
        assert_eq!(applied.preferences.global_theme, GlobalTheme::Custom);
        assert_eq!(
            applied.preferences.custom_theme.keyboard,
            Some(design.clone())
        );
        let restored = trials.finish(trial.id, false).unwrap();
        assert_eq!(restored.preferences, original.preferences);

        let (trial, _) = trials.begin("保留皮肤", design.clone()).unwrap();
        let kept = trials.finish(trial.id, true).unwrap();
        assert_eq!(kept.preferences.global_theme, GlobalTheme::Custom);
        assert_eq!(kept.preferences.custom_theme.keyboard, Some(design));
    }

    #[test]
    fn a_trial_from_another_theme_rebases_the_custom_theme_and_declining_restores_it() {
        let (_root, preferences, trials) = stores();
        let loaded = preferences.load().unwrap();
        let mut night = loaded.preferences;
        night.global_theme = GlobalTheme::Night;
        night.custom_theme.candidate_skin = Some("sakura".into());
        let original = preferences.save(loaded.revision, night).unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x0F1E2D,
            ..TouchKeyboardSkinDesign::default()
        };

        let (trial, applied) = trials.begin("夜间试用", design.clone()).unwrap();
        assert_eq!(applied.preferences.global_theme, GlobalTheme::Custom);
        assert_eq!(applied.preferences.custom_theme.base, GlobalTheme::Night);
        assert_eq!(applied.preferences.custom_theme.candidate_skin, None);
        assert_eq!(applied.preferences.custom_theme.keyboard, Some(design));

        let restored = trials.finish(trial.id, false).unwrap();
        assert_eq!(restored.preferences, original.preferences);
    }

    #[test]
    fn a_trial_while_custom_is_selected_changes_only_the_keyboard() {
        let (_root, preferences, trials) = stores();
        let loaded = preferences.load().unwrap();
        let mut custom = loaded.preferences;
        custom.global_theme = GlobalTheme::Custom;
        custom.custom_theme.base = GlobalTheme::Paper;
        custom.custom_theme.candidate_skin = Some("sakura".into());
        let original = preferences.save(loaded.revision, custom).unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x2D1E0F,
            ..TouchKeyboardSkinDesign::default()
        };

        let (trial, applied) = trials.begin("自定义试用", design.clone()).unwrap();
        assert_eq!(applied.preferences.custom_theme.base, GlobalTheme::Paper);
        assert_eq!(
            applied.preferences.custom_theme.candidate_skin.as_deref(),
            Some("sakura")
        );
        assert_eq!(applied.preferences.custom_theme.keyboard, Some(design));

        let restored = trials.finish(trial.id, false).unwrap();
        assert_eq!(restored.preferences, original.preferences);
    }

    #[test]
    fn restart_recovers_but_does_not_undo_a_later_explicit_selection() {
        let (root, preferences, trials) = stores();
        let design = TouchKeyboardSkinDesign {
            background: 0x654321,
            ..TouchKeyboardSkinDesign::default()
        };
        trials.begin("待恢复", design.clone()).unwrap();
        let recovered = KeyboardSkinTrialStore::new(root.path(), Arc::clone(&preferences));
        let restored = recovered.restore_pending().unwrap();
        assert_eq!(restored.preferences.custom_theme.keyboard, None);

        let (trial, applied) = trials.begin("不覆盖后续选择", design).unwrap();
        let mut later = applied.preferences;
        later.global_theme = GlobalTheme::Night;
        preferences.save(applied.revision, later).unwrap();
        let current = trials.finish(trial.id, false).unwrap();
        assert_eq!(current.preferences.global_theme, GlobalTheme::Night);
    }

    #[test]
    fn malformed_pending_record_is_preserved() {
        let (root, _preferences, trials) = stores();
        fs::write(root.path().join("KeyboardSkinTrial.json"), b"not json").unwrap();
        let before = fs::read(root.path().join("KeyboardSkinTrial.json")).unwrap();
        assert!(matches!(
            trials.restore_pending(),
            Err(KeyboardSkinTrialError::Json(_))
        ));
        assert_eq!(
            fs::read(root.path().join("KeyboardSkinTrial.json")).unwrap(),
            before
        );
    }

    #[test]
    fn a_pending_record_with_a_nil_id_is_rejected() {
        let (root, _preferences, trials) = stores();
        let record = TrialRecord {
            id: Uuid::nil(),
            name: "合成试用".into(),
            previous_theme: GlobalTheme::System,
            previous_base: GlobalTheme::System,
            previous_candidate_skin: None,
            previous_design: None,
            design: TouchKeyboardSkinDesign::default(),
        };
        fs::write(
            root.path().join("KeyboardSkinTrial.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            trials.restore_pending(),
            Err(KeyboardSkinTrialError::Invalid)
        ));
    }
}
