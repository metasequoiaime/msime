use super::*;
use serde_json::json;

const PAGE: &str = RELEASES_PAGE_URL;

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn hex(character: char) -> String {
    character.to_string().repeat(64)
}

fn release(tag: &str, assets: Value) -> Value {
    json!({ "tag_name": tag, "html_url": format!("{PAGE}/tag/{tag}"), "assets": assets })
}

/// The chosen package's name and digest, or `None` when no release was selected at all.
fn pick(
    releases: &[Value],
    platform: &str,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<(Option<String>, Option<String>)> {
    select_platform_release(releases, platform, edition, arch)
        .map(|update| (update.installer_name, update.installer_sha256))
}

fn chosen(name: &str, sha256: Option<String>) -> Option<(Option<String>, Option<String>)> {
    Some((Some(name.to_owned()), sha256))
}

const NOTHING: Option<(Option<String>, Option<String>)> = Some((None, None));

// The editions publish under the same platform tag and are told apart by asset name only.
fn linux_release() -> Value {
    release(
        "linux-v1.2.0",
        json!([
            { "name": "msime-linux_1.2.0_amd64.deb", "digest": digest('a') },
            { "name": "msime-linux-1.2.0-linux-x86_64.tar.gz", "digest": digest('b') },
            { "name": "msime-linux-wubi_1.2.0_amd64.deb", "digest": digest('c') },
            { "name": "msime-linux-wubi-1.2.0-linux-x86_64.tar.gz", "digest": digest('d') },
            { "name": "msime-linux-pinyin_1.2.0_amd64.deb", "digest": digest('e') },
            { "name": "SHA256SUMS", "digest": digest('f') },
        ]),
    )
}

fn windows_release() -> Value {
    release(
        "windows-v1.2.0",
        json!([
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": digest('a') },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe.sha256", "digest": digest('b') },
            // msime-windows' installer name. This repository never publishes it; it is here to show full does not take it for its own.
            { "name": "MetasequoiaIME_Setup_v1.2.0.exe", "digest": digest('e') },
            { "name": "MetasequoiaIME-Wubi_Setup_v1.2.0.exe", "digest": digest('c') },
            { "name": "MetasequoiaIME-Pinyin_Setup_v1.2.0.exe", "digest": digest('d') },
        ]),
    )
}

#[test]
fn full_picks_its_own_linux_package_beside_the_other_editions() {
    for edition in [None, Some("full")] {
        assert_eq!(
            pick(&[linux_release()], "linux", edition, None),
            chosen("msime-linux_1.2.0_amd64.deb", Some(hex('a')))
        );
    }
    // Without a .deb full falls back to its own tarball, never the Wubi edition's.
    let mut without_deb = linux_release();
    without_deb["assets"]
        .as_array_mut()
        .unwrap()
        .retain(|asset| !asset["name"].as_str().unwrap().ends_with(".deb"));
    assert_eq!(
        pick(&[without_deb], "linux", None, None),
        chosen("msime-linux-1.2.0-linux-x86_64.tar.gz", Some(hex('b')))
    );
}

#[test]
fn an_edition_picks_only_its_own_package_and_installer() {
    assert_eq!(
        pick(&[linux_release()], "linux", Some("wubi"), None),
        chosen("msime-linux-wubi_1.2.0_amd64.deb", Some(hex('c')))
    );
    assert_eq!(
        pick(&[linux_release()], "linux", Some("pinyin"), None),
        chosen("msime-linux-pinyin_1.2.0_amd64.deb", Some(hex('e')))
    );
    for edition in [None, Some("full")] {
        assert_eq!(
            pick(&[windows_release()], "windows", edition, None),
            chosen("MetasequoiaIME-Full_Setup_v1.2.0.exe", Some(hex('a')))
        );
    }
    assert_eq!(
        pick(&[windows_release()], "windows", Some("wubi"), None),
        chosen("MetasequoiaIME-Wubi_Setup_v1.2.0.exe", Some(hex('c')))
    );
    // The release has no asset for this edition yet: the version is still offered, with no installer to verify.
    assert_eq!(
        pick(&[windows_release()], "windows", Some("cantonese"), None),
        NOTHING
    );
    // Something that is not an edition id selects no asset.
    assert_eq!(
        pick(&[windows_release()], "windows", Some("wubi|.*"), None),
        NOTHING
    );
}

#[test]
fn an_edition_picks_its_own_package_for_the_hosts_architecture() {
    let mut both = linux_release();
    both["assets"].as_array_mut().unwrap().extend([
        json!({ "name": "msime-linux_1.2.0_arm64.deb", "digest": digest('1') }),
        json!({ "name": "msime-linux-wubi_1.2.0_arm64.deb", "digest": digest('2') }),
        json!({ "name": "msime-linux-wubi-1.2.0-linux-aarch64.tar.gz", "digest": digest('3') }),
    ]);
    let both = [both];
    assert_eq!(
        pick(&both, "linux", None, Some("aarch64")),
        chosen("msime-linux_1.2.0_arm64.deb", Some(hex('1')))
    );
    assert_eq!(
        pick(&both, "linux", Some("wubi"), Some("aarch64")),
        chosen("msime-linux-wubi_1.2.0_arm64.deb", Some(hex('2')))
    );
    assert_eq!(
        pick(&both, "linux", Some("wubi"), Some("x86_64")),
        chosen("msime-linux-wubi_1.2.0_amd64.deb", Some(hex('c')))
    );
    // pinyin has no aarch64 package in this release, so an aarch64 host is offered none of the x86_64 ones.
    assert_eq!(
        pick(&both, "linux", Some("pinyin"), Some("aarch64")),
        NOTHING
    );
}

#[test]
fn linux_assets_yield_a_digest_only_when_well_formed_and_unambiguous() {
    let digest_a = hex('a');
    let one = |assets: Value| [release("linux-v1.2.0", assets)];
    let full = |assets: Value| {
        select_platform_release(&one(assets), "linux", None, None).map(|update| {
            (
                update.installer_name,
                update.installer_sha256,
                update.signed,
            )
        })
    };
    assert_eq!(
        full(
            json!([{ "name": "msime-linux_1.2.0_amd64.deb", "digest": format!("sha256:{digest_a}") }])
        ),
        Some((
            Some("msime-linux_1.2.0_amd64.deb".to_owned()),
            Some(digest_a.clone()),
            Some(false)
        ))
    );
    // The tarball is the fallback when no .deb was uploaded.
    assert_eq!(
        full(
            json!([{ "name": "msime-linux-1.2.0-linux-x86_64.tar.gz", "digest": format!("sha256:{digest_a}") }])
        ),
        Some((
            Some("msime-linux-1.2.0-linux-x86_64.tar.gz".to_owned()),
            Some(digest_a.clone()),
            Some(false)
        ))
    );
    // Older responses omit the digest or send null; another algorithm, uppercase hex or a short value is not trusted either.
    for bad in [
        Value::Null,
        json!(format!("sha512:{digest_a}")),
        json!(format!("sha256:{}", digest_a.to_uppercase())),
        json!(format!("sha256:{}", &digest_a[1..])),
        json!(digest_a.clone()),
        json!(42),
    ] {
        assert_eq!(
            full(json!([{ "name": "msime-linux_1.2.0_amd64.deb", "digest": bad }])),
            Some((
                Some("msime-linux_1.2.0_amd64.deb".to_owned()),
                None,
                Some(false)
            ))
        );
    }
    assert_eq!(
        full(json!([{ "name": "msime-linux_1.2.0_amd64.deb" }])),
        Some((
            Some("msime-linux_1.2.0_amd64.deb".to_owned()),
            None,
            Some(false)
        ))
    );
    // Without the host's architecture, two architectures would make any single digest wrong for someone.
    let both = json!([
        { "name": "msime-linux_1.2.0_amd64.deb", "digest": format!("sha256:{digest_a}") },
        { "name": "msime-linux_1.2.0_arm64.deb", "digest": digest('b') },
        { "name": "msime-linux-1.2.0-1.x86_64.rpm", "digest": digest('c') },
        { "name": "msime-linux-1.2.0-1.aarch64.rpm", "digest": digest('d') },
        { "name": "msime-linux-1.2.0-linux-x86_64.tar.gz", "digest": digest('e') },
        { "name": "msime-linux-1.2.0-linux-aarch64.tar.gz", "digest": digest('f') },
    ]);
    assert_eq!(full(both.clone()), Some((None, None, Some(false))));
    let for_arch = |assets: Value, arch: &str| pick(&one(assets), "linux", None, Some(arch));
    assert_eq!(
        for_arch(both.clone(), "x86_64"),
        chosen("msime-linux_1.2.0_amd64.deb", Some(digest_a.clone()))
    );
    assert_eq!(
        for_arch(both.clone(), "aarch64"),
        chosen("msime-linux_1.2.0_arm64.deb", Some(hex('b')))
    );
    // Each architecture falls back to its own tarball, never another architecture's .deb.
    let tarballs_and_amd64_deb = json!([both[4], both[5], both[0]]);
    assert_eq!(
        for_arch(tarballs_and_amd64_deb, "aarch64"),
        chosen("msime-linux-1.2.0-linux-aarch64.tar.gz", Some(hex('f')))
    );
    // A release from before aarch64 packages offers an aarch64 host nothing rather than the x86_64 package.
    assert_eq!(for_arch(json!([both[0], both[4]]), "aarch64"), NOTHING);
    // An architecture no package is built for keeps every asset, and two of them still offer nothing.
    assert_eq!(for_arch(both.clone(), "riscv64"), NOTHING);
    assert_eq!(
        for_arch(json!([both[0]]), "riscv64"),
        chosen("msime-linux_1.2.0_amd64.deb", Some(digest_a.clone()))
    );
    // A name that would need shell quoting is never offered for the copyable command.
    assert_eq!(
        full(json!([{ "name": "--x;rm -rf ~.deb", "digest": format!("sha256:{digest_a}") }])),
        Some((None, None, Some(false)))
    );
    for assets in [
        Value::Null,
        json!("x"),
        json!([null, 3, { "digest": format!("sha256:{digest_a}") }]),
    ] {
        assert_eq!(full(assets), Some((None, None, Some(false))));
    }
    // Windows never takes a Linux package for its installer.
    let windows = select_platform_release(
        &[release(
            "windows-v1.2.0",
            json!([{ "name": "msime-linux_1.2.0_amd64.deb", "digest": format!("sha256:{digest_a}") }]),
        )],
        "windows",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        (
            windows.installer_name,
            windows.installer_sha256,
            windows.signed
        ),
        (None, None, Some(false))
    );
    // Other platforms ignore assets.
    let macos = select_platform_release(
        &[release(
            "macos-v1.2.0",
            json!([{ "name": "MetasequoiaIME_Setup_v1.2.0.exe", "digest": format!("sha256:{digest_a}") }]),
        )],
        "macos",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        (macos.installer_name, macos.installer_sha256, macos.signed),
        (None, None, None)
    );
}

#[test]
fn windows_assets_yield_the_installer_digest_and_mark_the_build_unsigned() {
    let digest_d = hex('d');
    let windows = |assets: Value| {
        select_platform_release(&[release("windows-v1.2.0", assets)], "windows", None, None).map(
            |update| {
                (
                    update.installer_name,
                    update.installer_sha256,
                    update.signed,
                )
            },
        )
    };
    // What release-windows.yml uploads: the installer and its .sha256 file.
    assert_eq!(
        windows(json!([
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": format!("sha256:{digest_d}") },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe.sha256", "digest": digest('e') },
        ])),
        Some((
            Some("MetasequoiaIME-Full_Setup_v1.2.0.exe".to_owned()),
            Some(digest_d.clone()),
            Some(false)
        ))
    );
    // An older response without digests keeps the name, so the page can point at the .sha256 file.
    assert_eq!(
        windows(json!([{ "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": null }])),
        Some((
            Some("MetasequoiaIME-Full_Setup_v1.2.0.exe".to_owned()),
            None,
            Some(false)
        ))
    );
    // Two installers are ambiguous; a name needing quoting never reaches the command.
    assert_eq!(
        windows(json!([
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": format!("sha256:{digest_d}") },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0-x86.exe", "digest": format!("sha256:{digest_d}") },
        ])),
        Some((None, None, Some(false)))
    );
    assert_eq!(
        windows(
            json!([{ "name": "Setup v1.2.0;calc.exe", "digest": format!("sha256:{digest_d}") }])
        ),
        Some((None, None, Some(false)))
    );
    assert_eq!(windows(Value::Null), Some((None, None, Some(false))));
}

#[test]
fn the_newest_release_wins_by_version_not_by_list_order() {
    let releases = [
        json!({ "tag_name": "windows-v0.9.0", "html_url": format!("{PAGE}/tag/windows-v0.9.0") }),
        json!({ "tag_name": "windows-v0.10.0", "html_url": format!("{PAGE}/tag/windows-v0.10.0") }),
        json!({ "tag_name": "windows-v2.0.0", "html_url": format!("{PAGE}/tag/windows-v2.0.0"), "draft": true }),
        json!({ "tag_name": "windows-v3.0.0", "html_url": format!("{PAGE}/tag/windows-v3.0.0"), "prerelease": true }),
        json!({ "tag_name": "windowsx-v4.0.0", "html_url": format!("{PAGE}/tag/windowsx-v4.0.0") }),
        json!({ "tag_name": "linux-v9.0.0", "html_url": format!("{PAGE}/tag/linux-v9.0.0") }),
    ];
    assert_eq!(
        select_platform_release(&releases, "windows", None, None)
            .map(|update| update.version.display),
        Some("0.10.0".to_owned())
    );
    assert_eq!(select_platform_release(&[], "windows", None, None), None);
}

#[test]
fn a_release_page_outside_the_shared_repository_is_not_offered() {
    let releases = [json!({
        "tag_name": "windows-v1.2.0",
        "html_url": "https://github.com/metasequoiaime/MSIME-Windows/releases/tag/v1.2.0",
    })];
    assert_eq!(
        select_platform_release(&releases, "windows", None, None),
        None
    );
    let quoted = [json!({
        "tag_name": "windows-v1.2.0",
        "html_url": format!("{PAGE}/tag/windows-v1.2.0\"onclick"),
    })];
    assert_eq!(
        select_platform_release(&quoted, "windows", None, None),
        None
    );
}

#[test]
fn versions_parse_and_compare_like_the_settings_pages_show_them() {
    let version = |value: &str| parse_version(value).map(|version| version.display);
    assert_eq!(version("v1.2.0"), Some("1.2.0".to_owned()));
    assert_eq!(version(" V0.10 "), Some("0.10".to_owned()));
    assert_eq!(version("1.2.0-beta.1"), Some("1.2.0".to_owned()));
    assert_eq!(version("1.2.0+42"), Some("1.2.0".to_owned()));
    for invalid in ["", "v", "1..2", "1.2.", "x1.2", "1.2 beta", "linux-v1.2.0"] {
        assert_eq!(version(invalid), None, "{invalid}");
    }
    let compare = |left: &str, right: &str| {
        compare_versions(
            &parse_version(left).unwrap(),
            &parse_version(right).unwrap(),
        )
    };
    assert_eq!(compare("1.2", "1.2.0"), Ordering::Equal);
    assert_eq!(compare("0.10.0", "0.9.9"), Ordering::Greater);
    assert_eq!(compare("1.2.0", "1.10.0"), Ordering::Less);
}

fn request(platform: &str, current_version: &str) -> UpdateCheckRequest {
    UpdateCheckRequest {
        platform: platform.to_owned(),
        current_version: current_version.to_owned(),
        edition: None,
        arch: None,
    }
}

#[test]
fn a_check_reports_available_current_or_no_release() {
    let list = serde_json::to_vec(&[windows_release()]).unwrap();
    match evaluate_update(&list, &request("windows", "1.1.9")).unwrap() {
        UpdateCheck::Available(update) => {
            assert_eq!(update.version.display, "1.2.0");
            assert_eq!(update.release_url, format!("{PAGE}/tag/windows-v1.2.0"));
            assert_eq!(
                update.installer_name.as_deref(),
                Some("MetasequoiaIME-Full_Setup_v1.2.0.exe")
            );
        }
        other => panic!("expected an update, got {other:?}"),
    }
    assert!(matches!(
        evaluate_update(&list, &request("windows", "v1.2.0")).unwrap(),
        UpdateCheck::Current(_)
    ));
    assert!(matches!(
        evaluate_update(&list, &request("windows", "2.0")).unwrap(),
        UpdateCheck::Current(_)
    ));
    // Another platform's newer release is not this platform's update.
    assert_eq!(
        evaluate_update(&list, &request("linux", "0.1.0")).unwrap(),
        UpdateCheck::None
    );
}

#[test]
fn a_check_refuses_a_bad_request_or_a_document_that_is_not_a_release_list() {
    let list = serde_json::to_vec(&[windows_release()]).unwrap();
    assert_eq!(
        evaluate_update(&list, &request("windows", "unknown")),
        Err(UpdateCheckError::Invalid)
    );
    assert_eq!(
        evaluate_update(&list, &request("", "1.0.0")),
        Err(UpdateCheckError::Invalid)
    );
    for document in [&b"{\"message\":\"rate limited\"}"[..], b"<html>", b""] {
        assert_eq!(
            evaluate_update(document, &request("windows", "1.0.0")),
            Err(UpdateCheckError::Unavailable)
        );
    }
}

#[test]
fn a_result_serialises_as_hosts_read_it() {
    let list = serde_json::to_vec(&[windows_release()]).unwrap();
    let available = evaluate_update(&list, &request("windows", "1.0.0")).unwrap();
    assert_eq!(
        serde_json::to_value(&available).unwrap(),
        json!({
            "status": "available",
            "update": {
                "version": { "display": "1.2.0", "parts": [1, 2, 0] },
                "release_url": format!("{PAGE}/tag/windows-v1.2.0"),
                "installer_name": "MetasequoiaIME-Full_Setup_v1.2.0.exe",
                "installer_sha256": hex('a'),
                "signed": false,
            }
        })
    );
    assert_eq!(
        serde_json::to_value(UpdateCheck::None).unwrap(),
        json!({ "status": "none" })
    );
}

#[test]
fn a_request_document_takes_only_the_known_fields() {
    let parsed: UpdateCheckRequest = serde_json::from_value(json!({
        "platform": "linux",
        "current_version": "1.0.0",
        "edition": "wubi",
        "arch": "aarch64",
    }))
    .unwrap();
    assert_eq!(parsed.edition.as_deref(), Some("wubi"));
    assert_eq!(parsed.arch.as_deref(), Some("aarch64"));
    assert!(serde_json::from_value::<UpdateCheckRequest>(json!({
        "platform": "linux",
        "current_version": "1.0.0",
        "url": "https://example.com",
    }))
    .is_err());
}
