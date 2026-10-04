//! The pinned inputs (`resources/dictionary-sources.lock.json`): where each file comes from and the SHA-256 it must have. Large or third-party inputs are downloaded into a cache directory on demand; nothing is fetched without a pin, and a cached file is reused only while its size and digest still match.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    fn file(&self, path: &str) -> Result<&PinnedFile> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{path} is not pinned in the sources lock"))
    }
}

/// The raw URL prefix of files pinned from the dictionary source repository: `<RAW><commit>/<path>`.
const DICTIONARY_RAW: &str = "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/";
/// Top-level directories of the dictionary source repository the builder reads; with a checkout, a path under them resolves from the checkout even when the lock has no entry for it.
const DICTIONARY_DIRECTORIES: [&str; 2] = ["sources/", "custom/"];
/// msime-dictionary files that copy or are generated from an upstream at a commit msime records outside the file itself (a lock reference, the Mozc revision, or the licence texts and notices in `resources/licenses`), as an exact path or a directory prefix, with the upstream's name. The manifest's references and `mozc_revision`, `source_commit` in the language databases and the shipped licence texts all name those commits, so even with `--dictionary` these files must still match the lock's size and SHA-256: replacing one with a newer upstream version needs that record, the lock entry and the licences in msime updated first. This covers the tables msime's own generators write from such an upstream (`hkcancor-counts` from HKCanCor, `english-supplement` from SCOWL), whose headers name the msime commit that pins them in the lock.
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
fn upstream_reference(path: &str) -> Option<&'static str> {
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

/// Resolves the inputs a stage reads: hand-maintained files from the repository, pinned files from the cache (downloading them unless offline).
pub struct Sources {
    pub lock: Lock,
    pub repository_inputs: PathBuf,
    pub cache: PathBuf,
    pub offline: bool,
    /// A msime-dictionary checkout (`--dictionary`). Files the lock pins from that repository, and paths under `sources/` or `custom/` the lock does not pin, are read from it instead of the cache; its Git commit pins their content, so the lock's size and SHA-256 are not checked, except for the upstream data in `UPSTREAM_FILES`, which must still match the lock.
    pub dictionary: Option<PathBuf>,
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

    /// Where `path` is read from in the `--dictionary` checkout, or `None` when it comes from the lock (no checkout given, or the lock pins it from another repository).
    pub fn checkout_file(&self, path: &str) -> Option<PathBuf> {
        let checkout = self.dictionary.as_ref()?;
        let from_checkout = match self.lock.files.iter().find(|file| file.path == path) {
            Some(file) => file.url.starts_with(DICTIONARY_RAW),
            None => DICTIONARY_DIRECTORIES
                .iter()
                .any(|directory| path.starts_with(directory)),
        };
        from_checkout.then(|| checkout.join(path))
    }

    /// A pinned input, verified and cached; with `--dictionary`, a msime-dictionary file read from the checkout (upstream data still checked against the lock).
    pub fn pinned(&self, path: &str) -> Result<PathBuf> {
        if let Some(resolved) = self.checkout_file(path) {
            if !resolved.is_file() {
                bail!(
                    "{path} is not in the dictionary checkout at {}",
                    resolved.display()
                );
            }
            if let Some(upstream) = upstream_reference(path) {
                let file = self.lock.file(path).with_context(|| {
                    format!("{path} is {upstream} data, which the dictionary checkout cannot add without a lock entry")
                })?;
                if !matches(&resolved, file)? {
                    bail!(
                        "{path} in the dictionary checkout at {} differs from the size and SHA-256 the sources lock pins; it is {upstream} data at an upstream commit msime records, so update that commit (the lock reference or Mozc revision and resources/licenses), the lock entry and the licence texts in msime before building from this checkout",
                        resolved.display()
                    );
                }
            }
            return Ok(resolved);
        }
        let file = self.lock.file(path)?;
        let target = self.cache.join(&file.path);
        if target.is_file() && matches(&target, file)? {
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

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut stream = File::open(path).with_context(|| format!("opening {}", path.display()))?;
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

/// 断言 `file` 是固定提交中的 msime-dictionary 仓库文件。
#[cfg(test)]
pub(crate) fn assert_dictionary_repository_file(file: &PinnedFile) {
    const RAW: &str = "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/";
    let rest = file.url.strip_prefix(RAW).unwrap_or_else(|| {
        panic!(
            "{} is not a pinned msime-dictionary repository file",
            file.url
        )
    });
    let (commit, path) = rest
        .split_once('/')
        .unwrap_or_else(|| panic!("{}", file.url));
    assert_eq!(
        commit.len(),
        40,
        "{} must pin a full commit, not a release tag",
        file.url
    );
    assert!(
        commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{}",
        file.url
    );
    assert_eq!(path, file.path, "{}", file.url);
}

fn matches(path: &Path, file: &PinnedFile) -> Result<bool> {
    Ok(std::fs::metadata(path)?.len() == file.size && sha256_file(path)? == file.sha256)
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
        let mut output = File::create(incoming)?;
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
    use std::io::Cursor;

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

    fn dictionary_file(path: &str, sha256: &str) -> PinnedFile {
        PinnedFile {
            path: path.into(),
            url: format!("{DICTIONARY_RAW}{}/{path}", "a".repeat(40)),
            sha256: sha256.into(),
            size: 1,
        }
    }

    /// With `--dictionary`, a file the lock pins from msime-dictionary is read from the checkout even though its content no longer matches the pin, and nothing is downloaded; a file pinned from elsewhere still goes through the cache.
    #[test]
    fn a_checkout_overrides_files_pinned_from_the_dictionary_repository() {
        let checkout = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join("custom")).unwrap();
        std::fs::write(checkout.path().join("custom/words.txt"), b"edited").unwrap();
        let mut lock = lock_with(dictionary_file("custom/words.txt", &"0".repeat(64)));
        lock.files.push(PinnedFile {
            path: "ecdict/ecdict.csv".into(),
            url: "http://127.0.0.1:9/unreachable".into(),
            sha256: "0".repeat(64),
            size: 1,
        });
        let sources = Sources {
            lock,
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: Some(checkout.path().into()),
        };
        assert_eq!(
            sources.pinned("custom/words.txt").unwrap(),
            checkout.path().join("custom/words.txt")
        );
        assert!(!cache.path().join("custom/words.txt").exists());
        let error = sources.pinned("ecdict/ecdict.csv").unwrap_err().to_string();
        assert!(error.contains("--offline"), "{error}");
        assert_eq!(sources.checkout_file("ecdict/ecdict.csv"), None);
    }

    /// With `--dictionary`, a new file under `sources/` or `custom/` resolves from the checkout without a lock entry; a pinned file missing from the checkout is an error rather than a fallback to the cache.
    #[test]
    fn a_checkout_resolves_dictionary_paths_the_lock_does_not_pin() {
        let checkout = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(checkout.path().join("sources/pinyin")).unwrap();
        std::fs::write(checkout.path().join("sources/pinyin/new.txt"), b"new").unwrap();
        std::fs::create_dir_all(cache.path().join("custom")).unwrap();
        std::fs::write(cache.path().join("custom/words.txt"), b"w").unwrap();
        let sources = Sources {
            lock: lock_with(dictionary_file(
                "custom/words.txt",
                &hex::encode(Sha256::digest(b"w")),
            )),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: Some(checkout.path().into()),
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

    /// With `--dictionary`, upstream data (here rime-cantonese's) is read from the checkout only while it matches the lock: a replaced file, or a new one the lock does not pin, fails instead of shipping under the old upstream commit.
    #[test]
    fn a_checkout_cannot_change_upstream_data_the_lock_pins() {
        let checkout = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let directory = checkout.path().join("sources/cantonese");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("essay-cantonese.txt"), b"e").unwrap();
        std::fs::write(directory.join("new.txt"), b"n").unwrap();
        let mut file = dictionary_file(
            "sources/cantonese/essay-cantonese.txt",
            &hex::encode(Sha256::digest(b"e")),
        );
        let mut sources = Sources {
            lock: lock_with(dictionary_file(
                "sources/cantonese/essay-cantonese.txt",
                &hex::encode(Sha256::digest(b"e")),
            )),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: Some(checkout.path().into()),
        };
        assert_eq!(
            sources
                .pinned("sources/cantonese/essay-cantonese.txt")
                .unwrap(),
            directory.join("essay-cantonese.txt")
        );
        let error = format!(
            "{:#}",
            sources.pinned("sources/cantonese/new.txt").unwrap_err()
        );
        assert!(error.contains("not pinned in the sources lock"), "{error}");

        file.sha256 = "0".repeat(64);
        sources.lock = lock_with(file);
        let error = sources
            .pinned("sources/cantonese/essay-cantonese.txt")
            .unwrap_err()
            .to_string();
        assert!(error.contains("rime-cantonese"), "{error}");
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

    /// Without `--dictionary` a path the lock does not pin is still an error, even under `sources/`, and pinned files come from the cache.
    #[test]
    fn without_a_checkout_unpinned_dictionary_paths_are_rejected() {
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(cache.path().join("custom")).unwrap();
        std::fs::write(cache.path().join("custom/words.txt"), b"w").unwrap();
        std::fs::create_dir_all(cache.path().join("sources/pinyin")).unwrap();
        std::fs::write(cache.path().join("sources/pinyin/new.txt"), b"new").unwrap();
        let sources = Sources {
            lock: lock_with(dictionary_file(
                "custom/words.txt",
                &hex::encode(Sha256::digest(b"w")),
            )),
            repository_inputs: cache.path().into(),
            cache: cache.path().into(),
            offline: true,
            dictionary: None,
        };
        assert_eq!(sources.checkout_file("sources/pinyin/new.txt"), None);
        let error = sources
            .pinned("sources/pinyin/new.txt")
            .unwrap_err()
            .to_string();
        assert!(error.contains("not pinned in the sources lock"), "{error}");
        assert_eq!(
            sources.pinned("custom/words.txt").unwrap(),
            cache.path().join("custom/words.txt")
        );
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

    #[test]
    fn the_repository_lock_parses_and_pins_every_file_once() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/dictionary-sources.lock.json");
        let lock = Lock::load(&path).unwrap();
        let mut paths: Vec<_> = lock.files.iter().map(|file| file.path.as_str()).collect();
        let count = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), count);
        for file in &lock.files {
            assert_eq!(file.sha256.len(), 64, "{}", file.path);
            assert!(file.url.starts_with("https://"), "{}", file.path);
            if file.path.starts_with("sources/japanese/")
                || file.path.starts_with("sources/korean/")
            {
                assert_dictionary_repository_file(file);
            }
        }
    }

    // The manifest reports references["msime-dictionary"].commit as the source commit of a release, so every file fetched from that repository has to come from that same commit.
    #[test]
    fn every_dictionary_repository_file_is_pinned_to_the_referenced_commit() {
        const RAW: &str = "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/";
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/dictionary-sources.lock.json");
        let lock = Lock::load(&path).unwrap();
        let commit = &lock.references["msime-dictionary"].commit;
        let mut pinned = 0;
        for file in lock.files.iter().filter(|file| file.url.starts_with(RAW)) {
            assert_dictionary_repository_file(file);
            assert!(
                file.url.starts_with(&format!("{RAW}{commit}/")),
                "{} is not pinned to the referenced msime-dictionary commit {commit}",
                file.url
            );
            pinned += 1;
        }
        assert!(pinned > 0, "no msime-dictionary file is pinned");
    }
}
