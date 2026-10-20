//! Linux 的 IBus 和 Fcitx5 宿主从运行时选项文档读偏好和已安装皮肤的目录（`candidate_skin_catalog`），两样都不去别处读。MCP 改了偏好、写了或删了皮肤之后经这里把新结果发布进去，与设置应用的 `sync_runtime_options`、`publish_candidate_skin_catalog` 同一套规则：上限、皮肤目录的裁剪和「目录没变就不写」都由共享的 `skin::linux_runtime_options` 决定，这里只做文件读写。
//!
//! macOS 和 Windows 的宿主直接读偏好存储和皮肤目录，这份文档里没有它们要的东西，所以调用方只在 Linux 上调用这里。

use msime_client_core::file_lock::{self, PrivateDirectory};
use msime_client_core::preferences::Preferences;
use msime_client_services::skin::catalog;
use msime_client_services::skin::linux_runtime_options::{self, LINUX_RUNTIME_OPTIONS_LIMIT};
use serde_json::Value;
use std::ffi::OsStr;
use std::path::Path;

pub const TOO_LARGE: &str = "the runtime options would be too large for the input method to read";

/// 换掉文档里的偏好，并按新偏好重新发布 `skins` 下的皮肤目录，其余键照文件里现有的保留。
pub fn publish_preferences(
    path: &Path,
    skins: &Path,
    preferences: &Preferences,
) -> Result<(), String> {
    let (directory, name) = open(path)?;
    publish_preferences_at(&directory, name, skins, preferences)
}

fn publish_preferences_at(
    directory: &PrivateDirectory,
    name: &OsStr,
    skins: &Path,
    preferences: &Preferences,
) -> Result<(), String> {
    let mut document = read_at(directory, name)?;
    let mut host = serde_json::to_value(preferences).map_err(|error| error.to_string())?;
    // IBus 和 Fcitx5 宿主从不画屏幕键盘，它的照片不进它们的副本。
    if let Some(design) = host
        .pointer_mut("/custom_theme/keyboard")
        .and_then(Value::as_object_mut)
    {
        design.remove("photo");
    }
    document["preferences"] = host;
    let bytes = linux_runtime_options::runtime_options_with_skin_catalog(
        &mut document,
        skins,
        &catalog::scan(skins),
    )
    .map_err(|_| TOO_LARGE)?;
    write_at(directory, name, &bytes)
}

/// 重新扫描 `skins` 并把皮肤目录发布进文档，其余键照文件里现有的保留。目录与已发布的一样时不写文件，返回 `false`。
pub fn republish_skin_catalog(path: &Path, skins: &Path) -> Result<bool, String> {
    let (directory, name) = open(path)?;
    let mut document = read_at(&directory, name)?;
    let Some(bytes) =
        linux_runtime_options::republish_skin_catalog(&mut document, skins, &catalog::scan(skins))
            .map_err(|_| TOO_LARGE)?
    else {
        return Ok(false);
    };
    write_at(&directory, name, &bytes)?;
    Ok(true)
}

/// 打开文档所在的目录：之后的读和写都相对这个已打开的目录进行，路径上的目录被换成符号链接也写不到别处。
fn open(path: &Path) -> Result<(PrivateDirectory, &OsStr), String> {
    let parent = path.parent().ok_or("cannot read the runtime options")?;
    let name = path.file_name().ok_or("cannot read the runtime options")?;
    let directory =
        file_lock::open_private_directory(parent).map_err(|_| "cannot read the runtime options")?;
    Ok((directory, name))
}

fn read_at(directory: &PrivateDirectory, name: &OsStr) -> Result<Value, String> {
    let file = file_lock::open_private_file_at(directory, name)
        .map_err(|_| "cannot read the runtime options")?;
    let bytes = crate::bounded::read(file, LINUX_RUNTIME_OPTIONS_LIMIT as u64).map_err(
        |error| match error {
            crate::bounded::ReadError::TooLarge => TOO_LARGE,
            crate::bounded::ReadError::Io => "cannot read the runtime options",
        },
    )?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|_| "cannot parse the runtime options")?;
    if !document.is_object() {
        return Err("cannot parse the runtime options".into());
    }
    Ok(document)
}

fn write_at(directory: &PrivateDirectory, name: &OsStr, bytes: &[u8]) -> Result<(), String> {
    file_lock::write_private_file_at(directory, name, bytes)
        .map_err(|_| "cannot write the runtime options".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_skin(skins: &Path, id: &str, surface: &str) {
        let package = skins.join(id);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("skin.toml"),
            format!("schema_version = 1\nid = '{id}'\nname = 'Sample'\nversion = '1.0'\nbase = 'paper'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\nmin_width_dip = 10\n[candidate.light]\nsurface = '{surface}'\n"),
        )
        .unwrap();
    }

    #[test]
    fn the_skin_catalog_is_republished_only_when_it_changed() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        let skins = directory.path().join("skins");
        std::fs::write(
            &options,
            br#"{"api_version":1,"resources":"/resources","preferences":{"candidate_page_size":7}}"#,
        )
        .unwrap();
        write_skin(&skins, "sakura", "#fff0f5");

        assert!(republish_skin_catalog(&options, &skins).unwrap());
        let written = std::fs::read(&options).unwrap();
        let document: Value = serde_json::from_slice(&written).unwrap();
        assert_eq!(document["resources"], "/resources");
        assert_eq!(document["preferences"]["candidate_page_size"], 7);
        assert_eq!(
            document["candidate_skin_catalog"]["packages"][0]["candidate"]["light"]["surface"],
            "#FFF0F5"
        );

        // 没变时不碰文件：宿主按文件变化重载，白写一次就白重载一次。
        std::fs::write(&options, &written).unwrap();
        let before = std::fs::metadata(&options).unwrap().modified().unwrap();
        assert!(!republish_skin_catalog(&options, &skins).unwrap());
        assert_eq!(std::fs::read(&options).unwrap(), written);
        assert_eq!(
            std::fs::metadata(&options).unwrap().modified().unwrap(),
            before
        );
    }

    #[test]
    fn preferences_are_published_with_the_skin_catalog_they_select() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        let skins = directory.path().join("skins");
        std::fs::write(
            &options,
            br#"{"api_version":1,"candidate_skin_catalog":{"packages":[]}}"#,
        )
        .unwrap();
        write_skin(&skins, "sakura", "#fff0f5");
        let mut preferences = Preferences::default();
        preferences.custom_theme.candidate_skin = Some("sakura".into());

        publish_preferences(&options, &skins, &preferences).unwrap();
        let document: Value = serde_json::from_slice(&std::fs::read(&options).unwrap()).unwrap();
        assert_eq!(document["api_version"], 1);
        assert_eq!(
            document["preferences"]["custom_theme"]["candidate_skin"],
            "sakura"
        );
        assert_eq!(
            document["candidate_skin_catalog"]["packages"][0]["id"],
            json!("sakura")
        );
    }

    #[cfg(unix)]
    #[test]
    fn publishing_runtime_options_stays_in_an_open_parent_after_replacement() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("state");
        let outside = root.path().join("outside");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let options = original.join("runtime-options.json");
        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        let directory = msime_client_core::file_lock::open_private_directory(&original).unwrap();

        let moved = root.path().join("moved");
        std::fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();

        publish_preferences_at(
            &directory,
            OsStr::new("runtime-options.json"),
            &moved.join("skins"),
            &Preferences::default(),
        )
        .unwrap();

        let written: Value =
            serde_json::from_slice(&std::fs::read(moved.join("runtime-options.json")).unwrap())
                .unwrap();
        assert!(written.get("preferences").is_some());
        assert!(!outside.join("runtime-options.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn publishing_runtime_options_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let outside_options = outside.path().join("runtime-options.json");
        std::fs::write(&outside_options, br#"{"api_version":1}"#).unwrap();

        let result = publish_preferences(
            &linked.join("runtime-options.json"),
            &linked.join("skins"),
            &Preferences::default(),
        );

        assert!(result.is_err());
        assert_eq!(
            std::fs::read(&outside_options).unwrap(),
            br#"{"api_version":1}"#
        );
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
        publish_preferences(&options, &directory.path().join("skins"), &preferences).unwrap();
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
            publish_preferences(&options, &directory.path().join("skins"), &preferences)
                .unwrap_err(),
            TOO_LARGE
        );
    }

    #[test]
    fn an_oversized_source_is_rejected_before_json_decoding() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, vec![b'x'; LINUX_RUNTIME_OPTIONS_LIMIT + 1]).unwrap();
        assert_eq!(
            publish_preferences(
                &options,
                &directory.path().join("skins"),
                &Preferences::default()
            )
            .unwrap_err(),
            TOO_LARGE
        );
    }

    #[cfg(unix)]
    #[test]
    fn publishing_rejects_a_symlinked_runtime_document() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("runtime-options.json");
        std::fs::write(&target, br#"{"api_version":1}"#).unwrap();
        let linked = directory.path().join("runtime-options.json");
        symlink(&target, &linked).unwrap();

        assert!(publish_preferences(
            &linked,
            &directory.path().join("skins"),
            &Preferences::default()
        )
        .is_err());
        assert_eq!(std::fs::read(&target).unwrap(), br#"{"api_version":1}"#);
    }
}
