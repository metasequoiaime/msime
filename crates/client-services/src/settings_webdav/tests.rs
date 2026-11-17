//! 假网盘执行与生产相同的 GET/PROPFIND/MOVE，验证两台设备的实际同步状态。
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

#[path = "recovery_tests.rs"]
mod recovery;
#[path = "scheduled_tests.rs"]
mod scheduled;

#[derive(Default)]
struct Disk {
    files: BTreeMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
    requests: Vec<(String, String)>,
    unauthorized: bool,
    race_next_append: bool,
    competing_append: Option<Vec<u8>>,
    append_hook: Option<Box<dyn FnOnce() + Send>>,
    lose_append_response: bool,
}
#[derive(Default)]
struct Drive(Mutex<Disk>);

impl DavHttp for Drive {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        let mut disk = self.0.lock().unwrap();
        disk.requests
            .push((request.method.into(), request.path.into()));
        let response = |status, body| DavResponse {
            status,
            body,
            retry_after: None,
        };
        if disk.unauthorized {
            return Ok(response(401, Vec::new()));
        }
        let parent = |path: &str| {
            path.trim_end_matches('/')
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/"))
                .unwrap_or_default()
        };
        let path = request.path.to_owned();
        let status = match request.method {
            "MKCOL" => {
                if disk.directories.contains(&path) {
                    405
                } else if !parent(&path).is_empty() && !disk.directories.contains(&parent(&path)) {
                    409
                } else {
                    disk.directories.insert(path);
                    201
                }
            }
            "GET" => {
                return Ok(match disk.files.get(&path) {
                    Some(bytes) => response(200, bytes.clone()),
                    None => response(404, Vec::new()),
                });
            }
            "PUT" => {
                if !disk.directories.contains(&parent(&path)) {
                    409
                } else {
                    disk.files
                        .insert(path, request.body.unwrap_or_default().to_vec());
                    201
                }
            }
            "MOVE" => {
                let destination = request.destination.unwrap().to_owned();
                if disk.race_next_append && destination.starts_with("msime-settings/v1/e-") {
                    disk.race_next_append = false;
                    if let Some(bytes) = disk.competing_append.take() {
                        disk.files.insert(destination, bytes);
                    }
                    return Ok(response(412, Vec::new()));
                }
                if disk.files.contains_key(&destination) {
                    412
                } else if !disk.directories.contains(&parent(&destination)) {
                    409
                } else if let Some(bytes) = disk.files.remove(&path) {
                    let settings_event = destination.starts_with("msime-settings/v1/e-");
                    disk.files.insert(destination, bytes);
                    if settings_event {
                        if let Some(hook) = disk.append_hook.take() {
                            hook();
                        }
                        if std::mem::take(&mut disk.lose_append_response) {
                            return Err(RelayError::Unavailable);
                        }
                    }
                    201
                } else {
                    404
                }
            }
            "DELETE" => {
                disk.files.remove(&path);
                if disk.directories.remove(&path) {
                    disk.files.retain(|file, _| !file.starts_with(&path));
                    disk.directories
                        .retain(|directory| !directory.starts_with(&path));
                }
                204
            }
            "PROPFIND" => {
                if !disk.directories.contains(&path) {
                    return Ok(response(404, Vec::new()));
                }
                let mut paths = BTreeSet::from([path.clone()]);
                paths.extend(
                    disk.files
                        .keys()
                        .chain(disk.directories.iter())
                        .filter(|file| parent(file) == path)
                        .cloned(),
                );
                let body = format!(
                    "<d:multistatus xmlns:d=\"DAV:\">{}</d:multistatus>",
                    paths
                        .iter()
                        .map(|file| format!(
                            "<d:response><d:href>/dav/{file}</d:href></d:response>"
                        ))
                        .collect::<String>()
                );
                return Ok(response(207, body.into_bytes()));
            }
            method => panic!("unexpected method: {method}"),
        };
        Ok(response(status, Vec::new()))
    }
}

fn service(root: &std::path::Path, drive: &Arc<Drive>, name: &str) -> SettingsWebDav {
    SettingsWebDav {
        config: RelayConfig::webdav(
            "https://dav.example.test/dav",
            "synthetic-user",
            "synthetic-password",
        )
        .unwrap(),
        directory: root.join(name).join("settings-sync"),
        device_name: format!("合成{name}"),
        http: drive.clone(),
        create_identity: true,
        clock: Arc::new(Clock),
    }
}

fn preferences(root: &std::path::Path, name: &str) -> PreferencesStore {
    PreferencesStore::new(root.join(name).join("preferences"))
}

fn edit(
    store: &PreferencesStore,
    action: impl FnOnce(&mut msime_client_core::preferences::Preferences),
) {
    let mut snapshot = store.load().unwrap();
    action(&mut snapshot.preferences);
    store.save(snapshot.revision, snapshot.preferences).unwrap();
}

fn paired(
    root: &std::path::Path,
    drive: &Arc<Drive>,
) -> (
    SettingsWebDav,
    SettingsWebDav,
    PreferencesStore,
    PreferencesStore,
) {
    let laptop = service(root, drive, "laptop");
    let phone = service(root, drive, "phone");
    let laptop_settings = preferences(root, "laptop");
    let phone_settings = preferences(root, "phone");
    laptop.run(&laptop_settings).unwrap();
    assert_eq!(
        phone.run(&phone_settings).unwrap_err(),
        SettingsWebDavError::Pending
    );
    let device = phone.devices().unwrap().device_id;
    laptop.approve(&device).unwrap();
    phone.run(&phone_settings).unwrap();
    (laptop, phone, laptop_settings, phone_settings)
}

#[test]
fn two_devices_pair_sync_independent_edits_and_keep_local_credentials() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !settings.chinese_punctuation;
        settings.voice_input.asr_token = "synthetic-private-token".into();
    });
    let punctuation = phone_settings
        .load()
        .unwrap()
        .preferences
        .chinese_punctuation;
    assert!(laptop.run(&laptop_settings).unwrap().uploaded);
    let combined = phone.run(&phone_settings).unwrap();
    assert!(combined.uploaded && combined.downloaded && combined.conflicts.is_empty());
    laptop.run(&laptop_settings).unwrap();
    assert_eq!(
        phone_settings
            .load()
            .unwrap()
            .preferences
            .candidate_page_size,
        9
    );
    assert_eq!(
        laptop_settings
            .load()
            .unwrap()
            .preferences
            .chinese_punctuation,
        punctuation
    );
    assert_eq!(
        phone_settings
            .load()
            .unwrap()
            .preferences
            .voice_input
            .asr_token,
        "synthetic-private-token"
    );
    assert!(!laptop.run(&laptop_settings).unwrap().uploaded);
    let disk = drive.0.lock().unwrap();
    assert!(disk.requests.iter().all(|(_, path)| path.starts_with(ROOT)));
    assert!(disk
        .files
        .values()
        .all(|bytes| !String::from_utf8_lossy(bytes).contains("synthetic-private-token")));
    assert!(disk
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("msime-settings/v1/e-"))
        .all(|(_, bytes)| !String::from_utf8_lossy(bytes).contains("candidate_page_size")));
}

#[test]
fn joining_adopts_remote_choices_and_a_later_reset_reaches_the_other_device() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let laptop = service(root.path(), &drive, "laptop");
    let phone = service(root.path(), &drive, "phone");
    let laptop_settings = preferences(root.path(), "laptop");
    let phone_settings = preferences(root.path(), "phone");
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| settings.candidate_page_size = 8);
    laptop.run(&laptop_settings).unwrap();
    assert_eq!(
        phone.run(&phone_settings).unwrap_err(),
        SettingsWebDavError::Pending
    );
    laptop.approve(&phone.devices().unwrap().device_id).unwrap();
    let joined = phone.run(&phone_settings).unwrap();
    assert!(joined.downloaded && !joined.uploaded);
    assert_eq!(
        phone_settings
            .load()
            .unwrap()
            .preferences
            .candidate_page_size,
        9
    );

    let default_page_size =
        msime_client_core::preferences::Preferences::default().candidate_page_size;
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = default_page_size
    });
    assert!(laptop.run(&laptop_settings).unwrap().uploaded);
    let reset = phone.run(&phone_settings).unwrap();
    assert!(reset.downloaded && !reset.uploaded);
    assert_eq!(
        phone_settings
            .load()
            .unwrap()
            .preferences
            .candidate_page_size,
        default_page_size
    );
    assert!(!phone.run(&phone_settings).unwrap().uploaded);
}

#[test]
fn a_same_field_conflict_keeps_both_device_choices_and_remote_bytes() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| settings.candidate_page_size = 8);
    laptop.run(&laptop_settings).unwrap();
    let files = drive.0.lock().unwrap().files.clone();
    let conflict = phone.run(&phone_settings).unwrap();
    assert_eq!(conflict.conflicts, ["candidate_page_size"]);
    assert!(!conflict.uploaded && !conflict.downloaded);
    assert_eq!(
        phone_settings
            .load()
            .unwrap()
            .preferences
            .candidate_page_size,
        8
    );
    assert_eq!(drive.0.lock().unwrap().files, files);
}

#[test]
fn rollback_and_unauthorized_responses_never_replace_preferences() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let service = service(root.path(), &drive, "laptop");
    let preferences = preferences(root.path(), "laptop");
    service.run(&preferences).unwrap();
    let before = preferences.load().unwrap();
    drive.0.lock().unwrap().unauthorized = true;
    assert_eq!(
        service.run(&preferences).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::Unauthorized)
    );
    drive.0.lock().unwrap().unauthorized = false;
    drive
        .0
        .lock()
        .unwrap()
        .files
        .remove("msime-settings/v1/e-000000000000.json");
    assert_eq!(
        service.run(&preferences).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::Rollback)
    );
    assert_eq!(preferences.load().unwrap(), before);
}

#[test]
fn an_ime_without_an_existing_identity_does_not_create_files_or_connect() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let service = service(root.path(), &drive, "ime").existing_identity_only();
    assert_eq!(
        service.run(&preferences(root.path(), "ime")).unwrap_err(),
        SettingsWebDavError::Relay(RelayError::IdentityMissing)
    );
    assert!(!service.directory.exists());
    assert!(drive.0.lock().unwrap().requests.is_empty());
}

#[test]
fn concurrent_append_is_not_overwritten_and_retry_keeps_local_changes() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let service = service(root.path(), &drive, "laptop");
    let settings = preferences(root.path(), "laptop");
    service.run(&settings).unwrap();
    edit(&settings, |settings| settings.candidate_page_size = 9);
    drive.0.lock().unwrap().race_next_append = true;
    assert_eq!(
        service.run(&settings).unwrap_err(),
        SettingsWebDavError::Conflict
    );
    assert!(service.run(&settings).unwrap().uploaded);
    assert_eq!(settings.load().unwrap().preferences.candidate_page_size, 9);
}

#[test]
fn a_lost_append_does_not_replay_downloaded_settings_as_local_edits() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let (laptop, phone, laptop_settings, phone_settings) = paired(root.path(), &drive);
    let original_punctuation = laptop_settings
        .load()
        .unwrap()
        .preferences
        .chinese_punctuation;
    edit(&laptop_settings, |settings| {
        settings.candidate_page_size = 9
    });
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = !original_punctuation
    });
    phone.run(&phone_settings).unwrap();

    let encoded = phone.keyring().load_keystore().unwrap().unwrap();
    let keystore = crate::cloud::clipboard_e2e::Keystore::parse(&encoded).unwrap();
    let cipher = SettingsSyncCipher::from_keystore(&keystore).unwrap();
    let bytes = drive.0.lock().unwrap().files["msime-settings/v1/e-000000000001.json"].clone();
    let previous = cipher
        .open_event(
            &msime_client_core::settings_sync::SealedSettingsEvent::from_json(&bytes).unwrap(),
        )
        .unwrap();
    edit(&phone_settings, |settings| {
        settings.chinese_punctuation = original_punctuation
    });
    let event = SettingsSyncEvent::new(
        2,
        keystore.device_id(),
        previous.head().unwrap(),
        SettingsSyncDocument::from_preferences(&phone_settings.load().unwrap().preferences)
            .unwrap(),
    )
    .unwrap();
    let competitor = cipher.seal_event(&event).unwrap().to_json().unwrap();
    {
        let mut disk = drive.0.lock().unwrap();
        disk.race_next_append = true;
        disk.competing_append = Some(competitor.clone());
    }
    let before = laptop_settings.load().unwrap();
    assert_eq!(
        laptop.run(&laptop_settings).unwrap_err(),
        SettingsWebDavError::Conflict
    );
    assert_eq!(laptop_settings.load().unwrap(), before);
    assert_eq!(
        drive.0.lock().unwrap().files["msime-settings/v1/e-000000000002.json"],
        competitor
    );

    assert!(laptop.run(&laptop_settings).unwrap().uploaded);
    let result = laptop_settings.load().unwrap().preferences;
    assert_eq!(result.candidate_page_size, 9);
    assert_eq!(result.chinese_punctuation, original_punctuation);
    phone.run(&phone_settings).unwrap();
    assert_eq!(phone_settings.load().unwrap().preferences, result);
}
