use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// Serves fixed bytes per URL and records what was asked for.
struct MapFetcher {
    files: HashMap<String, Vec<u8>>,
    requested: Mutex<Vec<String>>,
}

impl MapFetcher {
    fn new(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            files: files.into_iter().collect(),
            requested: Mutex::new(Vec::new()),
        }
    }
}

impl Fetcher for MapFetcher {
    fn fetch<'a>(&'a self, url: &str) -> Result<Box<dyn Read + 'a>, LocalModelError> {
        self.requested.lock().unwrap().push(url.to_owned());
        self.files
            .get(url)
            .map(|bytes| Box::new(bytes.as_slice()) as Box<dyn Read + 'a>)
            .ok_or(LocalModelError::HttpStatus(404))
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
