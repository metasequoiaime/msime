//! Unit tests for the parent module: packing, installing, the service and the transport.

use super::*;
use crate::account::{
    AccountChallenge, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
};
use crate::plugins::{kind_directory, MANIFEST_FILE};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const ROWS: &str = "[[commands]]\ntrigger = 'sig'\ntitle = '签名'\ntemplate = '张三 {date}'\n";

fn command_manifest(id: &str, version: &str) -> String {
    format!("schema_version = 1\nkind = 'command_table'\nid = '{id}'\nname = '签名'\nversion = '{version}'\nlicense = 'CC0-1.0'\ndescription = '常用签名'\n{ROWS}")
}

/// An installed command table under `root`, with a notice file and a hidden file a file manager left behind.
fn installed_commands(root: &Path, id: &str, version: &str) -> PathBuf {
    let pack = kind_directory(root, PluginKind::CommandTable).join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join(MANIFEST_FILE), command_manifest(id, version)).unwrap();
    fs::write(pack.join("README.md"), "签名指令表\n").unwrap();
    fs::write(pack.join(".DS_Store"), [0_u8; 8]).unwrap();
    pack
}

fn publication() -> Uuid {
    Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap()
}

fn download_of(packed: &PackedPlugin) -> CommunityPluginDownload {
    CommunityPluginDownload {
        id: publication(),
        kind: packed.kind,
        plugin_id: packed.plugin_id.clone(),
        version: packed.version.clone(),
        size: packed.archive.len() as u64,
        sha256: packed.sha256.clone(),
        archive: BASE64.encode(&packed.archive),
    }
}

fn packed_signature() -> PackedPlugin {
    let root = tempfile::tempdir().unwrap();
    installed_commands(root.path(), "signature", "1.0");
    pack(root.path(), PluginKind::CommandTable, "signature").unwrap()
}

fn member_names(archive: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect()
}

#[test]
fn pack_builds_the_same_top_level_archive_every_time() {
    let root = tempfile::tempdir().unwrap();
    installed_commands(root.path(), "signature", "1.0");
    let first = pack(root.path(), PluginKind::CommandTable, "signature").unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    let second = pack(root.path(), PluginKind::CommandTable, "signature").unwrap();
    assert_eq!(first, second);
    assert_eq!(member_names(&first.archive), ["README.md", MANIFEST_FILE]);
    assert_eq!(first.file_count, 2);
    assert_eq!(first.plugin_id, "signature");
    assert_eq!(first.version, "1.0");
    assert_eq!(first.license, "CC0-1.0");
    assert_eq!(first.suggested_name, "签名");
    assert_eq!(first.suggested_description, "常用签名");
    assert_eq!(first.sha256, hex::encode(Sha256::digest(&first.archive)));
    let request =
        CommunityPluginPublishRequest::new(publication(), "签名".into(), String::new(), &first);
    assert_eq!(BASE64.decode(&request.archive).unwrap(), first.archive);
    assert_eq!(request.kind, PluginKind::CommandTable);
}

#[test]
fn pack_refuses_effects_built_in_and_invalid_packs() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        pack(root.path(), PluginKind::Effect, "glow")
            .unwrap_err()
            .code,
        KIND
    );
    assert_eq!(
        pack(root.path(), PluginKind::Sound, "default")
            .unwrap_err()
            .code,
        "plugin_reserved"
    );
    assert_eq!(
        pack(root.path(), PluginKind::CommandTable, "missing")
            .unwrap_err()
            .code,
        "plugin_invalid"
    );
    let pack_directory = installed_commands(root.path(), "signature", "1.0");
    fs::write(pack_directory.join("stray.bin"), b"x").unwrap();
    assert_eq!(
        pack(root.path(), PluginKind::CommandTable, "signature")
            .unwrap_err()
            .code,
        "plugin_invalid"
    );
}

#[test]
fn an_archive_past_the_server_limit_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    // A linear congruential sequence, which deflate cannot shrink below the limit.
    let mut state = 0x2545_f491_u32;
    let noise: Vec<u8> = (0..MAX_COMMUNITY_ARCHIVE_BYTES + 1024)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect();
    fs::write(directory.path().join("noise.ogg"), noise).unwrap();
    let names = ["noise.ogg".to_owned()];
    assert_eq!(
        write_archive(directory.path(), names.iter())
            .unwrap_err()
            .code,
        TOO_LARGE
    );
}

#[test]
fn install_imports_a_verified_download() {
    let packed = packed_signature();
    let root = tempfile::tempdir().unwrap();
    let summary = install(
        root.path(),
        &download_of(&packed),
        PluginKind::CommandTable,
        "signature",
    )
    .unwrap();
    assert_eq!(summary.id, "signature");
    assert_eq!(summary.kind(), PluginKind::CommandTable);
    let loaded = load_package(root.path(), None, PluginKind::CommandTable, "signature").unwrap();
    assert_eq!(loaded.version, "1.0");
    assert!(kind_directory(root.path(), PluginKind::CommandTable)
        .join("signature")
        .join("README.md")
        .is_file());
}

#[test]
fn install_refuses_a_download_that_does_not_match_its_listing() {
    let packed = packed_signature();
    let root = tempfile::tempdir().unwrap();
    let installed = installed_commands(root.path(), "signature", "0.9");
    let before = fs::read(installed.join(MANIFEST_FILE)).unwrap();

    let mut wrong_size = download_of(&packed);
    wrong_size.size += 1;
    let mut wrong_sum = download_of(&packed);
    wrong_sum.sha256 = "0".repeat(64);
    let mut not_base64 = download_of(&packed);
    not_base64.archive = "@@@@".into();
    for download in [wrong_size, wrong_sum, not_base64] {
        assert_eq!(
            install(
                root.path(),
                &download,
                PluginKind::CommandTable,
                "signature"
            )
            .unwrap_err()
            .code,
            CHECKSUM
        );
    }

    let mut wrong_version = download_of(&packed);
    wrong_version.version = "2.0".into();
    let mut wrong_id = download_of(&packed);
    wrong_id.plugin_id = "other".into();
    let mut wrong_kind = download_of(&packed);
    wrong_kind.kind = PluginKind::Sound;
    let mut effect = download_of(&packed);
    effect.kind = PluginKind::Effect;
    for download in [wrong_version, wrong_id, wrong_kind, effect] {
        assert_eq!(
            install(root.path(), &download, download.kind, &download.plugin_id)
                .unwrap_err()
                .code,
            MISMATCH
        );
    }
    // The response agrees with itself but names another pack than the listing the user confirmed.
    let self_consistent = download_of(&packed);
    assert_eq!(
        install(
            root.path(),
            &self_consistent,
            PluginKind::CommandTable,
            "other"
        )
        .unwrap_err()
        .code,
        MISMATCH
    );
    assert_eq!(
        install(
            root.path(),
            &self_consistent,
            PluginKind::Sound,
            "signature"
        )
        .unwrap_err()
        .code,
        MISMATCH
    );

    let mut not_a_zip = download_of(&packed);
    let bytes = b"not a zip archive".to_vec();
    not_a_zip.size = bytes.len() as u64;
    not_a_zip.sha256 = hex::encode(Sha256::digest(&bytes));
    not_a_zip.archive = BASE64.encode(&bytes);
    assert_eq!(
        install(
            root.path(),
            &not_a_zip,
            PluginKind::CommandTable,
            "signature"
        )
        .unwrap_err()
        .code,
        "plugin_archive"
    );

    assert_eq!(fs::read(installed.join(MANIFEST_FILE)).unwrap(), before);
}

#[test]
fn install_replaces_an_installed_pack_whole() {
    let packed = packed_signature();
    let root = tempfile::tempdir().unwrap();
    let installed = installed_commands(root.path(), "signature", "0.9");
    fs::write(installed.join("OLD.txt"), "old").unwrap();
    install(
        root.path(),
        &download_of(&packed),
        PluginKind::CommandTable,
        "signature",
    )
    .unwrap();
    let loaded = load_package(root.path(), None, PluginKind::CommandTable, "signature").unwrap();
    assert_eq!(loaded.version, "1.0");
    assert!(!loaded.directory.join("OLD.txt").exists());
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
            email: None,
            avatar_url: None,
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

fn item() -> CommunityPlugin {
    CommunityPlugin {
        id: publication(),
        kind: PluginKind::CommandTable,
        plugin_id: "signature".into(),
        name: "签名".into(),
        description: "常用签名".into(),
        author: "Fixture".into(),
        version: "1.0".into(),
        license: "CC0-1.0".into(),
        size: 512,
        sha256: "a".repeat(64),
        downloads: 3,
        rating_count: 1,
        rating_average: 4.0,
        owned: false,
        my_rating: 0,
        created_at: "2026-09-01T00:00:00Z".into(),
        moderation: None,
    }
}

fn publish_request() -> CommunityPluginPublishRequest {
    CommunityPluginPublishRequest {
        id: publication(),
        name: "签名".into(),
        description: "常用签名".into(),
        kind: PluginKind::CommandTable,
        plugin_id: "signature".into(),
        version: "1.0".into(),
        archive: BASE64.encode(b"PK"),
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

impl CommunityPluginApi for FakeApi {
    fn community_plugins(
        &self,
        _: usize,
        _: &str,
        _: Option<PluginKind>,
        _: bool,
        bearer: Option<&str>,
    ) -> Result<CommunityPluginPage, AccountError> {
        self.call(bearer)?;
        Ok(CommunityPluginPage {
            plugins: vec![item()],
            has_more: false,
            skipped: 0,
        })
    }
    fn community_plugin(
        &self,
        id: Uuid,
        bearer: Option<&str>,
    ) -> Result<CommunityPlugin, AccountError> {
        self.call(bearer)?;
        let mut value = item();
        value.id = id;
        Ok(value)
    }
    fn publish_community_plugin(
        &self,
        request: &CommunityPluginPublishRequest,
        bearer: &str,
    ) -> Result<CommunityPlugin, AccountError> {
        self.call(Some(bearer))?;
        let mut value = item();
        value.id = request.id;
        Ok(value)
    }
    fn download_community_plugin(
        &self,
        id: Uuid,
        bearer: &str,
    ) -> Result<CommunityPluginDownload, AccountError> {
        self.call(Some(bearer))?;
        let mut download = download_of(&packed_signature());
        download.id = id;
        Ok(download)
    }
    fn rate_community_plugin(&self, _: Uuid, _: u8, bearer: &str) -> Result<(), AccountError> {
        self.call(Some(bearer))
    }
    fn delete_community_plugin(&self, _: Uuid, bearer: &str) -> Result<(), AccountError> {
        self.call(Some(bearer))
    }
}

fn service(
    api: &FakeApi,
    storage: MemoryStorage,
) -> BackendCommunityPluginService<FakeApi, MemoryStorage> {
    let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
    BackendCommunityPluginService::new(api.clone(), session)
}

#[test]
fn service_refuses_bad_requests_and_anonymous_writes_before_any_transport_call() {
    let api = FakeApi::default();
    let service = service(&api, MemoryStorage::default());
    assert_eq!(service.detail(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.download(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(service.rate(Uuid::nil(), 5), Err(AccountError::Invalid));
    assert_eq!(service.rate(publication(), 6), Err(AccountError::Invalid));
    assert_eq!(service.delete(Uuid::nil()), Err(AccountError::Invalid));
    assert_eq!(
        service.list(MAX_COMMUNITY_OFFSET + 1, "", None, false),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        service.list(0, &"a".repeat(129), None, false),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        service.list(0, "", Some(PluginKind::Effect), false),
        Err(AccountError::Invalid)
    );
    let mut request = publish_request();
    request.name = " 签名".into();
    assert_eq!(service.publish(&request), Err(AccountError::Invalid));
    let mut request = publish_request();
    request.plugin_id = "default".into();
    request.kind = PluginKind::Sound;
    assert_eq!(service.publish(&request), Err(AccountError::Invalid));
    let mut request = publish_request();
    request.archive = "A".repeat(base64_length(MAX_COMMUNITY_ARCHIVE_BYTES) + 4);
    assert_eq!(service.publish(&request), Err(AccountError::Invalid));
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);

    assert_eq!(
        service.publish(&publish_request()),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(
        service.download(publication()),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(
        service.rate(publication(), 4),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(
        service.delete(publication()),
        Err(AccountError::Unauthorized)
    );
    assert_eq!(api.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn service_reads_anonymously_and_writes_refresh_once() {
    let api = FakeApi::default();
    let storage = MemoryStorage::default();
    let anonymous = service(&api, storage.clone());
    assert_eq!(anonymous.list(0, "", None, false).unwrap().plugins.len(), 1);
    assert_eq!(
        anonymous.detail(publication()).unwrap().plugin_id,
        "signature"
    );
    assert!(storage.load().unwrap().is_none());

    let api = FakeApi::default();
    let storage = MemoryStorage::default();
    *storage.0.lock().unwrap() = Some(saved(b'a'));
    let signed_in = service(&api, storage);
    assert_eq!(
        signed_in.publish(&publish_request()).unwrap().id,
        publication()
    );
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(api.calls.load(Ordering::SeqCst), 2);
    assert_eq!(signed_in.download(publication()).unwrap().id, publication());
    signed_in.rate(publication(), 4).unwrap();
    signed_in.delete(publication()).unwrap();
    assert_eq!(api.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(api.calls.load(Ordering::SeqCst), 5);
}

#[test]
fn responses_are_validated() {
    assert!(validate_item(&item()).is_ok());
    let mut effect = item();
    effect.kind = PluginKind::Effect;
    let mut builtin = item();
    builtin.kind = PluginKind::Music;
    builtin.plugin_id = "msime-music-lofi".into();
    let mut oversized = item();
    oversized.size = MAX_COMMUNITY_ARCHIVE_BYTES as u64 + 1;
    let mut empty = item();
    empty.size = 0;
    let mut sum = item();
    sum.sha256 = "A".repeat(64);
    let mut rating = item();
    rating.my_rating = 6;
    let mut license = item();
    license.license = String::new();
    for bad in [effect, builtin, oversized, empty, sum, rating, license] {
        assert_eq!(validate_item(&bad), Err(AccountError::Unavailable));
    }

    let duplicate = CommunityPluginPage {
        plugins: vec![item(), item()],
        has_more: false,
        skipped: 0,
    };
    let endless = CommunityPluginPage {
        plugins: Vec::new(),
        has_more: true,
        skipped: 0,
    };
    let long = CommunityPluginPage {
        plugins: (0..=MAXIMUM_PAGE_ITEMS)
            .map(|index| {
                let mut value = item();
                value.id = Uuid::from_u128(index as u128 + 1);
                value
            })
            .collect(),
        has_more: false,
        skipped: 0,
    };
    for page in [duplicate, endless, long] {
        assert_eq!(validate_page(page), Err(AccountError::Unavailable));
    }

    let packed = packed_signature();
    assert!(validate_download(&download_of(&packed)).is_ok());
    let mut nil = download_of(&packed);
    nil.id = Uuid::nil();
    let mut sum = download_of(&packed);
    sum.sha256 = "z".repeat(64);
    let mut empty = download_of(&packed);
    empty.archive.clear();
    for bad in [nil, sum, empty] {
        assert_eq!(validate_download(&bad), Err(AccountError::Unavailable));
    }
}

#[test]
fn items_with_unknown_fields_are_refused() {
    let mut value = serde_json::to_value(item()).unwrap();
    assert_eq!(value["kind"], "command_table");
    value["extra"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<CommunityPlugin>(value).is_err());
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
fn transport_lists_by_kind_with_an_encoded_search() {
    let response =
        serde_json::to_vec(&serde_json::json!({ "plugins": [item()], "has_more": false })).unwrap();
    let (origin, received) = serve_once(response);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let page = client
        .community_plugins(20, "签 名", Some(PluginKind::CommandTable), false, None)
        .unwrap();
    assert_eq!(page.plugins, vec![item()]);
    let (head, _) = received.recv().unwrap();
    assert!(head.starts_with(
        "GET /v1/community/plugins?offset=20&q=%E7%AD%BE%20%E5%90%8D&kind=command_table HTTP/1.1"
    ));
    assert!(!head.contains("authorization:"));
}

#[test]
fn transport_lists_the_installable_items_of_a_page_that_also_holds_effect_packs() {
    // The server lists effect packs too, which this client cannot install; they are left out and counted, and the rest of the page still reads.
    let mut effect = serde_json::to_value(item()).unwrap();
    effect["id"] = "10000000-0000-4000-8000-000000000002".into();
    effect["kind"] = "effect".into();
    effect["plugin_id"] = "sparkle".into();
    let mut sound = item();
    sound.kind = PluginKind::Sound;
    sound.plugin_id = "rain".into();
    let response = serde_json::to_vec(&serde_json::json!({
        "plugins": [effect, serde_json::to_value(&sound).unwrap()],
        "has_more": true,
    }))
    .unwrap();
    let (origin, _received) = serve_once(response);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let page = client.community_plugins(0, "", None, false, None).unwrap();
    assert_eq!(page.plugins, vec![sound]);
    assert!(page.has_more);
    assert_eq!(page.skipped, 1);
}

#[test]
fn a_page_of_only_effect_packs_still_pages_on() {
    let mut effect = item();
    effect.kind = PluginKind::Effect;
    let page = validate_page(CommunityPluginPage {
        plugins: vec![effect],
        has_more: true,
        skipped: 0,
    })
    .unwrap();
    assert!(page.plugins.is_empty());
    assert!(page.has_more);
    assert_eq!(page.skipped, 1);
}

#[test]
fn a_page_cannot_claim_skipped_items_itself() {
    let value = serde_json::json!({ "plugins": [], "has_more": false, "skipped": 3 });
    assert!(serde_json::from_value::<CommunityPluginPage>(value).is_err());
}

#[test]
fn transport_publishes_an_archive_larger_than_the_account_default() {
    let mut request = publish_request();
    request.archive = "A".repeat(4_000_000);
    let (origin, received) = serve_once(serde_json::to_vec(&item()).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client
            .publish_community_plugin(&request, &token(b'c'))
            .unwrap(),
        item()
    );
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with("POST /v1/community/plugins HTTP/1.1"));
    assert!(head.contains("authorization: Bearer "));
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["id"], request.id.to_string());
    assert_eq!(body["name"], "签名");
    assert_eq!(body["description"], "常用签名");
    assert_eq!(body["kind"], "command_table");
    assert_eq!(body["plugin_id"], "signature");
    assert_eq!(body["version"], "1.0");
    assert_eq!(body["archive"].as_str().unwrap().len(), 4_000_000);
    assert_eq!(body.as_object().unwrap().len(), 7);
}

#[test]
fn transport_downloads_with_an_empty_object_and_checks_the_echo() {
    let download = download_of(&packed_signature());
    let (origin, received) = serve_once(serde_json::to_vec(&download).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client
            .download_community_plugin(publication(), &token(b'd'))
            .unwrap(),
        download
    );
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with(&format!(
        "POST /v1/community/plugins/{}/download HTTP/1.1",
        publication().hyphenated()
    )));
    assert_eq!(body, b"{}");

    let mut other = download_of(&packed_signature());
    other.id = Uuid::from_u128(7);
    let (origin, _received) = serve_once(serde_json::to_vec(&other).unwrap());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client.download_community_plugin(publication(), &token(b'd')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn transport_rates_and_deletes_by_id() {
    let (origin, received) = serve_once(br#"{"stars":4}"#.to_vec());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    client
        .rate_community_plugin(publication(), 4, &token(b'e'))
        .unwrap();
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with(&format!(
        "PUT /v1/community/plugins/{}/rating HTTP/1.1",
        publication().hyphenated()
    )));
    assert_eq!(body, br#"{"stars":4}"#);

    let (origin, received) = serve_once(br#"{"deleted":true}"#.to_vec());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    client
        .delete_community_plugin(publication(), &token(b'e'))
        .unwrap();
    let (head, _) = received.recv().unwrap();
    assert!(head.starts_with(&format!(
        "DELETE /v1/community/plugins/{} HTTP/1.1",
        publication().hyphenated()
    )));

    let (origin, _received) = serve_once(br#"{"deleted":false}"#.to_vec());
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(
        client.delete_community_plugin(publication(), &token(b'e')),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn the_own_list_asks_for_the_moderation_state_and_reads_it() {
    let mut own = serde_json::to_value(item()).unwrap();
    own["owned"] = true.into();
    own["moderation"] = "removed".into();
    let response =
        serde_json::to_vec(&serde_json::json!({ "plugins": [own], "has_more": false })).unwrap();
    let (origin, received) = serve_once(response);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let page = client
        .community_plugins(0, "", None, true, Some(&"a".repeat(43)))
        .unwrap();
    assert_eq!(
        page.plugins[0].moderation,
        Some(crate::community::CommunityModeration::Removed)
    );
    assert!(page.plugins[0].moderation.unwrap().is_removed());
    // A state a newer server adds still reads.
    assert_eq!(
        serde_json::from_value::<crate::community::CommunityModeration>("frozen".into()).unwrap(),
        crate::community::CommunityModeration::Unknown
    );
    let (head, _) = received.recv().unwrap();
    assert!(head.starts_with(
        "GET /v1/community/plugins?offset=0&q=&scope=mine&fields=moderation HTTP/1.1"
    ));
    // Without the field the item serializes exactly as before.
    assert!(serde_json::to_value(item())
        .unwrap()
        .get("moderation")
        .is_none());
    assert_eq!(
        client.community_plugins(0, "", None, true, None),
        Err(AccountError::Unauthorized)
    );
}
