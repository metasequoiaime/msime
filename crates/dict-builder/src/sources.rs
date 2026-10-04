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

/// Resolves the inputs a stage reads: hand-maintained files from the repository, pinned files from the cache (downloading them unless offline).
pub struct Sources {
    pub lock: Lock,
    pub repository_inputs: PathBuf,
    pub cache: PathBuf,
    pub offline: bool,
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

    /// A pinned input, verified and cached.
    pub fn pinned(&self, path: &str) -> Result<PathBuf> {
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

/// 断言 `file` 是 msime-dictionary 某个 `sources-vX.Y.Z` release 的附件：附件是平铺的，URL 末段就是锁文件路径的文件名。
#[cfg(test)]
pub(crate) fn assert_dictionary_release_asset(file: &PinnedFile) {
    const RELEASES: &str =
        "https://github.com/metasequoiaime/msime-dictionary/releases/download/sources-v";
    let rest = file.url.strip_prefix(RELEASES).unwrap_or_else(|| {
        panic!(
            "{} is not a msime-dictionary sources release asset",
            file.url
        )
    });
    let (version, name) = rest
        .split_once('/')
        .unwrap_or_else(|| panic!("{}", file.url));
    let parts: Vec<&str> = version.split('.').collect();
    assert!(
        parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())),
        "{}",
        file.url
    );
    assert_eq!(Some(name), file.path.rsplit('/').next(), "{}", file.url);
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
        let mut output = File::create(&incoming)?;
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
            if file.path.starts_with("ja/") || file.path.starts_with("ko/") {
                assert_dictionary_release_asset(file);
            }
        }
    }
}
