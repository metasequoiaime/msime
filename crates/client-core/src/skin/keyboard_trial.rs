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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

const MAXIMUM_RECORD_BYTES: u64 = 2_000_000;
const MAXIMUM_NAME_GRAPHEMES: usize = 32;
const RECORD_FILE: &str = "KeyboardSkinTrial.json";
const LOCK_FILE: &str = "KeyboardSkinTrial.lock";

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
    /// 试用前自定义主题的底和两个槽位的皮肤包。从别的主题试用时，自定义主题改以那个主题为底并清掉皮肤包，所以放弃试用时把它们都放回去。
    previous_base: GlobalTheme,
    previous_candidate_skin: Option<String>,
    /// 试用前深色槽位的皮肤包。旧版本写的记录没有这个键，读出来是 `None`，与它们当时没有深色槽位一致。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_candidate_skin_dark: Option<String>,
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

struct TrialLock {
    directory: file_lock::PrivateDirectory,
    _lock: File,
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
        let lock = self.lock()?;
        self.restore_locked(&lock)?;
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
            previous_candidate_skin_dark: snapshot
                .preferences
                .custom_theme
                .candidate_skin_dark
                .clone(),
            previous_design: snapshot.preferences.custom_theme.keyboard.clone(),
            design: design.clone(),
        };
        self.write_record(&lock, &record)?;
        let mut preferences = snapshot.preferences;
        // 从别的主题试用键盘设计时，原来的主题留在候选窗下面：它成为自定义主题的底，自定义主题里留着的皮肤包清掉。`native` 不能当底（见 `GlobalTheme::is_base`），从它试用时底退回 `system`。已经选着 `custom` 时只换键盘。
        if preferences.global_theme != GlobalTheme::Custom {
            preferences.custom_theme.base = if preferences.global_theme.is_base() {
                preferences.global_theme
            } else {
                GlobalTheme::System
            };
            preferences.custom_theme.candidate_skin = None;
            preferences.custom_theme.candidate_skin_dark = None;
        }
        preferences.global_theme = GlobalTheme::Custom;
        preferences.custom_theme.keyboard = Some(design);
        let applied = match self.preferences.save(snapshot.revision, preferences) {
            Ok(applied) => applied,
            Err(error) => {
                let _ = self.remove_record(&lock);
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
        let lock = self.lock()?;
        let Some(record) = self.pending(&lock)? else {
            return Ok(self.preferences.load()?);
        };
        if record.id != id {
            return Ok(self.preferences.load()?);
        }
        if keep {
            self.remove_record(&lock)?;
            return Ok(self.preferences.load()?);
        }
        self.restore_record(&lock, record)
    }

    pub fn restore_pending(&self) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        let lock = self.lock()?;
        self.restore_locked(&lock)
    }

    fn restore_locked(
        &self,
        lock: &TrialLock,
    ) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        match self.pending(lock)? {
            Some(record) => self.restore_record(lock, record),
            None => Ok(self.preferences.load()?),
        }
    }

    fn restore_record(
        &self,
        lock: &TrialLock,
        record: TrialRecord,
    ) -> Result<PreferencesSnapshot, KeyboardSkinTrialError> {
        let snapshot = self.preferences.load()?;
        if snapshot.preferences.global_theme != GlobalTheme::Custom
            || snapshot.preferences.custom_theme.keyboard.as_ref() != Some(&record.design)
        {
            self.remove_record(lock)?;
            return Ok(snapshot);
        }
        let mut preferences = snapshot.preferences;
        if record.previous_theme != GlobalTheme::Custom {
            preferences.custom_theme.base = record.previous_base;
            preferences.custom_theme.candidate_skin = record.previous_candidate_skin;
            preferences.custom_theme.candidate_skin_dark = record.previous_candidate_skin_dark;
        }
        preferences.global_theme = record.previous_theme;
        preferences.custom_theme.keyboard = record.previous_design;
        let restored = self.preferences.save(snapshot.revision, preferences)?;
        self.remove_record(lock)?;
        Ok(restored)
    }

    fn lock(&self) -> Result<TrialLock, KeyboardSkinTrialError> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        let directory = file_lock::open_private_directory(&self.directory)?;
        let lock =
            file_lock::open_private_lock_file_at(&directory, std::ffi::OsStr::new(LOCK_FILE))?;
        file_lock::exclusive(&lock)?;
        Ok(TrialLock {
            directory,
            _lock: lock,
        })
    }

    fn pending(&self, lock: &TrialLock) -> Result<Option<TrialRecord>, KeyboardSkinTrialError> {
        let file = match file_lock::open_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(RECORD_FILE),
        ) {
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

    fn write_record(
        &self,
        lock: &TrialLock,
        record: &TrialRecord,
    ) -> Result<(), KeyboardSkinTrialError> {
        let bytes = serde_json::to_vec(record)?;
        if bytes.len() as u64 > MAXIMUM_RECORD_BYTES {
            return Err(KeyboardSkinTrialError::Invalid);
        }
        file_lock::write_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(RECORD_FILE),
            &bytes,
        )?;
        Ok(())
    }

    fn remove_record(&self, lock: &TrialLock) -> Result<(), KeyboardSkinTrialError> {
        match file_lock::remove_private_file_at(&lock.directory, std::ffi::OsStr::new(RECORD_FILE))
        {
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
        night.custom_theme.candidate_skin_dark = Some("midnight".into());
        let original = preferences.save(loaded.revision, night).unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x0F1E2D,
            ..TouchKeyboardSkinDesign::default()
        };

        let (trial, applied) = trials.begin("夜间试用", design.clone()).unwrap();
        assert_eq!(applied.preferences.global_theme, GlobalTheme::Custom);
        assert_eq!(applied.preferences.custom_theme.base, GlobalTheme::Night);
        assert_eq!(applied.preferences.custom_theme.candidate_skin, None);
        assert_eq!(applied.preferences.custom_theme.candidate_skin_dark, None);
        assert_eq!(applied.preferences.custom_theme.keyboard, Some(design));

        let restored = trials.finish(trial.id, false).unwrap();
        assert_eq!(restored.preferences, original.preferences);
    }

    #[test]
    fn a_trial_from_the_native_theme_draws_over_system_and_declining_restores_it() {
        let (_root, preferences, trials) = stores();
        let loaded = preferences.load().unwrap();
        let mut native = loaded.preferences;
        native.global_theme = GlobalTheme::Native;
        let original = preferences.save(loaded.revision, native).unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x1E2D0F,
            ..TouchKeyboardSkinDesign::default()
        };

        let (trial, applied) = trials.begin("原生试用", design.clone()).unwrap();
        assert_eq!(applied.preferences.global_theme, GlobalTheme::Custom);
        assert_eq!(applied.preferences.custom_theme.base, GlobalTheme::System);
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
        custom.custom_theme.candidate_skin_dark = Some("midnight".into());
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
        assert_eq!(
            applied
                .preferences
                .custom_theme
                .candidate_skin_dark
                .as_deref(),
            Some("midnight")
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
        // 恢复的是试用前的键盘皮肤，新安装的默认值里没有。
        let before = preferences
            .load()
            .unwrap()
            .preferences
            .custom_theme
            .keyboard;
        trials.begin("待恢复", design.clone()).unwrap();
        let recovered = KeyboardSkinTrialStore::new(root.path(), Arc::clone(&preferences));
        let restored = recovered.restore_pending().unwrap();
        assert_eq!(restored.preferences.custom_theme.keyboard, before);

        let (trial, applied) = trials.begin("不覆盖后续选择", design).unwrap();
        let mut later = applied.preferences;
        later.global_theme = GlobalTheme::Night;
        preferences.save(applied.revision, later).unwrap();
        let current = trials.finish(trial.id, false).unwrap();
        assert_eq!(current.preferences.global_theme, GlobalTheme::Night);
    }

    #[test]
    fn a_pending_record_from_before_the_dark_slot_still_restores() {
        let (root, preferences, trials) = stores();
        let loaded = preferences.load().unwrap();
        let mut night = loaded.preferences;
        night.global_theme = GlobalTheme::Night;
        let original = preferences.save(loaded.revision, night).unwrap();
        let design = TouchKeyboardSkinDesign {
            background: 0x0F2D1E,
            ..TouchKeyboardSkinDesign::default()
        };
        let (trial, _) = trials.begin("旧版记录", design).unwrap();
        // 旧版本写的记录没有 `previous_candidate_skin_dark`；`deny_unknown_fields` 之下它仍要读得出来。
        let path = root.path().join("KeyboardSkinTrial.json");
        let written: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(written.get("previous_candidate_skin_dark").is_none());
        let mut legacy = written;
        legacy["previous_candidate_skin"] = "sakura".into();
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let restored = trials.finish(trial.id, false).unwrap();
        assert_eq!(restored.preferences.global_theme, GlobalTheme::Night);
        assert_eq!(
            restored.preferences.custom_theme.candidate_skin.as_deref(),
            Some("sakura")
        );
        assert_eq!(restored.preferences.custom_theme.candidate_skin_dark, None);
        assert_eq!(
            restored.preferences.custom_theme.base,
            original.preferences.custom_theme.base
        );
        assert!(!path.exists());
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
            previous_candidate_skin_dark: None,
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

    #[cfg(unix)]
    #[test]
    fn trial_record_stays_bound_to_the_locked_directory_after_replacement() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("trial");
        fs::create_dir(&directory).unwrap();
        let preferences = Arc::new(PreferencesStore::new(root.path().join("preferences")));
        let trials = KeyboardSkinTrialStore::new(&directory, preferences);
        let lock = trials.lock().unwrap();
        let record = TrialRecord {
            id: Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
            name: "合成试用".into(),
            previous_theme: GlobalTheme::System,
            previous_base: GlobalTheme::System,
            previous_candidate_skin: None,
            previous_candidate_skin_dark: None,
            previous_design: None,
            design: TouchKeyboardSkinDesign::default(),
        };

        let moved = root.path().join("trial-moved");
        fs::rename(&directory, &moved).unwrap();
        fs::create_dir(&directory).unwrap();

        trials.write_record(&lock, &record).unwrap();

        assert!(moved.join("KeyboardSkinTrial.json").exists());
        assert!(!directory.join("KeyboardSkinTrial.json").exists());
        fs::remove_dir_all(moved).unwrap();
    }
}
