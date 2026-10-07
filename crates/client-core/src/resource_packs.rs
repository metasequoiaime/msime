//! 按需下载的资源包：macOS 发布包不再内置的日文词典、粤拼/注音词库，桌面发布包不再内置的手写模型和桌面落定重排模型。
//!
//! 每个资源包安装在 `<state_root>/resource-packs/<id>/`，文件平铺，最后写入的 `msime-model.json` 标记安装完整。文件名、URL、长度和 SHA-256 全部来自仓库内审过的锁文件，下载、校验、暂存和整体发布复用 [`crate::voice::local_models::install_files`]。

use crate::resources::{ResourceSet, ON_DEMAND_JAPANESE_ARTIFACTS};
use crate::voice::local_models::{self, InstallProgress, LocalModelError, MANIFEST_FILE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

/// 资源包根目录相对 state_root 的名字。
pub const DIRECTORY: &str = "resource-packs";

const DESKTOP_LOCK: &str = include_str!("../../../resources/desktop-dictionary.lock.json");
const LANGUAGE_LOCK: &str = include_str!("../../../resources/language-dictionaries.lock.json");
const HANDWRITING_LOCK: &str = include_str!("../../../resources/handwriting-model.lock.json");
const SETTLED_MODEL_LOCK: &str = include_str!("../../../resources/settled-model.lock.json");

/// 读语言词库包里 `msime-<方案>.db` 的输入方案，即偏好里的方案名。
const LANGUAGE_DICTIONARY_SCHEMES: [&str; 3] = ["cantonese", "zhuyin", "stroke"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourcePack {
    Japanese,
    LanguageDictionaries,
    Handwriting,
    /// 桌面神经联想在输入停顿后使用的落定重排模型（`sentence-model-desktop.safetensors`）。
    SettledModel,
}

/// 手写模型锁文件多一个说明来源的 `source` 字段，所以这里不拒绝未知字段，只取安装需要的两项。
#[derive(Deserialize)]
struct PackLock {
    source_commit: String,
    artifacts: Vec<crate::resources::Artifact>,
}

impl ResourcePack {
    pub const ALL: [ResourcePack; 4] = [
        ResourcePack::Japanese,
        ResourcePack::LanguageDictionaries,
        ResourcePack::Handwriting,
        ResourcePack::SettledModel,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ResourcePack::Japanese => "japanese",
            ResourcePack::LanguageDictionaries => "language-dictionaries",
            ResourcePack::Handwriting => "handwriting",
            ResourcePack::SettledModel => "settled-model",
        }
    }

    pub fn from_id(id: &str) -> Option<ResourcePack> {
        ResourcePack::ALL.into_iter().find(|pack| pack.id() == id)
    }

    /// 选用这些输入方案时需要该资源包。手写和落定重排模型不对应输入方案。语言词库包只列出锁文件确实固定了 `msime-<方案>.db` 的方案：词库还没发布的方案下载了也装不上，不能当作由这个包提供。
    pub fn schemes(self) -> &'static [&'static str] {
        static LANGUAGE_SCHEMES: OnceLock<Vec<&'static str>> = OnceLock::new();
        match self {
            ResourcePack::Japanese => &["japanese"],
            ResourcePack::LanguageDictionaries => LANGUAGE_SCHEMES.get_or_init(|| {
                let pinned = &self.set().artifacts;
                let pinned_names: HashSet<&str> = pinned
                    .iter()
                    .map(|artifact| artifact.name.as_str())
                    .collect();
                LANGUAGE_DICTIONARY_SCHEMES
                    .into_iter()
                    .filter(|scheme| pinned_names.contains(format!("msime-{scheme}.db").as_str()))
                    .collect()
            }),
            ResourcePack::Handwriting | ResourcePack::SettledModel => &[],
        }
    }

    /// 该资源包固定的文件清单，取自仓库内的锁文件。
    pub fn set(self) -> &'static ResourceSet {
        static SETS: OnceLock<[ResourceSet; 4]> = OnceLock::new();
        let sets = SETS.get_or_init(|| {
            let desktop: ResourceSet =
                serde_json::from_str(DESKTOP_LOCK).expect("desktop dictionary lock is valid");
            let language: ResourceSet =
                serde_json::from_str(LANGUAGE_LOCK).expect("language dictionaries lock is valid");
            let handwriting: PackLock =
                serde_json::from_str(HANDWRITING_LOCK).expect("handwriting model lock is valid");
            let settled: ResourceSet =
                serde_json::from_str(SETTLED_MODEL_LOCK).expect("settled model lock is valid");
            [
                desktop.only(&ON_DEMAND_JAPANESE_ARTIFACTS),
                language,
                ResourceSet {
                    source_commit: handwriting.source_commit,
                    artifacts: handwriting.artifacts,
                },
                settled,
            ]
        });
        match self {
            ResourcePack::Japanese => &sets[0],
            ResourcePack::LanguageDictionaries => &sets[1],
            ResourcePack::Handwriting => &sets[2],
            ResourcePack::SettledModel => &sets[3],
        }
    }

    /// 安装时写入的 `msime-model.json` 内容。判断已安装的版本是否还能用只看其中每个文件的名字、SHA-256 和长度，见 [`ResourcePack::manifest_matches`]。
    pub fn manifest(self) -> Value {
        let set = self.set();
        serde_json::json!({
            "pack": self.id(),
            "source_commit": set.source_commit,
            "artifacts": set.artifacts,
        })
    }

    /// 已安装的 `msime-model.json` 是否对应当前锁文件固定的同一组字节：资源包 id 相同，且文件的 (名字, SHA-256, 长度) 集合与锁文件一致。
    ///
    /// 不比较 URL 和 `source_commit`：词库每发一次新的 dict-v，没变的文件也会换一个带版本号的下载地址和来源提交，只按它们判断会把字节完全相同的已装文件当成过期，日文随之降级到没有词典。
    fn manifest_matches(self, manifest: &Value) -> bool {
        if manifest.get("pack").and_then(Value::as_str) != Some(self.id()) {
            return false;
        }
        let Some(installed) = manifest.get("artifacts").and_then(Value::as_array) else {
            return false;
        };
        let Some(mut installed) = installed
            .iter()
            .map(|artifact| {
                Some((
                    artifact.get("name")?.as_str()?,
                    artifact.get("sha256")?.as_str()?.to_ascii_lowercase(),
                    artifact.get("size")?.as_u64()?,
                ))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let mut pinned = self
            .set()
            .artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.name.as_str(),
                    artifact.sha256.to_ascii_lowercase(),
                    artifact.size,
                )
            })
            .collect::<Vec<_>>();
        installed.sort_unstable();
        pinned.sort_unstable();
        installed == pinned
    }

    /// 下载总字节数。
    pub fn size(self) -> u64 {
        self.set()
            .artifacts
            .iter()
            .map(|artifact| artifact.size)
            .sum()
    }
}

/// `<state_root>/resource-packs`。
pub fn root(state_root: &Path) -> PathBuf {
    state_root.join(DIRECTORY)
}

/// 资源包的每一层父目录都必须是真实目录。只检查最后一层会让 `resource-packs` 自身的符号链接把读取导向 state_root 外部；系统自己的符号链接（macOS 的 `/var`、`/tmp`，Android 的 `/data/user/0`）由 `msime-path-trust` 列出。
fn resource_root_is_safe(state_root: &Path) -> bool {
    let mut current = Some(root(state_root));
    while let Some(path) = current {
        if msime_path_trust::is_trusted_system_alias(&path) {
            break;
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return false;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
        current = path.parent().map(Path::to_path_buf);
    }
    true
}

/// 已发布的资源包目录：真实目录（不是符号链接），且带有普通文件形式的 `msime-model.json`。
fn published_directory(state_root: &Path, pack: ResourcePack) -> Option<PathBuf> {
    if !resource_root_is_safe(state_root) {
        return None;
    }
    let directory = root(state_root).join(pack.id());
    if !fs::symlink_metadata(&directory).ok()?.file_type().is_dir() {
        return None;
    }
    if !fs::symlink_metadata(directory.join(MANIFEST_FILE))
        .ok()?
        .file_type()
        .is_file()
    {
        return None;
    }
    Some(directory)
}

/// 已下载资源包里的某个文件。只有 `name` 属于该资源包、资源包已完整发布、且该文件是普通文件时才返回路径；暂存目录、符号链接和缺少 `msime-model.json` 的目录都不算。
pub fn installed_file(state_root: &Path, pack: ResourcePack, name: &str) -> Option<PathBuf> {
    if !pack
        .set()
        .artifacts
        .iter()
        .any(|artifact| artifact.name == name)
    {
        return None;
    }
    let directory = published_directory(state_root, pack)?;
    let manifest = local_models::installed_manifest(&root(state_root), pack.id())?;
    if !pack.manifest_matches(&manifest) {
        return None;
    }
    let path = directory.join(name);
    fs::symlink_metadata(&path)
        .ok()?
        .file_type()
        .is_file()
        .then_some(path)
}

/// 资源包锁钉住的每个文件在已发布目录里都是普通文件，不是符号链接。
fn artifacts_are_regular_files(directory: &Path, pack: ResourcePack) -> bool {
    pack.set().artifacts.iter().all(|artifact| {
        fs::symlink_metadata(directory.join(&artifact.name))
            .is_ok_and(|metadata| metadata.file_type().is_file())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackState {
    Missing,
    Installed,
    /// 已安装，但 `msime-model.json` 记录的文件字节（名字、SHA-256、长度）与当前锁文件不一致，需要重新下载。
    Outdated,
}

/// 设置页展示用的资源包状态。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResourcePackStatus {
    pub id: &'static str,
    pub state: PackState,
    pub size: u64,
    pub schemes: &'static [&'static str],
}

/// 每个资源包在 `state_root` 下的安装状态。
pub fn list(state_root: &Path) -> Vec<ResourcePackStatus> {
    let packs_root = root(state_root);
    ResourcePack::ALL
        .into_iter()
        .map(|pack| {
            let state = match published_directory(state_root, pack) {
                Some(directory) => match local_models::installed_manifest(&packs_root, pack.id()) {
                    // 与 `installed_file` 同一个标准：钉住的每个文件都得是普通文件（不是符号链接），少了或被换掉的按过期处理，补齐时会重新下载修复。
                    Some(manifest)
                        if pack.manifest_matches(&manifest)
                            && artifacts_are_regular_files(&directory, pack) =>
                    {
                        PackState::Installed
                    }
                    Some(_) => PackState::Outdated,
                    // 标记文件在但读不出来（损坏或过大）也按过期处理，重新下载即可修复。
                    None => PackState::Outdated,
                },
                None => PackState::Missing,
            };
            ResourcePackStatus {
                id: pack.id(),
                state,
                size: pack.size(),
                schemes: pack.schemes(),
            }
        })
        .collect()
}

/// 下载、校验并发布一个资源包到 `<state_root>/resource-packs/<id>`，替换旧安装。阻塞调用，不要放在 UI 线程；`cancel` 在每个数据块之间轮询。
pub fn install(
    state_root: &Path,
    pack: ResourcePack,
    mirror: &str,
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    local_models::install_files(
        &root(state_root),
        pack.id(),
        &pack.set().artifacts,
        &pack.manifest(),
        mirror,
        progress,
        cancel,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 伪造一个已发布的资源包目录：所有文件加上当前的 `msime-model.json`。
    fn publish_fake(state_root: &Path, pack: ResourcePack) -> PathBuf {
        let directory = root(state_root).join(pack.id());
        fs::create_dir_all(&directory).unwrap();
        for artifact in &pack.set().artifacts {
            fs::write(directory.join(&artifact.name), b"bytes").unwrap();
        }
        fs::write(
            directory.join(MANIFEST_FILE),
            serde_json::to_vec(&pack.manifest()).unwrap(),
        )
        .unwrap();
        directory
    }

    #[test]
    fn every_pack_set_validates() {
        for pack in ResourcePack::ALL {
            pack.set().validate().unwrap();
            assert!(pack.size() > 0, "{}", pack.id());
        }
    }

    #[test]
    fn the_language_pack_offers_exactly_the_schemes_whose_dictionary_is_pinned() {
        let pinned: Vec<&str> = ResourcePack::LanguageDictionaries
            .set()
            .artifacts
            .iter()
            .filter_map(|artifact| artifact.name.strip_prefix("msime-")?.strip_suffix(".db"))
            .collect();
        assert_eq!(ResourcePack::LanguageDictionaries.schemes(), pinned);
        assert!(pinned.contains(&"cantonese") && pinned.contains(&"zhuyin"));
    }

    #[test]
    fn the_japanese_pack_is_exactly_the_desktop_lock_subset() {
        let desktop: ResourceSet = serde_json::from_str(DESKTOP_LOCK).unwrap();
        let japanese = ResourcePack::Japanese.set();
        assert_eq!(japanese.source_commit, desktop.source_commit);
        assert_eq!(japanese.artifacts.len(), ON_DEMAND_JAPANESE_ARTIFACTS.len());
        for artifact in &japanese.artifacts {
            assert!(ON_DEMAND_JAPANESE_ARTIFACTS.contains(&artifact.name.as_str()));
            let locked = desktop
                .artifacts
                .iter()
                .find(|locked| locked.name == artifact.name)
                .unwrap();
            assert_eq!(
                (&artifact.url, &artifact.sha256, artifact.size),
                (&locked.url, &locked.sha256, locked.size)
            );
        }
        assert_eq!(ResourcePack::LanguageDictionaries.set().artifacts.len(), 6);
        assert_eq!(ResourcePack::Handwriting.set().artifacts.len(), 2);
        assert_eq!(ResourcePack::SettledModel.set().artifacts.len(), 1);
        assert_eq!(
            ResourcePack::SettledModel.set().artifacts[0].name,
            "sentence-model-desktop.safetensors"
        );
        assert!(ResourcePack::SettledModel.schemes().is_empty());
    }

    #[test]
    fn ids_round_trip() {
        for pack in ResourcePack::ALL {
            assert_eq!(ResourcePack::from_id(pack.id()), Some(pack));
            assert_eq!(
                serde_json::to_value(pack).unwrap(),
                Value::String(pack.id().into())
            );
        }
        assert_eq!(ResourcePack::from_id("unknown"), None);
        assert_eq!(ResourcePack::from_id(""), None);
    }

    #[test]
    fn installed_file_only_finds_published_pack_files() {
        let state = tempfile::tempdir().unwrap();
        let pack = ResourcePack::Japanese;
        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            None
        );
        let directory = publish_fake(state.path(), pack);
        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            Some(directory.join("msime-japanese.dat"))
        );
        // 不属于该资源包的名字，即便文件存在也不认。
        fs::write(directory.join("msime-pinyin.db"), b"bytes").unwrap();
        assert_eq!(installed_file(state.path(), pack, "msime-pinyin.db"), None);
        assert_eq!(installed_file(state.path(), pack, MANIFEST_FILE), None);
        assert_eq!(
            installed_file(
                state.path(),
                ResourcePack::Handwriting,
                "msime-japanese.dat"
            ),
            None
        );
        // 没有 msime-model.json 的目录不算安装完整。
        fs::remove_file(directory.join(MANIFEST_FILE)).unwrap();
        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            None
        );
    }

    #[test]
    fn installed_file_ignores_an_outdated_published_pack() {
        let state = tempfile::tempdir().unwrap();
        let pack = ResourcePack::Japanese;
        let directory = publish_fake(state.path(), pack);
        fs::write(
            directory.join(MANIFEST_FILE),
            serde_json::to_vec(&serde_json::json!({
                "pack": pack.id(),
                "source_commit": "old"
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            None
        );
    }

    #[test]
    fn installed_file_ignores_staging_directories() {
        let state = tempfile::tempdir().unwrap();
        let staging = root(state.path())
            .join(".staging-japanese-abc")
            .join("model");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("msime-japanese.dat"), b"bytes").unwrap();
        fs::write(staging.join(MANIFEST_FILE), b"{}").unwrap();
        assert_eq!(
            installed_file(state.path(), ResourcePack::Japanese, "msime-japanese.dat"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn installed_file_rejects_symlinks() {
        use msime_path_trust::untrusted_symlink as symlink;
        let state = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let pack = ResourcePack::LanguageDictionaries;

        // 资源包目录本身是符号链接。
        let external = publish_fake(outside.path(), pack);
        fs::create_dir_all(root(state.path())).unwrap();
        symlink(&external, root(state.path()).join(pack.id())).unwrap();
        assert_eq!(
            installed_file(state.path(), pack, "msime-cantonese.db"),
            None
        );
        fs::remove_file(root(state.path()).join(pack.id())).unwrap();

        // 资源包里的文件是符号链接。
        let directory = publish_fake(state.path(), pack);
        fs::remove_file(directory.join("msime-cantonese.db")).unwrap();
        symlink(
            external.join("msime-cantonese.db"),
            directory.join("msime-cantonese.db"),
        )
        .unwrap();
        assert_eq!(
            installed_file(state.path(), pack, "msime-cantonese.db"),
            None
        );
        assert_eq!(
            installed_file(state.path(), pack, "msime-zhuyin.db"),
            Some(directory.join("msime-zhuyin.db"))
        );

        // 资源包父目录是符号链接时，也不能把外部文件当作已安装资源。
        let linked_state = tempfile::tempdir().unwrap();
        let linked_root = root(linked_state.path());
        symlink(root(outside.path()), &linked_root).unwrap();
        assert_eq!(
            installed_file(linked_state.path(), pack, "msime-cantonese.db"),
            None
        );
        assert!(list(linked_state.path())
            .iter()
            .all(|status| status.state == PackState::Missing));
    }

    #[test]
    fn list_reports_missing_installed_and_outdated() {
        let state = tempfile::tempdir().unwrap();
        let statuses = list(state.path());
        assert_eq!(statuses.len(), ResourcePack::ALL.len());
        assert!(statuses
            .iter()
            .all(|status| status.state == PackState::Missing));
        assert_eq!(
            statuses[1].schemes,
            ResourcePack::LanguageDictionaries.schemes()
        );
        assert_eq!(statuses[2].size, ResourcePack::Handwriting.size());

        let directory = publish_fake(state.path(), ResourcePack::Handwriting);
        let state_of = |id: &str| {
            list(state.path())
                .into_iter()
                .find(|status| status.id == id)
                .unwrap()
                .state
        };
        assert_eq!(state_of("handwriting"), PackState::Installed);
        fs::write(
            directory.join(MANIFEST_FILE),
            serde_json::to_vec(&serde_json::json!({"pack": "handwriting", "source_commit": "old"}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(state_of("handwriting"), PackState::Outdated);
        fs::write(directory.join(MANIFEST_FILE), b"{ not json").unwrap();
        assert_eq!(state_of("handwriting"), PackState::Outdated);
        assert_eq!(
            serde_json::to_value(PackState::Outdated).unwrap(),
            Value::String("outdated".into())
        );
    }

    /// 清单对得上但钉住的文件少了、是目录或是符号链接时，`installed_file` 找不到它，列表也不能报已安装，否则补齐会跳过它。
    #[test]
    fn list_reports_a_pack_with_a_missing_or_irregular_file_as_outdated() {
        let state = tempfile::tempdir().unwrap();
        let pack = ResourcePack::LanguageDictionaries;
        let state_of = || {
            list(state.path())
                .into_iter()
                .find(|status| status.id == pack.id())
                .unwrap()
                .state
        };
        let directory = publish_fake(state.path(), pack);
        assert_eq!(state_of(), PackState::Installed);

        let file = directory.join("msime-cantonese.db");
        fs::remove_file(&file).unwrap();
        assert_eq!(state_of(), PackState::Outdated);

        fs::create_dir(&file).unwrap();
        assert_eq!(state_of(), PackState::Outdated);
        fs::remove_dir(&file).unwrap();

        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            let target = outside.path().join("msime-cantonese.db");
            fs::write(&target, b"bytes").unwrap();
            msime_path_trust::untrusted_symlink(&target, &file).unwrap();
            assert_eq!(state_of(), PackState::Outdated);
            fs::remove_file(&file).unwrap();
        }

        fs::write(&file, b"bytes").unwrap();
        assert_eq!(state_of(), PackState::Installed);
    }

    /// 同一组字节换了下载地址和来源提交（词库发布新的 dict-v 时没变的文件就是这样）仍算已安装；任何一个文件的字节变了才算过期。
    #[test]
    fn only_changed_bytes_make_an_installed_pack_outdated() {
        let state = tempfile::tempdir().unwrap();
        let pack = ResourcePack::Japanese;
        let directory = publish_fake(state.path(), pack);
        let state_of = || {
            list(state.path())
                .into_iter()
                .find(|status| status.id == pack.id())
                .unwrap()
                .state
        };
        let rewrite = |edit: &dyn Fn(&mut Value)| {
            let mut manifest = pack.manifest();
            edit(&mut manifest);
            fs::write(
                directory.join(MANIFEST_FILE),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
        };

        rewrite(&|manifest| {
            manifest["source_commit"] = Value::String("0".repeat(40));
            for artifact in manifest["artifacts"].as_array_mut().unwrap() {
                let url = artifact["url"]
                    .as_str()
                    .unwrap()
                    .replace("dict-v", "dict-v0-old-");
                artifact["url"] = Value::String(url);
                // 摘要大小写不同不算字节不同。
                let digest = artifact["sha256"].as_str().unwrap().to_ascii_uppercase();
                artifact["sha256"] = Value::String(digest);
            }
            manifest["artifacts"].as_array_mut().unwrap().reverse();
        });
        assert_eq!(state_of(), PackState::Installed);
        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            Some(directory.join("msime-japanese.dat"))
        );

        rewrite(&|manifest| {
            let artifact = &mut manifest["artifacts"][0];
            artifact["size"] = Value::from(artifact["size"].as_u64().unwrap() + 1);
        });
        assert_eq!(state_of(), PackState::Outdated);
        assert_eq!(
            installed_file(state.path(), pack, "msime-japanese.dat"),
            None
        );

        rewrite(&|manifest| {
            manifest["artifacts"][0]["sha256"] = Value::String("0".repeat(64));
        });
        assert_eq!(state_of(), PackState::Outdated);

        // 少一个文件、或者是别的资源包的清单，都不算同一组字节。
        rewrite(&|manifest| {
            manifest["artifacts"].as_array_mut().unwrap().pop();
        });
        assert_eq!(state_of(), PackState::Outdated);
        rewrite(&|manifest| {
            manifest["pack"] = Value::String("handwriting".into());
        });
        assert_eq!(state_of(), PackState::Outdated);
    }

    /// 资源包只从本项目的固定发布地址下载：msime-dictionary 和 chinese-ime-lm 的 GitHub Release 资产，或钉在 40 位提交上的 msime-engine 原始文件。
    #[test]
    fn every_url_is_immutable() {
        const RELEASE: &str =
            "https://github.com/metasequoiaime/msime-dictionary/releases/download/";
        const MODEL_RELEASE: &str =
            "https://github.com/metasequoiaime/chinese-ime-lm/releases/download/";
        const ENGINE: &str = "https://raw.githubusercontent.com/metasequoiaime/msime-engine/";
        for pack in ResourcePack::ALL {
            for artifact in &pack.set().artifacts {
                let url = artifact.url.as_str();
                let pinned_engine = url.strip_prefix(ENGINE).is_some_and(|rest| {
                    rest.split_once('/')
                        .is_some_and(|(commit, _)| crate::is_lower_hex(commit, 40))
                });
                assert!(
                    url.starts_with(RELEASE) || url.starts_with(MODEL_RELEASE) || pinned_engine,
                    "{url}"
                );
            }
        }
    }
}
