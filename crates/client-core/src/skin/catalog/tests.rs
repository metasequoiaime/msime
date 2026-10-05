//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;
use tempfile::tempdir;
fn manifest(id: &str) -> String {
    format!("schema_version = 1\nid = '{id}'\nname = 'Sample'\nversion = '1.0'\nbase = 'night'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n")
}

fn resource_package(root: &Path) -> std::path::PathBuf {
    let skin = root.join("sample");
    fs::create_dir_all(skin.join("images")).unwrap();
    fs::write(skin.join("skin.toml"), manifest("sample")).unwrap();
    skin
}

#[test]
fn scan_external_skin_under_non_ascii_directory() {
    let root = tempdir().unwrap();
    let non_ascii_root = root.path().join("用户目录").join("skins");
    let skin = resource_package(&non_ascii_root);

    let package = load(&non_ascii_root, "sample").unwrap();
    assert_eq!(package.id, "sample");
    assert_eq!(package.name, "Sample");
    assert_eq!(package.base, super::super::theme::GlobalTheme::Night);

    let catalog = scan(&non_ascii_root);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages.len(), 1);
    assert_eq!(catalog.packages[0].id, "sample");
    assert!(skin.join("skin.toml").exists());
}

#[test]
fn toolbar_source_distinguishes_absent_empty_and_declared_utf8_text() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("undeclared.css"), ".other {}").unwrap();
    assert_eq!(read_toolbar_stylesheet(root.path(), "sample"), Ok(None));
    fs::write(skin.join("toolbar.css"), "").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(String::new()))
    );
    fs::write(
        skin.join("toolbar.css"),
        "\u{feff}.status-bar { color: #123456; } /* 示例 */",
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(".status-bar { color: #123456; } /* 示例 */".into()))
    );
}

#[test]
fn toolbar_source_rechecks_manifest_and_rejects_invalid_encoding_and_size() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("toolbar.css"), [0xff, 0xfe]).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidEncoding)
    );
    fs::File::create(skin.join("toolbar.css"))
        .unwrap()
        .set_len(MAX_RESOURCE_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::TooLarge)
    );
    fs::write(skin.join("replacement.css"), ".replacement {}").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!(
            "toolbar_stylesheet = 'replacement.css'\n{}",
            manifest("sample")
        ),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(".replacement {}".into()))
    );
    fs::write(skin.join("skin.toml"), "invalid").unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidPackage)
    );
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "../sample"),
        Err(ResourceError::InvalidPath)
    );
}

#[cfg(unix)]
#[test]
fn toolbar_source_rejects_package_alias_and_escaping_stylesheet() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("outside.css"), ".outside {}").unwrap();
    symlink(outside.path().join("outside.css"), skin.join("toolbar.css")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidPackage)
    );
    symlink(&skin, root.path().join("alias")).unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "alias"),
        Err(ResourceError::InvalidPackage)
    );
}

#[test]
fn resource_reader_preserves_bytes_and_assigns_supported_content_types() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    for (extension, content_type) in [
        ("css", "text/css; charset=utf-8"),
        ("PNG", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
        ("ico", "image/x-icon"),
        ("bmp", "image/bmp"),
        ("avif", "image/avif"),
        ("woff", "font/woff"),
        ("woff2", "font/woff2"),
        ("ttf", "font/ttf"),
        ("otf", "font/otf"),
    ] {
        let relative = format!("images/sample.{extension}");
        fs::write(skin.join(&relative), [0, 1, 255]).unwrap();
        let result = read_resource(root.path(), "sample", &relative).unwrap();
        assert_eq!(result.content_type, content_type);
        assert_eq!(result.bytes, [0, 1, 255]);
    }
}

#[test]
fn stylesheet_reader_accepts_only_package_local_utf8_css() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::create_dir_all(skin.join("styles")).unwrap();
    fs::write(
        skin.join("styles/imported.css"),
        b"\xEF\xBB\xBF.imported {}",
    )
    .unwrap();
    fs::write(skin.join("styles/broken.css"), [0xff, 0xfe]).unwrap();
    fs::write(skin.join("styles/image.png"), b"not-an-image-decoder-test").unwrap();
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/imported.css").unwrap(),
        ".imported {}"
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/broken.css"),
        Err(ResourceError::InvalidEncoding)
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/image.png"),
        Err(ResourceError::UnsupportedType)
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "../outside.css"),
        Err(ResourceError::InvalidPath)
    );
}

#[test]
fn resource_reader_rejects_paths_urls_and_non_asset_types() {
    let root = tempdir().unwrap();
    resource_package(root.path());
    for relative in [
        "",
        "../sample.png",
        "/sample.png",
        "C:/sample.png",
        "images\\sample.png",
        "images//sample.png",
        "./sample.png",
        "%2e%2e/sample.png",
        "https://example.invalid/a.png",
        "sample.png?x=1",
    ] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::InvalidPath)
        );
    }
    assert_eq!(
        read_resource(root.path(), "../sample", "sample.png"),
        Err(ResourceError::InvalidPath)
    );
    for relative in [
        "skin.toml",
        "code.js",
        "page.html",
        "settings.json",
        "program.exe",
        "unknown",
    ] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::UnsupportedType)
        );
    }
}

#[test]
fn resource_reader_requires_current_valid_manifest() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("sample.png"), b"synthetic").unwrap();
    assert!(read_resource(root.path(), "sample", "sample.png").is_ok());
    fs::write(skin.join("skin.toml"), "invalid").unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "sample.png"),
        Err(ResourceError::InvalidPackage)
    );
    assert_eq!(
        read_resource(root.path(), "absent", "sample.png"),
        Err(ResourceError::InvalidPackage)
    );
}

#[test]
fn resource_reader_rejects_missing_directory_and_oversized_assets() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    assert_eq!(
        read_resource(root.path(), "sample", "absent.png"),
        Err(ResourceError::Unavailable)
    );
    fs::create_dir(skin.join("directory.png")).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "directory.png"),
        Err(ResourceError::Unavailable)
    );
    let file = fs::File::create(skin.join("large.png")).unwrap();
    file.set_len(MAX_RESOURCE_BYTES as u64).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "large.png")
            .unwrap()
            .bytes
            .len(),
        MAX_RESOURCE_BYTES
    );
    file.set_len(MAX_RESOURCE_BYTES as u64 + 1).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "large.png"),
        Err(ResourceError::TooLarge)
    );
}

#[cfg(unix)]
#[test]
fn resource_reader_rejects_symlink_escapes_and_package_aliases() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("sample.png"), b"synthetic").unwrap();
    symlink(outside.path().join("sample.png"), skin.join("escape.png")).unwrap();
    symlink(outside.path(), skin.join("escape")).unwrap();
    for relative in ["escape.png", "escape/sample.png"] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::InvalidPath)
        );
    }
    symlink(&skin, root.path().join("alias")).unwrap();
    assert_eq!(
        read_resource(root.path(), "alias", "escape.png"),
        Err(ResourceError::InvalidPackage)
    );
    fs::write(skin.join("images/local.png"), b"local").unwrap();
    symlink(skin.join("images/local.png"), skin.join("local.png")).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "local.png")
            .unwrap()
            .bytes,
        b"local"
    );
}

fn scan_manifest(body: &str) -> SkinCatalog {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), body).unwrap();
    scan(root.path())
}

#[test]
fn candidate_palettes_preserve_both_themes_and_serialize_host_names() {
    let body = format!("{}\n[candidate.dark]\naccent = '#123456'\nselected = '#234567'\nhover = '#345678'\nsurface = '#456789'\nborder = '#56789a'\ntext = '#6789ab'\nnumber = '#789abc'\ntranslation = '#89abcd'\nshow_selected_bar = false\n[candidate.light]\ntext = '#123'\nshow_selected_bar = true\n", manifest("sample"));
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let json = serde_json::to_value(&catalog.packages[0]).unwrap();
    assert_eq!(
        json["candidate"]["dark"],
        serde_json::json!({
            "accent": "#123456", "selected": "#234567", "hover": "#345678",
            "surface": "#456789", "border": "#56789a", "text": "#6789ab",
            "number": "#789abc", "translation": "#89abcd", "showSelectedBar": false,
        })
    );
    assert_eq!(json["candidate"]["light"]["text"], "#123");
    assert_eq!(json["candidate"]["light"]["showSelectedBar"], true);
    assert!(json["candidate"]["light"]["accent"].is_null());
    assert_eq!(
        scan_manifest(&manifest("sample")).packages[0].candidate,
        CandidateColors::default()
    );
}

#[test]
fn candidate_color_fields_enforce_types_and_utf8_byte_limits() {
    for theme in ["dark", "light"] {
        for key in [
            "accent",
            "selected",
            "hover",
            "surface",
            "border",
            "text",
            "number",
            "translation",
        ] {
            for value in [
                "false".to_owned(),
                "7".to_owned(),
                "[]".to_owned(),
                "{}".to_owned(),
                format!("'{}'", "a".repeat(81)),
                format!("'{}'", "色".repeat(27)),
            ] {
                let body = format!(
                    "{}\n[candidate.{theme}]\n{key} = {value}\n",
                    manifest("sample")
                );
                let catalog = scan_manifest(&body);
                assert!(catalog.packages.is_empty(), "accepted {theme}.{key}");
                assert_eq!(catalog.issues.len(), 1);
            }
            for value in [String::new(), "a".repeat(80)] {
                let body = format!(
                    "{}\n[candidate.{theme}]\n{key} = '{value}'\n",
                    manifest("sample")
                );
                assert_eq!(scan_manifest(&body).packages.len(), 1);
            }
        }
    }
}

#[test]
fn candidate_tables_and_selected_bar_reject_wrong_types() {
    for suffix in [
        "[candidate]\ndark = false",
        "[candidate]\nlight = []",
        "[candidate.dark]\nshow_selected_bar = 'false'",
        "[candidate.light]\nshow_selected_bar = 1",
    ] {
        let catalog = scan_manifest(&format!("{}\n{suffix}\n", manifest("sample")));
        assert!(catalog.packages.is_empty());
        assert_eq!(catalog.issues[0].reason, "invalid candidate colors");
    }
    let body = format!("candidate = false\n{}", manifest("sample"));
    assert!(scan_manifest(&body).packages.is_empty());
}

#[test]
fn preserves_capabilities_dimensions_and_relative_resources_for_hosts() {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir_all(skin.join("images")).unwrap();
    fs::write(skin.join("toolbar.css"), "/* fixture */").unwrap();
    fs::write(skin.join("images/preview.svg"), "<svg/>").unwrap();
    let body = format!(
        "toolbar_stylesheet = 'toolbar.css'\npreview = 'images/preview.svg'\n{}",
        manifest("sample")
    )
    .replace("['vertical']", "['vertical', 'horizontal']")
    .replace("min_width_dip = 10", "min_width_dip = 320.5")
    .replace("top_inset_dip = 0", "top_inset_dip = 24.5")
    .replace("width_dip = 0", "width_dip = 180");
    fs::write(skin.join("skin.toml"), body).unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.layouts, ["vertical", "horizontal"]);
    assert_eq!(package.themes, ["light"]);
    assert!(package.supports("vertical", "light"));
    assert!(package.supports("horizontal", "light"));
    assert!(!package.supports("vertical", "dark"));
    assert!(!package.supports("unknown", "light"));
    let json = serde_json::to_value(&catalog).unwrap();
    let package = &json["packages"][0];
    assert_eq!(package["minWidthDip"], 320.5);
    assert_eq!(package["decorationTopDip"], 24.5);
    assert_eq!(package["decorationWidthDip"], 180.0);
    assert_eq!(package["toolbarStylesheet"], "toolbar.css");
    assert_eq!(package["preview"], "images/preview.svg");
    assert_eq!(
        package["layouts"],
        serde_json::json!(["vertical", "horizontal"])
    );
}

#[test]
fn compatibility_does_not_inherit_unlisted_base_modes() {
    let catalog = scan_manifest(&manifest("sample"));
    let package = &catalog.packages[0];
    assert_eq!(package.base, super::super::theme::GlobalTheme::Night);
    assert!(package.supports("vertical", "light"));
    assert!(!package.supports("horizontal", "light"));
    assert!(!package.supports("vertical", "dark"));
    assert_eq!(package.toolbar_stylesheet, None);
    assert_eq!(package.preview, None);
}

#[test]
fn rejects_duplicate_supported_layouts_and_themes() {
    for (from, to) in [
        ("['vertical']", "['vertical', 'vertical']"),
        ("['light']", "['light', 'light']"),
    ] {
        let result = scan_manifest(&manifest("sample").replace(from, to));
        assert!(result.packages.is_empty());
        assert_eq!(result.issues[0].reason, "invalid supports");
    }
}

#[test]
fn rejects_wrong_numeric_types_and_nonfinite_or_out_of_range_dimensions() {
    for field in ["min_width_dip = 10", "top_inset_dip = 0", "width_dip = 0"] {
        let key = field.split(" = ").next().unwrap();
        for value in ["'10'", "false", "[]", "{}", "nan", "inf", "-1", "1001"] {
            let result =
                scan_manifest(&manifest("sample").replace(field, &format!("{key} = {value}")));
            assert!(result.packages.is_empty(), "accepted {key}={value}");
            assert_eq!(result.issues.len(), 1);
        }
    }
    assert_eq!(scan_manifest(&manifest("sample")).packages.len(), 1);
    let defaults = manifest("sample")
        .replace("min_width_dip = 10\n", "")
        .replace("top_inset_dip = 0\n", "")
        .replace("width_dip = 0\n", "");
    assert_eq!(scan_manifest(&defaults).packages.len(), 1);
}

#[test]
fn rejects_theme_ids_as_external_skin_folders() {
    let root = tempdir().unwrap();
    for id in [
        "system", "shuishan", "light", "paper", "night", "ink", "custom",
    ] {
        let skin = root.path().join(id);
        fs::create_dir(&skin).unwrap();
        fs::write(skin.join("skin.toml"), manifest(id)).unwrap();
    }
    let catalog = scan(root.path());
    assert!(catalog.packages.is_empty());
    assert_eq!(catalog.issues.len(), 7);
}

#[test]
fn toolbar_stylesheet_must_be_a_single_regular_css_file() {
    for (resource, directory) in [
        ("toolbar.css", false),
        ("toolbar.css", true),
        ("nested/toolbar.css", false),
        (".css", false),
    ] {
        let root = tempdir().unwrap();
        let skin = root.path().join("sample");
        fs::create_dir_all(skin.join("nested")).unwrap();
        let path = skin.join(resource);
        if directory {
            fs::create_dir(path).unwrap();
        } else {
            fs::write(path, "/* fixture */").unwrap();
        }
        fs::write(
            skin.join("skin.toml"),
            format!("toolbar_stylesheet = '{resource}'\n{}", manifest("sample")),
        )
        .unwrap();
        let result = scan(root.path());
        assert_eq!(
            result.packages.len(),
            usize::from(resource == "toolbar.css" && !directory)
        );
    }
}
#[test]
fn scans_valid_and_rejects_unsafe_manifests() {
    let dir = tempdir().unwrap();
    let skin = dir.path().join("sample_skin");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), "schema_version = 1\nid = 'sample_skin'\nname = 'Sample'\nversion = '1.0'\nbase = 'system'\nauthor = 'Test'\ndescription = 'Demo'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\n").unwrap();
    let catalog = scan(dir.path());
    assert_eq!(
        catalog.packages,
        vec![SkinSummary {
            id: "sample_skin".into(),
            name: "Sample".into(),
            version: "1.0".into(),
            // A system base draws over the platform tokens.
            base: super::super::theme::GlobalTheme::System,
            author: Some("Test".into()),
            description: Some("Demo".into()),
            layouts: vec!["vertical".into()],
            themes: vec!["light".into()],
            min_width_dip: 0.0,
            corner_radius_dip: None,
            decoration_top_dip: 0.0,
            decoration_width_dip: 0.0,
            decoration_image: None,
            decoration_align: DecorationAlign::Right,
            background: None,
            toolbar: SkinToolbar::default(),
            toolbar_stylesheet: None,
            preview: None,
            candidate: CandidateColors::default(),
            license: None,
        }],
        "{catalog:?}"
    );
}

#[test]
fn reports_invalid_ids_and_oversized_manifests_without_loading_them() {
    let dir = tempdir().unwrap();
    let invalid = dir.path().join("Bad");
    fs::create_dir(&invalid).unwrap();
    fs::write(invalid.join("skin.toml"), "schema_version = 1").unwrap();
    let huge = dir.path().join("huge");
    fs::create_dir(&huge).unwrap();
    fs::write(huge.join("skin.toml"), vec![b'x'; 65_537]).unwrap();
    let catalog = scan(dir.path());
    assert!(catalog.packages.is_empty());
    assert!(catalog
        .issues
        .iter()
        .any(|issue| issue.folder == "huge" && issue.reason.contains("too large")));
}

#[test]
fn rejects_manifest_id_mismatch_and_missing_base() {
    let dir = tempdir().unwrap();
    for (folder, body) in [
        (
            "mismatch",
            "schema_version = 1\nid = 'other'\nname = 'X'\nversion = '1'\nbase = 'night'",
        ),
        (
            "baseless",
            "schema_version = 1\nid = 'baseless'\nname = 'X'\nversion = '1'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\n",
        ),
    ] {
        let path = dir.path().join(folder);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("skin.toml"), body).unwrap();
    }
    let catalog = scan(dir.path());
    assert!(catalog.packages.is_empty());
    assert_eq!(catalog.issues.len(), 2);
    assert!(catalog
        .issues
        .iter()
        .any(|issue| issue.folder == "baseless" && issue.reason == "base must be a string"));
}

#[test]
fn base_names_system_or_a_builtin_theme() {
    use super::super::theme::GlobalTheme;
    for (base, expected) in [
        ("system", GlobalTheme::System),
        ("shuishan", GlobalTheme::Shuishan),
        ("light", GlobalTheme::Light),
        ("paper", GlobalTheme::Paper),
        ("night", GlobalTheme::Night),
        ("ink", GlobalTheme::Ink),
    ] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.issues.is_empty(), "{base}: {catalog:?}");
        assert_eq!(catalog.packages[0].base, expected, "{base}");
    }
    // The msime-windows packages' `fluent` is the native Fluent look, which is `system` here.
    let catalog = scan_manifest(&manifest("sample").replace("base = 'night'", "base = 'fluent'"));
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages[0].base, GlobalTheme::System);
    assert_eq!(
        serde_json::to_value(&catalog.packages[0]).unwrap()["base"],
        "system"
    );
    // msime-windows 的其他内置外观同样画在 `system` 之上。
    for base in [
        "wechat",
        "graphite",
        "willow_green",
        "autumn_osmanthus",
        "microsoft",
    ] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.issues.is_empty(), "{base}: {catalog:?}");
        assert_eq!(catalog.packages[0].base, GlobalTheme::System, "{base}");
    }
    // `custom` is not a base, and the other retired or misspelt ids are refused rather than read as system.
    for base in ["custom", "Fluent", "Night", "Wechat", "default"] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.packages.is_empty(), "{base}");
        assert_eq!(
            catalog.issues[0].reason, "base must be system or a built-in theme",
            "{base}"
        );
    }
}

#[test]
fn host_catalog_keeps_only_what_candidate_hosts_draw() {
    let body = "schema_version = 1\nid = 'sample'\nname = '樱花'\nversion = '1.0'\nbase = 'paper'\ntoolbar_stylesheet = 'toolbar.css'\n[supports]\nlayouts = ['vertical']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#FFF0F5'\ntext = '#123'\nhover = '#abcdef'\nborder = 'rgba(0,0,0,0.1)'\nshow_selected_bar = true\n[candidate.dark]\nselected = '#ff69b4'\nborder = '#ff000080'\n";
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), body).unwrap();
    fs::write(skin.join("toolbar.css"), ".bar {}").unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    // The title is the manifest name; paths and stylesheets stay out, and every colour `theme::resolve` reads is published in the one form it emits.
    let published = host_candidate_catalog(&catalog, root.path(), "absent");
    assert_eq!(
        published,
        serde_json::json!({"packages": [{
            "id": "sample",
            "title": "樱花",
            "base": "paper",
            "layouts": ["vertical"],
            "candidate": {
                "light": {
                    "surface": "#FFF0F5",
                    "text": "#112233",
                    "hover": "#ABCDEF",
                    "border": "#0000001A",
                    "show_selected_bar": true,
                },
                "dark": {"selected": "#FF69B4", "border": "#FF000080"},
            },
        }]})
    );
}

#[test]
fn a_published_entry_resolves_exactly_as_the_scanned_package() {
    use super::super::theme::{resolve, GlobalTheme, ThemePackage};
    use crate::preferences::CandidateLayout;
    // Every slot, each in a form the Windows presenter accepts but the contract does not emit.
    let palette = "surface = '#fef'\nborder = 'rgba(0, 0, 0, 0.25)'\ntext = 'rgb(17, 34, 51)'\nnumber = '#abc8'\naccent = '#ff69b4'\nselected = 'transparent'\nhover = 'rgba(255,255,255,0.5)'\ntranslation = 'rgba(1, 2, 3, .5)'\nshow_selected_bar = false\n";
    let bases = ["system", "paper", "night"];
    for base in bases {
        let body = format!(
            "schema_version = 1\nid = 'sample'\nname = 'S'\nversion = '1'\nbase = '{base}'\n[supports]\nlayouts = ['vertical']\nthemes = ['light', 'dark']\n[candidate_window]\n[candidate_window.decoration]\n[candidate.light]\n{palette}[candidate.dark]\n{}",
            palette.replace("#fef", "#102030")
        );
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("sample")).unwrap();
        fs::write(root.path().join("sample/skin.toml"), body).unwrap();
        let catalog = scan(root.path());
        assert!(catalog.issues.is_empty(), "{catalog:?}");
        let scanned = ThemePackage::from(&catalog.packages[0]);
        let entry = host_candidate_catalog(&catalog, root.path(), "sample")["packages"][0].clone();
        let published = ThemePackage::from_host_catalog_entry(entry).unwrap();
        let custom = crate::preferences::CustomTheme {
            candidate_skin: Some("sample".into()),
            ..Default::default()
        };
        for dark in [false, true] {
            // The package declares only the vertical layout, so the horizontal one checks that both sources carry the same gate.
            for layout in [CandidateLayout::Vertical, CandidateLayout::Horizontal] {
                let from_root = resolve(GlobalTheme::Custom, &custom, dark, layout, Some(&scanned));
                assert_eq!(
                    from_root.candidate_skin.is_some(),
                    layout == CandidateLayout::Vertical,
                    "{base} {dark} {layout:?}"
                );
                assert_eq!(
                    resolve(GlobalTheme::Custom, &custom, dark, layout, Some(&published)),
                    from_root,
                    "{base} {dark} {layout:?}"
                );
            }
        }
    }
}

#[test]
fn host_catalog_carries_a_decoration_only_for_decorated_packages() {
    let root = tempdir().unwrap();
    let decorated = root.path().join("sakura");
    fs::create_dir_all(decorated.join("images")).unwrap();
    fs::write(decorated.join("images/ears.png"), b"png").unwrap();
    let body = format!("preview = 'images/ears.png'\n{}", manifest("sakura"))
        .replace("top_inset_dip = 0", "top_inset_dip = 24.5")
        .replace("width_dip = 0", "width_dip = 180");
    fs::write(decorated.join("skin.toml"), body).unwrap();
    // A preview without a decoration is only a picture for the settings page.
    let plain = root.path().join("plain");
    fs::create_dir(&plain).unwrap();
    fs::write(plain.join("preview.png"), b"png").unwrap();
    let body = format!("preview = 'preview.png'\n{}", manifest("plain"));
    fs::write(plain.join("skin.toml"), body).unwrap();
    // A decoration whose preview is not an image has nothing a host could draw.
    let styled = root.path().join("styled");
    fs::create_dir(&styled).unwrap();
    fs::write(styled.join("look.css"), ".x {}").unwrap();
    let body = format!("preview = 'look.css'\n{}", manifest("styled"))
        .replace("top_inset_dip = 0", "top_inset_dip = 10")
        .replace("width_dip = 0", "width_dip = 10");
    fs::write(styled.join("skin.toml"), body).unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let published = host_candidate_catalog(&catalog, root.path(), "");
    let package = |id: &str| {
        published["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["id"] == id)
            .unwrap()
            .clone()
    };
    let expected = decorated.join("images/ears.png");
    assert_eq!(
        package("sakura"),
        serde_json::json!({
            "id": "sakura",
            "title": "Sample",
            "base": "night",
            "layouts": ["vertical"],
            "candidate": {"light": {}},
            "decoration_top_dip": 24.5,
            "decoration_width_dip": 180.0,
            "decoration_image": expected.to_str().unwrap(),
        })
    );
    assert_eq!(
        package("plain"),
        serde_json::json!({"id": "plain", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}})
    );
    assert_eq!(
        package("styled"),
        serde_json::json!({"id": "styled", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}})
    );
    // The host opens the path as published, so a relative root publishes none.
    assert!(
        host_candidate_catalog(&catalog, Path::new("skins"), "")["packages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|package| package.get("decoration_image").is_none())
    );
}

#[test]
fn host_catalog_drops_undeclared_themes_and_caps_its_size() {
    // manifest() declares only the light theme, as Windows would not draw this skin in dark.
    let body = format!(
        "{}\n[candidate.light]\nborder = 'transparent'\n[candidate.dark]\nsurface = '#000000'\n",
        manifest("sample")
    );
    let catalog = scan_manifest(&body);
    assert_eq!(
        host_candidate_catalog(&catalog, Path::new("/skins"), "")["packages"][0]["candidate"],
        serde_json::json!({"light": {"border": "#00000000"}})
    );
    // A package without colours for a declared theme still lists, with an empty palette for that theme so the entry still says the package may be drawn in it.
    let bare = host_candidate_catalog(
        &scan_manifest(&manifest("sample")),
        Path::new("/skins"),
        "sample",
    );
    assert_eq!(
        bare,
        serde_json::json!({"packages": [{"id": "sample", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}}]})
    );

    let root = tempdir().unwrap();
    for index in 0..HOST_CATALOG_MAX_PACKAGES + 3 {
        let id = format!("skin{index:02}");
        fs::create_dir(root.path().join(&id)).unwrap();
        fs::write(root.path().join(&id).join("skin.toml"), manifest(&id)).unwrap();
    }
    let catalog = scan(root.path());
    assert_eq!(catalog.packages.len(), HOST_CATALOG_MAX_PACKAGES + 3);
    let ids = |published: serde_json::Value| {
        published["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|package| package["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let listed: Vec<_> = catalog
        .packages
        .iter()
        .map(|package| package.id.clone())
        .collect();
    assert_eq!(
        ids(host_candidate_catalog(&catalog, root.path(), "absent")),
        listed[..HOST_CATALOG_MAX_PACKAGES]
    );
    // A selected skin beyond the cap takes the last place instead of disappearing from the host's menu and colours.
    let beyond = listed[HOST_CATALOG_MAX_PACKAGES + 1].clone();
    let published = ids(host_candidate_catalog(&catalog, root.path(), &beyond));
    assert_eq!(published.len(), HOST_CATALOG_MAX_PACKAGES);
    assert_eq!(
        published[..HOST_CATALOG_MAX_PACKAGES - 1],
        listed[..HOST_CATALOG_MAX_PACKAGES - 1]
    );
    assert_eq!(published.last(), Some(&beyond));
}

#[test]
fn external_ids_are_the_folder_names_the_scan_lists() {
    for id in ["sample", "0day", "a.b_c-d"] {
        assert!(is_external_id(id), "{id}");
    }
    for id in [
        "",
        "Sample",
        ".hidden",
        "-dash",
        "a b",
        "system",
        "night",
        "custom",
        "fluent",
        "wechat",
        "graphite",
        "willow_green",
        "autumn_osmanthus",
        "microsoft",
        "default",
        &"a".repeat(65),
    ] {
        assert!(!is_external_id(id), "{id}");
    }
}

/// msime-windows 的外观作为 `base` 时，包没写的颜色和圆角按该外观补齐，深浅各补各的；包自己写的、以及读得懂的颜色保持原样，读不懂的换成外观的颜色。
#[test]
fn a_windows_look_base_fills_what_the_package_leaves_out() {
    let body = manifest("sample").replace("base = 'night'", "base = 'wechat'")
        + "[candidate.dark]\naccent = '#123456'\ntext = 'not a colour'\n[toolbar.light]\nicon = '#abcdef'\n";
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.base, super::super::theme::GlobalTheme::System);
    let dark = &package.candidate.dark;
    assert_eq!(dark.accent.as_deref(), Some("#123456"));
    assert_eq!(dark.text.as_deref(), Some("#B7B7B7"));
    assert_eq!(dark.surface.as_deref(), Some("#151515"));
    assert_eq!(dark.hover.as_deref(), Some("#07C16052"));
    assert_eq!(dark.show_selected_bar, Some(false));
    // 翻译色跟随序号色，外观不补。
    assert_eq!(dark.translation, None);
    let light = &package.candidate.light;
    assert_eq!(light.surface.as_deref(), Some("#F7F7F7"));
    assert_eq!(light.selected.as_deref(), Some("#07C160"));
    assert_eq!(package.corner_radius_dip, Some(5.0));
    assert_eq!(package.toolbar.corner_radius_dip, Some(8.0));
    assert_eq!(package.toolbar.light.icon.as_deref(), Some("#ABCDEF"));
    assert_eq!(package.toolbar.light.handle.as_deref(), Some("#07C160"));
    assert_eq!(package.toolbar.dark.background.as_deref(), Some("#151515"));

    // 包自己写的圆角优先；`fluent` 就是原生配色，什么也不补。
    let own_radius = scan_manifest(
        &manifest("sample")
            .replace("base = 'night'", "base = 'graphite'")
            .replace(
                "min_width_dip = 10\n",
                "min_width_dip = 10\ncorner_radius_dip = 12\n",
            ),
    );
    assert_eq!(own_radius.packages[0].corner_radius_dip, Some(12.0));
    let fluent = scan_manifest(&manifest("sample").replace("base = 'night'", "base = 'fluent'"));
    assert_eq!(fluent.packages[0].candidate, CandidateColors::default());
    assert_eq!(fluent.packages[0].corner_radius_dip, None);
    assert_eq!(fluent.packages[0].toolbar, SkinToolbar::default());
}

/// The full-TOML manifest the native hosts must accept exactly as the settings page does (Windows parses skin.toml with toml++): literal strings, a multi-line array, an inline table, a digit separator, a unicode escape and a `#` inside a literal string.
const FULL_TOML_MANIFEST: &str = r#"schema_version = 1
id = 'full-toml'
name = "\u6768\u67f3 Full"
version = '1.0'
base = 'ink'
description = 'hash # inside a literal'
toolbar_stylesheet = 'toolbar.css'

[supports]
layouts = [
  'horizontal', # trailing comment
  'vertical',
]
themes = ['dark', 'light']

[candidate_window]
min_width_dip = 1_0
decoration = { top_inset_dip = 0, width_dip = 0 }

[candidate.light]
accent = '#c45c7a'
show_selected_bar = false
"#;

#[test]
fn load_package_accepts_full_toml_manifests() {
    let root = tempdir().unwrap();
    let skin = root.path().join("full-toml");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("skin.toml"), FULL_TOML_MANIFEST).unwrap();
    fs::write(skin.join("toolbar.css"), ".toolbar {}").unwrap();
    let package = load_package(root.path(), "full-toml").unwrap();
    assert_eq!(package.name, "杨柳 Full");
    assert_eq!(package.base, super::super::theme::GlobalTheme::Ink);
    assert_eq!(
        package.description.as_deref(),
        Some("hash # inside a literal")
    );
    assert_eq!(package.layouts, ["horizontal", "vertical"]);
    assert_eq!(package.min_width_dip, 10.0);
    assert_eq!(
        (package.decoration_top_dip, package.decoration_width_dip),
        (0.0, 0.0)
    );
    assert_eq!(package.toolbar_stylesheet.as_deref(), Some("toolbar.css"));
    assert_eq!(package.candidate.light.accent.as_deref(), Some("#c45c7a"));
    assert_eq!(package.candidate.light.show_selected_bar, Some(false));
    // One package resolves to what the full scan reports for it.
    assert_eq!(scan(root.path()).packages, vec![package]);
}

#[test]
fn load_package_rejects_theme_mismatched_missing_and_aliased_packages() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("ink")).unwrap();
    fs::write(root.path().join("ink/skin.toml"), manifest("ink")).unwrap();
    assert_eq!(
        load_package(root.path(), "ink"),
        Err("invalid skin id".into())
    );
    fs::create_dir_all(root.path().join("renamed")).unwrap();
    fs::write(root.path().join("renamed/skin.toml"), manifest("original")).unwrap();
    assert_eq!(
        load_package(root.path(), "renamed"),
        Err("manifest id does not match folder".into())
    );
    assert!(load_package(root.path(), "absent").is_err());
    assert!(load_package(root.path(), "../escape").is_err());
    resource_package(root.path());
    #[cfg(unix)]
    {
        // scan() skips a symlinked package directory, so resolving one by id must not draw it either.
        std::os::unix::fs::symlink(root.path().join("sample"), root.path().join("alias")).unwrap();
        assert!(load_package(root.path(), "alias").is_err());
    }
    assert!(load_package(root.path(), "sample").is_ok());
}

#[test]
#[cfg(unix)]
fn a_fifo_manifest_is_rejected_without_blocking() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("pipe")).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(root.path().join("pipe/skin.toml"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        load_package(root.path(), "pipe"),
        Err("skin.toml is not a regular file".into())
    );
    assert_eq!(scan(root.path()).issues[0].folder, "pipe");
}

#[test]
#[cfg(unix)]
fn a_symlinked_manifest_is_rejected_even_when_it_stays_inside_the_package() {
    let root = tempdir().unwrap();
    let skin = root.path().join("linked");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("real.toml"), manifest("linked")).unwrap();
    std::os::unix::fs::symlink(skin.join("real.toml"), skin.join("skin.toml")).unwrap();
    assert_eq!(
        load_package(root.path(), "linked"),
        Err("skin.toml is not a regular file".into())
    );
    assert!(scan(root.path()).packages.is_empty());
}

#[test]
#[cfg(unix)]
fn a_symlinked_catalog_root_is_not_scanned() {
    use std::os::unix::fs::symlink;

    let state = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let package = resource_package(outside.path());
    fs::write(package.join("images/sample.png"), b"synthetic").unwrap();
    let root = state.path().join("skins");
    symlink(outside.path(), &root).unwrap();

    let catalog = scan(&root);
    assert!(catalog.packages.is_empty());
    assert!(catalog.issues.is_empty());
    assert!(load_package(&root, "sample").is_err());
    assert!(read_resource(&root, "sample", "images/sample.png").is_err());
}

/// The layout msime-skins (github.com/metasequoiaime/msime-skins) writes: a decoration with its own image and alignment, a background image, a corner radius, a toolbar palette per mode, a translation colour and licence metadata.
fn styled_package(root: &Path) -> std::path::PathBuf {
    let skin = root.join("bigfish");
    fs::create_dir_all(skin.join("assets")).unwrap();
    fs::write(skin.join("assets/character.png"), b"png").unwrap();
    fs::write(skin.join("assets/background.png"), b"png").unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 1\nid = 'bigfish'\nname = '蓝色大肥鱼'\nversion = '1.0.0'\nbase = 'system'\n\
         [supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['dark', 'light']\n\
         [candidate_window]\nmin_width_dip = 176\ncorner_radius_dip = 12\n\
         [candidate_window.decoration]\nimage = 'assets/character.png'\ntop_inset_dip = 104\nwidth_dip = 96\nalign = 'left'\n\
         [candidate_window.background]\nimage = 'assets/background.png'\nfit = 'contain'\nopacity = 0.35\n\
         [candidate.dark]\nnumber = '#E0C07A'\ntranslation = '#9FB4E0'\n\
         [candidate.light]\ntranslation = 'rgba(90, 106, 150, .5)'\n\
         [toolbar]\ncorner_radius_dip = 8\n\
         [toolbar.dark]\nbackground = '#141B33'\nborder = 'rgba(91, 155, 255, 0.38)'\nhandle = '#5B9BFF'\ndivider = 'rgba(91, 155, 255, 0.28)'\nicon = '#E3EAFF'\nhover = 'not a colour'\n\
         [toolbar.light]\nbackground = '#f4f8ff'\n\
         [license]\ncode = 'MIT'\nassets = 'CC-BY-4.0'\nsource = 'synthetic'\n",
    )
    .unwrap();
    skin
}

#[test]
fn styled_manifests_load_every_drawn_key() {
    let root = tempdir().unwrap();
    styled_package(root.path());
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.corner_radius_dip, Some(12.0));
    assert_eq!(
        package.decoration_image.as_deref(),
        Some("assets/character.png")
    );
    assert_eq!(package.decoration_align, DecorationAlign::Left);
    assert_eq!(
        package.background,
        Some(SkinBackground {
            image: "assets/background.png".into(),
            fit: BackgroundFit::Contain,
            opacity: 0.35,
        })
    );
    // Toolbar colours arrive normalized; one a host could not read is left out.
    assert_eq!(
        package.toolbar,
        SkinToolbar {
            corner_radius_dip: Some(8.0),
            dark: ToolbarPalette {
                background: Some("#141B33".into()),
                border: Some("#5B9BFF61".into()),
                handle: Some("#5B9BFF".into()),
                divider: Some("#5B9BFF47".into()),
                icon: Some("#E3EAFF".into()),
                hover: None,
            },
            light: ToolbarPalette {
                background: Some("#F4F8FF".into()),
                ..Default::default()
            },
        }
    );
    assert_eq!(
        package.license,
        Some(SkinLicense {
            code: Some("MIT".into()),
            assets: Some("CC-BY-4.0".into()),
            source: Some("synthetic".into()),
        })
    );
    let json = serde_json::to_value(package).unwrap();
    assert_eq!(json["cornerRadiusDip"], 12.0);
    assert_eq!(json["decorationImage"], "assets/character.png");
    assert_eq!(json["decorationAlign"], "left");
    assert_eq!(
        json["background"],
        serde_json::json!({"image": "assets/background.png", "fit": "contain", "opacity": 0.35})
    );
    assert_eq!(json["toolbar"]["cornerRadiusDip"], 8.0);
    assert_eq!(json["toolbar"]["dark"]["handle"], "#5B9BFF");
    assert_eq!(json["candidate"]["dark"]["translation"], "#9FB4E0");
}

#[test]
fn defaults_leave_new_keys_to_the_host() {
    // No decoration table at all, as msime-skins writes a package without one.
    let body = manifest("sample").replace(
        "[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n",
        "",
    );
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.decoration_top_dip, 0.0);
    assert_eq!(package.corner_radius_dip, None);
    assert_eq!(package.decoration_image, None);
    assert_eq!(package.decoration_align, DecorationAlign::Right);
    assert_eq!(package.background, None);
    assert_eq!(package.toolbar, SkinToolbar::default());
    assert_eq!(package.license, None);
    let json = serde_json::to_value(package).unwrap();
    assert!(json["cornerRadiusDip"].is_null());
    assert_eq!(json["decorationAlign"], "right");
    // A background without fit or opacity covers the card at full strength.
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("bg.webp"), b"webp").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!(
            "{}[candidate_window.background]\nimage = 'bg.webp'\n",
            manifest("sample")
        ),
    )
    .unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(
        catalog.packages[0].background,
        Some(SkinBackground {
            image: "bg.webp".into(),
            fit: BackgroundFit::Cover,
            opacity: 1.0,
        })
    );
}

#[test]
fn decoration_image_prefers_its_own_key_over_the_preview() {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("preview.png"), b"png").unwrap();
    fs::write(skin.join("ears.png"), b"png").unwrap();
    let decorated = |extra: &str| {
        format!("preview = 'preview.png'\n{}", manifest("sample"))
            .replace("top_inset_dip = 0", &format!("{extra}top_inset_dip = 20"))
            .replace("width_dip = 0", "width_dip = 40")
    };
    fs::write(skin.join("skin.toml"), decorated("image = 'ears.png'\n")).unwrap();
    assert_eq!(
        scan(root.path()).packages[0].decoration_image.as_deref(),
        Some("ears.png")
    );
    fs::write(skin.join("skin.toml"), decorated("")).unwrap();
    assert_eq!(
        scan(root.path()).packages[0].decoration_image.as_deref(),
        Some("preview.png")
    );
    // An undecorated package draws no decoration, whatever its preview.
    fs::write(
        skin.join("skin.toml"),
        format!("preview = 'preview.png'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(scan(root.path()).packages[0].decoration_image, None);
}

#[test]
fn styled_keys_out_of_bounds_reject_the_package() {
    let cases = [
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = 33",
            "invalid corner_radius_dip",
        ),
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = -1",
            "invalid corner_radius_dip",
        ),
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = '12'",
            "invalid corner_radius_dip",
        ),
        ("align = 'left'", "align = 'top'", "invalid decoration"),
        ("align = 'left'", "align = 3", "align must be a string"),
        (
            "image = 'assets/character.png'",
            "image = 'assets/missing.png'",
            "invalid decoration",
        ),
        (
            "image = 'assets/character.png'",
            "image = '../outside.png'",
            "invalid decoration",
        ),
        (
            "image = 'assets/character.png'",
            "image = 'skin.toml'",
            "invalid decoration",
        ),
        ("fit = 'contain'", "fit = 'tile'", "invalid background"),
        ("opacity = 0.35", "opacity = 1.5", "invalid background"),
        ("opacity = 0.35", "opacity = -0.1", "invalid background"),
        (
            "image = 'assets/background.png'",
            "image = 'assets/background.txt'",
            "invalid background",
        ),
        (
            "image = 'assets/background.png'",
            "image = '/etc/passwd.png'",
            "invalid background",
        ),
        (
            "[toolbar]\ncorner_radius_dip = 8",
            "[toolbar]\ncorner_radius_dip = 40",
            "invalid toolbar",
        ),
        (
            "background = '#141B33'",
            "background = 5",
            "invalid toolbar",
        ),
        (
            "background = '#141B33'",
            &format!("background = '{}'", "a".repeat(81)),
            "toolbar color exceeds 80 bytes",
        ),
        (
            "translation = '#9FB4E0'",
            "translation = 7",
            "invalid candidate colors",
        ),
        ("code = 'MIT'", "code = ''", "code has invalid length"),
    ];
    for (from, to, reason) in cases {
        let root = tempdir().unwrap();
        let skin = styled_package(root.path());
        let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
        assert_eq!(body.matches(from).count(), 1, "{from}");
        fs::write(skin.join("skin.toml"), body.replacen(from, to, 1)).unwrap();
        let catalog = scan(root.path());
        assert!(catalog.packages.is_empty(), "accepted {to}");
        assert_eq!(catalog.issues[0].reason, reason, "{to}");
    }
    // A decoration image needs a band to be drawn in.
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        body.replace("top_inset_dip = 104\nwidth_dip = 96\n", ""),
    )
    .unwrap();
    assert_eq!(scan(root.path()).issues[0].reason, "invalid decoration");
    // Tables where a table belongs.
    for (from, to) in [
        (
            "[candidate_window.background]\n",
            "[candidate_window]\nbackground = 1\n[x]\n",
        ),
        ("[toolbar.light]\n", "[toolbar]\nlight = 1\n[y]\n"),
    ] {
        let root = tempdir().unwrap();
        let skin = styled_package(root.path());
        let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
        fs::write(skin.join("skin.toml"), body.replacen(from, to, 1)).unwrap();
        assert!(scan(root.path()).packages.is_empty(), "accepted {to}");
    }
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("license = 1\n{}", body.replace("[license]\n", "[z]\n")),
    )
    .unwrap();
    assert_eq!(scan(root.path()).issues[0].reason, "invalid license");
}

#[test]
fn host_catalog_publishes_what_linux_draws_of_a_styled_package() {
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let catalog = scan(root.path());
    let published = host_candidate_catalog(&catalog, root.path(), "bigfish");
    // The background and toolbar stay out: neither Linux host can draw them.
    assert_eq!(
        published,
        serde_json::json!({"packages": [{
            "id": "bigfish",
            "title": "蓝色大肥鱼",
            "base": "system",
            "layouts": ["horizontal", "vertical"],
            "candidate": {
                "dark": {"number": "#E0C07A", "translation": "#9FB4E0"},
                "light": {"translation": "#5A6A9680"},
            },
            "decoration_top_dip": 104.0,
            "decoration_width_dip": 96.0,
            "decoration_image": skin.join("assets/character.png").to_str().unwrap(),
            "decoration_align": "left",
            "corner_radius_dip": 12.0,
        }]})
    );
}

/// `client_dialect.json` 是皮肤清单规则的共享用例表，各个实现都按它校验：这里的 `load`、msime-cloud 的 Go 移植（`internal/skins/client.go`）与种子脚本，以及 msime-windows 的 `CandidateSkinCatalog::Load`。它们各自保存一份副本，由各自仓库的同步脚本按本文件刷新，所以改规则先改这里。
///
/// 每个用例按 `_comment` 说明的顺序生成清单与旁边的文件（每个文件一个字节），接受的用例核对解析出的 `base`，拒绝的用例核对拒绝原因。
#[test]
fn shared_dialect_cases_match_the_loader() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("client_dialect.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 150, "用例表丢了用例：{}", cases.len());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let template = case["template"].as_str().unwrap();
        let (mut id, mut body, mut files) = if template == "raw" {
            let body = case["manifest"].as_str().unwrap().to_owned();
            ("sample".to_owned(), body, Vec::new())
        } else {
            let source = &fixture["templates"][template];
            (
                source["id"].as_str().unwrap().to_owned(),
                source["manifest"].as_str().unwrap().to_owned(),
                source["files"].as_array().unwrap().clone(),
            )
        };
        if let Some(own) = case["id"].as_str() {
            id = own.to_owned();
        }
        body = body.replace("{id}", &id);
        for edit in case["replace"].as_array().into_iter().flatten() {
            let from = edit[0].as_str().unwrap();
            assert!(body.contains(from), "{name}：清单里没有 {from:?}");
            body = body.replacen(from, edit[1].as_str().unwrap(), 1);
        }
        body = format!(
            "{}{body}{}",
            case["prepend"].as_str().unwrap_or(""),
            case["append"].as_str().unwrap_or("")
        );
        if let Some(pad_to) = case["pad_to"].as_u64() {
            body.push('#');
            body.push_str(&"x".repeat(pad_to as usize - body.len()));
        }
        if let Some(own) = case["files"].as_array() {
            files = own.clone();
        }

        let root = tempdir().unwrap();
        let skin = root.path().join(&id);
        fs::create_dir_all(&skin).unwrap();
        for file in &files {
            let path = skin.join(file.as_str().unwrap());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        fs::write(skin.join("skin.toml"), &body).unwrap();
        match (load(root.path(), &id), case["reason"].as_str()) {
            (Ok(package), None) => {
                if let Some(base) = case["base"].as_str() {
                    assert_eq!(package.base.id(), base, "{name}");
                }
            }
            (Err(reason), Some(expected)) => assert_eq!(reason, expected, "{name}"),
            (Ok(_), Some(expected)) => panic!("{name}：接受了，应以 {expected:?} 拒绝"),
            (Err(reason), None) => panic!("{name}：以 {reason:?} 拒绝，应接受"),
        }
    }
}
