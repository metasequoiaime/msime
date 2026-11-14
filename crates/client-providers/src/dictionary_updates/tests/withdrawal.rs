use super::*;

fn installed_pair() -> Fixture {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"previous synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let previous: serde_json::Value = serde_json::from_slice(&fixture.files.borrow()[API]).unwrap();
    fixture.release("dict-v2.0.11", b"withdrawn synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let mut releases: serde_json::Value =
        serde_json::from_slice(&fixture.files.borrow()[API]).unwrap();
    releases[0]["prerelease"] = json!(true);
    releases.as_array_mut().unwrap().push(previous[0].clone());
    fixture
        .files
        .borrow_mut()
        .insert(API.into(), serde_json::to_vec(&releases).unwrap());
    fixture.requests.borrow_mut().clear();
    fixture.ranges.borrow_mut().clear();
    fixture
}

fn edit_installed(fixture: &Fixture, change: impl FnOnce(&mut Installed)) {
    let path = root(&fixture.context.state_root).join("installed.json");
    let mut installed = read(&path).unwrap();
    change(&mut installed);
    write(&path, &installed).unwrap();
}

fn edit_releases(fixture: &Fixture, change: impl FnOnce(&mut serde_json::Value)) {
    let mut releases: serde_json::Value =
        serde_json::from_slice(&fixture.files.borrow()[API]).unwrap();
    change(&mut releases);
    fixture
        .files
        .borrow_mut()
        .insert(API.into(), serde_json::to_vec(&releases).unwrap());
}

fn expire_check(fixture: &Fixture) {
    let mut saved = status(&fixture.context);
    saved.checked_at = 0;
    write(
        &root(&fixture.context.state_root).join("status.json"),
        &saved,
    )
    .unwrap();
}

#[test]
fn withdrawn_active_switches_to_verified_previous_without_downloading_or_reloading() {
    let fixture = installed_pair();
    let state = &fixture.context.state_root;
    let active = selected(state, &fixture.context.baseline).unwrap();
    let loaded = directory(state, &active.desktop).unwrap();
    record_loaded(state, &loaded).unwrap();
    expire_check(&fixture);
    let result = check_with(&fixture.context, true, &fixture).unwrap();
    assert_eq!(result.version, "dict-v2.0.10");
    assert_eq!(result.phase, "rolled_back");
    assert!(result.enabled && result.pending_restart);
    let installed: Installed = read(&root(state).join("installed.json")).unwrap();
    assert_eq!(installed.previous.unwrap().version, "dict-v2.0.11");
    assert_eq!(
        read::<Loaded>(&root(state).join(LOADED)).unwrap().resources,
        loaded
    );
    assert_eq!(&*fixture.requests.borrow(), &[API.to_owned()]);
    assert!(fixture.ranges.borrow().is_empty());
    fixture.requests.borrow_mut().clear();
    check_with(&fixture.context, true, &fixture).unwrap();
    assert!(fixture.requests.borrow().is_empty());
}

#[test]
fn unusable_previous_keeps_the_withdrawn_active_and_never_downloads_an_older_release() {
    for case in [
        "absent",
        "different_baseline",
        "invalid_version",
        "higher_version",
        "same_version",
        "missing_file",
        "corrupt_file",
        "withdrawn",
        "draft",
        "unlisted",
    ] {
        let fixture = installed_pair();
        let state = &fixture.context.state_root;
        let installed: Installed = read(&root(state).join("installed.json")).unwrap();
        let previous = installed.previous.unwrap();
        let file = directory(state, &previous.desktop)
            .unwrap()
            .join("msime-pinyin.db");
        match case {
            "absent" => edit_installed(&fixture, |i| i.previous = None),
            "different_baseline" => edit_installed(&fixture, |i| {
                i.previous.as_mut().unwrap().baseline_generation = "c".repeat(64);
            }),
            "invalid_version" | "higher_version" | "same_version" => {
                edit_installed(&fixture, |i| {
                    i.previous.as_mut().unwrap().version = match case {
                        "invalid_version" => "invalid",
                        "higher_version" => "dict-v2.0.12",
                        _ => "dict-v2.0.11",
                    }
                    .into();
                })
            }
            "missing_file" => std::fs::remove_file(&file).unwrap(),
            "corrupt_file" => std::fs::write(
                &file,
                vec![b'x'; previous.desktop.artifacts[0].size as usize],
            )
            .unwrap(),
            "withdrawn" => edit_releases(&fixture, |r| r[1]["prerelease"] = json!(true)),
            "draft" => edit_releases(&fixture, |r| r[1]["draft"] = json!(true)),
            "unlisted" => edit_releases(&fixture, |r| {
                r.as_array_mut().unwrap().pop();
            }),
            _ => unreachable!(),
        }
        let before = std::fs::read(root(state).join("installed.json")).unwrap();
        let result = check_with(&fixture.context, false, &fixture).unwrap();
        assert_eq!(result.version, "dict-v2.0.11", "{case}");
        assert_eq!(
            std::fs::read(root(state).join("installed.json")).unwrap(),
            before,
            "{case}"
        );
        assert!(result.enabled, "{case}");
        assert!(fixture.ranges.borrow().is_empty(), "{case}");
    }
}

#[test]
fn missing_release_information_and_network_errors_do_not_infer_withdrawal() {
    for case in ["stable", "unlisted", "draft", "network_error"] {
        let fixture = installed_pair();
        match case {
            "stable" => edit_releases(&fixture, |r| r[0]["prerelease"] = json!(false)),
            "unlisted" => edit_releases(&fixture, |r| {
                r.as_array_mut().unwrap().remove(0);
            }),
            "draft" => edit_releases(&fixture, |r| r[0]["draft"] = json!(true)),
            "network_error" => {
                fixture.files.borrow_mut().remove(API);
            }
            _ => unreachable!(),
        }
        let result = check_with(&fixture.context, false, &fixture);
        if case == "network_error" {
            assert!(matches!(result, Err(UpdateError::Network)));
        } else {
            assert_eq!(result.unwrap().version, "dict-v2.0.11", "{case}");
        }
        assert_eq!(
            selected(&fixture.context.state_root, &fixture.context.baseline)
                .unwrap()
                .version,
            "dict-v2.0.11"
        );
    }
}

#[test]
fn manual_rollback_preserves_either_automatic_setting_and_skips_only_that_release() {
    for enabled in [true, false] {
        let fixture = Fixture::new();
        configure(&fixture.context, enabled).unwrap();
        fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
        check_with(&fixture.context, false, &fixture).unwrap();
        let result = rollback(&fixture.context).unwrap();
        assert_eq!(result.enabled, enabled);
        assert_eq!(result.version, "dict-v2.0.9");
        fixture.requests.borrow_mut().clear();
        assert_eq!(
            check_with(&fixture.context, false, &fixture)
                .unwrap()
                .version,
            "dict-v2.0.9"
        );
        assert_eq!(&*fixture.requests.borrow(), &[API.to_owned()]);
        configure(&fixture.context, !enabled).unwrap();
        configure(&fixture.context, enabled).unwrap();
        fixture.release("dict-v2.0.11", b"fixed synthetic dictionary", 1);
        expire_check(&fixture);
        let result = check_with(&fixture.context, true, &fixture).unwrap();
        // 关闭时自动检查不联网，显式手动检查仍可安装修复版。
        let result = if enabled {
            result
        } else {
            assert_eq!(result.version, "dict-v2.0.9");
            check_with(&fixture.context, false, &fixture).unwrap()
        };
        assert_eq!(result.version, "dict-v2.0.11");
        assert_eq!(result.enabled, enabled);
    }
}

#[test]
fn withdrawn_release_without_previous_can_still_install_a_higher_fixed_release() {
    let fixture = installed_pair();
    edit_installed(&fixture, |i| i.previous = None);
    let withdrawn: serde_json::Value =
        serde_json::from_slice(&fixture.files.borrow()[API]).unwrap();
    fixture.release("dict-v2.0.12", b"fixed synthetic dictionary", 1);
    edit_releases(&fixture, |r| {
        r.as_array_mut().unwrap().push(withdrawn[0].clone())
    });
    let result = check_with(&fixture.context, false, &fixture).unwrap();
    assert_eq!(result.version, "dict-v2.0.12");
    assert!(result.enabled);
}

#[test]
fn unreadable_skip_record_never_silently_reinstalls_the_rejected_version() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    rollback(&fixture.context).unwrap();
    let path = root(&fixture.context.state_root).join("rollback.json");
    for bytes in [b"broken".as_slice(), br#"{"skipped_version":"invalid"}"#] {
        std::fs::write(&path, bytes).unwrap();
        fixture.requests.borrow_mut().clear();
        assert!(matches!(
            check_with(&fixture.context, false, &fixture),
            Err(UpdateError::Invalid)
        ));
        assert_eq!(status(&fixture.context).version, "dict-v2.0.9");
        assert!(status(&fixture.context).enabled);
        assert_eq!(&*fixture.requests.borrow(), &[API.to_owned()]);
    }
}

#[test]
fn failing_to_record_the_skipped_version_leaves_selection_and_setting_untouched() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    let folder = root(&fixture.context.state_root);
    std::fs::create_dir(folder.join("rollback.json")).unwrap();
    let before = std::fs::read(folder.join("installed.json")).unwrap();
    assert!(rollback(&fixture.context).is_err());
    assert_eq!(
        std::fs::read(folder.join("installed.json")).unwrap(),
        before
    );
    assert!(status(&fixture.context).enabled);
    assert_eq!(status(&fixture.context).version, "dict-v2.0.10");
}

#[test]
fn an_explicit_second_rollback_can_restore_the_downloaded_slot_from_the_bundle() {
    let fixture = Fixture::new();
    fixture.release("dict-v2.0.10", b"synthetic dictionary", 1);
    check_with(&fixture.context, false, &fixture).unwrap();
    assert_eq!(rollback(&fixture.context).unwrap().version, "dict-v2.0.9");
    assert_eq!(rollback(&fixture.context).unwrap().version, "dict-v2.0.10");
    assert!(status(&fixture.context).enabled);
}

#[cfg(unix)]
#[test]
fn automatic_withdrawal_does_not_follow_a_symlinked_previous_generation() {
    let fixture = installed_pair();
    let state = &fixture.context.state_root;
    let installed: Installed = read(&root(state).join("installed.json")).unwrap();
    let previous = installed.previous.unwrap();
    let directory = directory(state, &previous.desktop).unwrap();
    let renamed = directory.with_extension("synthetic");
    std::fs::rename(&directory, &renamed).unwrap();
    std::os::unix::fs::symlink(&renamed, &directory).unwrap();
    assert_eq!(
        check_with(&fixture.context, false, &fixture)
            .unwrap()
            .version,
        "dict-v2.0.11"
    );
    assert!(fixture.ranges.borrow().is_empty());
}

#[test]
fn automatic_withdrawal_verifies_all_previous_language_artifacts() {
    for corrupt in [false, true] {
        let fixture = installed_pair();
        let mut languages: ResourceSet = serde_json::from_str(include_str!(
            "../../../../../resources/language-dictionaries.lock.json"
        ))
        .unwrap();
        let bytes = b"synthetic language artifact";
        for file in &mut languages.artifacts {
            file.url = format!("{DOWNLOAD}dict-v2.0.10/{}", file.name);
            file.size = bytes.len() as u64;
            file.sha256 = hex::encode(Sha256::digest(bytes));
        }
        let directory = store(&fixture.context.state_root)
            .install(&languages, |_| Ok(Box::new(io::Cursor::new(bytes))))
            .unwrap();
        edit_installed(&fixture, |i| {
            i.previous.as_mut().unwrap().languages = Some(languages.clone())
        });
        if corrupt {
            std::fs::write(
                directory.join(&languages.artifacts[0].name),
                vec![b'x'; bytes.len()],
            )
            .unwrap();
        }
        let result = check_with(&fixture.context, false, &fixture).unwrap();
        assert_eq!(
            result.version,
            if corrupt {
                "dict-v2.0.11"
            } else {
                "dict-v2.0.10"
            }
        );
        assert!(fixture.ranges.borrow().is_empty());
    }
}

#[test]
fn switching_off_during_the_release_request_cancels_automatic_slot_changes() {
    struct SwitchOff<'a>(&'a Fixture);
    impl Transport for SwitchOff<'_> {
        fn open(&self, url: &str, timeout: Duration) -> Result<Box<dyn Read>, UpdateError> {
            configure(&self.0.context, false).unwrap();
            self.0.open(url, timeout)
        }
        fn files(&self) -> &dyn Fetcher {
            self.0
        }
    }
    let fixture = installed_pair();
    expire_check(&fixture);
    let result = check_with(&fixture.context, true, &SwitchOff(&fixture)).unwrap();
    assert_eq!(result.phase, "cancelled");
    assert!(!result.enabled);
    assert_eq!(result.version, "dict-v2.0.11");
    assert!(fixture.ranges.borrow().is_empty());
}
