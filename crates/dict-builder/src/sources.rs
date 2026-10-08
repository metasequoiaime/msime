//! 构建输入的固定记录（`resources/dictionary-sources.lock.json`）：每个文件从哪里来、必须有怎样的 SHA-256。大文件和第三方输入按需下载到缓存目录；没有固定记录的文件一律不下载，缓存里的文件只在大小和摘要仍然一致时复用。msime-dictionary 的 `sources/` 和 `custom/` 不在锁文件里，只从 `--dictionary` checkout 读取；其中的上游数据按该 checkout 的 `upstream.lock.json` 校验。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Wikimedia refuses anonymous user agents with a 403 that looks like a deleted dump.
const USER_AGENT: &str = "msime-dictionary-build/1.0 (https://github.com/metasequoiaime/msime)";

#[derive(Debug, Deserialize)]
pub struct Lock {
    pub references: BTreeMap<String, Reference>,
    pub mozc: Reference,
    pub files: Vec<PinnedFile>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Reference {
    pub repository: String,
    pub commit: String,
}

#[derive(Debug, Deserialize)]
pub struct PinnedFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl Lock {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            crate::text::read(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    fn file(&self, path: &str) -> Result<&PinnedFile> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{path} is not pinned in the sources lock"))
    }
}

/// 构建器读取的 msime-dictionary 顶层目录；其下的路径只从 `--dictionary` checkout 读取，锁文件不固定其中任何文件。
const DICTIONARY_DIRECTORIES: [&str; 2] = ["sources/", "custom/"];
/// msime 认定为上游数据的 msime-dictionary 文件：原样复制自某个上游、或由 msime 的生成器从某个上游生成（`hkcancor-counts` 来自 HKCanCor，`english-supplement` 来自 SCOWL），写成精确路径或目录前缀，并配上上游名。manifest 的 references 和 `mozc_revision`、语言词库的 `source_commit` 以及随包许可证都写着这些上游的提交，所以 checkout 的 `upstream.lock.json` 必须用同一个上游名列出这些路径，并固定它们的大小和 SHA-256；记录里每个上游的提交必须等于锁文件的同名 reference（`mozc` 对应 `lock.mozc`）。要换成新版上游，先在 msime 改 reference 和 `resources/licenses`，再改 `upstream.lock.json`。
const UPSTREAM_FILES: [(&str, &str); 15] = [
    ("sources/pinyin/rime-ice.txt", "rime-ice"),
    (
        "sources/pinyin/rime-ice-supplement.txt",
        "rime-ice-supplement",
    ),
    (
        "sources/english/rime-ice-en-supplement.txt",
        "rime-ice-supplement",
    ),
    (crate::hkcancor::OUTPUT, crate::hkcancor::REFERENCE),
    ("sources/cantonese/", "rime-cantonese"),
    (
        crate::english_supplement::OUTPUT,
        crate::english_supplement::REFERENCE,
    ),
    ("sources/zhuyin/tsi.csv", "libchewing-data"),
    ("sources/zhuyin/word.csv", "libchewing-data"),
    ("sources/zhuyin/mcbopomofo-supplement.txt", "McBopomofo"),
    ("sources/zhuyin/phrase.occ", "McBopomofo"),
    ("sources/stroke/", "rime-stroke"),
    ("sources/japanese/", "mozc"),
    ("sources/korean/", "libhangul"),
    ("sources/wubi/wubi98.txt", "98wubi-tables"),
    ("sources/wubi/wubi98-fcitx.txt", "fcitx5-table-extra"),
];

/// The upstream reference `path` is a copy of or is generated from at a recorded commit, if any. The first matching entry of `UPSTREAM_FILES` wins, so an exact path listed before its directory's prefix (HKCanCor's counts beside rime-cantonese's files) names its own upstream.
pub(crate) fn upstream_reference(path: &str) -> Option<&'static str> {
    UPSTREAM_FILES
        .iter()
        .find(|(upstream, _)| {
            if upstream.ends_with('/') {
                path.starts_with(upstream)
            } else {
                path == *upstream
            }
        })
        .map(|(_, reference)| *reference)
}

/// msime-dictionary checkout 根目录下的上游记录文件。
pub const UPSTREAM_LOCK: &str = "upstream.lock.json";

/// `upstream.lock.json` 的内容：上游名到 repository 与 commit，以及上游数据文件的大小和 SHA-256。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamLock {
    version: u32,
    upstreams: BTreeMap<String, Reference>,
    files: Vec<UpstreamFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamFile {
    path: String,
    upstream: String,
    size: u64,
    sha256: String,
}

/// 构建读取的 msime-dictionary checkout（`--dictionary`），以及它的 `upstream.lock.json`；`open` 已经证明这份记录与锁文件一致。
pub struct Dictionary {
    pub root: PathBuf,
    upstream: UpstreamLock,
}

fn is_lowercase_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl Dictionary {
    /// 读取 `root/upstream.lock.json` 并与锁文件核对：版本必须是 1；每个上游的提交是 40 位小写十六进制，repository 和 commit 等于锁文件的同名 reference（`mozc` 对应 `lock.mozc`）；每个文件条目的上游名等于 msime 按 `UPSTREAM_FILES` 给它的上游名，并在 upstreams 里有记录；路径不重复。任何一项不符都报错。
    pub fn open(root: PathBuf, lock: &Lock) -> Result<Self> {
        let path = dictionary_checkout_path(&root, UPSTREAM_LOCK)?;
        if !path.is_file() {
            bail!(
                "{} has no {UPSTREAM_LOCK}; msime's builder needs msime-dictionary at or after the commit that added it",
                root.display()
            );
        }
        let text =
            crate::text::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let upstream: UpstreamLock =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if upstream.version != 1 {
            bail!(
                "{}: version {} is not 1, the only version this builder reads",
                path.display(),
                upstream.version
            );
        }
        for (name, recorded) in &upstream.upstreams {
            if !is_lowercase_hex(&recorded.commit, 40) {
                bail!(
                    "{}: the commit of {name}, {:?}, is not 40 lowercase hex digits",
                    path.display(),
                    recorded.commit
                );
            }
            let expected = if name == "mozc" {
                Some(&lock.mozc)
            } else {
                lock.references.get(name)
            };
            let Some(expected) = expected else {
                bail!("{name} is recorded in {UPSTREAM_LOCK} but resources/dictionary-sources.lock.json has no such reference; add it, with the licence texts that name it, in msime first");
            };
            if recorded.repository != expected.repository || recorded.commit != expected.commit {
                bail!(
                    "{name} is recorded in {UPSTREAM_LOCK} as {} at {}, but resources/dictionary-sources.lock.json has {} at {}; change the reference (the Mozc revision for mozc) and the commits resources/licenses names in msime first, then {UPSTREAM_LOCK}",
                    recorded.repository,
                    recorded.commit,
                    expected.repository,
                    expected.commit
                );
            }
        }
        let mut seen = std::collections::HashSet::new();
        for file in &upstream.files {
            if !seen.insert(file.path.as_str()) {
                bail!("{} is listed twice in {UPSTREAM_LOCK}", file.path);
            }
            let label = upstream_reference(&file.path);
            if label != Some(file.upstream.as_str()) {
                bail!(
                    "{} is listed under {} in {UPSTREAM_LOCK}, but msime treats it as {}",
                    file.path,
                    file.upstream,
                    label.unwrap_or("not upstream data")
                );
            }
            if !upstream.upstreams.contains_key(&file.upstream) {
                bail!(
                    "{} is listed under {}, which {UPSTREAM_LOCK} does not record among its upstreams",
                    file.path,
                    file.upstream
                );
            }
            if !is_lowercase_hex(&file.sha256, 64) {
                bail!(
                    "{}: the SHA-256 {UPSTREAM_LOCK} records, {:?}, is not 64 lowercase hex digits",
                    file.path,
                    file.sha256
                );
            }
        }
        Ok(Self { root, upstream })
    }

    fn file(&self, path: &str) -> Option<&UpstreamFile> {
        self.upstream.files.iter().find(|file| file.path == path)
    }
}

/// `path` 是否属于 msime-dictionary（在 `DICTIONARY_DIRECTORIES` 之下）。
fn is_dictionary_path(path: &str) -> bool {
    DICTIONARY_DIRECTORIES
        .iter()
        .any(|directory| path.starts_with(directory))
}

/// Resolves the inputs a stage reads: hand-maintained files from the repository, pinned files from the cache (downloading them unless offline).
pub struct Sources {
    pub lock: Lock,
    pub repository_inputs: PathBuf,
    pub cache: PathBuf,
    pub offline: bool,
    /// msime-dictionary checkout，`sources/` 和 `custom/` 只从这里读；内容由它的 Git 提交固定，上游数据另按它的 `upstream.lock.json` 校验。
    pub dictionary: Option<Dictionary>,
}

impl Sources {
    /// A hand-maintained input kept in the repository.
    pub fn repository(&self, path: &str) -> Result<PathBuf> {
        let resolved = self.repository_inputs.join(path);
        if !resolved.is_file() {
            bail!("missing repository input {}", resolved.display());
        }
        Ok(resolved)
    }

    /// `path` 在 `--dictionary` checkout 里的位置；没有 checkout，或 `path` 不属于 msime-dictionary 时为 `None`。
    pub fn checkout_file(&self, path: &str) -> Option<PathBuf> {
        self.dictionary
            .as_ref()
            .filter(|_| is_dictionary_path(path))
            .map(|dictionary| dictionary.root.join(path))
    }

    /// 解析一个输入：msime-dictionary 的路径只从 `--dictionary` checkout 读，其中的上游数据按 checkout 的 `upstream.lock.json` 校验；其他路径按锁文件校验并缓存，必要时下载。
    pub fn pinned(&self, path: &str) -> Result<PathBuf> {
        // 这个分支在查锁文件之前：即使锁文件里重新出现 `sources/` 或 `custom/` 的条目，也不会被读到。
        if is_dictionary_path(path) {
            let Some(dictionary) = self.dictionary.as_ref() else {
                bail!("{path} is msime-dictionary data, which the sources lock no longer pins; pass --dictionary <msime-dictionary checkout>");
            };
            let resolved = dictionary_checkout_path(&dictionary.root, path)?;
            if !resolved.is_file() {
                bail!(
                    "{path} is not in the dictionary checkout at {}",
                    resolved.display()
                );
            }
            if let Some(label) = upstream_reference(path) {
                let file = dictionary.file(path).with_context(|| {
                    format!("{path} is {label} data at an upstream commit msime records, but the checkout's {UPSTREAM_LOCK} does not list it")
                })?;
                if !matches(&resolved, file.size, &file.sha256)? {
                    bail!(
                        "{path} in the dictionary checkout at {} differs from the size and SHA-256 the checkout's {UPSTREAM_LOCK} records; it is {label} data at an upstream commit msime records, so to replace it change the {label} reference (the Mozc revision for mozc) and the licence texts in resources/licenses in msime first, then {UPSTREAM_LOCK}",
                        resolved.display()
                    );
                }
            }
            return Ok(resolved);
        }
        let file = self.lock.file(path)?;
        let target = self.cache.join(&file.path);
        if target.is_file() && matches(&target, file.size, &file.sha256)? {
            return Ok(target);
        }
        if self.offline {
            bail!(
                "{} is not cached at {} and --offline was given",
                file.path,
                target.display()
            );
        }
        download(file, &target)?;
        Ok(target)
    }
}

fn dictionary_checkout_path(root: &Path, path: &str) -> Result<PathBuf> {
    let relative = Path::new(path);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("{path} is not a relative dictionary source path");
    }
    let mut resolved = root.to_path_buf();
    for component in relative.components() {
        resolved.push(component);
        match std::fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("{} is a symbolic link", resolved.display());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(resolved)
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut stream = open_private(path).with_context(|| format!("opening {}", path.display()))?;
    sha256_reader(&mut stream)
}

fn open_private(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "dictionary input is not a regular file",
        ));
    }
    Ok(file)
}

pub(crate) fn read_private(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = open_private(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn sha256_reader(stream: &mut File) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn matches(path: &Path, size: u64, sha256: &str) -> Result<bool> {
    let mut stream = open_private(path)?;
    if stream.metadata()?.len() != size {
        return Ok(false);
    }
    Ok(sha256_reader(&mut stream)? == sha256)
}

fn download(file: &PinnedFile, target: &Path) -> Result<()> {
    eprintln!("[fetch] {} <- {}", file.path, file.url);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(3600))
        .build()?;
    let mut response = client
        .get(&file.url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .with_context(|| format!("downloading {}", file.url))?;
    let incoming = target.with_extension("incoming");
    write_pinned_response(&mut response, &incoming, file)?;
    std::fs::rename(&incoming, target)?;
    Ok(())
}

fn write_pinned_response<R: Read>(
    mut response: R,
    incoming: &Path,
    file: &PinnedFile,
) -> Result<()> {
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let result = (|| {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(incoming)?;
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            if (read as u64) > file.size.saturating_sub(written) {
                bail!(
                    "{}: response exceeds pinned size of {} bytes",
                    file.url,
                    file.size
                );
            }
            hasher.update(&buffer[..read]);
            output.write_all(&buffer[..read])?;
            written += read as u64;
        }
        output.sync_all()?;
        let digest = hex::encode(hasher.finalize());
        if written != file.size || digest != file.sha256 {
            bail!(
                "{}: got {written} bytes with sha256 {digest}, the lock pins {} bytes with {}",
                file.url,
                file.size,
                file.sha256
            );
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(incoming);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    /// 锁文件曾经固定 msime-dictionary 文件时用的 raw URL 前缀：`<RAW><commit>/<path>`。
    const DICTIONARY_RAW: &str =
        "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/";
    const CANTONESE_REPOSITORY: &str = "https://github.com/rime/rime-cantonese.git";
    const CHEWING_REPOSITORY: &str = "https://github.com/chewing/libchewing-data.git";
    const MOZC_REPOSITORY: &str = "https://github.com/google/mozc.git";

    fn lock_with(file: PinnedFile) -> Lock {
        Lock {
            references: BTreeMap::new(),
            mozc: Reference {
                repository: String::new(),
                commit: String::new(),
            },
            files: vec![file],
        }
    }

    #[cfg(unix)]
    #[test]
    fn lock_load_rejects_a_fifo_without_blocking() {
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sources.lock.json");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let (done, result) = mpsc::channel();
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || {
            done.send(Lock::load(&worker_path).is_err()).unwrap();
        });
        assert!(
            result.recv_timeout(Duration::from_secs(1)).unwrap(),
            "FIFO lock input must be rejected without blocking"
        );
        worker.join().unwrap();
    }

    /// 带指定 references 和 Mozc 修订的锁文件，不固定任何文件。
    fn lock_with_references(references: &[(&str, &str, String)], mozc: (&str, String)) -> Lock {
        Lock {
            references: references
                .iter()
                .map(|(name, repository, commit)| {
                    (
                        (*name).to_owned(),
                        Reference {
                            repository: (*repository).to_owned(),
                            commit: commit.clone(),
                        },
                    )
                })
                .collect(),
            mozc: Reference {
                repository: mozc.0.to_owned(),
                commit: mozc.1,
            },
            files: Vec::new(),
        }
    }

    /// 只有 rime-cantonese 一个 reference 的锁文件，以及与它一致的 upstreams。
    fn cantonese_lock() -> (Lock, serde_json::Value) {
        let lock = lock_with_references(
            &[("rime-cantonese", CANTONESE_REPOSITORY, "a".repeat(40))],
            (MOZC_REPOSITORY, "b".repeat(40)),
        );
        let upstreams = json!({
            "rime-cantonese": {"repository": CANTONESE_REPOSITORY, "commit": "a".repeat(40)}
        });
        (lock, upstreams)
    }

    /// 在临时目录里写出 checkout 的文件和 `upstream.lock.json`。
    fn checkout(files: &[(&str, &[u8])], record: serde_json::Value) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let target = directory.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        std::fs::write(
            directory.path().join(UPSTREAM_LOCK),
            serde_json::to_string_pretty(&record).unwrap(),
        )
        .unwrap();
        directory
    }

    fn dictionary_at(checkout: &tempfile::TempDir, lock: &Lock) -> Dictionary {
        Dictionary::open(checkout.path().into(), lock).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_parent_link_cannot_escape_the_checkout() {
        use std::os::unix::fs::symlink;

        let checkout = checkout(&[], json!({"version": 1, "upstreams": {}, "files": []}));
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join("custom")).unwrap();
        std::fs::write(outside.path().join("words.txt"), b"synthetic outside words").unwrap();
        symlink(outside.path(), checkout.path().join("custom/linked")).unwrap();
        let lock = lock_with_references(&[], ("", String::new()));
        let sources = Sources {
            dictionary: Some(dictionary_at(&checkout, &lock)),
            lock,
            repository_inputs: checkout.path().into(),
            cache: checkout.path().into(),
            offline: true,
        };

        assert!(sources.pinned("custom/linked/words.txt").is_err());
        assert!(sources.pinned("custom/../../outside.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_cannot_read_a_linked_upstream_lock() {
        use std::os::unix::fs::symlink;

        let checkout = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let lock_file = outside.path().join(UPSTREAM_LOCK);
        std::fs::write(&lock_file, br#"{"version":1,"upstreams":{},"files":[]}"#).unwrap();
        symlink(&lock_file, checkout.path().join(UPSTREAM_LOCK)).unwrap();
        let lock = lock_with_references(&[], ("", String::new()));

        assert!(Dictionary::open(checkout.path().to_path_buf(), &lock).is_err());
    }

    #[test]
    fn a_cached_file_is_used_only_while_it_matches_its_pin() {
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(cache.path().join("cn")).unwrap();
        std::fs::write(cache.path().join("cn/a.txt"), b"fixture").unwrap();
        let pinned = |sha256: String| PinnedFile {
            path: "cn/a.txt".into(),
            url: "http://127.0.0.1:9/unreachable".into(),
            sha256,
            size: 7,
        };
        let good = Sources {
            lock: lock_with(pinned(hex::encode(Sha256::digest(b"fixture")))),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: None,
        };
        assert_eq!(
            good.pinned("cn/a.txt").unwrap(),
            cache.path().join("cn/a.txt")
        );
        assert!(good.pinned("cn/b.txt").is_err());

        let stale = Sources {
            lock: lock_with(pinned("0".repeat(64))),
            ..good
        };
        let error = stale.pinned("cn/a.txt").unwrap_err().to_string();
        assert!(error.contains("--offline"), "{error}");
    }

    /// 有 `--dictionary` 时，msime-dictionary 的路径只从 checkout 读：锁文件里即使还留着一条摘要不符的 `custom/words.txt` 也不看，也不下载；其他仓库的文件照常走缓存。
    #[test]
    fn a_checkout_serves_dictionary_paths_and_other_inputs_use_the_cache() {
        let cache = tempfile::tempdir().unwrap();
        let mut lock = lock_with(PinnedFile {
            path: "custom/words.txt".into(),
            url: format!("{DICTIONARY_RAW}{}/custom/words.txt", "a".repeat(40)),
            sha256: "0".repeat(64),
            size: 1,
        });
        lock.files.push(PinnedFile {
            path: "ecdict/ecdict.csv".into(),
            url: "http://127.0.0.1:9/unreachable".into(),
            sha256: "0".repeat(64),
            size: 1,
        });
        let checkout = checkout(
            &[("custom/words.txt", b"edited")],
            json!({"version": 1, "upstreams": {}, "files": []}),
        );
        let sources = Sources {
            dictionary: Some(dictionary_at(&checkout, &lock)),
            lock,
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
        };
        assert_eq!(
            sources.pinned("custom/words.txt").unwrap(),
            checkout.path().join("custom/words.txt")
        );
        assert!(!cache.path().join("custom/words.txt").exists());
        let error = sources.pinned("ecdict/ecdict.csv").unwrap_err().to_string();
        assert!(error.contains("--offline"), "{error}");
        assert_eq!(sources.checkout_file("ecdict/ecdict.csv"), None);
        assert_eq!(
            sources.checkout_file("custom/words.txt"),
            Some(checkout.path().join("custom/words.txt"))
        );
    }

    /// 有 `--dictionary` 时，`sources/` 或 `custom/` 下的新文件不需要任何固定记录就从 checkout 读；checkout 里缺的文件直接报错，不退回缓存。
    #[test]
    fn a_checkout_resolves_dictionary_paths_the_lock_does_not_pin() {
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(cache.path().join("custom")).unwrap();
        std::fs::write(cache.path().join("custom/words.txt"), b"w").unwrap();
        let lock = lock_with(PinnedFile {
            path: "custom/words.txt".into(),
            url: format!("{DICTIONARY_RAW}{}/custom/words.txt", "a".repeat(40)),
            sha256: hex::encode(Sha256::digest(b"w")),
            size: 1,
        });
        let checkout = checkout(
            &[("sources/pinyin/new.txt", b"new")],
            json!({"version": 1, "upstreams": {}, "files": []}),
        );
        let sources = Sources {
            dictionary: Some(dictionary_at(&checkout, &lock)),
            lock,
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
        };
        assert_eq!(
            sources.pinned("sources/pinyin/new.txt").unwrap(),
            checkout.path().join("sources/pinyin/new.txt")
        );
        let error = sources
            .pinned("sources/pinyin/gone.txt")
            .unwrap_err()
            .to_string();
        assert!(error.contains("not in the dictionary checkout"), "{error}");
        let error = sources.pinned("custom/words.txt").unwrap_err().to_string();
        assert!(error.contains("not in the dictionary checkout"), "{error}");
        let error = sources.pinned("places/areas.csv").unwrap_err().to_string();
        assert!(error.contains("not pinned in the sources lock"), "{error}");
    }

    /// 有 `--dictionary` 时，上游数据（这里是 rime-cantonese 的）只在与 checkout 的 `upstream.lock.json` 一致时才读：记录没有列出的新文件、或字节与记录不符的文件都报错，而不是顶着旧的上游提交发布。
    #[test]
    fn a_checkout_cannot_change_upstream_data_its_record_pins() {
        let cache = tempfile::tempdir().unwrap();
        let (lock, upstreams) = cantonese_lock();
        let files: &[(&str, &[u8])] = &[
            ("sources/cantonese/essay-cantonese.txt", b"e"),
            ("sources/cantonese/new.txt", b"n"),
        ];
        let record = |sha256: String| {
            json!({
                "version": 1,
                "upstreams": upstreams.clone(),
                "files": [{
                    "path": "sources/cantonese/essay-cantonese.txt",
                    "upstream": "rime-cantonese",
                    "size": 1,
                    "sha256": sha256,
                }],
            })
        };
        let good = checkout(files, record(hex::encode(Sha256::digest(b"e"))));
        let mut sources = Sources {
            dictionary: Some(dictionary_at(&good, &lock)),
            lock,
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
        };
        assert_eq!(
            sources
                .pinned("sources/cantonese/essay-cantonese.txt")
                .unwrap(),
            good.path().join("sources/cantonese/essay-cantonese.txt")
        );
        let error = format!(
            "{:#}",
            sources.pinned("sources/cantonese/new.txt").unwrap_err()
        );
        assert!(error.contains("does not list it"), "{error}");

        let stale = checkout(files, record("0".repeat(64)));
        sources.dictionary = Some(dictionary_at(&stale, &sources.lock));
        let error = sources
            .pinned("sources/cantonese/essay-cantonese.txt")
            .unwrap_err()
            .to_string();
        assert!(error.contains("rime-cantonese"), "{error}");
        assert!(error.contains(UPSTREAM_LOCK), "{error}");
        assert!(!cache.path().join("sources/cantonese").exists());
        assert_eq!(upstream_reference("sources/pinyin/places.txt"), None);
        assert_eq!(upstream_reference("custom/words.txt"), None);
        assert_eq!(upstream_reference("sources/japanese/id.def"), Some("mozc"));
        assert_eq!(upstream_reference("sources/japanese/LICENSE"), Some("mozc"));
        assert_eq!(
            upstream_reference("sources/zhuyin/phrase.occ"),
            Some("McBopomofo")
        );
        assert_eq!(
            upstream_reference("sources/cantonese/hkcancor-word-counts.txt"),
            Some("hkcancor")
        );
        assert_eq!(
            upstream_reference("sources/cantonese/essay-cantonese.txt"),
            Some("rime-cantonese")
        );
        assert_eq!(
            upstream_reference("sources/english/scowl-words.txt"),
            Some("SCOWL")
        );
    }

    /// 没有 `--dictionary` 时，`sources/` 和 `custom/` 下的路径一律报错并提示传 `--dictionary`，即使锁文件固定了它、缓存里也有这个文件。
    #[test]
    fn without_a_checkout_dictionary_paths_are_rejected() {
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(cache.path().join("custom")).unwrap();
        std::fs::write(cache.path().join("custom/words.txt"), b"w").unwrap();
        std::fs::create_dir_all(cache.path().join("sources/pinyin")).unwrap();
        std::fs::write(cache.path().join("sources/pinyin/new.txt"), b"new").unwrap();
        let sources = Sources {
            lock: lock_with(PinnedFile {
                path: "custom/words.txt".into(),
                url: format!("{DICTIONARY_RAW}{}/custom/words.txt", "a".repeat(40)),
                sha256: hex::encode(Sha256::digest(b"w")),
                size: 1,
            }),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: None,
        };
        assert_eq!(sources.checkout_file("sources/pinyin/new.txt"), None);
        assert_eq!(sources.checkout_file("custom/words.txt"), None);
        for path in ["custom/words.txt", "sources/pinyin/new.txt"] {
            let error = sources.pinned(path).unwrap_err().to_string();
            assert!(error.contains("--dictionary"), "{error}");
        }
    }

    /// `Dictionary::open` 拒绝与锁文件或 `UPSTREAM_FILES` 不一致的记录。
    #[test]
    fn a_record_must_agree_with_the_lock() {
        let lock = lock_with_references(
            &[
                ("rime-cantonese", CANTONESE_REPOSITORY, "a".repeat(40)),
                ("libchewing-data", CHEWING_REPOSITORY, "c".repeat(40)),
            ],
            (MOZC_REPOSITORY, "b".repeat(40)),
        );
        let good = || {
            json!({
                "version": 1,
                "upstreams": {
                    "libchewing-data": {"repository": CHEWING_REPOSITORY, "commit": "c".repeat(40)},
                    "mozc": {"repository": MOZC_REPOSITORY, "commit": "b".repeat(40)},
                    "rime-cantonese": {"repository": CANTONESE_REPOSITORY, "commit": "a".repeat(40)},
                },
                "files": [
                    {"path": "sources/cantonese/essay-cantonese.txt", "upstream": "rime-cantonese", "size": 1, "sha256": "e".repeat(64)},
                    {"path": "sources/japanese/id.def", "upstream": "mozc", "size": 2, "sha256": "d".repeat(64)},
                    {"path": "sources/zhuyin/tsi.csv", "upstream": "libchewing-data", "size": 3, "sha256": "f".repeat(64)},
                ],
            })
        };
        let open = |record: serde_json::Value| {
            let directory = checkout(&[], record);
            Dictionary::open(directory.path().into(), &lock).map(|_| ())
        };
        open(good()).unwrap();

        let missing = tempfile::tempdir().unwrap();
        let error = Dictionary::open(missing.path().into(), &lock)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("has no upstream.lock.json"), "{error}");

        // 每一项是预期报错里的一段文字，以及把正例改坏的方法。
        type Mutation = fn(&mut serde_json::Value);
        let cases: Vec<(&str, Mutation)> = vec![
            ("version 2 is not 1", |record| record["version"] = json!(2)),
            (
                "resources/dictionary-sources.lock.json has https://github.com/rime/rime-cantonese.git at aaaa",
                |record| {
                    record["upstreams"]["rime-cantonese"]["commit"] = json!("9".repeat(40));
                },
            ),
            (
                "rime-cantonese-2 is recorded in upstream.lock.json but resources/dictionary-sources.lock.json has no such reference",
                |record| {
                    let reference = record["upstreams"]["rime-cantonese"].take();
                    let upstreams = record["upstreams"].as_object_mut().unwrap();
                    upstreams.remove("rime-cantonese");
                    upstreams.insert("rime-cantonese-2".into(), reference);
                    record["files"][0]["upstream"] = json!("rime-cantonese-2");
                },
            ),
            (
                "mozc is recorded in upstream.lock.json as https://github.com/google/mozc.git at 9999",
                |record| {
                    record["upstreams"]["mozc"]["commit"] = json!("9".repeat(40));
                },
            ),
            (
                "sources/zhuyin/tsi.csv is listed under McBopomofo in upstream.lock.json, but msime treats it as libchewing-data",
                |record| record["files"][2]["upstream"] = json!("McBopomofo"),
            ),
            (
                "custom/words.txt is listed under rime-cantonese in upstream.lock.json, but msime treats it as not upstream data",
                |record| {
                    let mut file = record["files"][0].clone();
                    file["path"] = json!("custom/words.txt");
                    record["files"].as_array_mut().unwrap().push(file);
                },
            ),
            (
                "sources/japanese/id.def is listed twice",
                |record| {
                    let file = record["files"][1].clone();
                    record["files"].as_array_mut().unwrap().push(file);
                },
            ),
            (
                "is not 40 lowercase hex digits",
                |record| {
                    record["upstreams"]["mozc"]["commit"] = json!("B".repeat(40));
                },
            ),
            (
                "sources/zhuyin/tsi.csv is listed under libchewing-data, which upstream.lock.json does not record",
                |record| {
                    record["upstreams"]
                        .as_object_mut()
                        .unwrap()
                        .remove("libchewing-data");
                },
            ),
            (
                "is not 64 lowercase hex digits",
                |record| record["files"][0]["sha256"] = json!("e".repeat(63)),
            ),
            (
                "unknown field",
                |record| record["files"][0]["url"] = json!("https://example.invalid/"),
            ),
        ];
        for (expected, mutate) in cases {
            let mut record = good();
            mutate(&mut record);
            let error = format!("{:#}", open(record).unwrap_err());
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn an_oversized_response_is_rejected_before_it_reaches_disk() {
        let directory = tempfile::tempdir().unwrap();
        let incoming = directory.path().join("fixture.incoming");
        let file = PinnedFile {
            path: "fixture.txt".into(),
            url: "https://synthetic.invalid/fixture.txt".into(),
            sha256: hex::encode(Sha256::digest(b"123")),
            size: 3,
        };

        let error = write_pinned_response(Cursor::new(b"12345"), &incoming, &file).unwrap_err();

        assert!(error.to_string().contains("exceeds pinned size"));
        assert!(!incoming.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_cached_symlink_is_not_used_even_when_its_target_matches() {
        use std::os::unix::fs::symlink;

        let cache = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("fixture.txt");
        std::fs::write(&target, b"fixture").unwrap();
        let cached = cache.path().join("fixture.txt");
        symlink(&target, &cached).unwrap();
        let file = PinnedFile {
            path: "fixture.txt".into(),
            url: "https://synthetic.invalid/fixture.txt".into(),
            sha256: hex::encode(Sha256::digest(b"fixture")),
            size: 7,
        };
        let sources = Sources {
            lock: lock_with(file),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: None,
        };

        assert!(sources.pinned("fixture.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn an_incoming_symlink_is_not_truncated_by_a_download() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.txt");
        std::fs::write(&target, b"keep").unwrap();
        let incoming = directory.path().join("fixture.incoming");
        symlink(&target, &incoming).unwrap();
        let file = PinnedFile {
            path: "fixture.txt".into(),
            url: "https://synthetic.invalid/fixture.txt".into(),
            sha256: hex::encode(Sha256::digest(b"fixture")),
            size: 7,
        };

        assert!(write_pinned_response(Cursor::new(b"fixture"), &incoming, &file).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
    }

    fn repository_lock() -> Lock {
        Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap()
    }

    /// 锁文件每个路径只固定一次，并且不再固定 msime-dictionary 的任何文件，也没有 `msime-dictionary` reference。
    #[test]
    fn the_repository_lock_parses_and_pins_every_file_once() {
        let lock = repository_lock();
        let mut paths: Vec<_> = lock.files.iter().map(|file| file.path.as_str()).collect();
        let count = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), count);
        for file in &lock.files {
            assert_eq!(file.sha256.len(), 64, "{}", file.path);
            assert!(file.url.starts_with("https://"), "{}", file.path);
            assert!(!file.url.starts_with(DICTIONARY_RAW), "{}", file.url);
            assert!(!is_dictionary_path(&file.path), "{}", file.path);
        }
        assert!(!lock.references.contains_key("msime-dictionary"));
    }

    /// `UPSTREAM_FILES` 的每个上游名都是锁文件的 reference（`mozc` 对应 `lock.mozc`），提交是完整的小写十六进制，所以按真实锁文件写出的 `upstream.lock.json` 能通过 `Dictionary::open`。
    #[test]
    fn every_upstream_name_is_a_lock_reference() {
        let lock = repository_lock();
        for (path, name) in UPSTREAM_FILES {
            let commit = if name == "mozc" {
                &lock.mozc.commit
            } else {
                &lock
                    .references
                    .get(name)
                    .unwrap_or_else(|| {
                        panic!("{path}: {name} is not a reference in the sources lock")
                    })
                    .commit
            };
            assert!(is_lowercase_hex(commit, 40), "{name}: {commit}");
        }
    }
}
