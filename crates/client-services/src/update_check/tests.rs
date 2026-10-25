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

/// 选中安装包的文件名和摘要；一个发布都没选中时为 `None`。
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

// 各版本发布在同一个平台 tag 下，只靠资产名区分。
fn linux_release() -> Value {
    release(
        "linux-v1.2.0",
        json!([
            { "name": "msime-linux_1.2.0_amd64.deb", "digest": digest('a') },
            { "name": "msime-linux-1.2.0-linux-x86_64.tar.gz", "digest": digest('b') },
            { "name": "msime-linux-wubi_1.2.0_amd64.deb", "digest": digest('c') },
            { "name": "msime-linux-wubi-1.2.0-linux-x86_64.tar.gz", "digest": digest('d') },
            { "name": "msime-linux-pinyin_1.2.0_amd64.deb", "digest": digest('e') },
            // glibc 2.28 系统用的 legacy 包（#6311）：包名同样是 msime-linux，附件按 msime-linux-legacy_ 命名，full 和各版本都不能选中它。
            { "name": "msime-linux-legacy_1.2.0_amd64.deb", "digest": digest('9') },
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
            // `msime-windows` 的安装程序名。本仓库从不发布它；放在这里是为了证明 `full` 不会把它当成自己的安装包。
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
    // 没有 `.deb` 时 `full` 退回到自己的 tarball，绝不会选五笔版的。
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
    // 该发布还没有这个版本的资产：仍然提供版本号，只是没有可供校验的安装包。
    assert_eq!(
        pick(&[windows_release()], "windows", Some("cantonese"), None),
        NOTHING
    );
    // 不是合法版本 id 的值选不出任何资产。
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
        json!({ "name": "msime-linux-legacy_1.2.0_arm64.deb", "digest": digest('8') }),
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
    // 这个发布里 `pinyin` 没有 aarch64 包，所以 aarch64 宿主拿不到任何 x86_64 包。
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
    // 没有上传 `.deb` 时退回到 tarball。
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
    // 较老的响应会省略摘要或给 `null`；其他算法、大写十六进制或长度不足的值同样不信任。
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
    // 不知道宿主架构时，有两种架构的包，无论展示哪一个摘要都会对某些用户是错的。
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
    // 每种架构都退回到自己的 tarball，绝不会选另一种架构的 `.deb`。
    let tarballs_and_amd64_deb = json!([both[4], both[5], both[0]]);
    assert_eq!(
        for_arch(tarballs_and_amd64_deb, "aarch64"),
        chosen("msime-linux-1.2.0-linux-aarch64.tar.gz", Some(hex('f')))
    );
    // 还没有 aarch64 包的旧发布不给 aarch64 宿主提供任何安装包，而不是给它 x86_64 的包。
    assert_eq!(for_arch(json!([both[0], both[4]]), "aarch64"), NOTHING);
    // 没有为之构建安装包的架构保留全部资产；有两个候选时仍然什么都不提供。
    assert_eq!(for_arch(both.clone(), "riscv64"), NOTHING);
    assert_eq!(
        for_arch(json!([both[0]]), "riscv64"),
        chosen("msime-linux_1.2.0_amd64.deb", Some(digest_a.clone()))
    );
    // 需要 shell 引号的文件名绝不会出现在可复制的命令里。
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
    // Windows 绝不会把 Linux 安装包当成自己的安装程序。
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
    // 其他平台不看资产。
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
    // `release-windows.yml` 上传的内容：安装程序和它的 `.sha256` 文件。
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
    // 不带摘要的旧响应仍保留文件名，页面可以据此指向 `.sha256` 文件。
    assert_eq!(
        windows(json!([{ "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": null }])),
        Some((
            Some("MetasequoiaIME-Full_Setup_v1.2.0.exe".to_owned()),
            None,
            Some(false)
        ))
    );
    // 两个安装程序有歧义；需要加引号的文件名绝不会进入命令。
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
        channel: None,
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
    // 别的平台的新发布不是本平台的更新。
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
                "channel": "stable",
                "build_run": null,
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
        "channel": "beta",
    }))
    .unwrap();
    assert_eq!(parsed.edition.as_deref(), Some("wubi"));
    assert_eq!(parsed.arch.as_deref(), Some("aarch64"));
    assert_eq!(parsed.channel.as_deref(), Some("beta"));
    assert_eq!(parsed.effective_channel(), Some(UpdateChannel::Beta));
    assert!(serde_json::from_value::<UpdateCheckRequest>(json!({
        "platform": "linux",
        "current_version": "1.0.0",
        "url": "https://example.com",
    }))
    .is_err());
}

#[test]
fn windows7_only_offers_its_own_edition_architecture_and_compatible_installer() {
    let compatible = release(
        "windows7-v1.2.0",
        json!([
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0_win7_x64.exe", "digest": digest('a') },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0_win7_x86.exe", "digest": digest('b') },
            { "name": "MetasequoiaIME-Wubi_Setup_v1.2.0_win7_x64.exe", "digest": digest('c') },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0.exe", "digest": digest('d') },
            { "name": "MetasequoiaIME-Full_Setup_v1.2.0_win7_x64.exe.sha256" }
        ]),
    );
    let releases = [windows_release(), compatible.clone()];
    for (architecture, suffix, hash) in [
        ("x86_64", "x64", 'a'),
        ("x64", "x64", 'a'),
        ("x86", "x86", 'b'),
        ("i686", "x86", 'b'),
    ] {
        assert_eq!(
            pick(&releases, "windows7", None, Some(architecture)),
            chosen(
                &format!("MetasequoiaIME-Full_Setup_v1.2.0_win7_{suffix}.exe"),
                Some(hex(hash))
            )
        );
    }
    for architecture in [None, Some("aarch64"), Some(""), Some("unknown")] {
        assert_eq!(pick(&releases, "windows7", None, architecture), None);
    }
    assert_eq!(
        pick(&releases, "windows7", Some("wubi"), Some("x86_64")),
        chosen(
            "MetasequoiaIME-Wubi_Setup_v1.2.0_win7_x64.exe",
            Some(hex('c'))
        )
    );
    assert_eq!(pick(&releases, "windows7", Some("wubi"), Some("x86")), None);
    assert_eq!(
        pick(&releases, "windows7", Some("pinyin"), Some("x86_64")),
        None
    );
    assert_eq!(
        pick(&[windows_release()], "windows7", None, Some("x86_64")),
        None
    );
    assert_eq!(pick(&[compatible], "windows", None, None), None);
    assert_eq!(
        pick(&releases, "windows", None, None),
        chosen("MetasequoiaIME-Full_Setup_v1.2.0.exe", Some(hex('a')))
    );
}

#[test]
fn windows7_skips_missing_ambiguous_and_unpublished_installers() {
    let name = "MetasequoiaIME-Full_Setup_v1.2.0_win7_x64.exe";
    let valid = release("windows7-v1.2.0", json!([{ "name": name }]));
    let missing = release(
        "windows7-v9.0.0",
        json!([{ "name": "MetasequoiaIME-Full_Setup_v9.0.0_win7_x86.exe" }]),
    );
    let duplicate_name = "MetasequoiaIME-Full_Setup_v8.0.0_win7_x64.exe";
    let duplicate = release(
        "windows7-v8.0.0",
        json!([{ "name": duplicate_name }, { "name": duplicate_name }]),
    );
    let mut prerelease = valid.clone();
    prerelease["prerelease"] = json!(true);
    let mut draft = valid.clone();
    draft["draft"] = json!(true);
    assert_eq!(
        pick(
            &[missing.clone(), duplicate.clone(), prerelease, draft],
            "windows7",
            None,
            Some("x86_64")
        ),
        None
    );
    assert_eq!(
        pick(
            &[missing, duplicate, valid.clone()],
            "windows7",
            None,
            Some("x86_64")
        ),
        chosen(name, None)
    );
    let request = UpdateCheckRequest {
        platform: "windows7".into(),
        current_version: "1.0.0".into(),
        edition: None,
        arch: Some("x86_64".into()),
        channel: None,
    };
    assert!(matches!(
        evaluate_update(
            serde_json::to_vec(&vec![valid]).unwrap().as_slice(),
            &request
        ),
        Ok(UpdateCheck::Available(_))
    ));
}

#[test]
fn windows7_rejects_mismatched_release_pages_and_installer_versions() {
    let wrong_version = release(
        "windows7-v2.0.0",
        json!([{ "name": "MetasequoiaIME-Full_Setup_v1.0.0_win7_x64.exe" }]),
    );
    let mut wrong_page = release(
        "windows7-v2.0.0",
        json!([{ "name": "MetasequoiaIME-Full_Setup_v2.0.0_win7_x64.exe" }]),
    );
    wrong_page["html_url"] = json!(format!("{PAGE}/tag/windows-v2.0.0"));
    let old_contract = release(
        "windows7-v2.0.0",
        json!([{ "name": "MetasequoiaIME-Full_Windows7_Setup_v2.0.0.exe" }]),
    );
    assert_eq!(
        pick(
            &[wrong_version, wrong_page, old_contract],
            "windows7",
            None,
            Some("x86_64")
        ),
        None
    );
}

// ---- 更新通道 ----

fn channel_request(platform: &str, current_version: &str, channel: &str) -> UpdateCheckRequest {
    UpdateCheckRequest {
        channel: Some(channel.to_owned()),
        ..request(platform, current_version)
    }
}

/// 带 `msime-build` 标记的发布，`prerelease` 照 `release-*.yml`：带后缀的都是预发布。
fn marked_release(tag: &str, run: u64, assets: Value) -> Value {
    let mut release = release(tag, assets);
    let channel = if tag.ends_with("-beta") {
        "beta"
    } else if tag.ends_with("-alpha") {
        "alpha"
    } else {
        "stable"
    };
    release["prerelease"] = Value::Bool(channel != "stable");
    release["body"] = Value::String(format!(
        "## 更新内容\n\n- 合成的更新条目\n\n<!-- msime-build {{\"run\":{run},\"source\":\"{}\",\"channel\":\"{channel}\"}} -->\n",
        "a".repeat(40)
    ));
    release
}

fn windows_assets(version: &str) -> Value {
    json!([
        { "name": format!("MetasequoiaIME-Full_Setup_v{version}.exe"), "digest": digest('a') },
        { "name": format!("MetasequoiaIME-Wubi_Setup_v{version}.exe"), "digest": digest('b') },
    ])
}

/// `scripts/publish-nightly.py` 发布的滚动 nightly：正文末尾是状态块，附件名在扩展名前多了 `-nightly.<run>.<attempt>`。
fn nightly(platforms: Value, assets: Value) -> Value {
    let state = json!({ "version": 1, "platforms": platforms, "checksum": "SHA256SUMS-900.1" });
    json!({
        "tag_name": "nightly",
        "prerelease": true,
        "html_url": format!("{PAGE}/tag/nightly"),
        "body": format!("各平台独立更新。\n\n| 平台 | 本轮结果 |\n\n<!-- msime-nightly-state\n{state}\n-->\n"),
        "assets": assets,
    })
}

fn nightly_entry(version: Option<&str>, run: Option<u64>, files: &[(&str, char)]) -> Value {
    let mut entry = json!({
        "source": "b".repeat(40),
        "updated_at": "2026-10-25T18:30:00+00:00",
        "run_url": "https://github.com/metasequoiaime/msime/actions/runs/900",
        "files": files.iter().map(|(name, hash)| json!({ "name": name, "sha256": hex(*hash) })).collect::<Vec<_>>(),
    });
    if let Some(version) = version {
        entry["version"] = json!(version);
    }
    if let Some(run) = run {
        entry["run"] = json!(run);
    }
    entry
}

fn published(files: &[(&str, char)]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|(name, hash)| json!({ "name": name, "digest": digest(*hash) }))
            .collect(),
    )
}

const WINDOWS_NIGHTLY: [(&str, char); 2] = [
    ("MetasequoiaIME-Full_Setup_v1.4.0-nightly.900.1.exe", '1'),
    ("MetasequoiaIME-Wubi_Setup_v1.4.0-nightly.900.1.exe", '2'),
];
const LINUX_NIGHTLY: [(&str, char); 6] = [
    ("msime-linux_1.4.0_amd64-nightly.900.1.deb", '3'),
    ("msime-linux_1.4.0_arm64-nightly.900.1.deb", '4'),
    ("msime-linux-wubi_1.4.0_amd64-nightly.900.1.deb", '5'),
    ("msime-linux-1.4.0-1.x86_64-nightly.900.1.rpm", '6'),
    ("msime-linux-1.4.0-1.aarch64-nightly.900.1.rpm", '7'),
    ("msime-linux-legacy_1.4.0_amd64-nightly.900.1.deb", '8'),
];

fn channel_releases() -> Vec<Value> {
    let mut assets: Vec<(&str, char)> = WINDOWS_NIGHTLY.to_vec();
    assets.extend(LINUX_NIGHTLY);
    vec![
        nightly(
            json!({
                "windows": nightly_entry(Some("1.4.0"), Some(900), &WINDOWS_NIGHTLY),
                "linux": nightly_entry(Some("1.4.0"), Some(900), &LINUX_NIGHTLY),
                // 加 `version` 之前写下的条目：不知道版本号，不提供。
                "macos": nightly_entry(None, None, &[]),
            }),
            published(&assets),
        ),
        marked_release("windows-v1.3.0-alpha", 800, windows_assets("1.3.0")),
        marked_release("windows-v1.3.0-beta", 700, windows_assets("1.3.0")),
        marked_release("windows-v1.2.0", 600, windows_assets("1.2.0")),
        // 没有标记的旧正式版。
        release("windows-v1.1.0", windows_assets("1.1.0")),
    ]
}

fn selected(
    releases: &[Value],
    platform: &str,
    channel: UpdateChannel,
) -> Option<(String, UpdateChannel, Option<u64>)> {
    select_channel_release(releases, platform, None, None, channel)
        .map(|update| (update.version.display, update.channel, update.build_run))
}

#[test]
fn the_stable_channel_is_unchanged_by_betas_alphas_and_nightlies() {
    let releases = channel_releases();
    assert_eq!(
        selected(&releases, "windows", UpdateChannel::Stable),
        Some(("1.2.0".into(), UpdateChannel::Stable, Some(600)))
    );
    assert_eq!(
        select_platform_release(&releases, "windows", None, None),
        select_channel_release(&releases, "windows", None, None, UpdateChannel::Stable)
    );
    // 请求不带通道就是稳定版。
    let list = serde_json::to_vec(&releases).unwrap();
    match evaluate_update(&list, &request("windows", "1.1.0")).unwrap() {
        UpdateCheck::Available(update) => {
            assert_eq!(update.version.display, "1.2.0");
            assert_eq!(update.channel, UpdateChannel::Stable);
            assert_eq!(
                update.installer_name.as_deref(),
                Some("MetasequoiaIME-Full_Setup_v1.2.0.exe")
            );
        }
        other => panic!("expected an update, got {other:?}"),
    }
}

#[test]
fn the_beta_channel_takes_only_exact_beta_tags() {
    let releases = channel_releases();
    assert_eq!(
        selected(&releases, "windows", UpdateChannel::Beta),
        Some(("1.3.0".into(), UpdateChannel::Beta, Some(700)))
    );
    let beta = select_channel_release(
        &releases,
        "windows",
        Some("wubi"),
        None,
        UpdateChannel::Beta,
    )
    .unwrap();
    assert_eq!(beta.release_url, format!("{PAGE}/tag/windows-v1.3.0-beta"));
    assert_eq!(
        beta.installer_name.as_deref(),
        Some("MetasequoiaIME-Wubi_Setup_v1.3.0.exe")
    );
    assert_eq!(beta.installer_sha256, Some(hex('b')));
    assert_eq!(beta.signed, Some(false));
    // 只有 alpha 和别的预发布：测试版通道里只剩正式版。
    for tag in [
        "windows-v1.5.0-alpha",
        "windows-v1.5.0-beta.1",
        "windows-v1.5.0-beta2",
        "windows-v1.5.0-rc-beta-x",
        "windows-1.5.0-beta",
    ] {
        let mut prerelease = release(tag, windows_assets("1.5.0"));
        prerelease["prerelease"] = Value::Bool(true);
        let releases = [
            prerelease,
            marked_release("windows-v1.2.0", 600, windows_assets("1.2.0")),
        ];
        assert_eq!(
            selected(&releases, "windows", UpdateChannel::Beta),
            Some(("1.2.0".into(), UpdateChannel::Stable, Some(600))),
            "{tag}"
        );
    }
    // beta 之后发了同一版本号的正式版（运行 ID 更大）：测试版用户拿到正式版。
    let releases = [
        marked_release("windows-v1.3.0", 750, windows_assets("1.3.0")),
        marked_release("windows-v1.3.0-beta", 700, windows_assets("1.3.0")),
    ];
    assert_eq!(
        selected(&releases, "windows", UpdateChannel::Beta),
        Some(("1.3.0".into(), UpdateChannel::Stable, Some(750)))
    );
}

#[test]
fn the_nightly_channel_reads_the_rolling_release_state_and_its_installers() {
    let releases = channel_releases();
    assert_eq!(
        selected(&releases, "windows", UpdateChannel::Nightly),
        Some(("1.4.0".into(), UpdateChannel::Nightly, Some(900)))
    );
    // 测试版和稳定版通道都看不到 nightly。
    assert_ne!(
        selected(&releases, "windows", UpdateChannel::Beta).map(|update| update.1),
        Some(UpdateChannel::Nightly)
    );
    let windows = |edition: Option<&str>| {
        select_channel_release(&releases, "windows", edition, None, UpdateChannel::Nightly).map(
            |update| {
                (
                    update.release_url,
                    update.installer_name,
                    update.installer_sha256,
                    update.signed,
                )
            },
        )
    };
    assert_eq!(
        windows(None),
        Some((
            format!("{PAGE}/tag/nightly"),
            Some("MetasequoiaIME-Full_Setup_v1.4.0-nightly.900.1.exe".to_owned()),
            Some(hex('1')),
            Some(false)
        ))
    );
    assert_eq!(
        windows(Some("wubi")).and_then(|update| update.1),
        Some("MetasequoiaIME-Wubi_Setup_v1.4.0-nightly.900.1.exe".to_owned())
    );
    assert_eq!(windows(Some("pinyin")).map(|update| update.1), Some(None));

    // Linux 的 nightly 只有 .deb 和 .rpm：按架构选 .deb，legacy 和别的版本不选。
    let linux = |edition: Option<&str>, arch: Option<&str>| {
        select_channel_release(&releases, "linux", edition, arch, UpdateChannel::Nightly)
            .map(|update| (update.installer_name, update.installer_sha256))
    };
    assert_eq!(
        linux(None, Some("x86_64")),
        Some((
            Some("msime-linux_1.4.0_amd64-nightly.900.1.deb".to_owned()),
            Some(hex('3'))
        ))
    );
    assert_eq!(
        linux(None, Some("aarch64")),
        Some((
            Some("msime-linux_1.4.0_arm64-nightly.900.1.deb".to_owned()),
            Some(hex('4'))
        ))
    );
    assert_eq!(
        linux(Some("wubi"), Some("x86_64")),
        Some((
            Some("msime-linux-wubi_1.4.0_amd64-nightly.900.1.deb".to_owned()),
            Some(hex('5'))
        ))
    );
    assert_eq!(linux(Some("wubi"), Some("aarch64")), Some((None, None)));
    // 架构不明：两个架构的 .deb 有歧义，什么都不选（与正式版相同）。
    assert_eq!(linux(None, None), Some((None, None)));
    // 状态里没有版本号的平台不提供 nightly。
    assert_eq!(selected(&releases, "macos", UpdateChannel::Nightly), None);
}

#[test]
fn a_nightly_installer_must_be_published_and_match_its_digest() {
    let entry = nightly_entry(Some("1.4.0"), Some(900), &WINDOWS_NIGHTLY[..1]);
    let installer = |assets: Value| {
        let release = nightly(json!({ "windows": entry.clone() }), assets);
        select_channel_release(&[release], "windows", None, None, UpdateChannel::Nightly)
            .map(|update| update.installer_name)
    };
    assert_eq!(
        installer(published(&WINDOWS_NIGHTLY[..1])),
        Some(Some(WINDOWS_NIGHTLY[0].0.to_owned()))
    );
    // 状态引用的附件不在发布里，或 GitHub 的摘要和状态不符：版本照样提供，安装包不提供。
    assert_eq!(installer(json!([])), Some(None));
    assert_eq!(
        installer(published(&[(WINDOWS_NIGHTLY[0].0, 'f')])),
        Some(None)
    );
    // GitHub 没给摘要时信状态里的。
    assert_eq!(
        installer(json!([{ "name": WINDOWS_NIGHTLY[0].0 }])),
        Some(Some(WINDOWS_NIGHTLY[0].0.to_owned()))
    );
}

#[test]
fn a_malformed_or_foreign_nightly_is_not_offered() {
    let entry = json!({ "windows": nightly_entry(Some("1.4.0"), Some(900), &WINDOWS_NIGHTLY) });
    let offered = |release: Value| {
        select_channel_release(&[release], "windows", None, None, UpdateChannel::Nightly).is_some()
    };
    let good = nightly(entry.clone(), published(&WINDOWS_NIGHTLY));
    assert!(offered(good.clone()));
    let mut foreign = good.clone();
    foreign["html_url"] = json!("https://github.com/someone/else/releases/tag/nightly");
    assert!(!offered(foreign));
    let mut draft = good.clone();
    draft["draft"] = json!(true);
    assert!(!offered(draft));
    let mut no_state = good.clone();
    no_state["body"] = json!("nightly source: ".to_owned() + &"a".repeat(40));
    assert!(!offered(no_state));
    let mut future = good.clone();
    future["body"] = json!(format!(
        "<!-- msime-nightly-state\n{}\n-->\n",
        json!({ "version": 2, "platforms": entry })
    ));
    assert!(!offered(future));
    for version in ["1.4", "v1.4.0", "1.4.0-beta", "unknown"] {
        let mut odd = nightly_entry(Some(version), Some(900), &WINDOWS_NIGHTLY);
        odd["version"] = json!(version);
        let release = nightly(json!({ "windows": odd }), published(&WINDOWS_NIGHTLY));
        assert_eq!(offered(release), version == "1.4", "{version}");
    }
    // 没有 `run` 的条目照样提供，运行 ID 为空。
    let release = nightly(
        json!({ "windows": nightly_entry(Some("1.4.0"), None, &WINDOWS_NIGHTLY) }),
        published(&WINDOWS_NIGHTLY),
    );
    assert_eq!(
        selected(&[release], "windows", UpdateChannel::Nightly),
        Some(("1.4.0".into(), UpdateChannel::Nightly, None))
    );
}

#[test]
fn builds_compare_by_version_then_run() {
    let releases = [
        nightly(
            json!({ "windows": nightly_entry(Some("1.2.0"), Some(900), &[]) }),
            json!([]),
        ),
        marked_release("windows-v1.2.0", 600, windows_assets("1.2.0")),
    ];
    let nightly_check = |current: &str, run: Option<u64>| {
        evaluate_releases(
            &releases,
            &channel_request("windows", current, "nightly"),
            run,
        )
        .unwrap()
    };
    // 同一版本号：运行 ID 更大的 nightly 更新，不管列表顺序。
    match nightly_check("1.2.0", Some(600)) {
        UpdateCheck::Available(update) => {
            assert_eq!(update.channel, UpdateChannel::Nightly);
            assert_eq!(update.build_run, Some(900));
        }
        other => panic!("expected the nightly, got {other:?}"),
    }
    assert!(matches!(
        nightly_check("1.2.0", Some(900)),
        UpdateCheck::Current(_)
    ));
    assert!(matches!(
        nightly_check("1.2.0", Some(901)),
        UpdateCheck::Current(_)
    ));
    // 版本号更高的总是更新，版本号更低的运行 ID 再大也不是。
    assert!(matches!(
        nightly_check("1.1.9", Some(5_000)),
        UpdateCheck::Available(_)
    ));
    assert!(matches!(
        nightly_check("1.3.0", Some(1)),
        UpdateCheck::Current(_)
    ));
    // 不知道自己运行 ID 的构建（本机构建、发行版自编包）只比版本号：同一版本号不算更新。
    assert!(matches!(
        nightly_check("1.2.0", None),
        UpdateCheck::Current(_)
    ));
    let stable_check =
        |run: Option<u64>| evaluate_releases(&releases, &request("windows", "1.2.0"), run).unwrap();
    assert!(matches!(stable_check(None), UpdateCheck::Current(_)));
    assert!(matches!(stable_check(Some(599)), UpdateCheck::Available(_)));
    // 没有标记的旧发布没有运行 ID：同一版本号不算更新。
    let unmarked = [release("windows-v1.2.0", windows_assets("1.2.0"))];
    assert!(matches!(
        evaluate_releases(&unmarked, &request("windows", "1.2.0"), Some(1)).unwrap(),
        UpdateCheck::Current(_)
    ));
}

#[test]
fn the_build_marker_and_build_run_parse_strictly() {
    let body = |text: &str| Value::String(text.to_owned());
    assert_eq!(
        build_marker_run(Some(&body(
            "说明\n\n<!-- msime-build {\"run\":123,\"source\":\"x\",\"channel\":\"beta\"} -->\n"
        ))),
        Some(123)
    );
    // GitHub 网页上编辑过的正文是 CRLF 换行，末尾可能多出空白。
    assert_eq!(
        build_marker_run(Some(&body(
            "说明\r\n\r\n<!-- msime-build {\"run\":123,\"source\":\"x\",\"channel\":\"beta\"} -->\r\n\r\n"
        ))),
        Some(123)
    );
    // 更新说明由模型按 PR 文字生成，外部贡献者能在中间塞进同名标记：只认末尾 `release-build-marker.py` 追加的那一行。
    assert_eq!(
        build_marker_run(Some(&body(
            "- 示例 <!-- msime-build {\"run\":<u64>,\"source\":\"x\"} -->\n\n<!-- msime-build {\"run\":123,\"source\":\"x\",\"channel\":\"stable\"} -->\n"
        ))),
        Some(123)
    );
    assert_eq!(
        build_marker_run(Some(&body(
            "<!-- msime-build {\"run\":99999999999,\"source\":\"x\",\"channel\":\"stable\"} -->\n\n<!-- msime-build {\"run\":123,\"source\":\"x\",\"channel\":\"stable\"} -->\n"
        ))),
        Some(123)
    );
    for text in [
        "没有标记",
        // 标记不在最后一行（比如混在更新说明里、或者后面又被人加了文字）都不算。
        "<!-- msime-build {\"run\":99999999999} -->\n\n- 合成的更新条目\n",
        "前缀 <!-- msime-build {\"run\":123} -->",
        "<!-- msime-build {\"run\":0} -->",
        "<!-- msime-build {\"run\":-1} -->",
        "<!-- msime-build {\"run\":\"123\"} -->",
        "<!-- msime-build {\"run\":123}",
        "<!-- msime-build not json -->",
    ] {
        assert_eq!(build_marker_run(Some(&body(text))), None, "{text}");
    }
    assert_eq!(build_marker_run(None), None);
    assert_eq!(build_marker_run(Some(&Value::Null)), None);

    assert_eq!(parse_build_run(Some("38812345678")), Some(38_812_345_678));
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("-1"),
        Some(" 1"),
        Some("1a"),
        Some("99999999999999999999"),
    ] {
        assert_eq!(parse_build_run(value), None, "{value:?}");
    }
    assert_eq!(
        parse_build_run(Some("9999999999999999999")),
        Some(9_999_999_999_999_999_999)
    );
}

#[test]
fn nightly_asset_names_drop_only_the_publisher_s_infix() {
    assert_eq!(
        strip_nightly_infix("MetasequoiaIME-Full_Setup_v1.4.0-nightly.900.1.exe").as_deref(),
        Some("MetasequoiaIME-Full_Setup_v1.4.0.exe")
    );
    assert_eq!(
        strip_nightly_infix("msime-linux_1.4.0_amd64-nightly.12.3.deb").as_deref(),
        Some("msime-linux_1.4.0_amd64.deb")
    );
    for name in [
        "MetasequoiaIME-Full_Setup_v1.4.0.exe",
        "x-nightly.900.exe",
        "x-nightly.a.1.exe",
        "x-nightly.1.1.",
        "-nightly.1.1.exe",
        "x-nightly.1.1.tar.gz",
    ] {
        assert_eq!(strip_nightly_infix(name), None, "{name}");
    }
}

#[test]
fn windows7_ignores_the_channel() {
    let compatible = release(
        "windows7-v1.2.0",
        json!([{ "name": "MetasequoiaIME-Full_Setup_v1.2.0_win7_x64.exe", "digest": digest('a') }]),
    );
    let mut beta = release(
        "windows7-v1.3.0-beta",
        json!([{ "name": "MetasequoiaIME-Full_Setup_v1.3.0_win7_x64.exe", "digest": digest('b') }]),
    );
    beta["prerelease"] = json!(true);
    let nightly = nightly(
        json!({ "windows7": nightly_entry(Some("1.4.0"), Some(900), &[]) }),
        json!([]),
    );
    let releases = [nightly, beta, compatible];
    for channel in ["stable", "beta", "nightly"] {
        let request = UpdateCheckRequest {
            arch: Some("x64".into()),
            ..channel_request("windows7", "1.0.0", channel)
        };
        assert_eq!(request.effective_channel(), Some(UpdateChannel::Stable));
        match evaluate_releases(&releases, &request, None).unwrap() {
            UpdateCheck::Available(update) => {
                assert_eq!(update.version.display, "1.2.0", "{channel}");
                assert_eq!(update.channel, UpdateChannel::Stable);
            }
            other => panic!("{channel}: expected the stable release, got {other:?}"),
        }
    }
    for channel in [UpdateChannel::Beta, UpdateChannel::Nightly] {
        assert_eq!(
            select_channel_release(&releases, "windows7", None, Some("x64"), channel)
                .map(|update| update.version.display),
            Some("1.2.0".to_owned())
        );
    }
}

#[test]
fn an_unknown_channel_makes_the_request_invalid() {
    let list = serde_json::to_vec(&channel_releases()).unwrap();
    for channel in ["canary", "Beta", "", "alpha"] {
        assert_eq!(
            evaluate_update(&list, &channel_request("windows", "1.0.0", channel)),
            Err(UpdateCheckError::Invalid),
            "{channel}"
        );
    }
    assert!(matches!(
        evaluate_update(&list, &channel_request("windows", "1.0.0", "beta")).unwrap(),
        UpdateCheck::Available(_)
    ));
}

#[test]
fn an_update_without_the_new_fields_still_reads() {
    let old = json!({
        "version": { "display": "1.2.0", "parts": [1, 2, 0] },
        "release_url": format!("{PAGE}/tag/windows-v1.2.0"),
        "installer_name": null,
        "installer_sha256": null,
        "signed": false,
    });
    let update: ReleaseUpdate = serde_json::from_value(old).unwrap();
    assert_eq!(update.channel, UpdateChannel::Stable);
    assert_eq!(update.build_run, None);
    let nightly = select_channel_release(
        &channel_releases(),
        "windows",
        None,
        None,
        UpdateChannel::Nightly,
    )
    .unwrap();
    let value = serde_json::to_value(&nightly).unwrap();
    assert_eq!(value["channel"], "nightly");
    assert_eq!(value["build_run"], 900);
    assert_eq!(
        serde_json::from_value::<ReleaseUpdate>(value).unwrap(),
        nightly
    );
}
