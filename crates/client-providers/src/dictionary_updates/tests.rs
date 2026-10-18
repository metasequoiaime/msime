use super::*;
use serde_json::json;
use std::cell::RefCell;

struct Fixture {
    _directory: tempfile::TempDir,
    context: UpdateContext,
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    requests: RefCell<Vec<String>>,
}

impl Transport for Fixture {
    fn open(&self, url: &str, _: Duration) -> Result<Box<dyn Read>, UpdateError> {
        self.requests.borrow_mut().push(url.to_owned());
        self.files
            .borrow()
            .get(url)
            .cloned()
            .map(|bytes| Box::new(io::Cursor::new(bytes)) as Box<dyn Read>)
            .ok_or(UpdateError::Network)
    }
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let resources = directory.path().join("resources");
        std::fs::create_dir(&resources).unwrap();
        let bytes = b"original synthetic dictionary";
        std::fs::write(resources.join("msime-pinyin.db"), bytes).unwrap();
        let baseline = ResourceSet {
            source_commit: "a".repeat(40),
            artifacts: vec![Artifact {
                name: "msime-pinyin.db".into(),
                url: format!("{DOWNLOAD}dict-v2.0.9/msime-pinyin.db"),
                sha256: hex::encode(Sha256::digest(bytes)),
                size: bytes.len() as u64,
            }],
        };
        let context = UpdateContext {
            state_root: directory.path().join("state"),
            resources,
            baseline,
            on_demand: Vec::new(),
            languages: None,
            format: 1,
        };
        Self {
            _directory: directory,
            context,
            files: RefCell::default(),
            requests: RefCell::default(),
        }
    }

    fn release(&self, tag: &str, data: &[u8], format: i32) {
        let hash = hex::encode(Sha256::digest(data));
        let manifest = serde_json::to_vec(&json!({
            "manifest_version": 1, "profile": "desktop", "format_version": format,
            "custom_dictionary_commit": "b".repeat(40),
            "engine_compatibility": { "dictionary_format": format, "japanese_model_magic": "MSJPDT1" },
            "licensing": { "includes_unlicensed_inputs": false },
            "files": { "msime-pinyin.db": { "sha256": hash, "size": data.len() } }
        })).unwrap();
        let descriptors = [("msime-pinyin.db", data.to_vec()), (MANIFEST, manifest)];
        let mut assets = Vec::new();
        for (name, bytes) in descriptors {
            let url = format!("{DOWNLOAD}{tag}/{name}");
            assets.push(
                json!({ "name": name, "browser_download_url": url, "size": bytes.len(),
                "digest": format!("sha256:{}", hex::encode(Sha256::digest(&bytes))) }),
            );
            self.files.borrow_mut().insert(url, bytes);
        }
        self.files.borrow_mut().insert(
            API.into(),
            serde_json::to_vec(&json!([
                { "tag_name": tag, "draft": false, "prerelease": false, "assets": assets }
            ]))
            .unwrap(),
        );
    }
}

#[test]
fn versions_are_numeric_and_reject_unstable_tags() {
    assert!(version("dict-v2.0.10") > version("dict-v2.0.9"));
    for invalid in [
        "v2.0.10",
        "dict-v2.0.10-beta",
        "dict-v2.0",
        "dict-v2.0.1/other",
        "dict-v2.0.-1",
    ] {
        assert_eq!(version(invalid), None);
    }
}

#[test]
fn a_complete_update_is_verified_and_original_bytes_remain() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"updated synthetic dictionary", 1);
    let result = check_with(&fixture.context, false, &fixture).unwrap();
    assert_eq!(result.phase, "ready");
    assert_eq!(result.version, "dict-v2.0.10");
    assert!(result.pending_restart && result.can_rollback);
    let active = selected(&fixture.context.state_root, &fixture.context.baseline).unwrap();
    let path = directory(&fixture.context.state_root, &active.desktop).unwrap();
    store(&fixture.context.state_root)
        .verify(&path, &active.desktop)
        .unwrap();
    assert_eq!(
        std::fs::read(path.join("msime-pinyin.db")).unwrap(),
        b"updated synthetic dictionary"
    );
    assert_eq!(
        std::fs::read(fixture.context.resources.join("msime-pinyin.db")).unwrap(),
        b"original synthetic dictionary"
    );
}

#[test]
fn corrupted_download_never_replaces_the_previous_selection() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"first updated dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    fixture.release("dict-v2.0.11", b"second updated dictionary", 1);
    fixture.files.borrow_mut().insert(
        format!("{DOWNLOAD}dict-v2.0.11/msime-pinyin.db"),
        b"broken".to_vec(),
    );
    assert!(matches!(
        check_with(&fixture.context, false, &fixture),
        Err(UpdateError::Integrity)
    ));
    assert_eq!(status(&fixture.context).version, "dict-v2.0.10");
    assert_eq!(status(&fixture.context).phase, "failed");
}

#[test]
fn incompatible_data_and_untrusted_asset_urls_are_rejected_before_download() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 2);
    assert!(matches!(
        check_with(&fixture.context, false, &fixture),
        Err(UpdateError::Incompatible)
    ));
    assert!(selected(&fixture.context.state_root, &fixture.context.baseline).is_none());
    assert!(!fixture
        .requests
        .borrow()
        .iter()
        .any(|url| url.ends_with("msime-pinyin.db")));
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    let mut releases: serde_json::Value =
        serde_json::from_slice(fixture.files.borrow().get(API).unwrap()).unwrap();
    releases[0]["assets"][1]["browser_download_url"] =
        json!("https://fixture.invalid/manifest.json");
    fixture
        .files
        .borrow_mut()
        .insert(API.into(), serde_json::to_vec(&releases).unwrap());
    assert!(matches!(
        check_with(&fixture.context, false, &fixture),
        Err(UpdateError::Invalid)
    ));
}

#[test]
fn disabled_and_daily_checks_do_not_access_the_network_but_manual_checks_do() {
    let fixture = Fixture::new();
    configure(&fixture.context, false).unwrap();
    check_with(&fixture.context, true, &fixture).unwrap();
    assert!(fixture.requests.borrow().is_empty());
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    configure(&fixture.context, true).unwrap();
    fixture.requests.borrow_mut().clear();
    check_with(&fixture.context, true, &fixture).unwrap();
    assert!(fixture.requests.borrow().is_empty());
}

#[test]
fn rollback_keeps_the_bundle_locator_and_disables_automatic_reinstallation() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let result = rollback(&fixture.context).unwrap();
    assert!(!result.enabled);
    assert_eq!(result.version, "dict-v2.0.9");
    assert!(selected(&fixture.context.state_root, &fixture.context.baseline).is_none());
    assert_eq!(
        bundled_resources(&fixture.context.state_root, &fixture.context.baseline),
        Some(fixture.context.resources.clone())
    );
}

#[test]
fn new_client_baselines_and_missing_required_components_do_not_use_old_selections() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let mut newer = fixture.context.baseline.clone();
    newer.source_commit = "c".repeat(40);
    assert!(selected(&fixture.context.state_root, &newer).is_none());
    let mut active = selected(&fixture.context.state_root, &fixture.context.baseline).unwrap();
    active.desktop.artifacts.clear();
    assert!(validate_selection(&active, &fixture.context.baseline).is_err());
}

#[test]
fn current_baseline_does_not_download_older_releases() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.8", b"older dictionary", 1);
    let status = check_with(&fixture.context, false, &fixture).unwrap();
    assert_eq!(status.phase, "current");
    assert_eq!(&*fixture.requests.borrow(), &[API.to_owned()]);
}

#[test]
fn an_existing_session_can_export_its_own_resource_generation_after_an_update() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"first updated dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let old = selected(&fixture.context.state_root, &fixture.context.baseline).unwrap();
    let path = directory(&fixture.context.state_root, &old.desktop).unwrap();
    fixture.release("dict-v2.0.11", b"second updated dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    fixture.release("dict-v2.0.12", b"third updated dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let pinned = set_for_directory(
        &fixture.context.state_root,
        &fixture.context.baseline,
        &path,
    )
    .unwrap();
    assert_eq!(
        pinned.generation().unwrap(),
        old.desktop.generation().unwrap()
    );
}

#[test]
fn newest_incompatible_release_does_not_hide_a_compatible_update() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"compatible dictionary", 1);
    let compatible: serde_json::Value =
        serde_json::from_slice(fixture.files.borrow().get(API).unwrap()).unwrap();
    fixture.release("dict-v3.0.0", b"incompatible dictionary", 2);
    let mut releases: serde_json::Value =
        serde_json::from_slice(fixture.files.borrow().get(API).unwrap()).unwrap();
    releases.as_array_mut().unwrap().push(compatible[0].clone());
    fixture
        .files
        .borrow_mut()
        .insert(API.into(), serde_json::to_vec(&releases).unwrap());
    assert_eq!(
        check_with(&fixture.context, false, &fixture)
            .unwrap()
            .version,
        "dict-v2.0.10"
    );
}

#[test]
fn repeated_check_of_an_installed_release_preserves_rollback_target() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"compatible dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    assert_eq!(
        check_with(&fixture.context, false, &fixture).unwrap().phase,
        "current"
    );
    assert_eq!(rollback(&fixture.context).unwrap().version, "dict-v2.0.9");
}

#[test]
fn interrupted_process_does_not_leave_the_controls_stuck_downloading() {
    let fixture = Fixture::new();
    configure(&fixture.context, true).unwrap();
    write(
        &root(&fixture.context.state_root).join("status.json"),
        &UpdateStatus {
            phase: "downloading".into(),
            ..UpdateStatus::default()
        },
    )
    .unwrap();
    assert_eq!(status(&fixture.context).phase, "interrupted");
    let lock = file_lock::ScopedFileLock::exclusive(
        file_lock::open_private_lock_file(root(&fixture.context.state_root).join("update.lock"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status(&fixture.context).phase, "downloading");
    drop(lock);
}

#[test]
fn a_language_download_failure_never_publishes_half_an_update() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"first dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    fixture.release("dict-v2.0.11", b"second dictionary", 1);
    let mut context = fixture.context.clone();
    let languages = fixture._directory.path().join("languages");
    std::fs::create_dir(&languages).unwrap();
    context.languages = Some(languages);
    let mut releases: serde_json::Value =
        serde_json::from_slice(fixture.files.borrow().get(API).unwrap()).unwrap();
    let pinned: ResourceSet = serde_json::from_str(include_str!(
        "../../../../resources/language-dictionaries.lock.json"
    ))
    .unwrap();
    for file in pinned.artifacts {
        let url = format!("{DOWNLOAD}dict-v2.0.11/{}", file.name);
        let bytes = b"synthetic licensed language data";
        releases[0]["assets"].as_array_mut().unwrap().push(json!({
            "name": file.name, "browser_download_url":url, "size": bytes.len(),
            "digest": format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
        }));
        if file.name != "msime-stroke.db" {
            fixture.files.borrow_mut().insert(url, bytes.to_vec());
        }
    }
    fixture
        .files
        .borrow_mut()
        .insert(API.into(), serde_json::to_vec(&releases).unwrap());
    assert!(check_with(&context, false, &fixture).is_err());
    assert_eq!(status(&context).version, "dict-v2.0.10");
    fixture.files.borrow_mut().insert(
        format!("{DOWNLOAD}dict-v2.0.11/msime-stroke.db"),
        b"synthetic licensed language data".to_vec(),
    );
    assert_eq!(
        check_with(&context, false, &fixture).unwrap().version,
        "dict-v2.0.11"
    );
    assert!(installed_file(&context.state_root, &context.baseline, "msime-stroke.db").is_some());
    rollback(&context).unwrap();
    assert!(installed_file(&context.state_root, &context.baseline, "msime-stroke.db").is_none());
}
