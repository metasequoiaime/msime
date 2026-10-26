//! 内存里的假网盘：语义照 WebDAV（MKCOL、PUT、GET、DELETE、MOVE 带 `Overwrite: F`、PROPFIND Depth 1），可以注入网盘真实会出的毛病——MOVE 先查后写不原子、不理会 `Overwrite: F`、新文件过一会儿才列得出来、429 限流、拒绝鉴权——用来证明日志在这些情况下的行为，以及「测试连接」能把不合格的网盘拦下来。

use super::*;
use crate::account::AccountClipboardE2eStatus;
use crate::clipboard_relay::RelayClipboard;
use sha2::Digest as _;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};

const URL: &str = "https://dav.example.test/dav";
const USER: &str = "synthetic@example.test";
const PASSWORD: &str = "synthetic-app-password";
const START_MS: u64 = 1_790_000_000_000;

#[derive(Default)]
struct Faults {
    /// MOVE 先查目标在不在，放开锁，等到同一目标的所有 MOVE 都查过以后再写：几个并发的 MOVE 都会成功。
    racy_move: Option<(String, Arc<Barrier>)>,
    /// MOVE 不理会 `Overwrite: F`，直接覆盖（坚果云对 `PUT If-None-Match: *` 就是这样）。
    ignore_overwrite: bool,
    /// 新建的文件在之后这么多次 PROPFIND 里列不出来。
    hide_new_for: u32,
    /// 接下来这么多个请求答 429。
    rate_limited: u32,
    retry_after: Option<u64>,
    unauthorized: bool,
    /// MKCOL 答 403（例如只读的共享文件夹）。
    forbid_mkcol: bool,
    /// 连不上：请求根本没有响应。
    unreachable: bool,
}

#[derive(Default)]
struct Disk {
    files: BTreeMap<String, Vec<u8>>,
    /// 集合，`""` 是用户选的基地址。
    dirs: BTreeSet<String>,
    hidden: BTreeMap<String, u32>,
    /// 每个请求：方法、路径和状态码。
    log: Vec<(String, String, u16)>,
    faults: Faults,
}

/// 假网盘。每个请求让时钟走一秒，几台设备交替操作时本机缓存的「刚读过」很快过期，与真实的使用节奏相近。
#[derive(Clone)]
struct FakeDav {
    disk: Arc<Mutex<Disk>>,
    clock: Clock,
}

impl FakeDav {
    fn new(clock: &Clock) -> Self {
        let mut disk = Disk::default();
        disk.dirs.insert(String::new());
        Self {
            disk: Arc::new(Mutex::new(disk)),
            clock: clock.clone(),
        }
    }

    fn with<T>(&self, edit: impl FnOnce(&mut Disk) -> T) -> T {
        edit(&mut self.disk.lock().unwrap())
    }

    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.with(|disk| disk.files.clone())
    }

    fn requests(&self) -> Vec<(String, String, u16)> {
        self.with(|disk| disk.log.clone())
    }

    fn clear_log(&self) {
        self.with(|disk| disk.log.clear());
    }
}

fn parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(index) => trimmed[..=index].to_owned(),
        None => String::new(),
    }
}

fn respond(status: u16) -> DavResponse {
    DavResponse {
        status,
        retry_after: None,
        body: Vec::new(),
    }
}

impl Disk {
    fn created(&mut self, path: &str) {
        if self.faults.hide_new_for > 0 {
            self.hidden
                .insert(path.to_owned(), self.faults.hide_new_for);
        }
    }

    fn listing(&mut self, dir: &str) -> Vec<u8> {
        let children: Vec<(String, bool)> = self
            .dirs
            .iter()
            .filter(|path| !path.is_empty() && parent(path) == dir && path.as_str() != dir)
            .map(|path| (path.clone(), true))
            .chain(
                self.files
                    .keys()
                    .filter(|path| parent(path) == dir)
                    .map(|path| (path.clone(), false)),
            )
            .collect();
        // 两种常见的写法：坚果云给完整地址、前缀 `D:`；sabre/dav 给绝对路径、前缀 `d:`。
        let mut xml =
            String::from(r#"<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:">"#);
        xml.push_str(&format!(
            "<D:response><D:href>/dav/{dir}</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
        ));
        for (index, (path, collection)) in children.into_iter().enumerate() {
            if let Some(remaining) = self.hidden.get_mut(&path) {
                if *remaining > 0 {
                    *remaining -= 1;
                    continue;
                }
            }
            let href = if index % 2 == 0 {
                format!("https://dav.example.test/dav/{path}")
            } else {
                format!("/dav/{path}")
            };
            let kind = if collection {
                "<d:resourcetype><d:collection/></d:resourcetype>"
            } else {
                "<d:resourcetype/>"
            };
            xml.push_str(&format!(
                r#"<d:response xmlns:d="DAV:"><d:href>{href}</d:href><d:propstat><d:prop>{kind}<d:getetag>"x"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"#
            ));
        }
        xml.push_str("</D:multistatus>");
        xml.into_bytes()
    }
}

impl DavHttp for FakeDav {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        self.clock.advance(1000);
        let mut disk = self.disk.lock().unwrap();
        if disk.faults.unreachable {
            return Err(RelayError::Unavailable);
        }
        let response = self.answer(&mut disk, request);
        let (response, disk) = match response {
            Answer::Now(response) => (response, disk),
            Answer::RacyMove { from, to, barrier } => {
                drop(disk);
                barrier.wait();
                let mut disk = self.disk.lock().unwrap();
                let bytes = disk.files.remove(&from).unwrap_or_default();
                disk.files.insert(to.clone(), bytes);
                disk.created(&to);
                (respond(201), disk)
            }
        };
        let mut disk = disk;
        disk.log.push((
            request.method.to_owned(),
            request.path.to_owned(),
            response.status,
        ));
        Ok(response)
    }
}

enum Answer {
    Now(DavResponse),
    RacyMove {
        from: String,
        to: String,
        barrier: Arc<Barrier>,
    },
}

impl FakeDav {
    fn answer(&self, disk: &mut Disk, request: &DavRequest<'_>) -> Answer {
        if disk.faults.unauthorized {
            return Answer::Now(respond(401));
        }
        if disk.faults.forbid_mkcol && request.method == "MKCOL" {
            return Answer::Now(respond(403));
        }
        if disk.faults.rate_limited > 0 {
            disk.faults.rate_limited -= 1;
            return Answer::Now(DavResponse {
                status: 429,
                retry_after: disk.faults.retry_after,
                body: Vec::new(),
            });
        }
        let path = request.path.to_owned();
        let status = match request.method {
            "MKCOL" => {
                if disk.dirs.contains(&path) || disk.files.contains_key(&path) {
                    405
                } else if !disk.dirs.contains(&parent(&path)) {
                    409
                } else {
                    disk.dirs.insert(path.clone());
                    disk.created(&path);
                    201
                }
            }
            "PUT" => {
                if !disk.dirs.contains(&parent(&path)) {
                    409
                } else {
                    let bytes = request.body.unwrap_or_default().to_vec();
                    let existed = disk.files.insert(path.clone(), bytes).is_some();
                    disk.created(&path);
                    if existed {
                        204
                    } else {
                        201
                    }
                }
            }
            "GET" => {
                return Answer::Now(match disk.files.get(&path) {
                    Some(bytes) => DavResponse {
                        status: 200,
                        retry_after: None,
                        body: bytes.clone(),
                    },
                    None => respond(404),
                })
            }
            "DELETE" => {
                if disk.dirs.contains(&path) && !path.is_empty() {
                    disk.dirs.retain(|dir| !dir.starts_with(&path));
                    disk.files.retain(|file, _| !file.starts_with(&path));
                    204
                } else if disk.files.remove(&path).is_some() {
                    204
                } else {
                    404
                }
            }
            "MOVE" => {
                let to = request.destination.unwrap_or_default().to_owned();
                if !disk.files.contains_key(&path) {
                    404
                } else if !disk.dirs.contains(&parent(&to)) {
                    409
                } else if disk.files.contains_key(&to) && !disk.faults.ignore_overwrite {
                    412
                } else if let Some((suffix, barrier)) = disk
                    .faults
                    .racy_move
                    .clone()
                    .filter(|(suffix, _)| to.ends_with(suffix.as_str()))
                {
                    let _ = suffix;
                    return Answer::RacyMove {
                        from: path,
                        to,
                        barrier,
                    };
                } else {
                    let bytes = disk.files.remove(&path).unwrap_or_default();
                    disk.files.insert(to.clone(), bytes);
                    disk.created(&to);
                    201
                }
            }
            "PROPFIND" => {
                if !disk.dirs.contains(&path) {
                    404
                } else {
                    return Answer::Now(DavResponse {
                        status: 207,
                        retry_after: None,
                        body: disk.listing(&path),
                    });
                }
            }
            _ => 405,
        };
        Answer::Now(respond(status))
    }
}

/// 假时钟：等待不真的等，只记下来并让时间往前走。
#[derive(Clone)]
struct Clock(Arc<(AtomicU64, Mutex<Vec<Duration>>)>);

impl Clock {
    fn new() -> Self {
        Self(Arc::new((AtomicU64::new(START_MS), Mutex::new(Vec::new()))))
    }

    fn advance(&self, ms: u64) {
        self.0 .0.fetch_add(ms, Ordering::SeqCst);
    }

    fn sleeps(&self) -> Vec<Duration> {
        self.0 .1.lock().unwrap().clone()
    }
}

impl Environment for Clock {
    fn now_ms(&self) -> u64 {
        self.0 .0.load(Ordering::SeqCst)
    }

    fn sleep(&self, duration: Duration) {
        self.0 .1.lock().unwrap().push(duration);
        self.advance(duration.as_millis() as u64);
    }
}

fn config() -> RelayConfig {
    RelayConfig::webdav(URL, USER, PASSWORD).unwrap()
}

fn relay(drive: &FakeDav, directory: &Path, locks: bool) -> VirtualRelay {
    VirtualRelay::with_parts(
        config(),
        directory.to_path_buf(),
        Box::new(drive.clone()),
        Box::new(drive.clock.clone()),
        locks,
    )
}

fn device(drive: &FakeDav, directory: &Path, name: &str) -> RelayClipboard {
    RelayClipboard::with_webdav(relay(drive, directory, false), directory, name)
}

fn texts(device: &RelayClipboard, search: &str) -> Vec<String> {
    device
        .list(search)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.text)
        .collect()
}

/// 两台设备：笔记本建钥，手机经它批准。
fn paired(drive: &FakeDav, root: &Path) -> (RelayClipboard, RelayClipboard) {
    let laptop = device(drive, &root.join("laptop"), "合成笔记本");
    let phone = device(drive, &root.join("phone"), "合成手机");
    laptop.list("").unwrap();
    phone.list("").unwrap();
    let entry = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    laptop.approve(&entry.device_id).unwrap();
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    (laptop, phone)
}

fn methods(drive: &FakeDav) -> Vec<String> {
    drive
        .requests()
        .into_iter()
        .map(|(method, _, _)| method)
        .collect()
}

// ---- 完整流程（与参考中继的 `tests/flow.rs` 相同） ----

#[test]
fn devices_pair_share_and_manage_a_clipboard_through_the_drive() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    let phone = device(&drive, &root.path().join("phone"), "合成手机");

    // 第一台建钥，马上可用。
    let page = laptop.list("").unwrap();
    assert!(page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert!(page.items.is_empty());

    // 第二台登记后等待批准，不能上传。
    let pending = phone.list("").unwrap();
    assert_eq!(pending.e2e.status, AccountClipboardE2eStatus::Pending);
    assert_eq!(
        phone.add("合成上传").unwrap_err().code(),
        "clipboard_device_pending"
    );
    let laptop_view = laptop.devices().unwrap();
    assert_eq!(laptop_view.devices.len(), 2);
    let phone_entry = laptop_view
        .devices
        .iter()
        .find(|entry| !entry.current)
        .unwrap()
        .clone();
    assert!(!phone_entry.approved);
    assert_eq!(phone_entry.name, "合成手机");
    assert_eq!(phone_entry.code, pending.e2e.code);

    // 批准之后互传。
    let approved = laptop.approve(&phone_entry.device_id).unwrap();
    assert!(approved.devices.iter().all(|entry| entry.approved));
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    laptop.add("合成的第一条").unwrap();
    phone.add("second synthetic line\n第二行").unwrap();
    assert_eq!(
        texts(&laptop, ""),
        ["second synthetic line\n第二行", "合成的第一条"]
    );
    // 相同内容只保留一条，并移到最前；搜索在本机解密后进行。
    phone.add("合成的第一条").unwrap();
    assert_eq!(
        texts(&phone, ""),
        ["合成的第一条", "second synthetic line\n第二行"]
    );
    assert_eq!(texts(&phone, "SECOND"), ["second synthetic line\n第二行"]);

    // 置顶、保留期、删除一条。
    let items = laptop.list("").unwrap().items;
    let older = items.last().unwrap().clone();
    phone.set_pinned(&older.id, true).unwrap();
    let page = laptop.list("").unwrap();
    assert_eq!(page.items[0].id, older.id);
    assert!(page.items[0].pinned);
    laptop.set_retention(7).unwrap();
    assert_eq!(phone.list("").unwrap().retention_days, 7);
    assert_eq!(laptop.set_retention(5), Err(RelayError::Invalid));
    phone.delete(&older.id).unwrap();
    assert_eq!(texts(&laptop, ""), ["合成的第一条"]);
    assert_eq!(phone.delete(&older.id).unwrap_err().code(), "not_found");

    // 移除手机：换钥，条目清空，手机回到待批准。
    let removed = laptop.remove(&phone_entry.device_id).unwrap();
    assert_eq!(removed.devices.len(), 1);
    assert!(laptop.list("").unwrap().items.is_empty());
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    let phone_again = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert_eq!(phone_again.device_id, phone_entry.device_id);
    laptop.approve(&phone_again.device_id).unwrap();
    phone.add("移除后的一条").unwrap();
    assert_eq!(texts(&laptop, ""), ["移除后的一条"]);

    // 手机重置：只有它自己持有新密钥，笔记本要重新批准。
    let reset = phone.reset().unwrap();
    assert_eq!(reset.status, AccountClipboardE2eStatus::Ready);
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    let laptop_id = reset
        .devices
        .iter()
        .find(|entry| !entry.current)
        .unwrap()
        .device_id
        .clone();
    phone.approve(&laptop_id).unwrap();

    // 清空只删条目，设备和密钥都在。
    laptop.add("清空前").unwrap();
    phone.clear().unwrap();
    let page = laptop.list("").unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);

    // 关闭开关删掉一切；再打开时第一台设备重新建钥。
    laptop.set_enabled(false).unwrap();
    let page = phone.list("").unwrap();
    assert!(!page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Disabled);
    assert_eq!(
        phone.add("关闭时上传").unwrap_err().code(),
        "clipboard_sync_disabled"
    );
    laptop.set_enabled(true).unwrap();
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
}

#[test]
fn a_fully_pinned_clipboard_reports_full_and_still_accepts_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let laptop = device(&drive, &root.path().join("laptop"), "合成设备");
    for index in 0..crate::cloud::clipboard_log::MAX_ITEMS {
        let item = laptop.add(&format!("合成条目 {index}")).unwrap();
        laptop.set_pinned(&item.id, true).unwrap();
    }
    assert_eq!(
        laptop.add("合成超限条目").unwrap_err().code(),
        "clipboard_full"
    );
    let before = laptop.list("").unwrap().items;
    assert_eq!(before.len(), crate::cloud::clipboard_log::MAX_ITEMS);
    let duplicate = before.last().unwrap();
    let uploaded = laptop.add(&duplicate.text).unwrap();
    assert_eq!(uploaded.id, duplicate.id);
    assert!(uploaded.pinned);
}

// ---- 网盘上的布局 ----

#[test]
fn the_drive_holds_only_numbered_files_and_never_plaintext_or_ids_in_names() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let (laptop, phone) = paired(&drive, root.path());
    let item = laptop.add("合成的秘密内容 synthetic-secret").unwrap();
    phone.set_pinned(&item.id, true).unwrap();
    let devices = laptop.devices().unwrap().devices;
    let files = drive.files();
    let format: serde_json::Value =
        serde_json::from_slice(&files["msime-clipboard/v1/format.json"]).unwrap();
    assert_eq!(format, json!({ "format": "msime-clipboard", "version": 1 }));
    for (path, bytes) in &files {
        let name = path.rsplit('/').next().unwrap();
        assert!(
            path == "msime-clipboard/v1/format.json"
                || (path.starts_with("msime-clipboard/v1/epoch-")
                    && (name == "snapshot.json" || event_name(name).is_some())),
            "{path}"
        );
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("synthetic-secret") && !text.contains("合成的秘密"));
        assert!(!path.contains(&item.id));
        for entry in &devices {
            assert!(!path.contains(&entry.device_id));
        }
    }
    // 临时目录里没有残留。
    assert!(files.keys().all(|path| !path.contains("/tmp/")));
}

#[test]
fn a_long_log_is_compacted_and_a_fresh_device_starts_from_the_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    laptop.list("").unwrap();
    for index in 0..70 {
        laptop.add(&format!("合成条目 {}", index % 5)).unwrap();
    }
    let epochs: BTreeSet<String> = drive
        .files()
        .keys()
        .filter_map(|path| path.split('/').nth(2).map(str::to_owned))
        .filter(|segment| segment.starts_with("epoch-"))
        .collect();
    assert!(epochs.len() >= 2, "{epochs:?}");
    // 宽限期过后，下一次操作删掉旧的 epoch。
    clock.advance((OLD_EPOCH_GRACE_SECS as u64 + 1) * 1000);
    laptop.list("").unwrap();
    let files = drive.files();
    assert!(!files
        .keys()
        .any(|path| path.starts_with("msime-clipboard/v1/epoch-000000/")));
    let newest = files
        .keys()
        .filter(|path| path.ends_with("/snapshot.json"))
        .max()
        .unwrap()
        .clone();

    // 同一个设备身份、但没有本机缓存的新安装：只读最新的快照和其后的事件，内容相同。
    let fresh_directory = root.path().join("fresh");
    std::fs::create_dir_all(&fresh_directory).unwrap();
    std::fs::copy(
        root.path()
            .join("laptop")
            .join(crate::clipboard_relay::RELAY_KEYSTORE_FILE),
        fresh_directory.join(crate::clipboard_relay::RELAY_KEYSTORE_FILE),
    )
    .unwrap();
    let fresh = device(&drive, &fresh_directory, "合成笔记本");
    drive.clear_log();
    assert_eq!(texts(&fresh, ""), texts(&laptop, ""));
    let gets: Vec<String> = drive
        .requests()
        .into_iter()
        .filter(|(method, _, _)| method == "GET")
        .map(|(_, path, _)| path)
        .collect();
    assert!(gets.contains(&newest));
    assert!(gets.iter().all(|path| !path.contains("epoch-000000/e-")));
}

#[test]
fn clearing_leaves_no_old_ciphertext_on_the_drive_after_the_grace_period() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    let item = laptop.add("合成清空前").unwrap();
    let ciphertext_marker = {
        let files = drive.files();
        let event = files
            .iter()
            .filter(|(path, _)| event_name(path.rsplit('/').next().unwrap()).is_some())
            .find(|(_, bytes)| String::from_utf8_lossy(bytes).contains("\"op\":\"add\""))
            .unwrap()
            .1
            .clone();
        let event: serde_json::Value = serde_json::from_slice(&event).unwrap();
        event["ciphertext"].as_str().unwrap().to_owned()
    };
    assert_eq!(item.text, "合成清空前");
    laptop.clear().unwrap();
    clock.advance((OLD_EPOCH_GRACE_SECS as u64 + 1) * 1000);
    laptop.list("").unwrap();
    assert!(drive
        .files()
        .values()
        .all(|bytes| !String::from_utf8_lossy(bytes).contains(&ciphertext_marker)));
}

// ---- 并发 ----

#[test]
fn a_device_whose_cache_is_behind_loses_the_number_and_writes_after_the_others() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let first = relay(&drive, &root.path().join("first"), true);
    let second = relay(&drive, &root.path().join("second"), true);
    first.send("GET", "/v1/users/me/clipboard", None).unwrap();
    second.send("GET", "/v1/users/me/clipboard", None).unwrap();
    let retention = |relay: &VirtualRelay, days: u32| {
        relay
            .send(
                "PUT",
                "/v1/users/me/clipboard/retention",
                Some(&format!(r#"{{"days":{days}}}"#)),
            )
            .unwrap()
            .status
    };
    assert_eq!(retention(&second, 7), 204);
    drive.clear_log();
    // 第一台的缓存落后一个事件：它抢的序号已经有人了，MOVE 被拒绝，重读之后接在后面写。
    assert_eq!(retention(&first, 30), 204);
    assert!(drive
        .requests()
        .iter()
        .any(|(method, _, status)| method == "MOVE" && *status == 412));
    drive.clock.advance(10_000);
    let page = second.send("GET", "/v1/users/me/clipboard", None).unwrap();
    let page: serde_json::Value = serde_json::from_str(&page.body).unwrap();
    assert_eq!(page["retention_days"], 30);
}

#[test]
fn concurrent_uploads_from_several_devices_all_land_once() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let (laptop, phone) = paired(&drive, root.path());
    let tablet = device(&drive, &root.path().join("tablet"), "合成平板");
    tablet.list("").unwrap();
    let entry = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| entry.name == "合成平板")
        .unwrap();
    laptop.approve(&entry.device_id).unwrap();
    std::thread::scope(|scope| {
        for (device, prefix) in [(&laptop, "甲"), (&phone, "乙"), (&tablet, "丙")] {
            scope.spawn(move || {
                for index in 0..4 {
                    device.add(&format!("合成{prefix}{index}")).unwrap();
                }
            });
        }
    });
    let mut all = texts(&laptop, "");
    all.sort();
    assert_eq!(all.len(), 12);
    all.dedup();
    assert_eq!(all.len(), 12);
}

#[test]
fn a_drive_that_lets_two_moves_win_is_caught_on_the_next_read() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let (laptop, phone) = paired(&drive, root.path());
    laptop.list("").unwrap();
    phone.list("").unwrap();
    // 两台设备抢同一个序号，而网盘的 MOVE 先查后写：两边都以为自己写成了。
    let next = drive
        .files()
        .keys()
        .filter_map(|path| event_name(path.rsplit('/').next().unwrap()))
        .max()
        .unwrap()
        + 1;
    drive.with(|disk| {
        disk.faults.racy_move = Some((format!("e-{next:012}.json"), Arc::new(Barrier::new(2))));
    });
    std::thread::scope(|scope| {
        scope.spawn(|| laptop.add("合成笔记本").unwrap());
        scope.spawn(|| phone.add("合成手机").unwrap());
    });
    drive.with(|disk| disk.faults.racy_move = None);
    // 笔记本再写一个接在它自己那个版本后面的事件；手机读到它时接不上自己记得的链头。
    laptop.add("合成笔记本二").unwrap();
    assert_eq!(phone.list("").unwrap_err(), RelayError::Rollback);
    assert_eq!(RelayError::Rollback.code(), "webdav_rollback");
}

#[test]
fn files_that_show_up_late_in_listings_only_cost_retries() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let (laptop, phone) = paired(&drive, root.path());
    laptop.list("").unwrap();
    drive.with(|disk| disk.faults.hide_new_for = 3);
    phone.add("合成手机").unwrap();
    // 笔记本还列不出手机写的事件；它抢同一个序号输掉，几次重读之后才看到，接在后面写。
    laptop.add("合成笔记本").unwrap();
    drive.with(|disk| {
        disk.faults.hide_new_for = 0;
        disk.hidden.clear();
    });
    assert_eq!(texts(&phone, ""), ["合成笔记本", "合成手机"]);

    // 一直列不出来：写入在有限次重试后放弃，不会写坏日志。
    drive.with(|disk| disk.faults.hide_new_for = 100);
    phone.add("合成手机二").unwrap();
    assert_eq!(
        laptop.add("合成笔记本二").unwrap_err(),
        RelayError::Unavailable
    );
    drive.with(|disk| {
        disk.faults.hide_new_for = 0;
        disk.hidden.clear();
    });
    drive.clock.advance(10_000);
    assert_eq!(texts(&laptop, ""), ["合成手机二", "合成笔记本", "合成手机"]);
}

// ---- 回退和改写 ----

#[test]
fn a_rewritten_history_is_refused_until_the_user_trusts_the_drive_again() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let (laptop, phone) = paired(&drive, root.path());
    laptop.add("合成一").unwrap();
    laptop.list("").unwrap();
    // 被移除之前的手机（还知道应用密码）把最近的事件换成另一段历史，并且写得比笔记本读到的还长。
    let mut files = drive.files();
    let mut events: Vec<(u64, String)> = files
        .keys()
        .filter_map(|path| Some((event_name(path.rsplit('/').next()?)?, path.clone())))
        .collect();
    events.sort();
    let (last, last_path) = events.last().unwrap().clone();
    let dir = parent(&last_path);
    let previous: serde_json::Value =
        serde_json::from_slice(&files[&events[events.len() - 2].1]).unwrap();
    let _ = previous;
    let head = hex::encode(sha2::Sha256::digest(&files[&events[events.len() - 2].1]));
    files.remove(&last_path);
    let mut prev = head;
    for seq in last..last + 3 {
        let bytes = json!({ "seq": seq, "prev": prev, "nonce": "00", "at": 0, "op": "retention", "days": 30 })
            .to_string()
            .into_bytes();
        prev = hex::encode(sha2::Sha256::digest(&bytes));
        files.insert(format!("{dir}e-{seq:012}.json"), bytes);
    }
    drive.with(|disk| disk.files = files);
    drive.clock.advance(10_000);
    assert_eq!(laptop.list("").unwrap_err().code(), "webdav_rollback");
    let _ = phone;

    // 整个文件夹被删掉也一样：本机读到过事件，就不会从头再建一份。
    drive.with(|disk| {
        disk.files.clear();
        disk.dirs.retain(|dir| dir.is_empty());
    });
    assert_eq!(laptop.list("").unwrap_err().code(), "webdav_rollback");
    assert!(drive.files().is_empty());

    // 用户确认是自己清理的：删掉本机的缓存（重新「测试连接」并保存时就是这样），从网盘现在的样子重新开始。
    std::fs::remove_file(root.path().join("laptop").join(WEBDAV_STATE_FILE)).unwrap();
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
}

#[test]
fn a_snapshot_with_an_older_key_generation_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    laptop.list("").unwrap();
    laptop.reset().unwrap();
    laptop.list("").unwrap();
    // 有人删掉本机所在的 epoch，留下一个只到第一把密钥的旧快照，号称是更新的 epoch。
    let files = drive.files();
    let current = files
        .keys()
        .filter(|path| path.ends_with("/snapshot.json"))
        .max()
        .unwrap()
        .clone();
    let mut snapshot: serde_json::Value = serde_json::from_slice(&files[&current]).unwrap();
    assert!(snapshot["state"]["generation"].as_u64().unwrap() >= 2);
    snapshot["state"]["generation"] = 1.into();
    snapshot["epoch"] = 9.into();
    snapshot["seq"] = 999.into();
    drive.with(|disk| {
        disk.files.clear();
        disk.dirs
            .retain(|dir| !dir.starts_with("msime-clipboard/v1/epoch-"));
        disk.dirs.insert("msime-clipboard/v1/epoch-000009/".into());
        disk.files
            .insert("msime-clipboard/v1/format.json".into(), format_bytes());
        disk.files.insert(
            "msime-clipboard/v1/epoch-000009/snapshot.json".into(),
            snapshot.to_string().into_bytes(),
        );
    });
    clock.advance(10_000);
    assert_eq!(laptop.list("").unwrap_err(), RelayError::Rollback);
}

// ---- 限流、鉴权和本机校验 ----

#[test]
fn rate_limits_are_waited_out_with_backoff_then_reported() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    laptop.list("").unwrap();

    clock.advance(10_000);
    drive.with(|disk| {
        disk.faults.rate_limited = 2;
        disk.faults.retry_after = Some(1);
    });
    laptop.list("").unwrap();
    assert_eq!(
        clock.sleeps(),
        [Duration::from_secs(1), Duration::from_secs(1)]
    );

    // 一直限流：退避三次（1、2、4 秒）之后报出来。
    clock.advance(10_000);
    drive.with(|disk| {
        disk.faults.rate_limited = 100;
        disk.faults.retry_after = None;
    });
    let failure = laptop.list("").unwrap_err();
    assert_eq!(failure, RelayError::RateLimited);
    assert_eq!(failure.code(), "webdav_rate_limited");
    assert_eq!(
        clock.sleeps()[2..],
        [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4)
        ]
    );

    // 要等一个小时：不等，马上报。
    clock.advance(10_000);
    drive.with(|disk| {
        disk.faults.rate_limited = 1;
        disk.faults.retry_after = Some(3600);
    });
    let before = clock.sleeps().len();
    assert_eq!(laptop.list("").unwrap_err(), RelayError::RateLimited);
    assert_eq!(clock.sleeps().len(), before);
}

#[test]
fn a_refused_password_is_a_relay_authorization_error() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    drive.with(|disk| disk.faults.unauthorized = true);
    let laptop = device(&drive, &root.path().join("laptop"), "合成笔记本");
    assert_eq!(laptop.list("").unwrap_err().code(), "relay_unauthorized");
    assert_eq!(laptop.clear().unwrap_err(), RelayError::Unauthorized);
}

#[test]
fn malformed_requests_are_answered_here_without_touching_the_drive() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    let relay = relay(&drive, root.path(), true);
    let response = relay
        .send("PUT", "/v1/users/me/clipboard/settings", Some("not json"))
        .unwrap();
    assert_eq!(response.status, 400);
    assert!(response.body.contains("invalid_json"));
    let response = relay
        .send("GET", "/v1/users/me/clipboard/keys?device_id=nope", None)
        .unwrap();
    assert_eq!(response.status, 400);
    assert!(drive.requests().is_empty());
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

// ---- 请求数和缓存 ----

#[test]
fn reads_cost_one_listing_and_reuse_a_cache_that_was_just_read() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    let relay = relay(&drive, root.path(), true);
    let device_id = "0".repeat(32);
    let keys = format!("/v1/users/me/clipboard/keys?device_id={device_id}");
    relay.send("GET", &keys, None).unwrap();
    // 第一次：建目录、格式文件和第一份快照。
    assert!(methods(&drive).contains(&"MKCOL".to_owned()));
    drive.clear_log();
    // 两秒内再读：直接用本机缓存。
    relay.send("GET", "/v1/users/me/clipboard", None).unwrap();
    assert!(drive.requests().is_empty());
    clock.advance(5_000);
    relay.send("GET", "/v1/users/me/clipboard", None).unwrap();
    assert_eq!(methods(&drive), ["PROPFIND"]);
    // 不改变状态的写入（已经开着的开关）在刚读过之后直接答，不写日志。
    drive.clear_log();
    clock.advance(5_000);
    let response = relay
        .send(
            "PUT",
            "/v1/users/me/clipboard/settings",
            Some(r#"{"enabled":true}"#),
        )
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(methods(&drive), ["PROPFIND"]);
    // 一次写入：PUT 临时文件再 MOVE。
    drive.clear_log();
    relay
        .send(
            "PUT",
            "/v1/users/me/clipboard/retention",
            Some(r#"{"days":7}"#),
        )
        .unwrap();
    assert_eq!(methods(&drive), ["PUT", "MOVE"]);
    // 本机缓存仅本人可读写。
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.path().join(WEBDAV_STATE_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn the_cache_belongs_to_one_drive_address_and_account() {
    let root = tempfile::tempdir().unwrap();
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    relay(&drive, root.path(), true)
        .send("GET", "/v1/users/me/clipboard", None)
        .unwrap();
    // 另一个账号的同一个文件夹：缓存不算数，从网盘重新读。
    let other = VirtualRelay::with_parts(
        RelayConfig::webdav(URL, "other@example.test", PASSWORD).unwrap(),
        root.path().to_path_buf(),
        Box::new(drive.clone()),
        Box::new(clock.clone()),
        true,
    );
    drive.clear_log();
    other.send("GET", "/v1/users/me/clipboard", None).unwrap();
    assert!(methods(&drive).contains(&"PROPFIND".to_owned()));

    // 换应用密码不换空间，换账号或地址才换。
    let same = RelayConfig::webdav(URL, USER, "another-password").unwrap();
    assert_eq!(same.space(), config().space());
    assert_ne!(
        RelayConfig::webdav(URL, "other@example.test", PASSWORD)
            .unwrap()
            .space(),
        config().space()
    );
    assert_ne!(
        RelayConfig::webdav("https://dav.example.test/other", USER, PASSWORD)
            .unwrap()
            .space(),
        config().space()
    );
    // 凭据不进 Debug 输出。
    let debug = format!("{:?}", config());
    assert!(!debug.contains(USER) && !debug.contains(PASSWORD));
    assert!(debug.contains("webdav"));
}

// ---- 测试连接 ----

#[test]
fn the_probe_passes_a_good_drive_and_leaves_nothing_behind() {
    let clock = Clock::new();
    let drive = FakeDav::new(&clock);
    probe_with(&drive, &clock).unwrap();
    assert!(drive.files().is_empty());
    assert!(drive
        .with(|disk| disk.dirs.clone())
        .iter()
        .all(|dir| dir.is_empty() || dir == ROOT));
}

#[test]
fn the_probe_refuses_drives_without_exclusive_create() {
    let clock = Clock::new();
    // 不理会 `Overwrite: F`：第二次创建也「成功」。
    let overwriting = FakeDav::new(&clock);
    overwriting.with(|disk| disk.faults.ignore_overwrite = true);
    assert_eq!(
        probe_with(&overwriting, &clock),
        Err(RelayError::Unsupported)
    );
    // MOVE 先查后写：几个同时抢的写入都成功。
    let racy = FakeDav::new(&clock);
    racy.with(|disk| {
        disk.faults.racy_move = Some(("race.json".into(), Arc::new(Barrier::new(PROBE_RACERS))))
    });
    assert_eq!(probe_with(&racy, &clock), Err(RelayError::Unsupported));
    // 文件夹不存在：建不了目录。
    let missing = FakeDav::new(&clock);
    missing.with(|disk| disk.dirs.clear());
    assert_eq!(probe_with(&missing, &clock), Err(RelayError::Unsupported));
    // 网盘答了、但不让建目录：不合格。
    let forbidden = FakeDav::new(&clock);
    forbidden.with(|disk| disk.faults.forbid_mkcol = true);
    assert_eq!(probe_with(&forbidden, &clock), Err(RelayError::Unsupported));
    // 根本连不上：报连不上，不说网盘不合格。
    let unreachable = FakeDav::new(&clock);
    unreachable.with(|disk| disk.faults.unreachable = true);
    assert_eq!(
        probe_with(&unreachable, &clock),
        Err(RelayError::Unavailable)
    );
    // 账号或密码不对。
    let refused = FakeDav::new(&clock);
    refused.with(|disk| disk.faults.unauthorized = true);
    assert_eq!(probe_with(&refused, &clock), Err(RelayError::Unauthorized));
    assert_eq!(RelayError::Unsupported.code(), "webdav_unsupported");
}

// ---- 多状态响应 ----

#[test]
fn listings_from_different_servers_are_read_by_local_name() {
    let nutstore = br#"<?xml version="1.0" encoding="UTF-8"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/dav/msime/msime-clipboard/v1/epoch-000003/</d:href></d:response><d:response><d:href>/dav/msime/msime-clipboard/v1/epoch-000003/e-000000000042.json</d:href><d:propstat><d:prop><d:getetag>"abc"</d:getetag></d:prop></d:propstat></d:response></d:multistatus>"#;
    assert_eq!(
        listing_names(nutstore).unwrap(),
        ["epoch-000003", "e-000000000042.json"]
    );
    let sabre = br#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns"><d:response><d:href>/remote.php/dav/files/u/msime/msime-clipboard/v1/</d:href></d:response><d:response><d:href>/remote.php/dav/files/u/msime/msime-clipboard/v1/epoch-000000/</d:href></d:response><d:response><d:href>/remote.php/dav/files/u/msime/msime-clipboard/v1/format.json</d:href></d:response></d:multistatus>"#;
    assert_eq!(
        listing_names(sabre).unwrap(),
        ["v1", "epoch-000000", "format.json"]
    );
    let prefixed = br#"<ns0:multistatus xmlns:ns0="DAV:"><ns0:response><ns0:href>https://dav.example.test/dav/a%20b&amp;c.json</ns0:href></ns0:response></ns0:multistatus>"#;
    let names = listing_names(prefixed).unwrap();
    assert_eq!(names.len(), 1);
    assert!(event_name(&names[0]).is_none());
    assert!(listing_names(b"\xff\xfe").is_none());
    assert_eq!(epoch_name("epoch-000012"), Some(12));
    assert_eq!(epoch_name("epoch-12"), None);
    assert_eq!(event_name("e-000000000007.json"), Some(7));
    assert_eq!(event_name("e-7.json"), None);
    assert_eq!(
        tmp_name(&format!("1790000000-{}.json", "a".repeat(32))),
        Some(1_790_000_000)
    );
}

#[test]
fn trusting_the_drive_again_drops_only_the_local_log_cache() {
    let root = tempfile::tempdir().unwrap();
    let drive = FakeDav::new(&Clock::new());
    relay(&drive, root.path(), true)
        .send("GET", "/v1/users/me/clipboard", None)
        .unwrap();
    std::fs::write(root.path().join("keep-me"), b"x").unwrap();
    // 测试连接没通过：本机记得的位置原样留着。
    assert_eq!(
        trust_after(Err(RelayError::Unsupported), Some(root.path())),
        Err(RelayError::Unsupported)
    );
    assert!(root.path().join(WEBDAV_STATE_FILE).exists());
    // 通过了：只删日志缓存，目录里的其他文件不动；再来一次也没事。
    trust_after(Ok(()), Some(root.path())).unwrap();
    assert!(!root.path().join(WEBDAV_STATE_FILE).exists());
    assert!(root.path().join("keep-me").exists());
    trust_after(Ok(()), Some(root.path())).unwrap();
    trust_after(Ok(()), None).unwrap();
    // 相对路径在联网之前就拒绝。
    assert_eq!(
        probe_and_trust(&config(), Some(Path::new("relative"))),
        Err(RelayError::Invalid)
    );
}

#[test]
fn http_paths_never_leave_the_chosen_folder() {
    let dav = HttpDav::new(&config()).unwrap();
    assert_eq!(
        dav.url("msime-clipboard/v1/format.json").unwrap().as_str(),
        "https://dav.example.test/dav/msime-clipboard/v1/format.json"
    );
    for path in [
        "../x",
        "/msime-clipboard/",
        "msime-clipboard//x",
        "msime-clipboard/%2e%2e/x",
        "msime-clipboard/X",
        "msime-clipboard/a?b",
    ] {
        assert!(dav.url(path).is_err(), "{path}");
    }
    assert!(HttpDav::new(&RelayConfig::new(URL, "synthetic-relay-token").unwrap()).is_err());
}
