use super::*;

#[cfg(unix)]
#[test]
fn interrupted_adoption_rejects_a_source_below_a_symlinked_ancestor() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source_real = outside.path().join("source");
    std::fs::create_dir(&source_real).unwrap();
    let linked = root.path().join("linked");
    symlink(outside.path(), &linked).unwrap();
    let source = linked.join("source");
    let staging = root.path().join("staging");
    std::fs::create_dir_all(staging.join("model")).unwrap();
    std::fs::write(staging.join(ADOPTION_SOURCE), source.to_str().unwrap()).unwrap();
    std::fs::write(staging.join("model/fixture.bin"), b"synthetic model").unwrap();

    restore_interrupted_adoption(&staging);

    assert!(staging.join("model/fixture.bin").exists());
    assert!(!source_real.join("fixture.bin").exists());
}

#[cfg(unix)]
#[test]
fn interrupted_adoption_ignores_a_symlinked_staging_directory() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let staging = outside.path().join("staging");
    std::fs::create_dir_all(staging.join("model")).unwrap();
    std::fs::write(staging.join(ADOPTION_SOURCE), source.to_str().unwrap()).unwrap();
    std::fs::write(staging.join("model/fixture.bin"), b"outside file").unwrap();
    symlink(&staging, root.path().join(".staging-pack-attacker")).unwrap();

    remove_leftovers(root.path(), "pack");

    assert!(!source.join("fixture.bin").exists());
    assert!(outside.path().join("staging/model/fixture.bin").exists());
}

#[cfg(unix)]
#[test]
fn interrupted_adoption_ignores_a_symlinked_model_directory() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external_file = outside.path().join("fixture.bin");
    std::fs::write(&external_file, b"outside file").unwrap();
    let staging = root.path().join("staging");
    std::fs::create_dir(&staging).unwrap();
    std::fs::write(staging.join(ADOPTION_SOURCE), source.to_str().unwrap()).unwrap();
    symlink(outside.path(), staging.join("model")).unwrap();

    restore_interrupted_adoption(&staging);

    assert!(external_file.exists());
    assert!(!source.join("fixture.bin").exists());
}

#[cfg(unix)]
#[test]
fn staging_file_creation_rejects_a_symlink() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("outside.bin");
    std::fs::write(&target, b"keep").unwrap();
    let path = directory.path().join("staging.bin");
    symlink(&target, &path).unwrap();

    assert!(create_private_file(&path).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn embedded_file_writing_rejects_a_symlink() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("outside.bin");
    std::fs::write(&target, b"keep").unwrap();
    let path = directory.path().join("embedded.bin");
    symlink(&target, &path).unwrap();

    assert!(write_private_bytes(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"keep");
}
use std::collections::HashMap;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn relative_component_capacity_covers_each_valid_component() {
    for (path, expected) in [("", 0), ("encoder.onnx", 1), ("tokenizer/merges.txt", 2)] {
        assert_eq!(relative_component_capacity(path), expected);
        assert!(
            relative_components(path).unwrap().len() <= relative_component_capacity(path),
            "{path}"
        );
    }
}

#[test]
fn ancestor_capacity_matches_the_absolute_path_components() {
    let root = Path::new("/synthetic/state/models");
    assert_eq!(ancestor_capacity(root), root.components().count());
}

/// Serves fixed bytes per URL, honouring a resume offset unless `ranges` is off, and records what was asked for.
struct MapFetcher {
    files: HashMap<String, Vec<u8>>,
    requested: Mutex<Vec<String>>,
    offsets: Mutex<Vec<u64>>,
    ranges: bool,
}

impl MapFetcher {
    fn new(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            files: files.into_iter().collect(),
            requested: Mutex::new(Vec::new()),
            offsets: Mutex::new(Vec::new()),
            ranges: true,
        }
    }

    /// A source that ignores `Range` and always answers with the whole body.
    fn without_ranges(mut self) -> Self {
        self.ranges = false;
        self
    }
}

impl Fetcher for MapFetcher {
    fn fetch<'a>(&'a self, url: &str, offset: u64) -> Result<Fetched<'a>, LocalModelError> {
        self.requested.lock().unwrap().push(url.to_owned());
        self.offsets.lock().unwrap().push(offset);
        let bytes = self
            .files
            .get(url)
            .ok_or(LocalModelError::HttpStatus(404))?;
        let start = if self.ranges { offset } else { 0 };
        let tail = bytes
            .get(start as usize..)
            .ok_or(LocalModelError::HttpStatus(416))?;
        Ok(Fetched {
            reader: Box::new(tail),
            offset: start,
        })
    }
}

/// Serves the first `cut` bytes of each body and then fails, like a connection that drops mid-download.
struct DroppingFetcher {
    inner: MapFetcher,
    cut: usize,
}

impl Fetcher for DroppingFetcher {
    fn fetch<'a>(&'a self, url: &str, offset: u64) -> Result<Fetched<'a>, LocalModelError> {
        let fetched = self.inner.fetch(url, offset)?;
        let cut = self.cut;
        Ok(Fetched {
            reader: Box::new(fetched.reader.take(cut as u64).chain(FailingReader)),
            offset: fetched.offset,
        })
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::ConnectionReset, "dropped"))
    }
}

enum Member<'a> {
    File(&'a str, &'a [u8]),
    Symlink(&'a str, &'a str),
    /// A regular file whose name is written into the header raw, bypassing the builder's own path checks.
    RawFile(&'a str, &'a [u8]),
}

fn tar_bz2(members: &[Member<'_>]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for member in members {
        match member {
            Member::File(path, data) => {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_entry_type(tar::EntryType::Regular);
                builder.append_data(&mut header, path, *data).unwrap();
            }
            Member::Symlink(path, target) => {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_entry_type(tar::EntryType::Symlink);
                builder.append_link(&mut header, path, target).unwrap();
            }
            Member::RawFile(path, data) => {
                let mut header = tar::Header::new_old();
                header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_entry_type(tar::EntryType::Regular);
                header.set_cksum();
                builder.append(&header, *data).unwrap();
            }
        }
    }
    let tar = builder.into_inner().unwrap();
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    encoder.write_all(&tar).unwrap();
    encoder.finish().unwrap()
}

const ARCHIVE_URL: &str = "https://example.test/models/fixture.tar.bz2";
const EXTRA_URL: &str = "https://example.test/models/vad.onnx";
const EXTRA: &[u8] = b"synthetic vad weights";

/// A catalog entry for a synthetic archive, shaped like the real ones: an archive root, files by role including a directory, one downloaded and one embedded extra.
fn fixture_model(archive: &[u8]) -> CatalogModel {
    let vocab = catalog().models[0]
        .extra
        .iter()
        .find(|extra| extra.resource.is_some())
        .expect("the default model embeds its vocabulary")
        .clone();
    let manifest = serde_json::json!({
        "id": "fixture",
        "kind": "online_transducer",
        "files": {"encoder": "encoder.onnx", "tokens": "tokens.txt", "tokenizer": "tokenizer", "vad": "vad.onnx", "bpe_vocab": "bpe.vocab"},
    });
    CatalogModel {
        id: "fixture".into(),
        title: "Fixture".into(),
        description: String::new(),
        kind: "online_transducer".into(),
        default: false,
        streaming: true,
        languages: vec!["zh".into()],
        archive: CatalogArchive {
            name: "fixture.tar.bz2".into(),
            url: ARCHIVE_URL.into(),
            sha256: hex::encode(Sha256::digest(archive)),
            size: archive.len() as u64,
            root: "fixture-root".into(),
        },
        extra: vec![
            CatalogExtra {
                name: "vad.onnx".into(),
                url: Some(EXTRA_URL.into()),
                resource: None,
                sha256: hex::encode(Sha256::digest(EXTRA)),
                size: EXTRA.len() as u64,
            },
            CatalogExtra {
                name: "bpe.vocab".into(),
                ..vocab
            },
        ],
        files: [
            ("encoder", "encoder.onnx"),
            ("tokens", "tokens.txt"),
            ("tokenizer", "tokenizer"),
            ("vad", "vad.onnx"),
            ("bpe_vocab", "bpe.vocab"),
        ]
        .into_iter()
        .map(|(role, path)| (role.to_owned(), path.to_owned()))
        .collect(),
        hotwords: "native".into(),
        modeling_unit: "cjkchar+bpe".into(),
        installed_size: 1024,
        memory: "约 0.5 GB".into(),
        desktop_only: false,
        license: CatalogLicense {
            spdx: "Apache-2.0".into(),
            source: "https://example.test".into(),
            terms: None,
            notice: String::new(),
        },
        manifest,
    }
}

fn good_archive() -> Vec<u8> {
    tar_bz2(&[
        Member::File("fixture-root/encoder.onnx", b"encoder"),
        Member::File("fixture-root/tokens.txt", b"tokens"),
        Member::File("fixture-root/tokenizer/vocab.json", b"{}"),
        Member::File("fixture-root/tokenizer/merges.txt", b"merges"),
        Member::File("fixture-root/test_wavs/0.wav", b"RIFF"),
        Member::File("fixture-root/README.md", b"unrelated"),
    ])
}

fn fetcher_for(archive: &[u8], mirror: &str) -> MapFetcher {
    MapFetcher::new([
        (mirrored(mirror, ARCHIVE_URL), archive.to_vec()),
        (mirrored(mirror, EXTRA_URL), EXTRA.to_vec()),
    ])
}

fn run(
    root: &Path,
    model: &CatalogModel,
    mirror: &str,
    fetcher: &MapFetcher,
    cancel: &AtomicBool,
) -> (Result<PathBuf, LocalModelError>, Vec<InstallProgress>) {
    let mut events = Vec::new();
    let result = install_model(
        root,
        model,
        mirror,
        fetcher,
        &mut |event| events.push(event),
        cancel,
    );
    (result, events)
}

/// Nothing but the installed models themselves may remain in the root.
fn root_entries(root: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != MODEL_LOCK_FILE)
        .collect();
    names.sort();
    names
}

#[test]
fn the_embedded_catalog_is_complete_and_its_resources_match_their_checksums() {
    let catalog = catalog();
    assert!(!catalog.models.is_empty());
    assert_eq!(default_model_id(), "x-asr-zh-en-streaming");
    assert_eq!(
        catalog.models.iter().filter(|model| model.default).count(),
        1
    );
    for model in &catalog.models {
        assert!(model.archive.url.starts_with("https://"), "{}", model.id);
        assert!(!model.files.is_empty(), "{}", model.id);
        assert!(matches!(model.hotwords.as_str(), "native" | "pinyin"));
        assert_eq!(model.manifest["id"], model.id.as_str());
        assert!(memory_bytes(&model.memory) > 0, "{}", model.id);
        for path in model.files.values() {
            assert!(relative_components(path).is_some(), "{path}");
        }
        for extra in &model.extra {
            assert!(single_component(&extra.name).is_some());
            assert!(extra.url.is_some() != extra.resource.is_some());
            if let Some(resource) = &extra.resource {
                let (_, bytes) = EMBEDDED_RESOURCES
                    .iter()
                    .find(|(name, _)| name == resource)
                    .expect("embedded resource");
                assert_eq!(bytes.len() as u64, extra.size);
                assert_eq!(hex::encode(Sha256::digest(bytes)), extra.sha256);
            }
        }
    }
}

#[test]
fn memory_estimates_are_read_as_bytes() {
    assert_eq!(memory_bytes("约 0.5 GB"), 500_000_000);
    assert_eq!(memory_bytes("约 1.5 GB"), 1_500_000_000);
    assert_eq!(memory_bytes("300 MB"), 300_000_000);
    assert_eq!(memory_bytes("unknown"), 0);
}

#[test]
fn installing_keeps_only_what_the_model_names_and_writes_the_manifest_last() {
    let root = tempfile::tempdir().unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let mirror = "https://mirror.example.test/";
    let fetcher = fetcher_for(&archive, mirror);
    let (result, events) = run(
        root.path(),
        &model,
        mirror,
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(installed, root.path().join("fixture"));

    // Both downloads went through the mirror as a prefix.
    assert_eq!(
        *fetcher.requested.lock().unwrap(),
        vec![
            format!("https://mirror.example.test/{ARCHIVE_URL}"),
            format!("https://mirror.example.test/{EXTRA_URL}"),
        ]
    );
    assert_eq!(
        fs::read(installed.join("encoder.onnx")).unwrap(),
        b"encoder"
    );
    assert_eq!(
        fs::read(installed.join("tokenizer/merges.txt")).unwrap(),
        b"merges"
    );
    assert_eq!(fs::read(installed.join("vad.onnx")).unwrap(), EXTRA);
    assert!(installed.join("bpe.vocab").is_file());
    assert!(!installed.join("test_wavs").exists());
    assert!(!installed.join("README.md").exists());
    let manifest: Value =
        serde_json::from_slice(&fs::read(installed.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(manifest, model.manifest);
    assert_eq!(root_entries(root.path()), vec!["fixture"]);

    let stages: Vec<_> = events.iter().map(|event| event.stage).collect();
    assert_eq!(stages.first(), Some(&"download"));
    assert_eq!(stages.last(), Some(&"done"));
    for stage in ["verify", "extract"] {
        assert!(stages.contains(&stage), "{stage}");
    }
    let total = archive.len() as u64;
    assert!(events
        .iter()
        .all(|event| event.total == total && event.downloaded <= total));
}

#[test]
fn reinstalling_replaces_the_previous_install() {
    let root = tempfile::tempdir().unwrap();
    let stale = root.path().join("fixture");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("stale.bin"), b"old").unwrap();
    // A staging directory a killed install left behind is cleared too.
    fs::create_dir_all(root.path().join(".staging-fixture-dead")).unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
    let installed = result.unwrap();
    assert!(!installed.join("stale.bin").exists());
    assert!(installed.join(MANIFEST_FILE).is_file());
    assert_eq!(root_entries(root.path()), vec!["fixture"]);
}

#[test]
fn reinstalling_replaces_a_regular_model_slot_without_leaving_an_old_file() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("fixture"), b"stray slot file").unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));

    let installed = result.unwrap();
    assert!(installed.join(MANIFEST_FILE).is_file());
    assert_eq!(root_entries(root.path()), vec!["fixture"]);
}

#[test]
fn installing_clears_a_stray_file_left_by_an_interrupted_install() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join(".staging-fixture-dead"), b"stray leftover").unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));

    assert!(result.is_ok());
    assert!(!root.path().join(".staging-fixture-dead").exists());
    assert_eq!(root_entries(root.path()), vec!["fixture"]);
}

#[cfg(unix)]
#[test]
fn installing_clears_a_stray_link_without_touching_its_target() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep.txt"), b"keep").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(".old-fixture-dead")).unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));

    assert!(result.is_ok());
    assert!(!root.path().join(".old-fixture-dead").exists());
    assert_eq!(fs::read(outside.path().join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn a_checksum_mismatch_leaves_nothing_behind() {
    let root = tempfile::tempdir().unwrap();
    let archive = good_archive();
    let mut model = fixture_model(&archive);
    model.archive.sha256 = "0".repeat(64);
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
    assert!(matches!(result, Err(LocalModelError::ChecksumMismatch(_))));
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn a_short_or_long_download_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let archive = good_archive();
    for size in [archive.len() as u64 - 1, archive.len() as u64 + 1] {
        let mut model = fixture_model(&archive);
        model.archive.size = size;
        let fetcher = fetcher_for(&archive, "");
        let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
        assert!(matches!(result, Err(LocalModelError::SizeMismatch(_))));
        assert!(root_entries(root.path()).is_empty());
    }
}

#[test]
fn unsafe_members_fail_the_install() {
    let cases: Vec<Vec<u8>> = vec![
        tar_bz2(&[Member::RawFile("fixture-root/../escape.onnx", b"x")]),
        tar_bz2(&[Member::RawFile("/fixture-root/encoder.onnx", b"x")]),
        tar_bz2(&[Member::File("elsewhere/encoder.onnx", b"x")]),
        tar_bz2(&[
            Member::File("fixture-root/tokens.txt", b"tokens"),
            Member::Symlink("fixture-root/encoder.onnx", "../../outside"),
        ]),
        tar_bz2(&[Member::Symlink("fixture-root/encoder.onnx", "/etc/passwd")]),
    ];
    for archive in cases {
        let root = tempfile::tempdir().unwrap();
        let model = fixture_model(&archive);
        let fetcher = fetcher_for(&archive, "");
        let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
        assert!(
            matches!(result, Err(LocalModelError::UnsafeArchive(_))),
            "{result:?}"
        );
        assert!(root_entries(root.path()).is_empty());
        assert!(!root.path().parent().unwrap().join("escape.onnx").exists());
    }
}

#[test]
fn a_link_inside_the_model_is_materialised_as_a_copy() {
    let archive = tar_bz2(&[
        Member::File("fixture-root/encoder.int8.onnx", b"encoder"),
        Member::Symlink("fixture-root/encoder.onnx", "encoder.int8.onnx"),
        Member::File("fixture-root/tokens.txt", b"tokens"),
        Member::File("fixture-root/tokenizer/vocab.json", b"{}"),
    ]);
    let root = tempfile::tempdir().unwrap();
    let mut model = fixture_model(&archive);
    model
        .files
        .insert("encoder_int8".into(), "encoder.int8.onnx".into());
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
    let installed = result.unwrap();
    let encoder = installed.join("encoder.onnx");
    assert!(!fs::symlink_metadata(&encoder)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(encoder).unwrap(), b"encoder");
}

#[test]
fn a_link_copy_is_rejected_before_writing_past_the_expansion_budget() {
    let files = tempfile::tempdir().unwrap();
    let source = files.path().join("source.bin");
    let destination = files.path().join("destination.bin");
    fs::write(&source, b"1234").unwrap();
    let mut written = 0;

    assert!(matches!(
        copy_link_with_budget(&source, &destination, &mut written, 3),
        Err(LocalModelError::UnsafeArchive(message)) if message == "archive expands too far"
    ));
    assert_eq!(written, 0);
    assert!(!destination.exists());
}

#[test]
fn a_missing_model_file_fails_the_install() {
    let archive = tar_bz2(&[Member::File("fixture-root/encoder.onnx", b"encoder")]);
    let root = tempfile::tempdir().unwrap();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
    assert!(matches!(result, Err(LocalModelError::MissingFile(_))));
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn cancelling_stops_the_install_and_cleans_up() {
    let root = tempfile::tempdir().unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(true));
    assert!(matches!(result, Err(LocalModelError::Cancelled)));
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn only_https_mirrors_and_absolute_roots_are_accepted() {
    let root = tempfile::tempdir().unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");
    let (result, _) = run(
        root.path(),
        &model,
        "http://mirror.example.test",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(LocalModelError::InvalidMirror)));
    let (result, _) = run(
        Path::new("relative/root"),
        &model,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(LocalModelError::InvalidRoot)));
    assert!(fetcher.requested.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_roots_are_rejected_before_installing() {
    let target = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("models");
    std::os::unix::fs::symlink(target.path(), &root).unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(&root, &model, "", &fetcher, &AtomicBool::new(false));

    assert!(matches!(result, Err(LocalModelError::InvalidRoot)));
    assert!(fetcher.requested.lock().unwrap().is_empty());
    assert!(root.join(model.id).symlink_metadata().is_err());
    assert!(target.path().read_dir().unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn symlinked_root_ancestors_are_rejected_before_installing() {
    let target = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let linked = parent.path().join("linked");
    msime_path_trust::untrusted_symlink(target.path(), &linked).unwrap();
    let root = linked.join("missing").join("models");
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(&root, &model, "", &fetcher, &AtomicBool::new(false));

    assert!(matches!(result, Err(LocalModelError::InvalidRoot)));
    assert!(fetcher.requested.lock().unwrap().is_empty());
    assert!(!target.path().join("missing/models").exists());
    assert!(list(&root).iter().all(|status| !status.installed));
    assert!(matches!(
        remove(&root, default_model_id()),
        Err(LocalModelError::InvalidRoot)
    ));
}

#[cfg(unix)]
#[test]
fn symlinked_root_ancestors_with_existing_descendants_are_rejected() {
    let target = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    fs::create_dir_all(target.path().join("inner/models")).unwrap();
    let linked = parent.path().join("linked");
    msime_path_trust::untrusted_symlink(target.path(), &linked).unwrap();
    let root = linked.join("inner/models");
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(&root, &model, "", &fetcher, &AtomicBool::new(false));

    assert!(matches!(result, Err(LocalModelError::InvalidRoot)));
    assert!(fetcher.requested.lock().unwrap().is_empty());
    assert!(target
        .path()
        .join("inner/models")
        .read_dir()
        .unwrap()
        .next()
        .is_none());
}

#[cfg(unix)]
#[test]
fn dangling_model_slots_can_be_replaced_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("fixture");
    let missing = root.path().join("missing");
    std::os::unix::fs::symlink(&missing, &target).unwrap();
    let archive = good_archive();
    let model = fixture_model(&archive);
    let fetcher = fetcher_for(&archive, "");

    let (result, _) = run(root.path(), &model, "", &fetcher, &AtomicBool::new(false));
    let installed = result.unwrap();
    assert!(installed.join(MANIFEST_FILE).is_file());
    assert!(!fs::symlink_metadata(&installed)
        .unwrap()
        .file_type()
        .is_symlink());

    let remove_root = tempfile::tempdir().unwrap();
    let remove_id = default_model_id();
    let dangling = remove_root.path().join(remove_id);
    std::os::unix::fs::symlink(remove_root.path().join("missing"), &dangling).unwrap();
    remove(remove_root.path(), remove_id).unwrap();
    assert!(fs::symlink_metadata(&dangling).is_err());
}

#[test]
fn removing_a_stray_file_model_slot_succeeds_and_cleans_it() {
    let root = tempfile::tempdir().unwrap();
    let id = default_model_id();
    let target = root.path().join(id);
    fs::write(&target, b"stray slot").unwrap();

    remove(root.path(), id).unwrap();

    assert!(fs::symlink_metadata(&target).is_err());
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn listing_reports_installed_models_and_removal_accepts_catalog_ids_only() {
    let root = tempfile::tempdir().unwrap();
    let id = default_model_id();
    let models = list(root.path());
    assert_eq!(models.len(), catalog().models.len());
    assert!(models.iter().all(|model| !model.installed));
    let status = models.iter().find(|model| model.id == id).unwrap();
    assert!(status.default && status.streaming);
    assert_eq!(status.path, root.path().join(id).to_string_lossy());
    assert_eq!(status.memory, 500_000_000);
    assert!(status.archive_size > 0 && status.installed_size > 0);
    assert!(models
        .iter()
        .any(|model| model.desktop_only && model.id == "fun-asr-nano"));

    let installed = root.path().join(id);
    fs::create_dir_all(&installed).unwrap();
    fs::write(installed.join(MANIFEST_FILE), b"{}").unwrap();
    assert!(list(root.path())
        .iter()
        .any(|model| model.id == id && model.installed));

    for rejected in ["../outside", "", "unknown", "x-asr-zh-en-streaming/.."] {
        assert!(matches!(
            remove(root.path(), rejected),
            Err(LocalModelError::UnknownModel)
        ));
    }
    remove(root.path(), id).unwrap();
    assert!(root_entries(root.path()).is_empty());
    // Removing what is not installed is not an error.
    remove(root.path(), id).unwrap();
}

#[cfg(unix)]
#[test]
fn listing_rejects_symlinked_model_directories_and_manifests() {
    let root = tempfile::tempdir().unwrap();
    let id = default_model_id();

    let external = tempfile::tempdir().unwrap();
    let external_model = external.path().join("model");
    fs::create_dir(&external_model).unwrap();
    fs::write(external_model.join(MANIFEST_FILE), b"{}").unwrap();

    let linked_model = root.path().join(id);
    std::os::unix::fs::symlink(&external_model, &linked_model).unwrap();
    assert!(!list(root.path())
        .into_iter()
        .any(|model| model.id == id && model.installed));
    fs::remove_file(&linked_model).unwrap();

    let installed = root.path().join(id);
    fs::create_dir(&installed).unwrap();
    let external_manifest = external.path().join("external-manifest.json");
    fs::write(&external_manifest, b"{}").unwrap();
    std::os::unix::fs::symlink(&external_manifest, installed.join(MANIFEST_FILE)).unwrap();
    assert!(!list(root.path())
        .into_iter()
        .any(|model| model.id == id && model.installed));
}

const PACK_A_URL: &str = "https://example.test/packs/a.dat";
const PACK_B_URL: &str = "https://example.test/packs/b.txt";
const PACK_A: &[u8] = b"synthetic dictionary bytes";
const PACK_B: &[u8] = b"licence";

fn pack_artifact(name: &str, url: &str, bytes: &[u8]) -> crate::resources::Artifact {
    crate::resources::Artifact {
        name: name.into(),
        url: url.into(),
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}

fn pack_files() -> Vec<crate::resources::Artifact> {
    vec![
        pack_artifact("a.dat", PACK_A_URL, PACK_A),
        pack_artifact("b.txt", PACK_B_URL, PACK_B),
    ]
}

fn pack_fetcher(mirror: &str, b: &[u8]) -> MapFetcher {
    MapFetcher::new([
        (mirrored(mirror, PACK_A_URL), PACK_A.to_vec()),
        (mirrored(mirror, PACK_B_URL), b.to_vec()),
    ])
}

fn run_pack(
    root: &Path,
    id: &str,
    files: &[crate::resources::Artifact],
    mirror: &str,
    fetcher: &dyn Fetcher,
    cancel: &AtomicBool,
) -> (Result<PathBuf, LocalModelError>, Vec<InstallProgress>) {
    run_pack_from(root, id, files, &[mirror], fetcher, cancel)
}

fn run_pack_from(
    root: &Path,
    id: &str,
    files: &[crate::resources::Artifact],
    mirrors: &[&str],
    fetcher: &dyn Fetcher,
    cancel: &AtomicBool,
) -> (Result<PathBuf, LocalModelError>, Vec<InstallProgress>) {
    let mut events = Vec::new();
    let manifest = serde_json::json!({"pack": id});
    let result = install_files_with(
        root,
        id,
        files,
        &manifest,
        mirrors,
        fetcher,
        &mut |event| events.push(event),
        cancel,
    );
    (result, events)
}

#[test]
fn installing_files_publishes_them_with_the_manifest_and_reports_progress() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let mirror = "https://mirror.example.test/";
    let fetcher = pack_fetcher(mirror, PACK_B);
    let (result, events) = run_pack(
        root.path(),
        "pack",
        &files,
        mirror,
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(installed, root.path().join("pack"));
    assert_eq!(root_entries(root.path()), vec!["pack"]);
    assert_eq!(
        root_entries(&installed),
        vec!["a.dat", "b.txt", MANIFEST_FILE]
    );
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(fs::read(installed.join("b.txt")).unwrap(), PACK_B);
    assert_eq!(
        installed_manifest(root.path(), "pack"),
        Some(serde_json::json!({"pack": "pack"}))
    );
    assert_eq!(
        *fetcher.requested.lock().unwrap(),
        vec![
            format!("https://mirror.example.test/{PACK_A_URL}"),
            format!("https://mirror.example.test/{PACK_B_URL}"),
        ]
    );
    let total = (PACK_A.len() + PACK_B.len()) as u64;
    let last = events.last().unwrap();
    assert_eq!(last.stage, "done");
    assert_eq!((last.downloaded, last.total), (total, total));
    assert!(events
        .iter()
        .any(|event| event.stage == "download" && event.downloaded == total));
    assert!(events
        .iter()
        .all(|event| event.total == total && event.downloaded <= total));
}

#[test]
fn removal_waits_for_an_install_before_cleaning_its_staging_directory() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let id = default_model_id().to_owned();
    let install_id = id.clone();
    let root_path = root.path().to_owned();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let install = thread::spawn(move || {
        let fetcher = pack_fetcher("", PACK_B);
        let mut events = Vec::new();
        let result = install_files_with(
            &root_path,
            &install_id,
            &files,
            &serde_json::json!({"pack": "pack"}),
            &[],
            &fetcher,
            &mut |event| {
                events.push(event.clone());
                if event.stage == "verify" {
                    ready_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                }
            },
            &AtomicBool::new(false),
        );
        (result, events)
    });
    ready_rx.recv().unwrap();

    let remove_root = root.path().to_owned();
    let remove_id = id.clone();
    let (removed_tx, removed_rx) = mpsc::channel();
    let removal = thread::spawn(move || {
        let result = remove(&remove_root, &remove_id);
        removed_tx.send(result).unwrap();
    });

    // Without serialization, remove_leftovers deletes the install's staging directory and
    // returns while the installer is paused before publishing it.
    assert!(removed_rx.recv_timeout(Duration::from_millis(100)).is_err());
    release_tx.send(()).unwrap();

    let (install_result, _events) = install.join().unwrap();
    assert!(install_result.is_ok(), "install failed: {install_result:?}");
    let remove_result = removed_rx.recv().unwrap();
    removal.join().unwrap();
    assert!(remove_result.is_ok(), "remove failed: {remove_result:?}");
    assert!(!root.path().join(id).exists());
}

/// 摘要不符的文件不留下，也不发布任何东西；同一包里已经校验过的文件留在续传目录，下次不用再下。
#[test]
fn a_file_checksum_mismatch_publishes_nothing_and_keeps_only_verified_bytes() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let fetcher = pack_fetcher("", b"LICENCE");
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(LocalModelError::ChecksumMismatch(name)) if name == "b.txt"));
    assert_eq!(root_entries(root.path()), vec![".partial-pack"]);
    let partials = root.path().join(".partial-pack");
    assert_eq!(
        root_entries(&partials),
        vec![partial_path(&partials, &files[0])
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()]
    );

    // 只有一个文件、它又不符时，什么都不留。
    let root = tempfile::tempdir().unwrap();
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files[1..],
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(LocalModelError::ChecksumMismatch(_))));
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn cancelling_a_file_install_leaves_nothing_behind() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &pack_files(),
        "",
        &fetcher,
        &AtomicBool::new(true),
    );
    assert!(matches!(result, Err(LocalModelError::Cancelled)));
    assert!(root_entries(root.path()).is_empty());
}

#[test]
fn file_install_ids_must_be_a_single_visible_component() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = pack_fetcher("", PACK_B);
    for id in ["", "a/b", "/abs", "..", ".", ".hidden", "pack/.."] {
        let (result, _) = run_pack(
            root.path(),
            id,
            &pack_files(),
            "",
            &fetcher,
            &AtomicBool::new(false),
        );
        assert!(
            matches!(result, Err(LocalModelError::UnknownModel)),
            "{id}: {result:?}"
        );
    }
    assert!(fetcher.requested.lock().unwrap().is_empty());
    assert!(root_entries(root.path()).is_empty());
}

#[cfg(unix)]
#[test]
fn reinstalling_files_keeps_an_open_handle_to_the_old_file_readable() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &pack_files(),
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    // 模拟输入法映射着旧文件：句柄在重新安装期间保持打开。
    let mut open = fs::File::open(installed.join("a.dat")).unwrap();
    let mut files = pack_files();
    let replacement: &[u8] = b"a newer dictionary";
    files[0] = pack_artifact("a.dat", PACK_A_URL, replacement);
    let fetcher = MapFetcher::new([
        (PACK_A_URL.to_owned(), replacement.to_vec()),
        (PACK_B_URL.to_owned(), PACK_B.to_vec()),
    ]);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    let mut old = Vec::new();
    open.read_to_end(&mut old).unwrap();
    assert_eq!(old, PACK_A);
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), replacement);
    assert_eq!(root_entries(root.path()), vec!["pack"]);
}

#[cfg(unix)]
#[test]
fn installed_manifest_ignores_symlinks_and_oversized_files() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(installed_manifest(root.path(), "pack"), None);
    let pack = root.path().join("pack");
    fs::create_dir(&pack).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join(MANIFEST_FILE);
    fs::write(&external, b"{}").unwrap();
    std::os::unix::fs::symlink(&external, pack.join(MANIFEST_FILE)).unwrap();
    assert_eq!(installed_manifest(root.path(), "pack"), None);
    fs::remove_file(pack.join(MANIFEST_FILE)).unwrap();
    fs::write(
        pack.join(MANIFEST_FILE),
        vec![b' '; MAX_MANIFEST_BYTES as usize + 1],
    )
    .unwrap();
    assert_eq!(installed_manifest(root.path(), "pack"), None);
    fs::write(pack.join(MANIFEST_FILE), b"{}").unwrap();
    assert_eq!(
        installed_manifest(root.path(), "pack"),
        Some(serde_json::json!({}))
    );
    assert_eq!(installed_manifest(root.path(), "../pack"), None);
}

#[cfg(unix)]
#[test]
fn installed_manifest_rejects_a_symlinked_root() {
    let parent = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let pack = outside.path().join("pack");
    fs::create_dir(&pack).unwrap();
    fs::write(pack.join(MANIFEST_FILE), b"{}").unwrap();
    let linked = parent.path().join("models");
    std::os::unix::fs::symlink(outside.path(), &linked).unwrap();

    assert_eq!(installed_manifest(&linked, "pack"), None);
}

/// A partial file left in `<root>/.partial-<id>/` by an earlier install.
fn write_partial(
    root: &Path,
    id: &str,
    file: &crate::resources::Artifact,
    bytes: &[u8],
) -> PathBuf {
    let directory = root.join(format!(".partial-{id}"));
    fs::create_dir_all(&directory).unwrap();
    let path = partial_path(&directory, file).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn a_partial_file_is_resumed_from_where_it_stopped() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    write_partial(root.path(), "pack", &files[0], &PACK_A[..5]);
    let fetcher = pack_fetcher("", PACK_B);
    let (result, events) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(fs::read(installed.join("b.txt")).unwrap(), PACK_B);
    // 只请求了剩下的字节；装好后续传目录也清掉了。
    assert_eq!(*fetcher.offsets.lock().unwrap(), vec![5, 0]);
    assert_eq!(root_entries(root.path()), vec!["pack"]);
    // 进度从已有的字节开始，而不是从零。
    let first = events
        .iter()
        .find(|event| event.stage == "download")
        .unwrap();
    assert!(first.downloaded >= 5, "{first:?}");
}

#[test]
fn a_source_that_ignores_the_range_restarts_the_file() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    write_partial(root.path(), "pack", &files[0], &PACK_A[..5]);
    let fetcher = pack_fetcher("", PACK_B).without_ranges();
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(*fetcher.offsets.lock().unwrap(), vec![5, 0]);
}

#[test]
fn a_dropped_download_keeps_its_bytes_for_the_next_install() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let dropping = DroppingFetcher {
        inner: pack_fetcher("", PACK_B),
        cut: 10,
    };
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &dropping,
        &AtomicBool::new(false),
    );
    assert!(
        matches!(result, Err(LocalModelError::Network(_))),
        "{result:?}"
    );
    // 没有发布任何东西，只留下已收到的部分。
    assert_eq!(root_entries(root.path()), vec![".partial-pack"]);
    let partial = partial_path(&root.path().join(".partial-pack"), &files[0]).unwrap();
    assert_eq!(fs::read(&partial).unwrap(), &PACK_A[..10]);

    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert_eq!(fs::read(result.unwrap().join("a.dat")).unwrap(), PACK_A);
    assert_eq!(fetcher.offsets.lock().unwrap()[0], 10);
    assert_eq!(root_entries(root.path()), vec!["pack"]);
}

/// 全部下完、进入校验阶段时取消，下完的文件仍留在 `.partial-<id>` 里，下次安装不必重新下载。
#[test]
fn cancelling_while_verifying_keeps_the_finished_downloads() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let fetcher = pack_fetcher("", PACK_B);
    let cancel = AtomicBool::new(false);
    let manifest = serde_json::json!({"pack": "pack"});
    let result = install_files_with(
        root.path(),
        "pack",
        &files,
        &manifest,
        &[""],
        &fetcher,
        &mut |event| {
            if event.stage == "verify" {
                cancel.store(true, Ordering::SeqCst);
            }
        },
        &cancel,
    );
    assert!(
        matches!(result, Err(LocalModelError::Cancelled)),
        "{result:?}"
    );
    assert_eq!(root_entries(root.path()), vec![".partial-pack"]);
    let partials = root.path().join(".partial-pack");
    assert_eq!(
        fs::read(partial_path(&partials, &files[0]).unwrap()).unwrap(),
        PACK_A
    );
    assert_eq!(
        fs::read(partial_path(&partials, &files[1]).unwrap()).unwrap(),
        PACK_B
    );
}

/// 一份没下完、但摘要已经对不上的文件（比如锁文件换过字节而长度没变）在下完时被删掉，下一个源从头下载。
#[test]
fn a_corrupt_source_falls_back_to_the_next_one_from_scratch() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let mirror = "https://mirror.example.test/";
    let fetcher = MapFetcher::new([
        (mirrored(mirror, PACK_A_URL), PACK_A.to_vec()),
        (mirrored(mirror, PACK_B_URL), b"LICENCE".to_vec()),
        (PACK_B_URL.to_owned(), PACK_B.to_vec()),
    ]);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        mirror,
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(fs::read(installed.join("b.txt")).unwrap(), PACK_B);
    assert_eq!(
        *fetcher.requested.lock().unwrap(),
        vec![
            mirrored(mirror, PACK_A_URL),
            mirrored(mirror, PACK_B_URL),
            PACK_B_URL.to_owned(),
        ]
    );
    assert_eq!(*fetcher.offsets.lock().unwrap(), vec![0, 0, 0]);
}

/// 镜像只回了一小段错误页（长度不足，保留下来等续传）：下一个源接着它续传后摘要不符，于是删掉从头再下，安装照样成功。
#[test]
fn a_short_wrong_prefix_from_one_source_does_not_poison_the_next() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    let mirror = "https://mirror.example.test/";
    let fetcher = MapFetcher::new([
        (mirrored(mirror, PACK_A_URL), b"<html>".to_vec()),
        (PACK_A_URL.to_owned(), PACK_A.to_vec()),
        (mirrored(mirror, PACK_B_URL), PACK_B.to_vec()),
    ]);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        mirror,
        &fetcher,
        &AtomicBool::new(false),
    );
    let installed = result.unwrap();
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(
        fetcher.requested.lock().unwrap()[..3],
        [
            mirrored(mirror, PACK_A_URL),
            PACK_A_URL.to_owned(),
            PACK_A_URL.to_owned(),
        ]
    );
    assert_eq!(fetcher.offsets.lock().unwrap()[..3], [0, 6, 0]);
    assert_eq!(root_entries(root.path()), vec!["pack"]);
}

/// 上次安装留下的前缀已经不对：同一个源续传后摘要不符，删掉从头再下一次，而不是直接报错。
#[test]
fn a_stale_wrong_prefix_is_redownloaded_from_scratch() {
    let root = tempfile::tempdir().unwrap();
    let files = pack_files();
    write_partial(root.path(), "pack", &files[0], b"WRONG");
    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack(
        root.path(),
        "pack",
        &files,
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    assert_eq!(fs::read(result.unwrap().join("a.dat")).unwrap(), PACK_A);
    assert_eq!(*fetcher.offsets.lock().unwrap(), vec![5, 0, 0]);
}

#[test]
fn sources_are_tried_in_order_with_the_lock_url_last() {
    let first = "https://first.example.test/";
    let second = "https://second.example.test";
    let files = pack_files();

    // 第一个镜像没有这些文件，第二个有：不再碰原地址。
    let root = tempfile::tempdir().unwrap();
    let fetcher = MapFetcher::new([
        (mirrored(second, PACK_A_URL), PACK_A.to_vec()),
        (mirrored(second, PACK_B_URL), PACK_B.to_vec()),
    ]);
    let (result, _) = run_pack_from(
        root.path(),
        "pack",
        &files,
        &[first, "", second],
        &fetcher,
        &AtomicBool::new(false),
    );
    result.unwrap();
    assert_eq!(
        *fetcher.requested.lock().unwrap(),
        vec![
            mirrored(first, PACK_A_URL),
            mirrored(second, PACK_A_URL),
            mirrored(first, PACK_B_URL),
            mirrored(second, PACK_B_URL),
        ]
    );

    // 镜像都不行时才用锁文件里的原地址。
    let root = tempfile::tempdir().unwrap();
    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack_from(
        root.path(),
        "pack",
        &files,
        &[first, second],
        &fetcher,
        &AtomicBool::new(false),
    );
    result.unwrap();
    assert_eq!(
        fetcher.requested.lock().unwrap()[..3],
        [
            mirrored(first, PACK_A_URL),
            mirrored(second, PACK_A_URL),
            PACK_A_URL.to_owned(),
        ]
    );

    // 任何一个镜像不是合法的 https 前缀，整个安装在联网前就拒绝。
    let root = tempfile::tempdir().unwrap();
    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack_from(
        root.path(),
        "pack",
        &files,
        &[first, "http://plain.example.test/"],
        &fetcher,
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(LocalModelError::InvalidMirror)));
    assert!(fetcher.requested.lock().unwrap().is_empty());
}

/// 回环上的最小 HTTP/1.1 服务器怎样回答 `Range` 请求。
#[derive(Clone, Copy)]
enum RangeMode {
    Honour,
    Ignore,
    Unsatisfiable,
}

/// 依次接受 `connections` 个连接，每个连接回答一次 `body`；返回 URL 和一个结束时交出每次请求 `Range` 起点的线程。
fn serve(
    body: &'static [u8],
    mode: RangeMode,
    connections: usize,
) -> (String, std::thread::JoinHandle<Vec<Option<u64>>>) {
    use std::io::BufRead;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/file", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut ranges = Vec::with_capacity(connections);
        for _ in 0..connections {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut range = None;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("range") {
                        range = value
                            .trim()
                            .strip_prefix("bytes=")
                            .and_then(|value| value.strip_suffix('-'))
                            .and_then(|value| value.parse::<u64>().ok());
                    }
                }
            }
            ranges.push(range);
            let length = body.len();
            let (head, payload): (String, &[u8]) = match (range, mode) {
                (Some(start), RangeMode::Honour) => (
                    format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{length}\r\nContent-Length: {}\r\n",
                        length - 1,
                        length - start as usize
                    ),
                    &body[start as usize..],
                ),
                (Some(_), RangeMode::Unsatisfiable) => (
                    format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{length}\r\nContent-Length: 0\r\n"
                    ),
                    &[],
                ),
                _ => (
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\n"),
                    body,
                ),
            };
            stream
                .write_all(format!("{head}Connection: close\r\n\r\n").as_bytes())
                .unwrap();
            stream.write_all(payload).unwrap();
        }
        ranges
    });
    (url, server)
}

#[test]
fn the_http_fetcher_resumes_with_a_range_request() {
    const BODY: &[u8] = b"0123456789abcdef";
    let fetcher = HttpFetcher::build(false).unwrap();
    let body = |fetched: Fetched<'_>| {
        let mut bytes = Vec::new();
        let mut reader = fetched.reader;
        reader.read_to_end(&mut bytes).unwrap();
        bytes
    };

    // 新下载不带 Range。
    let (url, server) = serve(BODY, RangeMode::Honour, 1);
    let fetched = fetcher.fetch(&url, 0).unwrap();
    assert_eq!(fetched.offset, 0);
    assert_eq!(body(fetched), BODY);
    assert_eq!(server.join().unwrap(), vec![None]);

    // 服务器支持 Range：只拿剩下的字节。
    let (url, server) = serve(BODY, RangeMode::Honour, 1);
    let fetched = fetcher.fetch(&url, 10).unwrap();
    assert_eq!(fetched.offset, 10);
    assert_eq!(body(fetched), &BODY[10..]);
    assert_eq!(server.join().unwrap(), vec![Some(10)]);

    // 服务器不理 Range：整份返回，起点报告为 0。
    let (url, server) = serve(BODY, RangeMode::Ignore, 1);
    let fetched = fetcher.fetch(&url, 4).unwrap();
    assert_eq!(fetched.offset, 0);
    assert_eq!(body(fetched), BODY);
    assert_eq!(server.join().unwrap(), vec![Some(4)]);

    // 416：再请求一次整份。
    let (url, server) = serve(BODY, RangeMode::Unsatisfiable, 2);
    let fetched = fetcher.fetch(&url, 16).unwrap();
    assert_eq!(fetched.offset, 0);
    assert_eq!(body(fetched), BODY);
    assert_eq!(server.join().unwrap(), vec![Some(16), None]);
}

#[test]
fn content_range_starts_are_parsed_strictly() {
    assert_eq!(content_range_start("bytes 10-15/16"), Some(10));
    assert_eq!(content_range_start(" bytes 0-0/1 "), Some(0));
    assert_eq!(content_range_start("bytes */16"), None);
    assert_eq!(content_range_start("items 10-15/16"), None);
    assert_eq!(content_range_start("bytes x-15/16"), None);
}

const ARCHIVE_PACK_URL: &str = "https://example.test/runtime/runtime.aar";

/// A zip holding `members`, as the upstream `.aar` does.
fn zip_bytes(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in members {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn run_archive(
    root: &Path,
    archive_bytes: &[u8],
    files: &[crate::resources::Artifact],
    fetcher: &dyn Fetcher,
) -> (Result<PathBuf, LocalModelError>, Vec<InstallProgress>) {
    let archive = pack_artifact("runtime.aar", ARCHIVE_PACK_URL, archive_bytes);
    let members = [
        ("jni/arm64-v8a/liba.so".to_owned(), "liba.so".to_owned()),
        ("jni/arm64-v8a/libb.so".to_owned(), "libb.so".to_owned()),
    ];
    let mut events = Vec::new();
    let result = install_archive_members_with(
        root,
        "runtime",
        &archive,
        &members,
        files,
        &serde_json::json!({"pack": "runtime"}),
        &[],
        fetcher,
        &mut |event| events.push(event),
        &AtomicBool::new(false),
    );
    (result, events)
}

#[test]
fn archive_members_are_extracted_verified_and_published() {
    let archive = zip_bytes(&[
        ("jni/arm64-v8a/liba.so", b"library a"),
        ("jni/arm64-v8a/libb.so", b"library b"),
        ("jni/x86_64/liba.so", b"another abi"),
        ("classes.jar", b"java"),
    ]);
    let files = [
        pack_artifact("liba.so", ARCHIVE_PACK_URL, b"library a"),
        pack_artifact("libb.so", ARCHIVE_PACK_URL, b"library b"),
    ];
    let root = tempfile::tempdir().unwrap();
    let fetcher = MapFetcher::new([(ARCHIVE_PACK_URL.to_owned(), archive.clone())]);
    let (result, events) = run_archive(root.path(), &archive, &files, &fetcher);
    let installed = result.unwrap();
    assert_eq!(
        root_entries(&installed),
        vec!["liba.so", "libb.so", MANIFEST_FILE]
    );
    assert_eq!(fs::read(installed.join("liba.so")).unwrap(), b"library a");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(installed.join("libb.so"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o444);
    }
    // 只留下发布的目录，归档本身不留。
    assert_eq!(root_entries(root.path()), vec!["runtime"]);
    let total = archive.len() as u64;
    assert!(events
        .iter()
        .all(|event| event.total == total && event.downloaded <= total));
    assert_eq!(
        events
            .iter()
            .map(|event| event.stage)
            .filter(|stage| *stage != "download")
            .collect::<Vec<_>>(),
        ["verify", "done"]
    );

    // 成员的字节和锁文件不符：不发布；归档已经校验过，留着下次不用再下。
    let root = tempfile::tempdir().unwrap();
    let wrong = [
        pack_artifact("liba.so", ARCHIVE_PACK_URL, b"library a"),
        pack_artifact("libb.so", ARCHIVE_PACK_URL, b"library B"),
    ];
    let (result, _) = run_archive(root.path(), &archive, &wrong, &fetcher);
    assert!(
        matches!(&result, Err(LocalModelError::ChecksumMismatch(name)) if name == "libb.so"),
        "{result:?}"
    );
    assert_eq!(root_entries(root.path()), vec![".partial-runtime"]);

    // 归档里没有要的成员。
    let root = tempfile::tempdir().unwrap();
    let partial = zip_bytes(&[("jni/arm64-v8a/liba.so", b"library a")]);
    let fetcher = MapFetcher::new([(ARCHIVE_PACK_URL.to_owned(), partial.clone())]);
    let (result, _) = run_archive(root.path(), &partial, &files, &fetcher);
    assert!(
        matches!(result, Err(LocalModelError::MissingFile(_))),
        "{result:?}"
    );

    // 成员比锁文件长：写到锁文件的长度就停。
    let root = tempfile::tempdir().unwrap();
    let long = zip_bytes(&[
        ("jni/arm64-v8a/liba.so", b"library a, and then some"),
        ("jni/arm64-v8a/libb.so", b"library b"),
    ]);
    let fetcher = MapFetcher::new([(ARCHIVE_PACK_URL.to_owned(), long.clone())]);
    let (result, _) = run_archive(root.path(), &long, &files, &fetcher);
    assert!(
        matches!(result, Err(LocalModelError::SizeMismatch(_))),
        "{result:?}"
    );
}

fn adopt_source(directory: &Path) {
    fs::write(directory.join("a.dat"), PACK_A).unwrap();
    fs::write(directory.join("b.txt"), PACK_B).unwrap();
    fs::write(directory.join("unrelated.db"), b"stays").unwrap();
}

#[test]
fn adopting_moves_the_files_without_copying_and_publishes_them() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("resource-packs");
    let source = state.path().join("resources");
    fs::create_dir(&source).unwrap();
    adopt_source(&source);
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(source.join("a.dat")).unwrap().ino()
    };

    let installed = adopt_files(
        &root,
        "pack",
        &pack_files(),
        &serde_json::json!({"pack": "pack"}),
        &source,
    )
    .unwrap();
    assert_eq!(installed, root.join("pack"));
    assert_eq!(
        root_entries(&installed),
        vec!["a.dat", "b.txt", MANIFEST_FILE]
    );
    assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(
        installed_manifest(&root, "pack"),
        Some(serde_json::json!({"pack": "pack"}))
    );
    // 只拿走了资源包的文件。
    assert_eq!(root_entries(&source), vec!["unrelated.db"]);
    assert_eq!(root_entries(&root), vec!["pack"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(installed.join("a.dat")).unwrap().ino(), inode);
    }
}

#[test]
fn a_failed_adoption_puts_every_file_back() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("resource-packs");
    let source = state.path().join("resources");
    fs::create_dir(&source).unwrap();
    adopt_source(&source);
    fs::write(source.join("b.txt"), b"LICENCE").unwrap();

    let result = adopt_files(
        &root,
        "pack",
        &pack_files(),
        &serde_json::json!({"pack": "pack"}),
        &source,
    );
    assert!(
        matches!(&result, Err(LocalModelError::ChecksumMismatch(name)) if name == "b.txt"),
        "{result:?}"
    );
    assert_eq!(
        root_entries(&source),
        vec!["a.dat", "b.txt", "unrelated.db"]
    );
    assert_eq!(fs::read(source.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(fs::read(source.join("b.txt")).unwrap(), b"LICENCE");
    assert!(root_entries(&root).is_empty());
    assert_eq!(installed_manifest(&root, "pack"), None);

    // 少一个文件：什么都不移动。
    fs::remove_file(source.join("b.txt")).unwrap();
    let result = adopt_files(
        &root,
        "pack",
        &pack_files(),
        &serde_json::json!({"pack": "pack"}),
        &source,
    );
    assert!(matches!(result, Err(LocalModelError::MissingFile(name)) if name == "b.txt"));
    assert_eq!(root_entries(&source), vec!["a.dat", "unrelated.db"]);

    // 相对路径的来源目录不接受。
    let result = adopt_files(
        &root,
        "pack",
        &pack_files(),
        &serde_json::json!({"pack": "pack"}),
        Path::new("resources"),
    );
    assert!(matches!(result, Err(LocalModelError::InvalidRoot)));
}

/// 伪造一次在改名之后、发布之前被杀掉的收编：`moved` 里的文件已经从 `source` 挪进暂存目录。
fn interrupt_adoption(root: &Path, source: &Path, moved: &[&str]) -> PathBuf {
    let staging = root.join(".staging-pack-interrupted");
    fs::create_dir_all(staging.join("model")).unwrap();
    fs::write(
        staging.join(ADOPTION_SOURCE),
        source.to_str().unwrap().as_bytes(),
    )
    .unwrap();
    for name in moved {
        fs::rename(source.join(name), staging.join("model").join(name)).unwrap();
    }
    staging
}

#[test]
fn an_interrupted_adoption_is_put_back_and_adopted_on_the_next_try() {
    for moved in [&["a.dat", "b.txt"][..], &["a.dat"][..]] {
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("resource-packs");
        let source = state.path().join("resources");
        fs::create_dir(&source).unwrap();
        adopt_source(&source);
        let staging = interrupt_adoption(&root, &source, moved);

        let installed = adopt_files(
            &root,
            "pack",
            &pack_files(),
            &serde_json::json!({"pack": "pack"}),
            &source,
        )
        .unwrap();
        assert_eq!(fs::read(installed.join("a.dat")).unwrap(), PACK_A);
        assert_eq!(fs::read(installed.join("b.txt")).unwrap(), PACK_B);
        assert_eq!(root_entries(&source), vec!["unrelated.db"]);
        assert!(!staging.exists());
        assert_eq!(root_entries(&root), vec!["pack"]);
    }
}

#[test]
fn installing_puts_back_the_files_of_an_interrupted_adoption() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("resource-packs");
    let source = state.path().join("resources");
    fs::create_dir(&source).unwrap();
    adopt_source(&source);
    // 来源里已有同名文件时不覆盖。
    let staging = interrupt_adoption(&root, &source, &["a.dat", "b.txt"]);
    fs::write(source.join("b.txt"), b"newer").unwrap();

    let fetcher = pack_fetcher("", PACK_B);
    let (result, _) = run_pack(
        &root,
        "pack",
        &pack_files(),
        "",
        &fetcher,
        &AtomicBool::new(false),
    );
    result.unwrap();
    assert_eq!(fs::read(source.join("a.dat")).unwrap(), PACK_A);
    assert_eq!(fs::read(source.join("b.txt")).unwrap(), b"newer");
    assert!(!staging.exists());
}

#[cfg(unix)]
#[test]
fn adoption_refuses_a_symlinked_source_file() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("resource-packs");
    let source = state.path().join("resources");
    fs::create_dir(&source).unwrap();
    adopt_source(&source);
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("b.txt"), PACK_B).unwrap();
    fs::remove_file(source.join("b.txt")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("b.txt"), source.join("b.txt")).unwrap();

    let result = adopt_files(
        &root,
        "pack",
        &pack_files(),
        &serde_json::json!({"pack": "pack"}),
        &source,
    );
    assert!(matches!(result, Err(LocalModelError::UnsafeArchive(name)) if name == "b.txt"));
    assert!(source.join("a.dat").is_file());
    assert_eq!(installed_manifest(&root, "pack"), None);
}

/// Downloads the real default model once. Not run in CI; run by hand with
/// `MSIME_LOCAL_MODEL_ROOT=/tmp/msime-models-rs cargo test -p msime-client-core --lib real_install -- --ignored --nocapture`.
#[test]
#[ignore = "downloads the default model from GitHub"]
fn real_install_of_the_default_model() {
    let root =
        std::env::var("MSIME_LOCAL_MODEL_ROOT").unwrap_or_else(|_| "/tmp/msime-models-rs".into());
    let root = Path::new(&root);
    let mirror = std::env::var("MSIME_LOCAL_MODEL_MIRROR").unwrap_or_default();
    let mut last_stage = "";
    let path = install(
        root,
        default_model_id(),
        &mirror,
        &mut |event| {
            if event.stage != last_stage || event.downloaded == event.total {
                eprintln!("{} {}/{}", event.stage, event.downloaded, event.total);
                last_stage = event.stage;
            }
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    eprintln!("installed {}", path.display());
    assert!(path.join(MANIFEST_FILE).is_file());
    assert!(list(root)
        .iter()
        .any(|model| model.id == default_model_id() && model.installed));
}
