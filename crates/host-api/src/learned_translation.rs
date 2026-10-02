//! Bounded worker-thread operations on the private learned-gloss store.
use msime_client_core::is_bounded_text;
use msime_client_core::translation::store::{
    GlossDirection, GlossStoreError, TranslationGlossStore,
};
use msime_client_core::translation::{
    format_translation_gloss, is_cloud_translatable_chinese, is_cloud_translatable_english,
    is_supported_translation_language, is_valid_source_text, should_persist_translation,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

fn reject_symlinked_path(path: &Path) -> Result<(), &'static str> {
    let mut current = PathBuf::new();
    let mut saw_prefix_alias = false;
    let mut saw_real_component = false;
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::Prefix(_) | Component::RootDir => current.push(component),
            Component::CurDir => {}
            Component::ParentDir => current.push(component),
            Component::Normal(_) => {
                current.push(component);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let system_alias = path.is_absolute()
                            && !saw_real_component
                            && !saw_prefix_alias
                            && matches!(component, Component::Normal(name) if *name == std::ffi::OsStr::new("tmp") || *name == std::ffi::OsStr::new("var"));
                        if index + 1 == components.len()
                            || saw_real_component
                            || saw_prefix_alias
                            || !system_alias
                        {
                            return Err("learned translation storage unavailable");
                        }
                        saw_prefix_alias = true;
                    }
                    Ok(_) => saw_real_component = true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err("learned translation storage unavailable"),
                }
            }
        }
    }
    Ok(())
}

fn prepare_storage_directory(directory: &Path) -> Result<(), &'static str> {
    reject_symlinked_path(directory)?;
    fs::create_dir_all(directory).map_err(|_| "learned translation storage unavailable")?;
    let metadata =
        fs::symlink_metadata(directory).map_err(|_| "learned translation storage unavailable")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("learned translation storage unavailable");
    }
    Ok(())
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Action {
    Lookup,
    Remember,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    text: String,
    direction: GlossDirection,
    translation: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    directory: String,
    target_language: String,
    generation: u64,
    action: Action,
    items: Vec<Item>,
}

pub fn execute(bytes: &[u8]) -> Result<Value, &'static str> {
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| "invalid learned translation request")?;
    let directory = Path::new(&request.directory);
    if !is_bounded_text(&request.directory, 4096)
        || !directory.is_absolute()
        || directory.parent().is_none()
        || request.target_language == "zh"
        || !is_supported_translation_language(&request.target_language)
        || request.items.len() > 9
        || request.items.iter().any(|item| {
            !is_valid_source_text(&item.text)
                || !is_bounded_text(&item.text, 160)
                || match request.action {
                    Action::Lookup => item.translation.is_some(),
                    Action::Remember => item.translation.as_ref().is_none_or(|s| s.len() > 4096),
                }
        })
    {
        return Err("invalid learned translation parameters");
    }
    reject_symlinked_path(directory)?;
    let database = directory.join("translation-glosses.db");
    reject_symlinked_path(&database)?;
    // Preserve previous JSON records as read-only fallback. New writes use the
    // same Engine-owned user database as other native hosts, never resources.
    let legacy = TranslationGlossStore::new(directory);
    let mut translations = Vec::with_capacity(request.items.len());
    let mut saved = 0;
    if request.target_language != "en" {
        return Ok(json!({"generation":request.generation,"translations":[],"saved":0}));
    }
    for item in request.items {
        let chinese = item.direction == GlossDirection::ChineseToEnglish;
        let eligible = if chinese {
            is_cloud_translatable_chinese(&item.text)
        } else {
            is_cloud_translatable_english(&item.text)
        };
        if !eligible {
            continue;
        }
        let key = if chinese {
            item.text.clone()
        } else {
            item.text.to_ascii_lowercase()
        };
        match request.action {
            Action::Lookup => {
                // The glossary is keyed by the text a gloss was learned for, which is almost always Simplified, so a Traditional candidate - a Korean Hanja such as 韓, or any candidate under Traditional output - is looked up again under its Simplified characters when its own spelling finds nothing. The reply still names the candidate as shown.
                let mut keys = vec![key];
                if chinese {
                    let simplified =
                        msime_client_core::chinese_conversion::traditional_to_simplified_characters(
                            &item.text,
                        );
                    if simplified != item.text {
                        keys.push(simplified);
                    }
                }
                let mut found = None;
                for key in keys {
                    let learned = msime_engine::host::candidate_glosses_with_user(
                        "",
                        &request.directory,
                        &[(key.clone(), if chinese { 0 } else { 4 })],
                    )
                    .ok()
                    .and_then(|values| values.into_iter().next())
                    .filter(|text| !text.is_empty());
                    if learned.is_some() {
                        found = learned;
                        break;
                    }
                    let legacy_key = if chinese {
                        key.as_str()
                    } else {
                        item.text.as_str()
                    };
                    match legacy.lookup(&request.target_language, item.direction, legacy_key) {
                        Ok(Some(translation)) => {
                            found = Some(translation);
                            break;
                        }
                        Ok(None) | Err(GlossStoreError::InvalidRecord) => {}
                        Err(_) => return Err("learned translation storage unavailable"),
                    }
                }
                if let Some(translation) = found {
                    translations.push(json!({"text":item.text,"translation":translation}));
                }
            }
            Action::Remember => {
                // Entire batch shape was validated before any disk changes.
                let Some(gloss) =
                    format_translation_gloss(item.translation.as_deref().unwrap_or_default())
                else {
                    continue;
                };
                if !should_persist_translation(&key, &gloss) {
                    continue;
                }
                prepare_storage_directory(directory)?;
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options.open(&database) {
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err("learned translation storage unavailable"),
                }
                if !msime_engine::host::save_candidate_gloss(
                    &request.directory,
                    chinese,
                    &key,
                    &gloss,
                ) {
                    return Err("learned translation storage unavailable");
                }
                saved += 1;
            }
        }
    }
    Ok(json!({"generation":request.generation,"translations":translations,"saved":saved}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(root: &Path, action: &str, items: Value) -> Value {
        json!({"directory":root,"action":action,"generation":17,"target_language":"en","items":items})
    }
    fn run(value: &Value) -> Result<Value, &'static str> {
        execute(&serde_json::to_vec(value).unwrap())
    }
    #[test]
    fn roundtrip_both_directions_and_target_gate() {
        let root = tempfile::tempdir().unwrap();
        let write = request(
            root.path(),
            "remember",
            json!([
            {"text":"Hello","direction":"english_to_chinese","translation":"你好"},
            {"text":"测试","direction":"chinese_to_english","translation":"test"}]),
        );
        assert_eq!(run(&write).unwrap()["saved"], 2);
        let mut read = request(
            root.path(),
            "lookup",
            json!([
            {"text":"HELLO","direction":"english_to_chinese"},
            {"text":"测试","direction":"chinese_to_english"}]),
        );
        assert_eq!(
            run(&read).unwrap(),
            json!({"generation":17,"saved":0,"translations":[
            {"text":"HELLO","translation":"你好"},{"text":"测试","translation":"test"}]})
        );
        read["target_language"] = json!("fr");
        assert_eq!(run(&read).unwrap()["translations"], json!([]));
    }
    #[test]
    fn a_traditional_candidate_finds_the_gloss_learned_for_its_simplified_form() {
        let root = tempfile::tempdir().unwrap();
        let write = request(
            root.path(),
            "remember",
            json!([
            {"text":"韩","direction":"chinese_to_english","translation":"Han"},
            {"text":"寒","direction":"chinese_to_english","translation":"Cold"}]),
        );
        assert_eq!(run(&write).unwrap()["saved"], 2);
        let read = request(
            root.path(),
            "lookup",
            json!([
            {"text":"韓","direction":"chinese_to_english"},
            {"text":"寒","direction":"chinese_to_english"},
            {"text":"閑","direction":"chinese_to_english"}]),
        );
        // 韓 is answered under 韩 and named as shown; 寒 is the same in both scripts; 閑 has nothing learned under either spelling.
        assert_eq!(
            run(&read).unwrap()["translations"],
            json!([{"text":"韓","translation":"Han"},{"text":"寒","translation":"Cold"}])
        );
    }
    #[test]
    fn malformed_batch_never_partially_writes() {
        let root = tempfile::tempdir().unwrap();
        let valid = request(
            root.path(),
            "remember",
            json!([
            {"text":"Hello","direction":"english_to_chinese","translation":"你好"}]),
        );
        for (field, value) in [
            ("directory", json!("relative")),
            ("target_language", json!("invalid")),
            ("action", json!("invalid")),
            ("generation", json!(-1)),
        ] {
            let mut bad = valid.clone();
            bad[field] = value;
            assert!(run(&bad).is_err());
        }
        let mut bad = valid.clone();
        bad["items"]
            .as_array_mut()
            .unwrap()
            .push(json!({"text":"world","direction":"english_to_chinese"}));
        assert!(run(&bad).is_err());
        bad["items"] = json!(vec![valid["items"][0].clone(); 10]);
        assert!(run(&bad).is_err());
        assert!(!root.path().join("learned-translations-v1").exists());
        assert!(!root.path().join("translation-glosses.db").exists());
    }

    #[test]
    fn canonical_engine_store_and_legacy_fallback_interoperate() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().to_str().unwrap();
        let legacy = TranslationGlossStore::new(root.path());
        legacy
            .remember("en", GlossDirection::EnglishToChinese, "hello", "旧释义")
            .unwrap();
        let read = request(
            root.path(),
            "lookup",
            json!([
            {"text":"HELLO","direction":"english_to_chinese"},
            {"text":"测试","direction":"chinese_to_english"}]),
        );
        assert_eq!(
            run(&read).unwrap()["translations"][0]["translation"],
            "旧释义"
        );
        assert!(!root.path().join("translation-glosses.db").exists()); // Reads do not migrate or write.
        let write = request(
            root.path(),
            "remember",
            json!([
            {"text":"Hello","direction":"english_to_chinese","translation":"新释义"}]),
        );
        assert_eq!(run(&write).unwrap()["saved"], 1);
        assert_eq!(
            msime_engine::host::candidate_glosses_with_user("", directory, &[("hello".into(), 4)])
                .unwrap(),
            vec!["新释义"]
        );
        assert!(msime_engine::host::save_candidate_gloss(
            directory, true, "测试", "test"
        ));
        assert_eq!(
            run(&read).unwrap()["translations"],
            json!([
            {"text":"HELLO","translation":"新释义"},{"text":"测试","translation":"test"}])
        );
        assert_eq!(
            legacy
                .lookup("en", GlossDirection::EnglishToChinese, "hello")
                .unwrap()
                .as_deref(),
            Some("旧释义")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(root.path().join("translation-glosses.db"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
    }

    #[test]
    fn unsupported_results_do_not_create_database() {
        let root = tempfile::tempdir().unwrap();
        for gloss in [
            "HELLO".to_owned(),
            "".into(),
            "字".repeat(33),
            "bad\0text".into(),
        ] {
            let write = request(
                root.path(),
                "remember",
                json!([
                {"text":"hello","direction":"english_to_chinese","translation":gloss}]),
            );
            assert_eq!(run(&write).unwrap()["saved"], 0);
        }
        assert!(!root.path().join("translation-glosses.db").exists());
    }

    #[test]
    fn damaged_database_preserves_legacy_and_reports_write_failure() {
        let root = tempfile::tempdir().unwrap();
        TranslationGlossStore::new(root.path())
            .remember("en", GlossDirection::EnglishToChinese, "hello", "旧释义")
            .unwrap();
        let database = root.path().join("translation-glosses.db");
        std::fs::write(&database, b"synthetic damaged database").unwrap();
        let read = request(
            root.path(),
            "lookup",
            json!([
            {"text":"Hello","direction":"english_to_chinese"}]),
        );
        assert_eq!(
            run(&read).unwrap()["translations"][0]["translation"],
            "旧释义"
        );
        let write = request(
            root.path(),
            "remember",
            json!([
            {"text":"Hello","direction":"english_to_chinese","translation":"新释义"}]),
        );
        assert!(run(&write).is_err());
        assert_eq!(
            std::fs::read(database).unwrap(),
            b"synthetic damaged database"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_ancestor_before_creating_the_engine_database() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked = parent.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let directory = linked.join("missing");
        let write = request(
            &directory,
            "remember",
            json!([{"text":"Hello","direction":"english_to_chinese","translation":"你好"}]),
        );

        assert_eq!(run(&write), Err("learned translation storage unavailable"));
        assert!(!outside.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_existing_directory_below_a_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked = parent.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        std::fs::create_dir(outside.path().join("existing")).unwrap();
        let write = request(
            &linked.join("existing"),
            "remember",
            json!([{"text":"Hello","direction":"english_to_chinese","translation":"你好"}]),
        );

        assert_eq!(run(&write), Err("learned translation storage unavailable"));
        assert!(!outside
            .path()
            .join("existing/translation-glosses.db")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_engine_database() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("translation-glosses.db");
        let outside_database = outside.path().join("translation-glosses.db");
        std::fs::write(&outside_database, b"keep me").unwrap();
        symlink(&outside_database, &database).unwrap();
        let write = request(
            root.path(),
            "remember",
            json!([{"text":"Hello","direction":"english_to_chinese","translation":"你好"}]),
        );

        assert_eq!(run(&write), Err("learned translation storage unavailable"));
        assert_eq!(std::fs::read(outside_database).unwrap(), b"keep me");
    }
}
