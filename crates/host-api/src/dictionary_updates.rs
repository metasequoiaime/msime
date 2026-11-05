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
        mirrors: Vec::new(),
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
                let mut task = context.clone();
                task.mirrors = saved_mirrors(options, Path::new(state));
                let automatic = command == "automatic";
                let configured = configured_generation(options);
                if std::thread::Builder::new()
                    .name("dictionary-update".into())
                    .spawn(move || {
                        // 与下载版本同一个时机回收工作词库：装好了新版本，或确认已是最新。
                        if updates::check(&task, automatic).is_ok_and(|status| {
                            matches!(status.phase.as_str(), "ready" | "current")
                        }) {
                            remove_unused_working_generations(
                                &task.state_root,
                                &task.baseline,
                                configured.as_deref(),
                            );
                        }
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

/// 回收再也用不上的工作词库代次（#10735）。宿主为每个词库版本在 `<state>/user/dictionaries/<代次>` 准备一份工作词库（主词库、英文词库和 n-gram 表的副本，回放过用户日志，一份几十到上百 MB），代次就是那一版资源集合的 `generation()`：随包词库是本安装包的词库锁，在线更新的是下载版本的桌面代次。以前这里只建不删，每装一个新版本、每升级一次换了词库锁，就多留一份。
///
/// 留下：本安装包词库锁的代次，宿主配置记着的那一代（`configured`，即发来请求的 HostOptions 的 `dictionaries`），和下载版本还留着的那些（[`updates::retained_generations`]：当前、可回退的上一个、输入法最近载入的、有会话占用而没删的）。宿主配置那一代不能只靠锁保护：Linux 上自己下载的词库在升级换锁后刷新报 `dictionary_outdated`，宿主按设计继续用 `runtime-options.json` 记着的旧锁代次，而 Fcitx5 每次失焦都销毁会话、建会话时不重新准备代次，没有焦点的那一刻谁也不持锁，删了它之后每次建会话都失败，重启宿主也准备不回来。Linux 和 Android 的设置应用每次请求都现读 `runtime-options.json`，这里拿到的就是输入法此刻在用的那份。其余 64 位十六进制名字的目录逐个先取这一个目录上的独占词库访问锁：每个会话都以共享方式持着它所用代次的这把锁（`DictionaryAccess::try_session`），设置里改词这类维护也持着它，取不到就说明还有人在用，跳过，下次再试。升级后还没重启的旧版输入法进程用的就是上一个词库锁的代次，它的会话活着时也因此不会被删。取到锁之后再问一次哪些版本还留着，避开在这期间刚发布的新版本，然后改名成 `<代次>.<随机串>.removing` 认领再删，删到一半留下的目录由引擎准备代次时清理暂存目录的那一步收尾（`.removing` 后缀与它约定一致）。Windows 上目录里有打开的文件时改名可能失败，任何平台改名失败都保留原目录，留到下次持锁再试，不能释放租约后无锁重试。
///
/// 代次里的内容都能从资源和用户日志重新准备出来，但仍在使用的目录必须受租约保护。回收失败时只多留一份，不能绕过锁继续删除。
pub(crate) fn remove_unused_working_generations(
    state: &Path,
    baseline: &ResourceSet,
    configured: Option<&str>,
) {
    remove_unused_working_generations_using(state, baseline, configured, |source, target| {
        std::fs::rename(source, target)
    });
}

fn remove_unused_working_generations_using(
    state: &Path,
    baseline: &ResourceSet,
    configured: Option<&str>,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) {
    let Ok(current) = baseline.generation() else {
        return;
    };
    let parent = state.join("user").join("dictionaries");
    if !state.is_absolute() || msime_path_trust::reject_symlinked_components(&parent).is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return;
    };
    let candidates: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| {
            name.len() == 64
                && name.bytes().all(|b| b.is_ascii_hexdigit())
                && *name != current
                && Some(name.as_str()) != configured
        })
        .collect();
    for name in candidates {
        let path = parent.join(&name);
        if !std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
            continue;
        }
        // 两个根是同一个目录，去重后只取这个代次上的一把锁，不碰用户目录上那把（所有会话都持着它）。
        let Ok(Some(access)) = crate::DictionaryAccess::try_maintenance(&path, &path) else {
            continue;
        };
        let Some(retained) = updates::retained_generations(state) else {
            return;
        };
        if retained.contains(&name) {
            continue;
        }
        let claimed = parent.join(format!("{name}.{}.removing", attempt()));
        let moved = rename(&path, &claimed).is_ok();
        drop(access);
        if moved {
            let _ = std::fs::remove_dir_all(&claimed);
        }
    }
}

/// 宿主配置的工作词库目录（HostOptions 的 `dictionaries`）的代次名。只取文件名，不核对它是不是在 `user/dictionaries/` 下：多留一份只是多占空间。
fn configured_generation(options: &HostOptions) -> Option<String> {
    Path::new(&options.dictionaries)
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

/// 认领目录名里的随机串：引擎只认字母数字，这里用进程号和当前纳秒的十六进制。
fn attempt() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{:x}{nanos:x}", std::process::id())
}

/// 用户在设置里保存的下载镜像（`voice_input.asr_model_mirror`，资源包下载也用它）。读实时的偏好文件，用户改了镜像不必等宿主重新准备；读不了时退回宿主文档里的偏好副本。空串表示没填。
fn saved_mirrors(options: &HostOptions, state: &Path) -> Vec<String> {
    let mirror = msime_client_core::preferences::PreferencesStore::new(state)
        .load()
        .map(|snapshot| snapshot.preferences.voice_input.asr_model_mirror)
        .unwrap_or_else(|_| options.preferences.voice_input.asr_model_mirror.clone());
    if mirror.is_empty() {
        Vec::new()
    } else {
        vec![mirror]
    }
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
            mirrors: Vec::new(),
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

    /// #10735：每个词库版本在 `user/dictionaries/<代次>` 留一份工作词库。本安装包词库锁的、下载版本还留着的、还有会话或维护在用的留下，其余的删掉；不是代次名字的目录（暂存目录、别的东西）和符号链接不碰。
    #[test]
    fn unused_working_generations_are_removed_unless_bundled_retained_or_in_use() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let state = root.path().join("state");
        let mut baseline = crate::tests::synthetic_desktop_lock(&resources);
        hash_files(&resources, &mut baseline, "dict-v2.0.9");
        // 随包词库真正准备出来的工作词库，名字就是词库锁的代次。
        let document: Value = serde_json::from_str(
            &crate::prepare_host_with_updates(&resources, &state, Edition::full(), &baseline)
                .unwrap(),
        )
        .unwrap();
        let dictionaries = state.join("user/dictionaries");
        let bundled = dictionaries.join(baseline.generation().unwrap());
        assert_eq!(
            Path::new(document["dictionaries"].as_str().unwrap()),
            bundled
        );
        let working = |name: &str| {
            let path = dictionaries.join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("msime-pinyin.db"), b"working copy").unwrap();
            std::fs::write(
                path.join(msime_engine::assets::GENERATION_READY),
                b"ready\n",
            )
            .unwrap();
            path
        };
        let generation = |digit: char| digit.to_string().repeat(64);
        // 下载版本还留着清单的（当前、上一个、删不掉的）。
        let retained = working(&generation('1'));
        let downloads = state.join("dictionary-updates/generations");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::write(downloads.join(format!("{}.json", generation('1'))), b"{}").unwrap();
        // 输入法最近载入的那一版。
        let loaded = working(&generation('2'));
        std::fs::create_dir_all(downloads.join(generation('2'))).unwrap();
        updates::record_loaded(&state, &downloads.join(generation('2'))).unwrap();
        // 下载版本早已删掉的，和升级前那个词库锁的。
        let removed = working(&generation('3'));
        let previous_bundle = working(&generation('4'));
        // 还有会话在用的（例如升级后还没重启的旧版输入法）。
        let in_use = working(&generation('5'));
        let session = crate::DictionaryAccess::try_session(&state.join("user"), &in_use)
            .unwrap()
            .unwrap();
        let staging = dictionaries.join(format!("{}.attempt.incoming", generation('6')));
        std::fs::create_dir_all(&staging).unwrap();
        let unrelated = dictionaries.join("custom");
        std::fs::create_dir_all(&unrelated).unwrap();
        #[cfg(unix)]
        let linked = {
            let outside = root.path().join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("keep"), b"keep").unwrap();
            let linked = dictionaries.join(generation('7'));
            std::os::unix::fs::symlink(&outside, &linked).unwrap();
            linked
        };

        remove_unused_working_generations(&state, &baseline, None);
        for path in [&bundled, &retained, &loaded, &in_use, &staging, &unrelated] {
            assert!(path.is_dir(), "{path:?}");
        }
        assert!(bundled.join("msime-pinyin.db").is_file());
        for path in [&removed, &previous_bundle] {
            assert!(!path.exists(), "{path:?}");
        }
        #[cfg(unix)]
        {
            assert!(std::fs::symlink_metadata(&linked).unwrap().is_symlink());
            assert!(root.path().join("outside/keep").is_file());
        }
        let claimed = |dictionaries: &Path| {
            std::fs::read_dir(dictionaries)
                .unwrap()
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| name.ends_with(".removing"))
                .count()
        };
        assert_eq!(claimed(&dictionaries), 0);

        // 会话结束、下载版本也删掉之后，下一次检查把它们收走；本安装包的那份始终留着。
        drop(session);
        std::fs::remove_file(downloads.join(format!("{}.json", generation('1')))).unwrap();
        remove_unused_working_generations(&state, &baseline, None);
        assert!(!in_use.exists() && !retained.exists());
        assert!(bundled.is_dir() && loaded.is_dir());
        assert_eq!(claimed(&dictionaries), 0);

        // 下载记录读不出来时不知道该留哪一版，什么也不删。
        let unknown = working(&generation('8'));
        std::fs::write(state.join("dictionary-updates/installed.json"), b"{").unwrap();
        remove_unused_working_generations(&state, &baseline, None);
        assert!(unknown.is_dir());
    }

    #[test]
    fn failed_generation_claim_keeps_the_lease_until_rename_returns() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let state = root.path().join("state");
        let mut baseline = crate::tests::synthetic_desktop_lock(&resources);
        hash_files(&resources, &mut baseline, "dict-v2.0.9");
        let obsolete = state.join("user/dictionaries").join("a".repeat(64));
        std::fs::create_dir_all(&obsolete).unwrap();
        std::fs::write(obsolete.join("synthetic.db"), b"synthetic generation").unwrap();
        let mut attempts = 0;

        remove_unused_working_generations_using(&state, &baseline, None, |source, _| {
            attempts += 1;
            assert_eq!(source, obsolete);
            // 模拟系统拒绝改名，同时用真实访问锁检查任何一次尝试都有独占租约保护。
            assert!(
                crate::DictionaryAccess::try_session(source, source)
                    .unwrap()
                    .is_none(),
                "改名期间不能有新会话取得共享租约"
            );
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        });

        assert_eq!(attempts, 1, "失败后留到下次检查，不能释放租约后重试");
        assert_eq!(
            std::fs::read(obsolete.join("synthetic.db")).unwrap(),
            b"synthetic generation"
        );
        let session = crate::DictionaryAccess::try_session(&obsolete, &obsolete)
            .unwrap()
            .expect("失败后旧代次仍能建立会话");
        drop(session);
        remove_unused_working_generations(&state, &baseline, None);
        assert!(!obsolete.exists(), "下次持锁认领成功后仍可回收");
    }

    /// #10735：Linux 上自己下载的词库在升级换锁后刷新报 `dictionary_outdated`，宿主继续用 runtime-options.json 记着的旧锁代次。Fcitx5 失焦就销毁会话，此刻没有任何会话持锁，这一代也既不是新的随包代次、也不是下载版本；只有宿主配置记着它，删了输入法就再也建不了会话。
    #[test]
    fn the_generation_the_host_configuration_records_is_kept_without_a_session() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let state = root.path().join("state");
        let mut previous = crate::tests::synthetic_desktop_lock(&resources);
        hash_files(&resources, &mut previous, "dict-v2.0.9");
        // 升级前按旧锁准备好的宿主配置，就是 runtime-options.json 里那份。
        let document: Value = serde_json::from_str(
            &crate::prepare_host_with_updates(&resources, &state, Edition::full(), &previous)
                .unwrap(),
        )
        .unwrap();
        let options: HostOptions = serde_json::from_value(document).unwrap();
        let outdated = state
            .join("user/dictionaries")
            .join(previous.generation().unwrap());
        assert_eq!(Path::new(&options.dictionaries), outdated);
        assert!(outdated
            .join(msime_engine::assets::GENERATION_READY)
            .is_file());
        // 升级换了词库锁；用户下载的那份词库没人替换，新锁的代次从来没准备过。
        let mut upgraded = previous.clone();
        let changed = resources.join(&upgraded.artifacts[0].name);
        let mut bytes = std::fs::read(&changed).unwrap();
        bytes.extend_from_slice(b"upgraded");
        std::fs::write(&changed, bytes).unwrap();
        hash_files(&resources, &mut upgraded, "dict-v2.0.10");
        assert_ne!(
            upgraded.generation().unwrap(),
            previous.generation().unwrap()
        );

        let configured = configured_generation(&options);
        assert_eq!(
            configured.as_deref(),
            Some(previous.generation().unwrap().as_str())
        );
        remove_unused_working_generations(&state, &upgraded, configured.as_deref());
        assert!(outdated.join("msime-pinyin.db").is_file(), "{outdated:?}");
        // 宿主配置换到别的代次之后，这一代才按无人再用回收。
        remove_unused_working_generations(&state, &upgraded, None);
        assert!(!outdated.exists());
    }

    /// 检查任务用的镜像来自设置里保存的 `voice_input.asr_model_mirror`：读实时偏好文件，不用准备宿主时那份副本；实时文件读不了时才用副本。
    #[test]
    fn saved_mirror_comes_from_the_live_preferences_and_falls_back_to_the_document_copy() {
        use msime_client_core::preferences::{Preferences, PreferencesStore};
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        std::fs::create_dir(&state).unwrap();
        let mut copy = Preferences::default();
        copy.voice_input.asr_model_mirror = "https://copy.example.test/".into();
        let options: HostOptions = serde_json::from_value(json!({
            "api_version": 1,
            "resources": root.path().join("resources"),
            "user_data": root.path().join("user"),
            "cache": root.path().join("cache"),
            "dictionaries": root.path().join("dictionaries"),
            "preferences": copy,
            "preferences_directory": state,
        }))
        .unwrap();

        let store = PreferencesStore::new(&state);
        let mut saved = Preferences::default();
        saved.voice_input.asr_model_mirror = "https://saved.example.test/".into();
        store.save(store.load().unwrap().revision, saved).unwrap();
        assert_eq!(
            saved_mirrors(&options, &state),
            ["https://saved.example.test/"]
        );

        // 用户在设置里清空镜像：不再用副本里的旧值。
        let cleared = store.load().unwrap();
        store
            .save(cleared.revision, Preferences::default())
            .unwrap();
        assert!(saved_mirrors(&options, &state).is_empty());

        std::fs::write(state.join("preferences.json"), b"{").unwrap();
        assert_eq!(
            saved_mirrors(&options, &state),
            ["https://copy.example.test/"]
        );
    }
}
