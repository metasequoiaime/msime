//! Bounded worker-thread operations on the private learned-gloss store.
use msime_client_core::translation::store::{
    GlossDirection, GlossStoreError, TranslationGlossStore,
};
use msime_client_core::translation::{
    format_translation_gloss, is_cloud_translatable_chinese, is_cloud_translatable_english,
    should_persist_translation,
};
use msime_client_core::{is_bounded_text, is_bounded_text_with_chars};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

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
        || !["en", "fr", "ja", "es", "ru", "de", "ko"].contains(&request.target_language.as_str())
        || request.items.len() > 9
        || request.items.iter().any(|item| {
            item.text.is_empty()
                || !is_bounded_text_with_chars(&item.text, 160, 40)
                || match request.action {
                    Action::Lookup => item.translation.is_some(),
                    Action::Remember => item.translation.as_ref().is_none_or(|s| s.len() > 4096),
                }
        })
    {
        return Err("invalid learned translation parameters");
    }
    // Preserve previous JSON records as read-only fallback. New writes use the
    // same Engine-owned user database as other native hosts, never resources.
    let legacy = TranslationGlossStore::new(directory);
    let mut translations = Vec::new();
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
                let learned = msime_engine_bridge::candidate_glosses_with_user(
                    "",
                    &request.directory,
                    &[(key.clone(), if chinese { 0 } else { 4 })],
                )
                .ok()
                .and_then(|values| values.into_iter().next())
                .filter(|text| !text.is_empty());
                if let Some(translation) = learned {
                    translations.push(json!({"text":item.text,"translation":translation}));
                    continue;
                }
                match legacy.lookup(&request.target_language, item.direction, &item.text) {
                    Ok(Some(translation)) => {
                        translations.push(json!({"text":item.text,"translation":translation}))
                    }
                    Ok(None) | Err(GlossStoreError::InvalidRecord) => {}
                    Err(_) => return Err("learned translation storage unavailable"),
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
                std::fs::create_dir_all(directory)
                    .map_err(|_| "learned translation storage unavailable")?;
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options.open(directory.join("translation-glosses.db")) {
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err("learned translation storage unavailable"),
                }
                if !msime_engine_bridge::save_candidate_gloss(
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
            msime_engine_bridge::candidate_glosses_with_user("", directory, &[("hello".into(), 4)])
                .unwrap(),
            vec!["新释义"]
        );
        assert!(msime_engine_bridge::save_candidate_gloss(
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
}
