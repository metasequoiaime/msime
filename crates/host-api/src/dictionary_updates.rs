//! 所有设置宿主共用词库请求 `updates`；输入宿主只在准备或重启时选择已经完整安装的代次。

use crate::{HostOptions, ResourcePack, ResourceSet};
use msime_client_core::edition::Edition;
use msime_client_providers::{dictionary_updates as updates, resource_packs};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub(crate) fn request(
    options: &HostOptions,
    command: &str,
    enabled: Option<bool>,
) -> Result<Value, String> {
    let state = options
        .preferences_directory
        .as_deref()
        .filter(|s| Path::new(s).is_absolute())
        .ok_or("dictionary_update_invalid")?;
    let baseline = options
        .edition()
        .resource_set()
        .map_err(|_| "dictionary_update_invalid")?;
    let context = updates::UpdateContext {
        state_root: PathBuf::from(state),
        resources: PathBuf::from(&options.resources),
        baseline,
        on_demand: crate::ON_DEMAND_ARTIFACTS
            .iter()
            .flat_map(|group| group.iter().map(|name| (*name).to_owned()))
            .collect(),
        languages: options
            .language_dictionaries
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| {
                crate::installed_language_dictionaries(Path::new(&options.resources))
                    .map(PathBuf::from)
            })
            .or_else(|| {
                resource_packs::installed_file(
                    Path::new(state),
                    ResourcePack::LanguageDictionaries,
                    "msime-cantonese.db",
                )
                .and_then(|p| p.parent().map(Path::to_path_buf))
            }),
        format: msime_engine::format::FORMAT_VERSION,
    };
    let status = match command {
        "status" => updates::status(&context),
        "configure" => updates::configure(&context, enabled.ok_or("dictionary_update_invalid")?)
            .map_err(|e| e.to_string())?,
        "rollback" => updates::rollback(&context).map_err(|e| e.to_string())?,
        "automatic" if !updates::due(&context) => updates::status(&context),
        "automatic" | "check" => {
            // 每个进程只留一条任务；跨进程互斥与每天的节流在 providers 的持久层执行。
            static RUNNING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
            let tasks = RUNNING.get_or_init(Mutex::default);
            let mut running = tasks.lock().map_err(|_| "dictionary_update_storage")?;
            if running.insert(context.state_root.clone()) {
                let task = context.clone();
                let automatic = command == "automatic";
                if std::thread::Builder::new()
                    .name("dictionary-update".into())
                    .spawn(move || {
                        let _ = updates::check(&task, automatic);
                        if let Ok(mut running) = tasks.lock() {
                            running.remove(&task.state_root);
                        }
                    })
                    .is_err()
                {
                    running.remove(&context.state_root);
                    return Err("dictionary_update_storage".into());
                }
            }
            let mut status = updates::status(&context);
            status.phase = "checking".into();
            status
        }
        _ => return Err("dictionary_update_invalid".into()),
    };
    serde_json::to_value(status).map_err(|_| "dictionary_update_invalid".into())
}

/// 保留包内资源作为校验失败和回退的来源。这里不联网，也不改正在使用的工作词库。
pub(crate) fn selected_resources(
    state: &Path,
    baseline: &ResourceSet,
) -> Option<(PathBuf, ResourceSet)> {
    let selection = updates::selected(state, baseline)?;
    let directory = updates::directory(state, &selection.desktop).ok()?;
    // prepare_shipped_host_configuration 的标记按文件身份缓存完整校验；该处失败时调用方回到随包资源。
    Some((directory, selection.desktop))
}

pub(crate) fn languages(state: &Path, edition: &Edition) -> Option<PathBuf> {
    let selected = updates::selected(state, &edition.resource_set().ok()?)?;
    let set = selected.languages?;
    let path = updates::directory(state, &set).ok()?;
    verify(state, &path, &set)?;
    Some(path)
}

/// 可选词库各用自己的校验标记，避免语言词库与主词库互相挤掉缓存。
pub(crate) fn verify(state: &Path, directory: &Path, set: &ResourceSet) -> Option<()> {
    let marker = state
        .join("dictionary-updates/verification")
        .join(set.generation().ok()?);
    crate::verify_resources_once(directory, set, &marker, &[]).ok()
}

pub(crate) fn specification_for_options(
    options: &HostOptions,
) -> Result<ResourceSet, Box<dyn std::error::Error>> {
    let baseline = options.edition().resource_set()?;
    if let Some(state) = options.preferences_directory.as_deref() {
        if let Some(set) =
            updates::set_for_directory(Path::new(state), &baseline, Path::new(&options.resources))
        {
            return Ok(set);
        }
    }
    Ok(baseline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::resources::Artifact;
    use msime_engine::host::{
        dictionary_edit, dictionary_edit_bundled, DictionaryEntry, DictionaryKind, Session,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn hash_files(path: &Path, set: &mut ResourceSet, tag: &str) {
        for file in &mut set.artifacts {
            let bytes = std::fs::read(path.join(&file.name)).unwrap();
            *file = Artifact {
                name: file.name.clone(),
                url: format!(
                    "https://github.com/metasequoiaime/msime-dictionary/releases/download/{tag}/{}",
                    file.name
                ),
                size: bytes.len() as u64,
                sha256: hex::encode(Sha256::digest(bytes)),
            };
        }
    }

    #[test]
    fn dictionary_updates_activate_replay_user_data_rollback_and_fall_back_after_damage() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let state = root.path().join("state");
        let mut baseline = crate::tests::synthetic_desktop_lock(&resources);
        rusqlite::Connection::open(resources.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
        rusqlite::Connection::open(resources.join("msime-english.db")).unwrap().execute_batch(
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER,UNIQUE(word,display));
             INSERT INTO english_words VALUES('weightedfixture','weightedfixture',100),('deletedfixture','deletedfixture',100);"
        ).unwrap();
        hash_files(&resources, &mut baseline, "dict-v2.0.9");
        let prepare = || {
            let document =
                crate::prepare_host_with_updates(&resources, &state, Edition::full(), &baseline)
                    .unwrap();
            HostOptions::from_document(serde_json::from_str(&document).unwrap())
                .unwrap()
                .into_engine_options()
        };
        let old = prepare();
        let entry = |word: &str, weight| DictionaryEntry {
            kind: DictionaryKind::English,
            key: word.into(),
            value: word.into(),
            weight,
        };
        dictionary_edit(
            &old,
            None,
            Some(&entry("personalfixture", 123)),
            "fixture-add",
        )
        .unwrap();
        dictionary_edit_bundled(
            &old,
            &entry("weightedfixture", 100),
            Some(456),
            "fixture-edit",
        )
        .unwrap();
        dictionary_edit_bundled(&old, &entry("deletedfixture", 100), None, "fixture-delete")
            .unwrap();
        let mut session = Session::new(&old).unwrap();
        session.set_dedicated_english(true).unwrap();
        for byte in b"learnedfixture" {
            session.character(*byte, false).unwrap();
        }
        session
            .command(msime_engine::host::Command::CommitRaw)
            .unwrap();
        drop(session);

        let incoming = root.path().join("incoming");
        std::fs::create_dir(&incoming).unwrap();
        for file in &baseline.artifacts {
            std::fs::copy(resources.join(&file.name), incoming.join(&file.name)).unwrap();
        }
        rusqlite::Connection::open(incoming.join("msime-english.db"))
            .unwrap()
            .execute_batch("INSERT INTO english_words VALUES('remotefixture','remotefixture',200);")
            .unwrap();
        let mut updated = baseline.clone();
        updated.source_commit = "b".repeat(40);
        hash_files(&incoming, &mut updated, "dict-v2.0.10");
        let generation_root = state.join("dictionary-updates/generations");
        let installed = crate::ResourceStore::new(&generation_root)
            .install(&updated, |file| {
                Ok(Box::new(std::fs::File::open(incoming.join(&file.name))?))
            })
            .unwrap();
        let selection = updates::Selection {
            baseline_generation: baseline.generation().unwrap(),
            version: "dict-v2.0.10".into(),
            desktop: updated,
            languages: None,
            bundled_resources: resources.clone(),
        };
        msime_client_core::file_lock::replace_private_file(
            state.join("dictionary-updates/installed.json"),
            &serde_json::to_vec(&json!({"active":selection,"previous":null})).unwrap(),
        )
        .unwrap();
        let current = prepare();
        assert_eq!(
            Path::new(&current.resources),
            std::fs::canonicalize(&installed).unwrap()
        );
        assert_ne!(old.dictionaries, current.dictionaries);
        let assert_personal = |options: &msime_engine::host::EngineOptions| {
            let db = rusqlite::Connection::open(
                Path::new(&options.dictionaries).join("msime-english.db"),
            )
            .unwrap();
            for (word, expected) in [("personalfixture", 123), ("weightedfixture", 456)] {
                let weight: i64 = db
                    .query_row(
                        "SELECT weight FROM english_words WHERE word=?1",
                        [word],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(weight, expected);
            }
            let count = |word| {
                db.query_row::<i64, _, _>(
                    "SELECT count(*) FROM english_words WHERE word=?1",
                    [word],
                    |r| r.get(0),
                )
                .unwrap()
            };
            assert_eq!(count("deletedfixture"), 0);
            assert_eq!(count("learnedfixture"), 1);
        };
        assert_personal(&current);
        let db =
            rusqlite::Connection::open(Path::new(&current.dictionaries).join("msime-english.db"))
                .unwrap();
        assert_eq!(
            db.query_row::<i64, _, _>(
                "SELECT count(*) FROM english_words WHERE word='remotefixture'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        let context = updates::UpdateContext {
            state_root: state.clone(),
            resources: installed.clone(),
            baseline: baseline.clone(),
            on_demand: vec![],
            languages: None,
            format: 1,
        };
        updates::rollback(&context).unwrap();
        let reverted = prepare();
        assert_eq!(
            Path::new(&reverted.resources),
            std::fs::canonicalize(&resources).unwrap()
        );
        assert_personal(&reverted);
        updates::rollback(&context).unwrap();
        std::fs::write(installed.join("msime-english.db"), b"corrupt fixture").unwrap();
        assert_eq!(
            Path::new(&prepare().resources),
            std::fs::canonicalize(&resources).unwrap()
        );
    }

    /// 下载的代次目录没有 `helpcodes/`：新准备的文档记下当前安装包的目录，只经 `refresh_host_options` 换过 `resources`、没有这个键的旧文档从更新记录里找到随包目录。
    #[test]
    fn downloaded_generation_falls_back_to_the_bundled_helpcode_tables() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let state = root.path().join("state");
        let mut baseline = crate::tests::synthetic_desktop_lock(&resources);
        hash_files(&resources, &mut baseline, "dict-v2.0.9");
        std::fs::create_dir_all(resources.join("helpcodes")).unwrap();
        std::fs::write(resources.join("helpcodes/helpcode.txt"), "你=ab\n").unwrap();
        let prepare = || -> Value {
            serde_json::from_str(
                &crate::prepare_host_with_updates(&resources, &state, Edition::full(), &baseline)
                    .unwrap(),
            )
            .unwrap()
        };
        // 还在用随包词库时资源目录自己有码表，不需要回退。
        let shipped = prepare();
        assert!(shipped.get("helpcode_fallback_resources").is_none());
        assert_eq!(
            HostOptions::from_document(shipped)
                .unwrap()
                .into_engine_options()
                .helpcode_fallback_resources,
            None
        );

        let mut updated = baseline.clone();
        updated.source_commit = "b".repeat(40);
        hash_files(&resources, &mut updated, "dict-v2.0.10");
        let installed = crate::ResourceStore::new(state.join("dictionary-updates/generations"))
            .install(&updated, |file| {
                Ok(Box::new(std::fs::File::open(resources.join(&file.name))?))
            })
            .unwrap();
        assert!(!installed.join("helpcodes").exists());
        let selection = updates::Selection {
            baseline_generation: baseline.generation().unwrap(),
            version: "dict-v2.0.10".into(),
            desktop: updated,
            languages: None,
            bundled_resources: resources.clone(),
        };
        msime_client_core::file_lock::replace_private_file(
            state.join("dictionary-updates/installed.json"),
            &serde_json::to_vec(&json!({"active":selection,"previous":null})).unwrap(),
        )
        .unwrap();

        let mut document = prepare();
        let options = HostOptions::from_document(document.clone())
            .unwrap()
            .into_engine_options();
        assert_eq!(
            Path::new(&options.resources),
            std::fs::canonicalize(&installed).unwrap()
        );
        let fallback = options.helpcode_fallback_resources.unwrap();
        assert!(Path::new(&fallback)
            .join("helpcodes/helpcode.txt")
            .is_file());

        // 旧文档没有这个键：编译进来的词库锁与这里的合成锁不同，直接拿合成锁查更新记录。
        document
            .as_object_mut()
            .unwrap()
            .remove("helpcode_fallback_resources");
        let refreshed = HostOptions::from_document(document).unwrap();
        assert_eq!(
            crate::recorded_helpcode_bundle(&state, &baseline).as_deref(),
            resources.to_str()
        );
        assert_eq!(
            crate::helpcode_fallback_resources(
                None,
                Path::new(&refreshed.resources),
                None,
                Edition::full()
            ),
            None,
            "没有状态目录就没有更新记录可查"
        );
        // 更新记录里的随包目录没有码表时不回退。
        std::fs::remove_dir_all(resources.join("helpcodes")).unwrap();
        assert_eq!(crate::recorded_helpcode_bundle(&state, &baseline), None);
    }
}
