//! 按需下载的资源包：macOS 和 Android 发布包不再内置的日文词典、粤拼/注音/笔画词库，桌面发布包不再内置的手写模型和桌面落定重排模型，Android 发布包不再内置的离线释义词典和本地语音运行库，以及 macOS 发布包不再内置的五笔码表和本地语音运行库。
//!
//! 每个资源包安装在 `<state_root>/resource-packs/<id>/`，文件平铺，最后写入的 `msime-model.json` 标记安装完整。文件名、URL、长度和 SHA-256 全部来自仓库内审过的锁文件，下载、校验、暂存和整体发布复用 [`crate::voice::local_models::install_files`]；语音运行库从固定的归档里取出文件，见 [`ResourcePack::archive`]。
//!
//! 锁文件还没有固定内容的资源包（[`ResourcePack::pinned`] 为假，目前只可能是 macOS 语音运行库在第一次发布之前）不存在：[`ResourcePack::from_id`] 认不出它，[`list`] 不列出它，安装、导入和收编都报 `local_model_unknown`。

use crate::resources::{ResourceSet, ON_DEMAND_JAPANESE_ARTIFACTS, ON_DEMAND_WUBI_ARTIFACTS};
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
const GRAMMAR_MODELS_LOCK: &str = include_str!("../../../resources/grammar-models.lock.json");

/// `grammar-models.lock.json` 的形态：每个语法模型一条，各自带自己的上游提交和文件。
/// 四个模型来自三个仓库（万象、八股文、白霜/墨奇），所以不能共用一条 `source_commit`。
#[derive(Deserialize)]
struct GrammarModelsLock {
    models: Vec<GrammarModelLock>,
}

#[derive(Deserialize)]
struct GrammarModelLock {
    id: String,
    source_commit: String,
    artifacts: Vec<crate::resources::Artifact>,
}
const OFFLINE_GLOSSES_LOCK: &str = include_str!("../../../resources/offline-glosses.lock.json");
/// 各平台的 sherpa-onnx 运行库归档，`fetch_voice_runtime.py` 和打包脚本用的同一份锁。
const VOICE_RUNTIME_LOCK: &str = include_str!("../../../resources/voice-runtime.lock.json");
/// 从 Android 归档里取出的两个库各自的长度和 SHA-256。
const VOICE_RUNTIME_ANDROID_LOCK: &str =
    include_str!("../../../resources/voice-runtime-android.lock.json");
/// macOS 的 sherpa-onnx 运行库：上游 universal dylib 用本项目的 Developer ID 重签后打成 ZIP，作为本仓库 `release_tag` 那个 Release 的资产发布（`.github/workflows/publish-macos-voice-runtime.yml`）。输入法和它的识别辅助进程开着 hardened runtime，库验证只放行同一 Team ID 签名的库，上游原样的 dylib 下载下来也加载不了，所以不能像 Android 那样直接取上游归档。
const VOICE_RUNTIME_MACOS_LOCK: &str =
    include_str!("../../../resources/voice-runtime-macos.lock.json");
/// macOS 语音运行库包里唯一的文件，也是 `shared/voice/LocalAsr.cpp` 在 macOS 上加载的库名。
pub const MACOS_VOICE_RUNTIME_LIBRARY: &str = "libsherpa-onnx-c-api.dylib";
/// 重签的 macOS 语音运行库发布在本仓库的 Release 下，锁文件的 `release_tag` 接在后面。
const MACOS_VOICE_RUNTIME_RELEASES: &str =
    "https://github.com/metasequoiaime/msime/releases/download/";

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
    /// octagram 语法模型万象 LTS（CC-BY-4.0）：`wanxiang-lts-zh-hans.gram`。
    #[serde(rename = "grammar-model-wanxiang")]
    GrammarWanxiang,
    /// octagram 语法模型八股文·词级（LGPL-3.0）：`zh-hans-t-essay-bgw.gram`。
    #[serde(rename = "grammar-model-baguwen")]
    GrammarBaguwen,
    /// octagram 语法模型八股文·词级紧凑（LGPL-3.0）：`zh-hans-t-essay-bgw-compact.gram`。
    #[serde(rename = "grammar-model-baguwen-compact")]
    GrammarBaguwenCompact,
    /// octagram 语法模型白霜/墨奇（实验，GPL-3.0 且构建链许可未声明）：`zh-moqi.gram`。
    #[serde(rename = "grammar-model-baishuang")]
    GrammarBaishuang,
    /// 候选的非英文离线释义：`zh-<语言>.db` 六份加上随它们一起分发的 `offline-glosses-NOTICE.txt`（改编自 CC BY-SA 4.0 的维基词典）。
    OfflineGlosses,
    /// Android arm64 上本地语音识别加载的 `libonnxruntime.so` 和 `libsherpa-onnx-c-api.so`，取自 `resources/voice-runtime.lock.json` 固定的 sherpa-onnx `.aar`。
    VoiceRuntime,
    /// macOS 发布包不再内置的五笔码表 `msime-wubi.db`，即 `desktop-dictionary.lock.json` 里的那一项。
    Wubi,
    /// macOS 上本地语音识别加载的 `libsherpa-onnx-c-api.dylib`（universal），取自 `resources/voice-runtime-macos.lock.json` 固定的、用本项目 Developer ID 重签过的归档。锁文件还没固定归档时不存在，见 [`ResourcePack::pinned`]。
    MacosVoiceRuntime,
}

/// 手写模型锁文件多一个说明来源的 `source` 字段，所以这里不拒绝未知字段，只取安装需要的两项。
#[derive(Deserialize)]
struct PackLock {
    source_commit: String,
    artifacts: Vec<crate::resources::Artifact>,
}

/// 离线释义锁文件记录了构建它们的输入（`input`）和许可；资源包只取文件清单，来源版本用输入数据的 `source_revision`。
#[derive(Deserialize)]
struct OfflineGlossesLock {
    input: OfflineGlossesInput,
    artifacts: Vec<crate::resources::Artifact>,
}

#[derive(Deserialize)]
struct OfflineGlossesInput {
    source_revision: String,
}

#[derive(Deserialize)]
struct VoiceRuntimeLock {
    source_commit: String,
    platforms: std::collections::BTreeMap<String, VoiceRuntimeArchive>,
}

/// `voice-runtime.lock.json` 里一个平台的归档；只取下载需要的几项。
#[derive(Deserialize)]
struct VoiceRuntimeArchive {
    name: String,
    url: String,
    sha256: String,
    size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceRuntimeAndroidLock {
    source_commit: String,
    archive_platform: String,
    artifacts: Vec<ArchiveMemberLock>,
}

/// `voice-runtime-macos.lock.json`。`archive` 为 `null`、`artifacts` 为空表示重签的运行库还没发布：这时 macOS 发布包照旧内置运行库，资源包不存在。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceRuntimeMacosLock {
    /// sherpa-onnx 的来源提交，必须与 `voice-runtime.lock.json` 相同。
    source_commit: String,
    /// 重签的是 `voice-runtime.lock.json` 里哪个平台的上游归档。
    upstream_platform: String,
    /// 发布归档的 Release 标签，归档的 URL 必须在它下面。
    release_tag: String,
    /// 签名用的 Developer ID 所属的 Team ID。发布包只有用同一个 Team ID 签名时才省略内置的运行库（`platforms/macos/package-release.sh`），否则输入法加载不了下载的库。
    team_id: String,
    archive: Option<crate::resources::Artifact>,
    artifacts: Vec<ArchiveMemberLock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveMemberLock {
    name: String,
    member: String,
    sha256: String,
    size: u64,
}

/// 从一个固定的上游归档里取出的文件：先按锁文件下载并校验整个归档，再取出 `members` 列出的成员，逐个按各自的长度和 SHA-256 校验。
#[derive(Clone, Debug)]
pub struct PackArchive {
    /// 归档本身，名字、URL、长度和 SHA-256 取自锁文件。
    pub archive: crate::resources::Artifact,
    /// (归档里的成员路径, 安装后的文件名)，与资源包清单里的文件一一对应。
    pub members: Vec<(String, String)>,
}

impl ResourcePack {
    /// 每一种资源包，包括锁文件还没固定内容的；只要存在的那些用 [`ResourcePack::available`]。
    pub const ALL: [ResourcePack; 12] = [
        ResourcePack::Japanese,
        ResourcePack::LanguageDictionaries,
        ResourcePack::Handwriting,
        ResourcePack::SettledModel,
        ResourcePack::GrammarWanxiang,
        ResourcePack::GrammarBaguwen,
        ResourcePack::GrammarBaguwenCompact,
        ResourcePack::GrammarBaishuang,
        ResourcePack::OfflineGlosses,
        ResourcePack::VoiceRuntime,
        ResourcePack::Wubi,
        ResourcePack::MacosVoiceRuntime,
    ];

    /// 锁文件固定了内容、可以下载的资源包，顺序同 [`ResourcePack::ALL`]。
    pub fn available() -> impl Iterator<Item = ResourcePack> {
        ResourcePack::ALL.into_iter().filter(|pack| pack.pinned())
    }

    /// 是否有已固定且允许提供的资源包内容。macOS 语音运行库在重签归档固定前不可用，白霜语法模型因许可限制保持不可用。
    pub fn pinned(self) -> bool {
        match self {
            ResourcePack::MacosVoiceRuntime => macos_voice_runtime().is_some(),
            ResourcePack::GrammarBaishuang => false,
            _ => true,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            ResourcePack::Japanese => "japanese",
            ResourcePack::LanguageDictionaries => "language-dictionaries",
            ResourcePack::Handwriting => "handwriting",
            ResourcePack::SettledModel => "settled-model",
            ResourcePack::GrammarWanxiang => "grammar-model-wanxiang",
            ResourcePack::GrammarBaguwen => "grammar-model-baguwen",
            ResourcePack::GrammarBaguwenCompact => "grammar-model-baguwen-compact",
            ResourcePack::GrammarBaishuang => "grammar-model-baishuang",
            ResourcePack::OfflineGlosses => "offline-glosses",
            ResourcePack::VoiceRuntime => "voice-runtime",
            ResourcePack::Wubi => "wubi",
            ResourcePack::MacosVoiceRuntime => "voice-runtime-macos",
        }
    }

    /// 按 id 查找允许提供的已固定资源包（[`ResourcePack::pinned`]）。
    pub fn from_id(id: &str) -> Option<ResourcePack> {
        ResourcePack::available().find(|pack| pack.id() == id)
    }

    /// 选用这些输入方案时需要该资源包。手写和落定重排模型不对应输入方案。语言词库包只列出锁文件确实固定了 `msime-<方案>.db` 的方案：词库还没发布的方案下载了也装不上，不能当作由这个包提供。
    pub fn schemes(self) -> &'static [&'static str] {
        static LANGUAGE_SCHEMES: OnceLock<Vec<&'static str>> = OnceLock::new();
        match self {
            ResourcePack::Japanese => &["japanese"],
            ResourcePack::Wubi => &["wubi"],
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
            ResourcePack::Handwriting
            | ResourcePack::SettledModel
            | ResourcePack::GrammarWanxiang
            | ResourcePack::GrammarBaguwen
            | ResourcePack::GrammarBaguwenCompact
            | ResourcePack::GrammarBaishuang
            | ResourcePack::OfflineGlosses
            | ResourcePack::VoiceRuntime
            | ResourcePack::MacosVoiceRuntime => &[],
        }
    }

    /// 文件不单独下载、而是从一个归档里取出的资源包：两个语音运行库。Android 的归档是 sherpa-onnx 的上游 `.aar`（各 ABI 的库都在里面，只取 arm64-v8a 的两个）；macOS 的是本项目重签后发布的 ZIP，锁文件还没固定它时为 `None`。其余资源包为 `None`。
    pub fn archive(self) -> Option<&'static PackArchive> {
        static VOICE_RUNTIME: OnceLock<PackArchive> = OnceLock::new();
        match self {
            ResourcePack::VoiceRuntime => Some(VOICE_RUNTIME.get_or_init(|| {
                let (_, archive, members) = voice_runtime_locks();
                PackArchive {
                    archive,
                    members: members
                        .into_iter()
                        .map(|member| (member.member, member.name))
                        .collect(),
                }
            })),
            ResourcePack::MacosVoiceRuntime => macos_voice_runtime().map(|pinned| &pinned.archive),
            _ => None,
        }
    }

    /// 从语法模型锁里取一个模型，变成它自己的 `ResourceSet`。锁里没有这个 id 是编码错误，
    /// 直接 panic：`every_pack_set_validates` 会在测试里先撞上。
    fn grammar_model(lock: &GrammarModelsLock, id: &str) -> ResourceSet {
        let model = lock
            .models
            .iter()
            .find(|model| model.id == id)
            .unwrap_or_else(|| panic!("grammar models lock has no {id}"));
        ResourceSet {
            source_commit: model.source_commit.clone(),
            artifacts: model.artifacts.clone(),
        }
    }

    /// 该资源包固定的文件清单，取自仓库内的锁文件。从归档取出的文件（[`ResourcePack::archive`]），URL 记的是归档的地址，长度和 SHA-256 是取出后文件本身的。
    pub fn set(self) -> &'static ResourceSet {
        static SETS: OnceLock<[ResourceSet; 11]> = OnceLock::new();
        static UNPINNED: OnceLock<ResourceSet> = OnceLock::new();
        if self == ResourcePack::MacosVoiceRuntime {
            // 还没固定时是没有文件的空清单，它不能通过 `ResourceSet::validate`，也不会被列出或安装。
            return match macos_voice_runtime() {
                Some(pinned) => &pinned.set,
                None => UNPINNED.get_or_init(|| ResourceSet {
                    source_commit: macos_voice_runtime_lock().source_commit,
                    artifacts: Vec::new(),
                }),
            };
        }
        let sets = SETS.get_or_init(|| {
            let desktop: ResourceSet =
                serde_json::from_str(DESKTOP_LOCK).expect("desktop dictionary lock is valid");
            let language: ResourceSet =
                serde_json::from_str(LANGUAGE_LOCK).expect("language dictionaries lock is valid");
            let handwriting: PackLock =
                serde_json::from_str(HANDWRITING_LOCK).expect("handwriting model lock is valid");
            let settled: ResourceSet =
                serde_json::from_str(SETTLED_MODEL_LOCK).expect("settled model lock is valid");
            let grammar: GrammarModelsLock =
                serde_json::from_str(GRAMMAR_MODELS_LOCK).expect("grammar models lock is valid");
            let glosses: OfflineGlossesLock =
                serde_json::from_str(OFFLINE_GLOSSES_LOCK).expect("offline glosses lock is valid");
            let (runtime_commit, archive, members) = voice_runtime_locks();
            [
                desktop.only(&ON_DEMAND_JAPANESE_ARTIFACTS),
                language,
                ResourceSet {
                    source_commit: handwriting.source_commit,
                    artifacts: handwriting.artifacts,
                },
                settled,
                Self::grammar_model(&grammar, "wanxiang"),
                Self::grammar_model(&grammar, "baguwen"),
                Self::grammar_model(&grammar, "baguwen-compact"),
                Self::grammar_model(&grammar, "baishuang"),
                ResourceSet {
                    source_commit: glosses.input.source_revision,
                    artifacts: glosses.artifacts,
                },
                ResourceSet {
                    source_commit: runtime_commit,
                    artifacts: members
                        .into_iter()
                        .map(|member| crate::resources::Artifact {
                            name: member.name,
                            url: archive.url.clone(),
                            sha256: member.sha256,
                            size: member.size,
                        })
                        .collect(),
                },
                desktop.only(&ON_DEMAND_WUBI_ARTIFACTS),
            ]
        });
        match self {
            ResourcePack::Japanese => &sets[0],
            ResourcePack::LanguageDictionaries => &sets[1],
            ResourcePack::Handwriting => &sets[2],
            ResourcePack::SettledModel => &sets[3],
            ResourcePack::GrammarWanxiang => &sets[4],
            ResourcePack::GrammarBaguwen => &sets[5],
            ResourcePack::GrammarBaguwenCompact => &sets[6],
            ResourcePack::GrammarBaishuang => &sets[7],
            ResourcePack::OfflineGlosses => &sets[8],
            ResourcePack::VoiceRuntime => &sets[9],
            ResourcePack::Wubi => &sets[10],
            ResourcePack::MacosVoiceRuntime => unreachable!("handled above"),
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

    /// 下载总字节数。从归档取出的资源包下载的是整个归档。
    pub fn size(self) -> u64 {
        self.import_artifacts()
            .iter()
            .map(|artifact| artifact.size)
            .sum()
    }

    /// 安装时要从外面取得的文件：有归档的资源包只有那个归档，其余是锁文件里的全部文件。下载大小和不联网导入要的文件都按它算。
    pub fn import_artifacts(self) -> Vec<crate::resources::Artifact> {
        match self.archive() {
            Some(archive) => vec![archive.archive.clone()],
            None => self.set().artifacts.clone(),
        }
    }
}

/// Android 语音运行库：来源提交、`voice-runtime.lock.json` 里 `archive_platform` 那一项归档，和要从中取出的成员。两份锁的 `source_commit` 必须相同，否则成员的摘要对应的不是这份归档。
fn voice_runtime_locks() -> (String, crate::resources::Artifact, Vec<ArchiveMemberLock>) {
    let runtime: VoiceRuntimeLock =
        serde_json::from_str(VOICE_RUNTIME_LOCK).expect("voice runtime lock is valid");
    let android: VoiceRuntimeAndroidLock = serde_json::from_str(VOICE_RUNTIME_ANDROID_LOCK)
        .expect("Android voice runtime lock is valid");
    assert_eq!(
        runtime.source_commit, android.source_commit,
        "the Android voice runtime lock describes another sherpa-onnx release"
    );
    let archive = runtime
        .platforms
        .get(&android.archive_platform)
        .expect("the voice runtime lock pins the Android archive");
    (
        android.source_commit,
        crate::resources::Artifact {
            name: archive.name.clone(),
            url: archive.url.clone(),
            sha256: archive.sha256.clone(),
            size: archive.size,
        },
        android.artifacts,
    )
}

/// 锁文件固定好的 macOS 语音运行库：归档和从中取出的文件清单。
struct PinnedMacosVoiceRuntime {
    archive: PackArchive,
    set: ResourceSet,
}

fn macos_voice_runtime_lock() -> VoiceRuntimeMacosLock {
    serde_json::from_str(VOICE_RUNTIME_MACOS_LOCK).expect("macOS voice runtime lock is valid")
}

/// macOS 语音运行库锁文件里固定的内容；还没发布（`archive` 为 `null`）时为 `None`。
fn macos_voice_runtime() -> Option<&'static PinnedMacosVoiceRuntime> {
    static PINNED: OnceLock<Option<PinnedMacosVoiceRuntime>> = OnceLock::new();
    PINNED
        .get_or_init(|| pinned_macos_voice_runtime(macos_voice_runtime_lock()))
        .as_ref()
}

/// 把 macOS 语音运行库锁文件变成归档和文件清单。来源提交必须与 `voice-runtime.lock.json` 相同，否则锁的是另一个 sherpa-onnx 版本；`archive` 与 `artifacts` 必须同时为空或同时有内容，固定了归档就必须恰好取出 [`MACOS_VOICE_RUNTIME_LIBRARY`] 一个文件。
fn pinned_macos_voice_runtime(lock: VoiceRuntimeMacosLock) -> Option<PinnedMacosVoiceRuntime> {
    let runtime: VoiceRuntimeLock =
        serde_json::from_str(VOICE_RUNTIME_LOCK).expect("voice runtime lock is valid");
    assert_eq!(
        runtime.source_commit, lock.source_commit,
        "the macOS voice runtime lock describes another sherpa-onnx release"
    );
    assert!(
        runtime.platforms.contains_key(&lock.upstream_platform),
        "the macOS voice runtime lock names an upstream archive the voice runtime lock does not pin"
    );
    assert!(
        lock.team_id.len() == 10
            && lock
                .team_id
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()),
        "the macOS voice runtime lock names no Team ID"
    );
    let Some(archive) = lock.archive else {
        assert!(
            lock.artifacts.is_empty(),
            "the macOS voice runtime lock lists files without an archive"
        );
        return None;
    };
    assert!(
        lock.artifacts.len() == 1 && lock.artifacts[0].name == MACOS_VOICE_RUNTIME_LIBRARY,
        "the macOS voice runtime pack holds exactly {MACOS_VOICE_RUNTIME_LIBRARY}"
    );
    // 归档只能来自本仓库那个固定标签的 Release：标签下的资产一经发布就不再替换，锁文件钉的字节才一直下得到。
    assert!(
        archive.url
            == format!(
                "{MACOS_VOICE_RUNTIME_RELEASES}{}/{}",
                lock.release_tag, archive.name
            ),
        "the macOS voice runtime archive is not the asset of its release tag"
    );
    let members = lock
        .artifacts
        .iter()
        .map(|member| (member.member.clone(), member.name.clone()))
        .collect();
    let artifacts = lock
        .artifacts
        .into_iter()
        .map(|member| crate::resources::Artifact {
            name: member.name,
            url: archive.url.clone(),
            sha256: member.sha256,
            size: member.size,
        })
        .collect();
    Some(PinnedMacosVoiceRuntime {
        archive: PackArchive { archive, members },
        set: ResourceSet {
            source_commit: lock.source_commit,
            artifacts,
        },
    })
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
    /// 不联网安装（[`import`]）时用户要自己下载的文件：有归档的资源包是那个归档，其余是锁文件里的每个文件。
    pub import_files: Vec<local_models::LocalModelImportFile>,
}

/// 卸载一个已发布的资源包：删除 `<state_root>/resource-packs/<id>` 整层目录，
/// 返回是否有目录被删。只认安全 state_root 下的目录本身；符号链接不删（
/// `published_directory` 把它按未安装算），也不处理暂存残留。
pub fn remove(state_root: &Path, pack: ResourcePack) -> std::io::Result<bool> {
    if !resource_root_is_safe(state_root) {
        return Ok(false);
    }
    local_models::remove_installed(&root(state_root), pack.id(), true)
        .map_err(std::io::Error::other)
}

/// 每个资源包在 `state_root` 下的安装状态。
pub fn list(state_root: &Path) -> Vec<ResourcePackStatus> {
    let packs_root = root(state_root);
    ResourcePack::available()
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
                import_files: pack
                    .import_artifacts()
                    .iter()
                    .map(|artifact| local_models::LocalModelImportFile {
                        name: artifact.name.clone(),
                        url: artifact.url.clone(),
                        size: artifact.size,
                    })
                    .collect(),
            }
        })
        .collect()
}

/// 本项目自己的下载镜像前缀，按顺序排在用户镜像之后、锁文件原地址之前。形式与用户镜像相同（`https://mirror/` 加原地址）。镜像只提供字节，完整性始终按编译进来的锁文件校验。
///
/// `dl.msime.app` 是 Cloudflare R2 前面的一个 Worker（msime-web 的 README「国内镜像」）：国内连 GitHub Release 常常只有几十 KB/s。`gh/` 下按原地址存放放行名单里的文件（msime-dictionary、chinese-ime-lm 和 sherpa-onnx 的 release 资产，以及钉在提交上的 msime-engine raw 文件），没缓存过的由 Worker 从 GitHub 取一次后留存。名单之外的地址镜像答 403，按换源规则落到原地址。
pub const PROJECT_MIRRORS: &[&str] = &["https://dl.msime.app/gh/"];

/// 一个文件依次尝试的镜像前缀：先是用户填的 `mirrors`（空串跳过），再是 [`PROJECT_MIRRORS`]；都失败后用锁文件里的原地址。
pub fn download_prefixes<'a>(mirrors: &[&'a str]) -> Vec<&'a str> {
    let mut prefixes = Vec::with_capacity(mirrors.len() + PROJECT_MIRRORS.len());
    prefixes.extend(mirrors.iter().copied().filter(|mirror| !mirror.is_empty()));
    prefixes.extend(PROJECT_MIRRORS.iter().copied());
    prefixes
}

/// 下载、校验并发布一个资源包到 `<state_root>/resource-packs/<id>`，替换旧安装。阻塞调用，不要放在 UI 线程；`cancel` 在每个数据块之间轮询。
///
/// 每个文件按 [`download_prefixes`] 的顺序换源下载，没下完的部分下次用 HTTP Range 接着下（见 [`local_models::install_files`]）。
pub fn install(
    state_root: &Path,
    pack: ResourcePack,
    mirrors: &[&str],
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    if !pack.pinned() {
        return Err(LocalModelError::UnknownModel);
    }
    if let Some(archive) = pack.archive() {
        return local_models::install_archive_members(
            &root(state_root),
            pack.id(),
            &archive.archive,
            &archive.members,
            &pack.set().artifacts,
            &pack.manifest(),
            &download_prefixes(mirrors),
            progress,
            cancel,
        );
    }
    local_models::install_files(
        &root(state_root),
        pack.id(),
        &pack.set().artifacts,
        &pack.manifest(),
        &download_prefixes(mirrors),
        progress,
        cancel,
    )
}

/// 用用户自己下载好的文件安装一个资源包，不联网，替换旧安装。阻塞调用，不要放在 UI 线程。
///
/// 与模型的 [`local_models::import`] 相同：`files` 是宿主文件选择器给出的绝对路径，顺序和文件名都不看，[`ResourcePack::import_artifacts`] 里的每个文件先按长度找，长度相同的不止一个时再按 SHA-256 挑；找到后走与下载完全相同的校验、（有归档时）取成员、暂存和整体发布，所以文件不对时什么也不会装上。进度里的下载阶段报成 `import`；缺文件报 [`LocalModelError::MissingImportFile`]，读不出来报 [`LocalModelError::UnreadableImportFile`]。
pub fn import(
    state_root: &Path,
    pack: ResourcePack,
    files: &[PathBuf],
    progress: &mut dyn FnMut(InstallProgress),
    cancel: &AtomicBool,
) -> Result<PathBuf, LocalModelError> {
    if !pack.pinned() {
        return Err(LocalModelError::UnknownModel);
    }
    let artifacts = pack.import_artifacts();
    let fetcher = local_models::LocalFileFetcher::matching_sources(
        artifacts.iter().map(local_models::ImportSource::artifact),
        files,
        cancel,
    )?;
    let mut report = |event: InstallProgress| {
        progress(InstallProgress {
            stage: if event.stage == "download" {
                "import"
            } else {
                event.stage
            },
            ..event
        })
    };
    let result = match pack.archive() {
        Some(archive) => local_models::install_archive_members_with(
            &root(state_root),
            pack.id(),
            &archive.archive,
            &archive.members,
            &pack.set().artifacts,
            &pack.manifest(),
            &[],
            &fetcher,
            &mut report,
            cancel,
        ),
        None => local_models::install_files_with(
            &root(state_root),
            pack.id(),
            &pack.set().artifacts,
            &pack.manifest(),
            &[],
            &fetcher,
            &mut report,
            cancel,
        ),
    };
    local_models::imported(result)
}

/// 把本机已有的一组文件收编为已安装的资源包，不重新下载：升级前的发布包解压在 `source_dir`（比如 Android 的 `files/bootstrap/resources`）里的日文词典或语言词库，新版本不再随包带它们时用这一步保住。
///
/// 只取 `source_dir` 里属于该资源包的文件，同文件系统内改名移进暂存目录（不复制），在暂存目录里按编译进来的锁文件核对长度和 SHA-256，写入 `msime-model.json` 后整体发布到 `<state_root>/resource-packs/<id>`。任何一步失败都把已移走的文件改名放回 `source_dir`，什么也不发布，调用方照常改走下载。`source_dir` 和 `state_root` 不在同一个文件系统时改名失败，同样放回并报错。进程在改名之后、发布之前被杀时，文件留在暂存目录里；下一次收编或安装这个包时先按暂存目录里的来源记录把它们放回 `source_dir`，再照常进行。
///
/// 同一组字节已经安装好时（判断标准同 [`ResourcePack::manifest_matches`]）不碰 `source_dir`，直接返回已安装的目录。阻塞调用，要哈希整组文件（日文词典约 66 MB），不要放在 UI 线程。
pub fn adopt(
    state_root: &Path,
    pack: ResourcePack,
    source_dir: &Path,
) -> Result<PathBuf, LocalModelError> {
    if !pack.pinned() {
        return Err(LocalModelError::UnknownModel);
    }
    if !resource_root_is_safe(state_root) {
        return Err(LocalModelError::InvalidRoot);
    }
    if let Some(directory) = published_directory(state_root, pack) {
        let installed = local_models::installed_manifest(&root(state_root), pack.id())
            .is_some_and(|manifest| pack.manifest_matches(&manifest));
        if installed && artifacts_are_regular_files(&directory, pack) {
            return Ok(directory);
        }
    }
    let manifest = pack.manifest();
    debug_assert!(pack.manifest_matches(&manifest));
    local_models::adopt_files(
        &root(state_root),
        pack.id(),
        &pack.set().artifacts,
        &manifest,
        source_dir,
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
    fn remove_only_published_packs_and_keep_unpublished_files() {
        let dir = tempfile::tempdir().unwrap();
        let pack = ResourcePack::GrammarBaguwenCompact;
        assert!(!remove(dir.path(), pack).unwrap());
        let published = publish_fake(dir.path(), pack);
        assert!(remove(dir.path(), pack).unwrap());
        assert!(!published.exists());
        fs::create_dir_all(&published).unwrap();
        fs::write(published.join("synthetic.txt"), b"keep").unwrap();
        assert!(!remove(dir.path(), pack).unwrap());
        assert!(published.join("synthetic.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn remove_does_not_follow_pack_or_root_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let pack = ResourcePack::GrammarBaguwenCompact;
        let published = publish_fake(outside.path(), pack);
        fs::create_dir_all(root(dir.path())).unwrap();
        symlink(&published, root(dir.path()).join(pack.id())).unwrap();
        assert!(!remove(dir.path(), pack).unwrap());
        assert!(published.exists());
        // 删除流程会保留安装锁；根目录链接用独立目录验证，不能把锁当作泄漏清除。
        let linked_root = tempfile::tempdir().unwrap();
        symlink(root(outside.path()), root(linked_root.path())).unwrap();
        assert!(!remove(linked_root.path(), pack).unwrap());
        assert!(published.exists());
    }

    #[test]
    fn every_pack_set_validates() {
        for pack in ResourcePack::available() {
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
        let wubi = ResourcePack::Wubi.set();
        assert_eq!(wubi.source_commit, desktop.source_commit);
        assert_eq!(wubi.artifacts.len(), 1);
        let locked = desktop
            .artifacts
            .iter()
            .find(|locked| locked.name == ON_DEMAND_WUBI_ARTIFACTS[0])
            .unwrap();
        assert_eq!(
            (
                &wubi.artifacts[0].name,
                &wubi.artifacts[0].sha256,
                wubi.artifacts[0].size
            ),
            (&locked.name, &locked.sha256, locked.size)
        );
        assert_eq!(ResourcePack::Wubi.schemes(), ["wubi"]);
        assert!(ResourcePack::Wubi.archive().is_none());
        assert_eq!(ResourcePack::LanguageDictionaries.set().artifacts.len(), 6);
        assert_eq!(ResourcePack::Handwriting.set().artifacts.len(), 2);
        assert_eq!(ResourcePack::SettledModel.set().artifacts.len(), 1);
        assert_eq!(
            ResourcePack::SettledModel.set().artifacts[0].name,
            "sentence-model-desktop.safetensors"
        );
        assert!(ResourcePack::SettledModel.schemes().is_empty());
    }

    /// 离线释义包就是 `offline-glosses.lock.json` 固定的那七个文件：六种语言的词典加上必须随它们分发的 NOTICE。
    #[test]
    fn the_offline_glosses_pack_is_the_pinned_lock() {
        let lock: Value = serde_json::from_str(OFFLINE_GLOSSES_LOCK).unwrap();
        let pack = ResourcePack::OfflineGlosses;
        let names: Vec<&str> = pack
            .set()
            .artifacts
            .iter()
            .map(|artifact| artifact.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "zh-fr.db",
                "zh-ja.db",
                "zh-es.db",
                "zh-ru.db",
                "zh-de.db",
                "zh-ko.db",
                "offline-glosses-NOTICE.txt"
            ]
        );
        for (artifact, locked) in pack
            .set()
            .artifacts
            .iter()
            .zip(lock["artifacts"].as_array().unwrap())
        {
            assert_eq!(artifact.url, locked["url"].as_str().unwrap());
            assert_eq!(artifact.sha256, locked["sha256"].as_str().unwrap());
            assert_eq!(artifact.size, locked["size"].as_u64().unwrap());
        }
        assert_eq!(
            pack.set().source_commit,
            lock["input"]["source_revision"].as_str().unwrap()
        );
        assert!(pack.archive().is_none());
        assert!(pack.schemes().is_empty());
    }

    /// 语音运行库包从 `voice-runtime.lock.json` 固定的 Android `.aar` 里取 arm64-v8a 的两个库；下载大小是整个归档。
    #[test]
    fn the_voice_runtime_pack_comes_out_of_the_pinned_android_archive() {
        let runtime: Value = serde_json::from_str(VOICE_RUNTIME_LOCK).unwrap();
        let android = &runtime["platforms"]["android"];
        let pack = ResourcePack::VoiceRuntime;
        let archive = pack.archive().unwrap();
        assert_eq!(archive.archive.name, android["name"].as_str().unwrap());
        assert_eq!(archive.archive.url, android["url"].as_str().unwrap());
        assert_eq!(archive.archive.sha256, android["sha256"].as_str().unwrap());
        assert_eq!(archive.archive.size, android["size"].as_u64().unwrap());
        assert_eq!(pack.size(), archive.archive.size);
        assert_eq!(
            pack.set().source_commit,
            runtime["source_commit"].as_str().unwrap()
        );
        assert_eq!(
            archive.members,
            [
                (
                    "jni/arm64-v8a/libonnxruntime.so".to_owned(),
                    "libonnxruntime.so".to_owned()
                ),
                (
                    "jni/arm64-v8a/libsherpa-onnx-c-api.so".to_owned(),
                    "libsherpa-onnx-c-api.so".to_owned()
                ),
            ]
        );
        let names: Vec<&str> = pack
            .set()
            .artifacts
            .iter()
            .map(|artifact| artifact.name.as_str())
            .collect();
        assert_eq!(names, ["libonnxruntime.so", "libsherpa-onnx-c-api.so"]);
        assert!(pack
            .set()
            .artifacts
            .iter()
            .all(|artifact| artifact.url == archive.archive.url && artifact.size > 0));
        assert!(pack.schemes().is_empty());
        for other in ResourcePack::ALL {
            let archived =
                other == pack || (other == ResourcePack::MacosVoiceRuntime && other.pinned());
            assert_eq!(other.archive().is_some(), archived, "{}", other.id());
        }
    }

    /// 合成的 macOS 语音运行库锁：`archive` 为假时是还没发布的样子，否则固定 `zip` 这份归档和其中的 dylib。
    fn macos_lock(archive: Option<(&[u8], &[u8])>) -> VoiceRuntimeMacosLock {
        use sha2::{Digest, Sha256};
        let runtime: Value = serde_json::from_str(VOICE_RUNTIME_LOCK).unwrap();
        let mut lock = serde_json::json!({
            "source_commit": runtime["source_commit"],
            "upstream_platform": "macos",
            "release_tag": "voice-runtime-macos-v0.0.0",
            "team_id": "ABCDE12345",
            "archive": null,
            "artifacts": [],
        });
        if let Some((zip, library)) = archive {
            lock["archive"] = serde_json::json!({
                "name": "runtime.zip",
                "url": format!("{MACOS_VOICE_RUNTIME_RELEASES}voice-runtime-macos-v0.0.0/runtime.zip"),
                "sha256": hex::encode(Sha256::digest(zip)),
                "size": zip.len(),
            });
            lock["artifacts"] = serde_json::json!([{
                "name": MACOS_VOICE_RUNTIME_LIBRARY,
                "member": MACOS_VOICE_RUNTIME_LIBRARY,
                "sha256": hex::encode(Sha256::digest(library)),
                "size": library.len(),
            }]);
        }
        serde_json::from_value(lock).unwrap()
    }

    fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in members {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// 仓库里的 macOS 语音运行库锁能读，来源提交与上游运行库锁一致；还没发布时这个资源包不存在：认不出 id、不列出、装不上。
    #[test]
    fn the_macos_voice_runtime_pack_exists_only_once_its_archive_is_pinned() {
        let lock = macos_voice_runtime_lock();
        let runtime: Value = serde_json::from_str(VOICE_RUNTIME_LOCK).unwrap();
        assert_eq!(
            lock.source_commit,
            runtime["source_commit"].as_str().unwrap()
        );
        assert!(lock.release_tag.starts_with("voice-runtime-macos-v"));
        // 标签不能以 `macos-v` 开头：发布说明和更新检查按这个前缀找 macOS 的应用发布。
        assert!(!lock.release_tag.starts_with("macos-v"));
        let pack = ResourcePack::MacosVoiceRuntime;
        assert_eq!(pack.pinned(), lock.archive.is_some());
        assert!(pack.schemes().is_empty());
        let state = tempfile::tempdir().unwrap();
        let listed = list(state.path())
            .iter()
            .any(|status| status.id == pack.id());
        assert_eq!(listed, pack.pinned());
        assert_eq!(ResourcePack::from_id(pack.id()).is_some(), pack.pinned());
        if !pack.pinned() {
            assert!(pack.archive().is_none() && pack.set().artifacts.is_empty());
            let cancel = AtomicBool::new(false);
            assert!(matches!(
                install(state.path(), pack, &[], &mut |_| {}, &cancel),
                Err(LocalModelError::UnknownModel)
            ));
            assert!(matches!(
                import(state.path(), pack, &[], &mut |_| {}, &cancel),
                Err(LocalModelError::UnknownModel)
            ));
            assert!(matches!(
                adopt(state.path(), pack, state.path()),
                Err(LocalModelError::UnknownModel)
            ));
            assert!(!root(state.path()).exists());
        }
        assert!(pinned_macos_voice_runtime(macos_lock(None)).is_none());
    }

    /// 固定了归档的 macOS 锁：归档地址在本仓库的标签下，取出的只有 dylib 一个文件，URL 记归档的地址、长度和摘要是 dylib 本身的。
    #[test]
    fn a_pinned_macos_voice_runtime_lock_names_one_library_in_its_release_archive() {
        let library = b"signed universal dylib";
        let zip = zip_of(&[(MACOS_VOICE_RUNTIME_LIBRARY, library)]);
        let pinned = pinned_macos_voice_runtime(macos_lock(Some((&zip, library)))).unwrap();
        assert_eq!(
            pinned.archive.members,
            [(
                MACOS_VOICE_RUNTIME_LIBRARY.to_owned(),
                MACOS_VOICE_RUNTIME_LIBRARY.to_owned()
            )]
        );
        assert_eq!(pinned.set.artifacts.len(), 1);
        assert_eq!(pinned.set.artifacts[0].url, pinned.archive.archive.url);
        assert_eq!(pinned.set.artifacts[0].size, library.len() as u64);
        pinned.set.validate().unwrap();

        // 归档不在锁文件自己的标签下：拒绝。
        let mut elsewhere = macos_lock(Some((&zip, library)));
        elsewhere.archive.as_mut().unwrap().url =
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/runtime.zip".into();
        assert!(std::panic::catch_unwind(|| pinned_macos_voice_runtime(elsewhere)).is_err());
        // 有归档却没列文件、或列了别的文件：拒绝。
        let mut empty = macos_lock(Some((&zip, library)));
        empty.artifacts.clear();
        assert!(std::panic::catch_unwind(|| pinned_macos_voice_runtime(empty)).is_err());
        let mut other = macos_lock(Some((&zip, library)));
        other.artifacts[0].name = "libonnxruntime.dylib".into();
        assert!(std::panic::catch_unwind(|| pinned_macos_voice_runtime(other)).is_err());
    }

    /// 下载器按锁文件校验 macOS 语音运行库：归档的摘要、取出的 dylib 的摘要有一处对不上就什么也不发布；对得上时只发布那一个 dylib 和 `msime-model.json`。
    #[test]
    fn the_macos_voice_runtime_installs_only_the_pinned_bytes() {
        let library = b"signed universal dylib";
        let zip = zip_of(&[
            (MACOS_VOICE_RUNTIME_LIBRARY, library.as_slice()),
            ("README.txt", b"not installed"),
        ]);
        let run = |lock: VoiceRuntimeMacosLock, served: &[u8]| {
            let pinned = pinned_macos_voice_runtime(lock).unwrap();
            let picked = tempfile::tempdir().unwrap();
            let file = picked.path().join("downloaded.zip");
            fs::write(&file, served).unwrap();
            let state = tempfile::tempdir().unwrap();
            let cancel = AtomicBool::new(false);
            let fetcher = local_models::LocalFileFetcher::matching_sources(
                [local_models::ImportSource::artifact(
                    &pinned.archive.archive,
                )],
                &[file],
                &cancel,
            );
            let result = fetcher.and_then(|fetcher| {
                local_models::install_archive_members_with(
                    &root(state.path()),
                    "voice-runtime-macos",
                    &pinned.archive.archive,
                    &pinned.archive.members,
                    &pinned.set.artifacts,
                    &serde_json::json!({"pack": "voice-runtime-macos"}),
                    &[],
                    &fetcher,
                    &mut |_| {},
                    &cancel,
                )
            });
            let published = root(state.path()).join("voice-runtime-macos");
            (
                result,
                published.is_dir().then(|| {
                    let mut names: Vec<String> = fs::read_dir(&published)
                        .unwrap()
                        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                        .collect();
                    names.sort();
                    (
                        names,
                        fs::read(published.join(MACOS_VOICE_RUNTIME_LIBRARY)).unwrap(),
                    )
                }),
            )
        };

        let (result, published) = run(macos_lock(Some((&zip, library))), &zip);
        result.unwrap();
        assert_eq!(
            published,
            Some((
                vec![
                    MACOS_VOICE_RUNTIME_LIBRARY.to_owned(),
                    MANIFEST_FILE.to_owned()
                ],
                library.to_vec()
            ))
        );

        // 下到的归档和锁文件的摘要不符（长度相同、内容被改过）。
        let mut tampered = zip.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        let (result, published) = run(macos_lock(Some((&zip, library))), &tampered);
        assert!(
            matches!(result, Err(LocalModelError::ChecksumMismatch(_))),
            "{result:?}"
        );
        assert_eq!(published, None);

        // 归档本身对得上，但锁文件固定的 dylib 摘要不是归档里那一份（例如没重签的上游库）。
        let mut lock = macos_lock(Some((&zip, library)));
        lock.artifacts[0].sha256 = "0".repeat(64);
        let (result, published) = run(lock, &zip);
        assert!(
            matches!(&result, Err(LocalModelError::ChecksumMismatch(name)) if name == MACOS_VOICE_RUNTIME_LIBRARY),
            "{result:?}"
        );
        assert_eq!(published, None);
    }

    /// 已经装好同一组字节时，收编不碰来源目录，直接返回已安装的目录。
    #[test]
    fn adopting_an_installed_pack_leaves_the_source_alone() {
        let state = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let pack = ResourcePack::Japanese;
        let directory = publish_fake(state.path(), pack);
        fs::write(source.path().join("msime-japanese.dat"), b"old copy").unwrap();
        assert_eq!(adopt(state.path(), pack, source.path()).unwrap(), directory);
        assert_eq!(
            fs::read(source.path().join("msime-japanese.dat")).unwrap(),
            b"old copy"
        );
    }

    /// 来源目录里的字节和锁文件对不上：不发布，文件原样放回，调用方改走下载。
    #[test]
    fn adopting_bytes_that_do_not_match_the_lock_publishes_nothing() {
        let state = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let pack = ResourcePack::LanguageDictionaries;
        for artifact in &pack.set().artifacts {
            fs::write(source.path().join(&artifact.name), b"not the pinned bytes").unwrap();
        }
        let error = adopt(state.path(), pack, source.path()).unwrap_err();
        assert!(
            matches!(error, LocalModelError::SizeMismatch(_)),
            "{error:?}"
        );
        for artifact in &pack.set().artifacts {
            assert!(
                source.path().join(&artifact.name).is_file(),
                "{}",
                artifact.name
            );
        }
        assert!(list(state.path())
            .iter()
            .all(|status| status.state == PackState::Missing));
        // 缺一个文件同样失败。
        fs::remove_file(source.path().join("msime-stroke.db")).unwrap();
        assert!(matches!(
            adopt(state.path(), pack, source.path()),
            Err(LocalModelError::MissingFile(name)) if name == "msime-stroke.db"
        ));
    }

    #[test]
    fn import_files_are_the_archive_or_every_pinned_file() {
        let runtime = ResourcePack::VoiceRuntime.import_artifacts();
        assert_eq!(runtime.len(), 1);
        assert_eq!(
            runtime[0].name,
            ResourcePack::VoiceRuntime.archive().unwrap().archive.name
        );
        let dictionaries = ResourcePack::LanguageDictionaries.import_artifacts();
        assert_eq!(
            dictionaries.len(),
            ResourcePack::LanguageDictionaries.set().artifacts.len()
        );
        for pack in ResourcePack::available() {
            let listed = list(Path::new("/nonexistent"))
                .into_iter()
                .find(|status| status.id == pack.id())
                .unwrap();
            assert_eq!(
                listed
                    .import_files
                    .iter()
                    .map(|file| file.size)
                    .sum::<u64>(),
                pack.size()
            );
        }
    }

    #[test]
    fn importing_matches_by_size_and_publishes_nothing_on_wrong_bytes() {
        let state = tempfile::tempdir().unwrap();
        let picked = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        let pack = ResourcePack::LanguageDictionaries;
        let first = pack.import_artifacts()[0].name.clone();
        assert!(matches!(
            import(state.path(), pack, &[], &mut |_| {}, &cancel),
            Err(LocalModelError::MissingImportFile(name)) if name == first
        ));
        // Every file has the pinned length under a different name, but not the pinned bytes.
        let files: Vec<PathBuf> = pack
            .import_artifacts()
            .iter()
            .map(|artifact| {
                let path = picked.path().join(format!("download-{}", artifact.name));
                fs::File::create(&path)
                    .unwrap()
                    .set_len(artifact.size)
                    .unwrap();
                path
            })
            .collect();
        let mut stages = Vec::new();
        let error = import(
            state.path(),
            pack,
            &files,
            &mut |event| stages.push(event.stage),
            &cancel,
        )
        .unwrap_err();
        assert!(
            matches!(error, LocalModelError::ChecksumMismatch(_)),
            "{error:?}"
        );
        assert!(
            stages.contains(&"import") && !stages.contains(&"download"),
            "{stages:?}"
        );
        assert!(list(state.path())
            .iter()
            .all(|status| status.state == PackState::Missing));
        // The archive pack names its archive when nothing matches.
        let archive = ResourcePack::VoiceRuntime
            .archive()
            .unwrap()
            .archive
            .name
            .clone();
        assert!(matches!(
            import(state.path(), ResourcePack::VoiceRuntime, &files, &mut |_| {}, &cancel),
            Err(LocalModelError::MissingImportFile(name)) if name == archive
        ));
    }

    #[cfg(unix)]
    #[test]
    fn adoption_rejects_a_source_below_a_symlinked_ancestor() {
        use msime_path_trust::untrusted_symlink as symlink;
        use sha2::{Digest, Sha256};

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let source_real = outside.path().join("source");
        fs::create_dir(&source_real).unwrap();
        let bytes = b"synthetic model";
        fs::write(source_real.join("fixture.bin"), bytes).unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let source = linked.join("source");
        let file = crate::resources::Artifact {
            name: "fixture.bin".into(),
            url: "https://example.invalid/fixture.bin".into(),
            sha256: hex::encode(Sha256::digest(bytes)),
            size: bytes.len() as u64,
        };

        let result = crate::voice::local_models::adopt_files(
            &root.path().join("models"),
            "fixture",
            &[file],
            &serde_json::json!({}),
            &source,
        );

        assert!(matches!(
            result,
            Err(crate::voice::local_models::LocalModelError::InvalidRoot)
        ));
        assert_eq!(fs::read(source_real.join("fixture.bin")).unwrap(), bytes);
    }

    #[test]
    fn ids_round_trip() {
        for pack in ResourcePack::available() {
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
        assert_eq!(statuses.len(), ResourcePack::available().count());
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

    /// 用户镜像排在最前（空串跳过），然后是项目镜像；锁文件原地址由下载器最后补上。项目镜像必须是和用户镜像一样合法的 https 前缀。
    #[test]
    fn user_mirrors_come_before_project_mirrors() {
        let mut expected = vec!["https://first.example.test/", "https://second.example.test"];
        expected.extend(PROJECT_MIRRORS.iter().copied());
        assert_eq!(
            download_prefixes(&[
                "",
                "https://first.example.test/",
                "https://second.example.test"
            ]),
            expected
        );
        assert_eq!(download_prefixes(&[]), PROJECT_MIRRORS);
        assert!(PROJECT_MIRRORS
            .iter()
            .all(|mirror| !mirror.is_empty() && crate::preferences::valid_model_mirror(mirror)));
    }

    /// 资源包只从固定的发布地址下载：本项目 msime-dictionary 和 chinese-ime-lm 的 GitHub Release 资产、钉在 40 位提交上的 msime-engine 原始文件，或上游 sherpa-onnx 带版本号的发布资产。
    #[test]
    fn every_url_is_immutable() {
        const RELEASE: &str =
            "https://github.com/metasequoiaime/msime-dictionary/releases/download/";
        const MODEL_RELEASE: &str =
            "https://github.com/metasequoiaime/chinese-ime-lm/releases/download/";
        const ENGINE: &str = "https://raw.githubusercontent.com/metasequoiaime/msime-engine/";
        // 语音运行库来自上游 sherpa-onnx 带版本号的发布资产，字节由锁文件的 SHA-256 固定。
        const SHERPA_RELEASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v";
        // macOS 的语音运行库是重签后发布在本仓库固定标签下的资产，标签下的资产发布后不再替换。
        const MACOS_RUNTIME_RELEASE: &str =
            "https://github.com/metasequoiaime/msime/releases/download/voice-runtime-macos-v";
        // 语法模型的上游发布：万象（amzxyz）、八股文（lotem）。字节由锁文件的 SHA-256 固定。
        const GRAMMAR_UPSTREAM_RELEASES: [&str; 2] = [
            "https://github.com/amzxyz/rime-wanxiang-data/releases/download/",
            "https://github.com/lotem/rime-octagram-data/releases/download/",
        ];
        for pack in ResourcePack::ALL {
            for artifact in &pack.set().artifacts {
                let url = artifact.url.as_str();
                let pinned_engine = url.strip_prefix(ENGINE).is_some_and(|rest| {
                    rest.split_once('/')
                        .is_some_and(|(commit, _)| crate::is_lower_hex(commit, 40))
                });
                // 语法模型来自三个上游仓库，都不由本项目发版：八股文与万象是上游带标签的
                // Release 资产，白霜/墨奇是钉在 40 位提交上的原始文件。上游可以替换自己
                // 标签下的资产，所以这里的真正不变量是锁文件里的 SHA-256，不是地址。
                let upstream_release = GRAMMAR_UPSTREAM_RELEASES
                    .iter()
                    .any(|prefix| url.starts_with(prefix));
                // raw.githubusercontent.com/<owner>/<repo>/<40 位提交>/<文件>：第三段必须是提交。
                let pinned_raw = url
                    .strip_prefix("https://raw.githubusercontent.com/")
                    .is_some_and(|rest| {
                        rest.split('/')
                            .nth(2)
                            .is_some_and(|commit| crate::is_lower_hex(commit, 40))
                    });
                assert!(
                    url.starts_with(RELEASE)
                        || url.starts_with(MODEL_RELEASE)
                        || url.starts_with(SHERPA_RELEASE)
                        || url.starts_with(MACOS_RUNTIME_RELEASE)
                        || pinned_engine
                        || upstream_release
                        || pinned_raw,
                    "{url}"
                );
            }
        }
    }
}
