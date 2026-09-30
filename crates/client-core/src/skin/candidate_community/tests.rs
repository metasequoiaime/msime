//! Unit tests for the parent module, in their own file because the packing, installing and transport cases together outweigh the implementation.

use super::*;
use crate::account::{
    AccountChallenge, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const PREVIEW: &str = "preview.png";
const DECORATION: &str = "images/deco.jpg";
const BACKGROUND: &str = "images/bg.png";
const TOP: &str = "preview = 'preview.png'\n";
const DECORATION_IMAGE: &str = "image = 'images/deco.jpg'\n";
const BACKGROUND_TABLE: &str = "[candidate_window.background]\nimage = 'images/bg.png'\n";
const LICENSE: &str = "[license]\ncode = 'MIT'\nassets = 'CC-BY-4.0'\n";

fn png(length: usize) -> Vec<u8> {
    let mut bytes = PNG_MAGIC.to_vec();
    bytes.resize(length.max(PNG_MAGIC.len()), 7);
    bytes
}

fn jpeg(length: usize) -> Vec<u8> {
    let mut bytes = JPEG_MAGIC.to_vec();
    bytes.resize(length.max(JPEG_MAGIC.len()), 9);
    bytes
}

fn manifest(id: &str, top: &str, decoration: &str, background: &str, tail: &str) -> String {
    format!("schema_version = 1\nid = '{id}'\nname = '樱花候选'\nversion = '1.0'\nbase = 'paper'\n{top}[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 12\nwidth_dip = 40\n{decoration}{background}{tail}")
}

/// Write a package under `root/id` with the given manifest parts and the three standard images.
fn write_skin(
    root: &Path,
    id: &str,
    top: &str,
    decoration: &str,
    background: &str,
    tail: &str,
) -> PathBuf {
    let skin = root.join(id);
    fs::create_dir_all(skin.join("images")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        manifest(id, top, decoration, background, tail),
    )
    .unwrap();
    fs::write(skin.join(PREVIEW), png(1000)).unwrap();
    fs::write(skin.join(DECORATION), jpeg(2000)).unwrap();
    fs::write(skin.join(BACKGROUND), png(3000)).unwrap();
    skin
}

fn standard_skin(root: &Path, id: &str) -> PathBuf {
    write_skin(root, id, TOP, DECORATION_IMAGE, BACKGROUND_TABLE, LICENSE)
}

fn download_of(packed: PackedSkin) -> CandidateSkinPackage {
    CandidateSkinPackage {
        id: Uuid::parse_str("20000000-0000-4000-8000-000000000002").unwrap(),
        package_id: packed.package_id,
        manifest: packed.manifest,
        files: packed.files,
    }
}

fn standard_download() -> CandidateSkinPackage {
    let source = tempfile::tempdir().unwrap();
    standard_skin(source.path(), "sakura");
    download_of(pack(source.path(), "sakura").unwrap())
}

fn snapshot(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, directory: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(base).unwrap().to_owned(),
                    fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(directory, directory, &mut out);
    out
}

fn assert_no_helpers(root: &Path) {
    for entry in fs::read_dir(root).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.starts_with('.'), "leftover {name}");
    }
}

#[test]
fn pack_sends_the_manifest_verbatim_and_exactly_the_referenced_images() {
    let root = tempfile::tempdir().unwrap();
    let skin = standard_skin(root.path(), "sakura");
    fs::write(skin.join("images").join("unused.png"), png(10)).unwrap();
    let packed = pack(root.path(), "sakura").unwrap();
    assert_eq!(packed.package_id, "sakura");
    assert_eq!(
        packed.manifest,
        fs::read_to_string(skin.join("skin.toml")).unwrap()
    );
    assert_eq!(
        packed.files.keys().map(String::as_str).collect::<Vec<_>>(),
        [BACKGROUND, DECORATION, PREVIEW]
    );
    for (path, data) in &packed.files {
        assert_eq!(
            BASE64.decode(data).unwrap(),
            fs::read(skin.join(path)).unwrap()
        );
    }
    assert_eq!(packed.size, 6000);
    assert_eq!(packed.file_count, 3);
    assert_eq!(packed.license.assets.as_deref(), Some("CC-BY-4.0"));
    assert_eq!(packed.suggested_name, "樱花候选");
}

#[test]
fn pack_falls_back_to_the_preview_as_decoration_without_duplicating_it() {
    let root = tempfile::tempdir().unwrap();
    write_skin(root.path(), "sakura", TOP, "", "", LICENSE);
    let packed = pack(root.path(), "sakura").unwrap();
    assert_eq!(
        packed.files.keys().map(String::as_str).collect::<Vec<_>>(),
        [PREVIEW]
    );
}

#[test]
fn pack_refuses_what_the_server_would_refuse() {
    let cases: [(&str, &str, &str, &str, &str); 5] = [
        (
            "preview = 'preview.png'\ntoolbar_stylesheet = 'toolbar.css'\n",
            DECORATION_IMAGE,
            BACKGROUND_TABLE,
            LICENSE,
            FILE_TYPE,
        ),
        (
            "preview = 'toolbar.css'\n",
            DECORATION_IMAGE,
            BACKGROUND_TABLE,
            LICENSE,
            FILE_TYPE,
        ),
        (
            "",
            DECORATION_IMAGE,
            BACKGROUND_TABLE,
            LICENSE,
            PREVIEW_REQUIRED,
        ),
        (
            TOP,
            "image = 'images/deco.gif'\n",
            BACKGROUND_TABLE,
            LICENSE,
            FILE_TYPE,
        ),
        (
            TOP,
            DECORATION_IMAGE,
            BACKGROUND_TABLE,
            "",
            LICENSE_REQUIRED,
        ),
    ];
    for (top, decoration, background, tail, expected) in cases {
        let root = tempfile::tempdir().unwrap();
        let skin = write_skin(root.path(), "sakura", top, decoration, background, tail);
        fs::write(skin.join("toolbar.css"), b".toolbar {}").unwrap();
        fs::write(skin.join("images").join("deco.gif"), b"GIF89a").unwrap();
        assert_eq!(
            pack(root.path(), "sakura"),
            Err(expected),
            "{top}{decoration}{tail}"
        );
    }

    let root = tempfile::tempdir().unwrap();
    write_skin(
        root.path(),
        "sakura",
        TOP,
        DECORATION_IMAGE,
        BACKGROUND_TABLE,
        "[license]\ncode = 'MIT'\n",
    );
    assert_eq!(pack(root.path(), "sakura"), Err(LICENSE_REQUIRED));
}

#[test]
fn pack_enforces_the_size_limits_and_image_signatures() {
    let root = tempfile::tempdir().unwrap();
    let skin = standard_skin(root.path(), "sakura");
    fs::write(skin.join(BACKGROUND), png(MAX_PACKAGE_FILE_BYTES + 1)).unwrap();
    assert_eq!(pack(root.path(), "sakura"), Err(TOO_LARGE));

    fs::write(skin.join(BACKGROUND), png(MAX_PACKAGE_FILE_BYTES)).unwrap();
    fs::write(skin.join(DECORATION), jpeg(MAX_PACKAGE_FILE_BYTES)).unwrap();
    assert_eq!(pack(root.path(), "sakura"), Err(TOO_LARGE));

    fs::write(skin.join(DECORATION), jpeg(2000)).unwrap();
    fs::write(skin.join(BACKGROUND), png(3000)).unwrap();
    fs::write(skin.join(PREVIEW), png(MAX_PREVIEW_BYTES + 1)).unwrap();
    assert_eq!(pack(root.path(), "sakura"), Err(TOO_LARGE));

    fs::write(skin.join(PREVIEW), png(MAX_PREVIEW_BYTES)).unwrap();
    fs::write(skin.join(BACKGROUND), jpeg(3000)).unwrap();
    assert_eq!(pack(root.path(), "sakura"), Err(IMAGE_INVALID));

    fs::write(skin.join(BACKGROUND), png(3000)).unwrap();
    assert!(pack(root.path(), "sakura").is_ok());
}

#[test]
fn pack_refuses_server_builtin_and_invalid_packages() {
    let root = tempfile::tempdir().unwrap();
    for id in ["fluent", "wechat", "willow_green"] {
        standard_skin(root.path(), id);
        assert_eq!(pack(root.path(), id), Err(PACKAGE), "{id}");
    }
    let skin = standard_skin(root.path(), "broken");
    fs::write(skin.join("skin.toml"), "schema_version = 2\n").unwrap();
    assert_eq!(pack(root.path(), "broken"), Err(PACKAGE));
    assert_eq!(pack(root.path(), "missing"), Err(PACKAGE));
}

#[cfg(unix)]
#[test]
fn pack_refuses_a_symlinked_image() {
    let root = tempfile::tempdir().unwrap();
    let skin = standard_skin(root.path(), "sakura");
    fs::rename(skin.join(BACKGROUND), skin.join("images").join("real.png")).unwrap();
    std::os::unix::fs::symlink("real.png", skin.join(BACKGROUND)).unwrap();
    assert_eq!(pack(root.path(), "sakura"), Err(PACKAGE));
}

#[test]
fn install_writes_a_package_the_catalog_lists_without_leftovers() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    let package = standard_download();
    assert_eq!(install(&root, &package, false).unwrap(), "sakura");
    let catalog = catalog::scan(&root);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages.len(), 1);
    assert_eq!(catalog.packages[0].id, "sakura");
    assert_eq!(
        fs::read_to_string(root.join("sakura").join("skin.toml")).unwrap(),
        package.manifest
    );
    assert_eq!(
        fs::read(root.join("sakura").join(BACKGROUND)).unwrap(),
        png(3000)
    );
    assert_no_helpers(&root);
}

#[test]
fn install_asks_before_replacing_and_then_replaces_whole() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    fs::create_dir_all(root.join("sakura")).unwrap();
    fs::write(root.join("sakura").join("skin.toml"), b"old").unwrap();
    fs::write(root.join("sakura").join("stale.css"), b"stale").unwrap();
    let before = snapshot(&root);
    let package = standard_download();
    assert_eq!(install(&root, &package, false), Err(EXISTS));
    assert_eq!(snapshot(&root), before);
    assert_no_helpers(&root);

    assert_eq!(install(&root, &package, true).unwrap(), "sakura");
    assert!(!root.join("sakura").join("stale.css").exists());
    assert!(catalog::load_package(&root, "sakura").is_ok());
    assert_no_helpers(&root);
}

#[test]
fn install_rejections_leave_the_previous_skin_untouched() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    fs::create_dir_all(root.join("sakura")).unwrap();
    fs::write(root.join("sakura").join("skin.toml"), b"old").unwrap();
    let before = snapshot(&root);
    let good = standard_download();

    let rekeyed = |from: &str, to: &str| {
        let mut package = good.clone();
        let data = package.files.remove(from).unwrap();
        package.files.insert(to.to_owned(), data);
        package
    };
    let mut cases = vec![
        (rekeyed(BACKGROUND, "../bg.png"), FILE_PATH),
        (rekeyed(BACKGROUND, "/bg.png"), FILE_PATH),
        (rekeyed(BACKGROUND, "images\\bg.png"), FILE_PATH),
        (rekeyed(BACKGROUND, "PREVIEW.PNG"), FILE_PATH),
        (rekeyed(BACKGROUND, "skin.toml"), FILE_PATH),
        (rekeyed(BACKGROUND, "images/bg.svg"), FILE_TYPE),
        (rekeyed(BACKGROUND, "images/other.png"), PACKAGE),
    ];
    let mut oversize = good.clone();
    oversize.files.insert(
        BACKGROUND.to_owned(),
        BASE64.encode(png(MAX_PACKAGE_FILE_BYTES + 1)),
    );
    cases.push((oversize, TOO_LARGE));
    let mut mismatch = good.clone();
    mismatch.package_id = "other".to_owned();
    cases.push((mismatch, PACKAGE));
    let mut reserved = good.clone();
    reserved.package_id = "night".to_owned();
    reserved.manifest = reserved.manifest.replace("id = 'sakura'", "id = 'night'");
    cases.push((reserved, PACKAGE));
    let mut builtin = good.clone();
    builtin.package_id = "wechat".to_owned();
    builtin.manifest = builtin.manifest.replace("id = 'sakura'", "id = 'wechat'");
    cases.push((builtin, PACKAGE));
    let mut missing = good.clone();
    missing.files.remove(BACKGROUND);
    cases.push((missing, PACKAGE));
    let mut magic = good.clone();
    magic
        .files
        .insert(PREVIEW.to_owned(), BASE64.encode(b"not a png"));
    cases.push((magic, IMAGE_INVALID));
    let mut encoding = good.clone();
    encoding
        .files
        .insert(PREVIEW.to_owned(), "not base64!".to_owned());
    cases.push((encoding, IMAGE_INVALID));
    let mut extra_package = {
        let source = tempfile::tempdir().unwrap();
        write_skin(source.path(), "sakura", TOP, DECORATION_IMAGE, "", LICENSE);
        download_of(pack(source.path(), "sakura").unwrap())
    };
    extra_package
        .files
        .insert("images/extra.png".to_owned(), BASE64.encode(png(100)));
    cases.push((extra_package, PACKAGE));

    for (package, expected) in cases {
        let keys: Vec<_> = package.files.keys().cloned().collect();
        assert_eq!(
            install(&root, &package, true),
            Err(expected),
            "{} {keys:?}",
            package.package_id
        );
        assert_eq!(snapshot(&root), before, "{keys:?}");
        assert_no_helpers(&root);
    }
}

#[cfg(unix)]
#[test]
fn install_refuses_a_symlinked_root() {
    let state = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    std::os::unix::fs::symlink(outside.path(), &root).unwrap();
    assert_eq!(install(&root, &standard_download(), false), Err(STORAGE));
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}

#[test]
fn install_clears_what_an_interrupted_install_left_behind() {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("skins");
    fs::create_dir_all(root.join(STAGING_DIRECTORY).join("sakura")).unwrap();
    fs::write(root.join(STAGING_DIRECTORY).join("junk"), b"junk").unwrap();
    fs::create_dir_all(root.join(".replaced-sakura")).unwrap();
    assert_eq!(
        install(&root, &standard_download(), false).unwrap(),
        "sakura"
    );
    assert_no_helpers(&root);
    assert!(catalog::scan(&root).issues.is_empty());
}

fn token(byte: u8) -> String {
    std::iter::repeat_n(char::from(byte), 64).collect()
}

fn tokens(access: u8, refresh: u8) -> AccountTokens {
    AccountTokens {
        access_token: token(access),
        refresh_token: token(refresh),
        token_type: "Bearer".into(),
        expires_in: 900,
        user: AccountUser {
            id: "fixture-user".into(),
            display_name: "Fixture".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        },
    }
}

fn saved(access: u8) -> SavedAccountSession {
    SavedAccountSession {
        tokens: tokens(access, b'b'),
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 60_000,
    }
}

fn item() -> CandidateSkinItem {
    CandidateSkinItem {
        id: Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
        package_id: "sakura".into(),
        name: "樱花候选".into(),
        description: "仅用于协议测试".into(),
        author: "Fixture".into(),
        version: "1.0".into(),
        license: CandidateSkinLicense {
            code: "MIT".into(),
            assets: "CC-BY-4.0".into(),
            source: String::new(),
        },
        size: 6000,
        file_count: 3,
        downloads: 2,
        rating_count: 1,
        rating_average: 4.0,
        owned: false,
        my_rating: 0,
        created_at: "2026-09-30T00:00:00Z".into(),
    }
}

fn publish_request() -> CandidateSkinPublishRequest {
    let mut files = BTreeMap::new();
    files.insert(PREVIEW.to_owned(), BASE64.encode(png(100)));
    CandidateSkinPublishRequest {
        id: item().id,
        name: "樱花候选".into(),
        description: "公开说明".into(),
        manifest: manifest("sakura", TOP, "", "", LICENSE),
        files,
    }
}

#[derive(Clone, Default)]
struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

impl AccountSessionStorage for MemoryStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| AccountError::Storage)
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.0.lock().map_err(|_| AccountError::Storage)? = Some(session.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        *self.0.lock().map_err(|_| AccountError::Storage)? = None;
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeApi {
    calls: Arc<AtomicUsize>,
    refreshes: Arc<AtomicUsize>,
}

impl FakeApi {
    /// Count a call and refuse the stale access token, so the service has to refresh once.
    fn call(&self, bearer: Option<&str>) -> Result<(), AccountError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if bearer == Some(token(b'a').as_str()) {
            return Err(AccountError::Unauthorized);
        }
        Ok(())
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        Ok(HashMap::new())
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        Ok(tokens(b'c', b'd'))
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Ok(())
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Ok(())
    }
}

impl CandidateSkinCommunityApi for FakeApi {
    fn candidate_skins(
        &self,
        _: usize,
        _: &str,
        mine: bool,
        bearer: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError> {
        self.call(bearer)?;
        if mine && bearer.is_none() {
            return Err(AccountError::Unauthorized);
        }
        Ok(CandidateSkinPage {
            skins: vec![item()],
            has_more: false,
        })
    }
    fn candidate_skin(
        &self,
        id: Uuid,
        bearer: Option<&str>,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.call(bearer)?;
        let mut value = item();
        value.id = id;
        Ok(value)
    }
    fn candidate_skin_preview(&self, _: Uuid) -> Result<CandidateSkinPreview, AccountError> {
        self.call(None)?;
        Ok(CandidateSkinPreview {
            path: PREVIEW.into(),
            content_type: "image/png".into(),
            data: BASE64.encode(png(100)),
        })
    }
    fn publish_candidate_skin(
        &self,
        request: &CandidateSkinPublishRequest,
        bearer: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        self.call(Some(bearer))?;
        let mut value = item();
        value.id = request.id;
        Ok(value)
    }
    fn download_candidate_skin(
        &self,
        id: Uuid,
        bearer: &str,
    ) -> Result<CandidateSkinPackage, AccountError> {
        self.call(Some(bearer))?;
        let mut package = standard_download();
        package.id = id;
        Ok(package)
    }
    fn rate_candidate_skin(&self, _: Uuid, _: u8, bearer: &str) -> Result<(), AccountError> {
        self.call(Some(bearer))
    }
    fn unpublish_candidate_skin(&self, _: Uuid, bearer: &str) -> Result<(), AccountError> {
        self.call(Some(bearer))
    }
}

fn service(
    api: &FakeApi,
    storage: MemoryStorage,
) -> BackendCandidateSkinCommunityService<FakeApi, MemoryStorage> {
    let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
    BackendCandidateSkinCommunityService::new(api.clone(), session)
}

#[test]
fn service_refuses_nil_ids_and_anonymous_writes_before_any_transport_call() {
    let api = FakeApi::default();
    let service = service(&api, MemoryStorage::default());
    assert_eq!(service.detail(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.preview(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.download(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.rate(Uuid::nil(), 5), Err(AccountError::Invalid));
    assert_eq!(service.unpublish(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.rate(item().id, 0), Err(AccountError::Invalid));
    let mut request = publish_request();
    request.id = Uuid::nil();
    assert_eq!(service.publish(&request), Err(AccountError::Invalid));
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);

    assert_eq!(
        service.publish(&publish_request()),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(service.download(item().id), Err(AccountError::Unauthorized));
    assert_eq!(service.list(0, "", true), Err(AccountError::Unauthorized));
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn service_reads_anonymously_without_creating_a_session() {
    let api = FakeApi::default();
    let storage = MemoryStorage::default();
    let service = service(&api, storage.clone());
    assert_eq!(service.list(0, "", false).unwrap().skins.len(), 1);
    assert_eq!(service.detail(item().id).unwrap().package_id, "sakura");
    assert_eq!(
        service.preview(item().id).unwrap().content_type,
        "image/png"
    );
    assert!(storage.load().unwrap().is_none());
}

#[test]
fn service_writes_refresh_once_after_unauthorized() {
    let api = FakeApi::default();
    let storage = MemoryStorage::default();
    *storage.0.lock().unwrap() = Some(saved(b'a'));
    let service = service(&api, storage);
    let published = service.publish(&publish_request()).unwrap();
    assert_eq!(published.id, publish_request().id);
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(api.calls.load(Ordering::SeqCst), 2);
    assert_eq!(service.download(item().id).unwrap().id, item().id);
    service.rate(item().id, 4).unwrap();
    service.unpublish(item().id).unwrap();
    assert_eq!(service.list(0, "", true).unwrap().skins.len(), 1);
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(api.calls.load(Ordering::SeqCst), 6);
}

#[test]
fn responses_are_validated() {
    let mut value = item();
    value.package_id = "fluent".into();
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    value = item();
    value.package_id = "Night".into();
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    value = item();
    value.size = MAX_PACKAGE_BYTES as u64 + 1;
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    value = item();
    value.file_count = MAX_PACKAGE_FILES as u64 + 1;
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    value = item();
    value.license.assets = " ".into();
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    value = item();
    value.rating_average = f64::NAN;
    assert_eq!(validate_item(&value), Err(AccountError::Unavailable));
    let page = CandidateSkinPage {
        skins: vec![item(), item()],
        has_more: false,
    };
    assert_eq!(validate_page(&page), Err(AccountError::Unavailable));
    let page = CandidateSkinPage {
        skins: Vec::new(),
        has_more: true,
    };
    assert_eq!(validate_page(&page), Err(AccountError::Unavailable));

    let preview = |path: &str, content_type: &str, bytes: &[u8]| CandidateSkinPreview {
        path: path.into(),
        content_type: content_type.into(),
        data: BASE64.encode(bytes),
    };
    assert!(validate_preview(&preview("p.png", "image/png", &png(10))).is_ok());
    assert!(validate_preview(&preview("p.jpeg", "image/jpeg", &jpeg(10))).is_ok());
    for bad in [
        preview("p.png", "image/jpeg", &png(10)),
        preview("p.png", "image/png", &jpeg(10)),
        preview("p.webp", "image/webp", &png(10)),
        preview("../p.png", "image/png", &png(10)),
        preview("p.png", "image/png", &png(MAX_PREVIEW_BYTES + 1)),
        CandidateSkinPreview {
            path: "p.png".into(),
            content_type: "image/png".into(),
            data: "***".into(),
        },
    ] {
        assert_eq!(validate_preview(&bad), Err(AccountError::Unavailable));
    }
}

/// Serve one request on a loopback port, reading the whole body by its Content-Length, and hand the request back.
fn serve_once(response: Vec<u8>) -> (String, mpsc::Receiver<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut head = String::new();
        let mut length = 0_usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            head.push_str(&line);
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body).unwrap();
        sent.send((head, body)).unwrap();
        let mut stream = reader.into_inner();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
            response.len()
        )
        .unwrap();
        stream.write_all(&response).unwrap();
    });
    (origin, received)
}

#[test]
fn transport_lists_with_scope_and_encoded_search() {
    let response = serde_json::to_vec(&CandidateSkinPage {
        skins: vec![item()],
        has_more: false,
    })
    .unwrap();
    let (origin, received) = serve_once(response);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let page = client
        .candidate_skins(20, "樱 花", true, Some(&token(b'a')))
        .unwrap();
    assert_eq!(page.skins, vec![item()]);
    let (head, _) = received.recv().unwrap();
    assert!(head.starts_with(
        "GET /v1/community/candidate-skins?offset=20&q=%E6%A8%B1%20%E8%8A%B1&scope=mine HTTP/1.1"
    ));
    assert!(head.contains("authorization: Bearer "));
}

#[test]
fn transport_publishes_a_body_larger_than_the_account_default() {
    let mut request = publish_request();
    request.files.clear();
    for path in [PREVIEW, DECORATION, BACKGROUND] {
        request.files.insert(path.to_owned(), "A".repeat(960_000));
    }
    let mut response = item();
    response.id = request.id;
    let (origin, received) = serve_once(serde_json::to_vec(&response).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client
            .publish_candidate_skin(&request, &token(b'c'))
            .unwrap(),
        response
    );
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with("POST /v1/community/candidate-skins HTTP/1.1"));
    assert!(head.contains("authorization: Bearer "));
    assert!(body.len() > 2_880_000);
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["id"], request.id.to_string());
    assert_eq!(body["name"], "樱花候选");
    assert_eq!(body["description"], "公开说明");
    assert_eq!(body["manifest"], request.manifest);
    assert_eq!(body["files"][BACKGROUND].as_str().unwrap().len(), 960_000);
    assert_eq!(body.as_object().unwrap().len(), 5);
}

#[test]
fn transport_accepts_a_download_above_one_mebibyte() {
    let mut package = standard_download();
    package.id = item().id;
    for path in [PREVIEW, DECORATION, BACKGROUND] {
        package.files.insert(path.to_owned(), "A".repeat(1_170_000));
    }
    let (origin, received) = serve_once(serde_json::to_vec(&package).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client
            .download_candidate_skin(item().id, &token(b'd'))
            .unwrap(),
        package
    );
    let (head, _) = received.recv().unwrap();
    assert!(head.starts_with(&format!(
        "POST /v1/community/candidate-skins/{}/download HTTP/1.1",
        item().id.hyphenated()
    )));
}

#[test]
fn transport_rejects_a_mismatched_echo() {
    let mut package = standard_download();
    package.id = Uuid::parse_str("30000000-0000-4000-8000-000000000003").unwrap();
    let (origin, _received) = serve_once(serde_json::to_vec(&package).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client.download_candidate_skin(item().id, &token(b'd')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn other_account_requests_keep_the_one_mebibyte_body_limit() {
    // Nothing listens on this origin: the size check refuses the request before it is sent.
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();
    let body = "x".repeat(1024 * 1024);
    assert_eq!(
        client.json::<serde_json::Value, _>(Method::POST, "/v1/test", None, Some(&body)),
        Err(AccountError::Invalid)
    );
    let mut request = publish_request();
    request
        .files
        .insert(BACKGROUND.to_owned(), "A".repeat(MAX_PUBLISH_BODY_BYTES));
    assert_eq!(
        client.publish_candidate_skin(&request, &token(b'c')),
        Err(AccountError::Invalid)
    );
}
