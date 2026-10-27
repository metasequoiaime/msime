//! 确定的多设备模拟：几台合成设备在一个内存里的网盘上各自增词、改编码、删词，断网、交错、崩溃、重放，最后核对收敛到规定的结果。网盘的语义照 WebDAV（独占创建、集合、列目录），可以注入列表滞后、断网、写成之后才断、超大响应和回退；本机词库是一个能注入「正忙」「拒收」「写好之后崩溃」的集合；进程崩溃用「某一次写盘失败、随后从上一次写盘的状态重启」来模拟。

use super::crypto::DataKey;
use super::log::FORMAT;
use super::replica::LAPSE_SECS;
use super::schedule::{self, Trigger};
use super::sync::{epoch_dir, Options, OLD_EPOCH_GRACE_SECS, V1};
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const START_MS: u64 = 1_790_000_000_000;
const K1: &str = "11111111111111111111111111111111";
const K2: &str = "22222222222222222222222222222222";
const K3: &str = "33333333333333333333333333333333";

fn data_key(id: &str) -> DataKey {
    DataKey::new(id, &[id.as_bytes()[0]; 32]).unwrap()
}

fn ring(id: &str) -> KeyRing {
    KeyRing {
        current: Some(data_key(id)),
    }
}

fn pinyin(code: &str, word: &str) -> WordKey {
    WordKey::new(WordKind::Pinyin, code, word).unwrap()
}

// ---- 内存里的网盘 ----

#[derive(Default)]
struct Faults {
    /// 断网：每个请求都失败。
    offline: bool,
    /// 新建的文件在之后这么多次列目录里看不到。
    hide_new_for: u32,
    /// 下一次独占创建写成之后报错（写进去了，但设备以为没写成）。
    fail_after_create: bool,
    /// 再发这么多个请求之后断网。
    fail_after: Option<usize>,
    unauthorized: bool,
}

#[derive(Default)]
struct Disk {
    files: BTreeMap<String, Vec<u8>>,
    dirs: BTreeSet<String>,
    hidden: BTreeMap<String, u32>,
    /// 每一个写成过的文件，压缩删掉了也留着，用来数日志里出现过的操作。
    history: BTreeMap<String, Vec<u8>>,
    faults: Faults,
    requests: usize,
}

#[derive(Clone, Default)]
struct Share(Rc<RefCell<Disk>>);

fn parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(index) => trimmed[..=index].to_owned(),
        None => String::new(),
    }
}

impl Share {
    fn new() -> Self {
        let share = Self::default();
        share.0.borrow_mut().dirs.insert(String::new());
        share
    }

    fn with<T>(&self, edit: impl FnOnce(&mut Disk) -> T) -> T {
        edit(&mut self.0.borrow_mut())
    }

    fn snapshot(&self) -> (BTreeMap<String, Vec<u8>>, BTreeSet<String>) {
        self.with(|disk| (disk.files.clone(), disk.dirs.clone()))
    }

    fn restore(&self, snapshot: (BTreeMap<String, Vec<u8>>, BTreeSet<String>)) {
        self.with(|disk| {
            disk.files = snapshot.0;
            disk.dirs = snapshot.1;
        });
    }

    /// 日志里出现过的全部操作：(写入的设备, 词条, 是否存在, 时间戳)。用当时的格式直接解密，同时核对了文档里的事件格式和 aad。
    fn published(&self, keys: &[&str]) -> Vec<(String, WordKey, bool, u64)> {
        let mut ops = Vec::new();
        for (path, bytes) in self.with(|disk| disk.history.clone()) {
            if !path.rsplit('/').next().unwrap().starts_with("e-") {
                continue;
            }
            let event: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if event["type"] != "ops" {
                continue;
            }
            let aad = format!(
                "{FORMAT}-v1-event\0{}\0{}\0{}\0{}",
                event["seq"],
                event["prev"].as_str().unwrap(),
                event["at"],
                event["key_id"].as_str().unwrap()
            );
            let key_id = event["key_id"].as_str().unwrap();
            assert!(keys.contains(&key_id), "unexpected key {key_id}");
            let plain = data_key(key_id)
                .open(
                    super::crypto::Purpose::Event,
                    event["box"].as_str().unwrap(),
                    aad.as_bytes(),
                )
                .unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&plain).unwrap();
            for op in payload["ops"].as_array().unwrap() {
                let kind = WordKind::from_name(op[0].as_str().unwrap()).unwrap();
                ops.push((
                    payload["device"].as_str().unwrap().to_owned(),
                    WordKey::canonical(kind, op[1].as_str().unwrap(), op[2].as_str().unwrap())
                        .unwrap(),
                    op[4].as_bool().unwrap(),
                    op[3].as_u64().unwrap(),
                ));
            }
        }
        ops
    }
}

struct Connection {
    share: Share,
}

impl Connection {
    fn begin(&self) -> Result<(), SyncError> {
        self.share.with(|disk| {
            disk.requests += 1;
            if let Some(remaining) = disk.faults.fail_after.as_mut() {
                if *remaining == 0 {
                    disk.faults.offline = true;
                } else {
                    *remaining -= 1;
                }
            }
            if disk.faults.offline {
                Err(SyncError::Unavailable)
            } else if disk.faults.unauthorized {
                Err(SyncError::Unauthorized)
            } else {
                Ok(())
            }
        })
    }
}

impl RemoteStore for Connection {
    fn list(&mut self, dir: &str) -> Result<Option<Vec<String>>, SyncError> {
        self.begin()?;
        Ok(self.share.with(|disk| {
            if !disk.dirs.contains(dir) {
                return None;
            }
            let mut names = Vec::new();
            let children: Vec<String> = disk
                .dirs
                .iter()
                .filter(|path| !path.is_empty() && parent(path) == dir && path.as_str() != dir)
                .cloned()
                .chain(
                    disk.files
                        .keys()
                        .filter(|path| parent(path) == dir)
                        .cloned(),
                )
                .collect();
            for path in children {
                if let Some(remaining) = disk.hidden.get_mut(&path) {
                    if *remaining > 0 {
                        *remaining -= 1;
                        continue;
                    }
                }
                names.push(
                    path.trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .to_owned(),
                );
            }
            Some(names)
        }))
    }

    fn get(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, SyncError> {
        self.begin()?;
        match self.share.with(|disk| disk.files.get(path).cloned()) {
            Some(bytes) if bytes.len() as u64 > limit => Err(SyncError::TooLarge),
            bytes => Ok(bytes),
        }
    }

    fn create(&mut self, path: &str, bytes: &[u8]) -> Result<bool, SyncError> {
        self.begin()?;
        self.share.with(|disk| {
            if !disk.dirs.contains(&parent(path)) || disk.files.contains_key(path) {
                return Ok(false);
            }
            disk.files.insert(path.to_owned(), bytes.to_vec());
            disk.history.insert(path.to_owned(), bytes.to_vec());
            if disk.faults.hide_new_for > 0 {
                disk.hidden
                    .insert(path.to_owned(), disk.faults.hide_new_for);
            }
            if std::mem::take(&mut disk.faults.fail_after_create) {
                return Err(SyncError::Unavailable);
            }
            Ok(true)
        })
    }

    fn mkcol(&mut self, dir: &str) -> Result<(), SyncError> {
        self.begin()?;
        self.share.with(|disk| {
            if !disk.dirs.contains(&parent(dir)) {
                return Err(SyncError::Unavailable);
            }
            disk.dirs.insert(dir.to_owned());
            Ok(())
        })
    }

    fn delete(&mut self, path: &str) -> Result<(), SyncError> {
        self.begin()?;
        self.share.with(|disk| {
            disk.files.retain(|file, _| !file.starts_with(path));
            disk.dirs
                .retain(|dir| !dir.starts_with(path) || dir.is_empty());
        });
        Ok(())
    }
}

// ---- 本机词库 ----

#[derive(Default)]
struct Engine {
    words: BTreeSet<WordKey>,
    busy: bool,
    refuse: BTreeSet<WordKey>,
    /// 下一批写好之后报错（写进去了，进程随即退出）。
    crash_after_apply: bool,
    applies: usize,
    /// 每个请求编号写过什么：同一个编号重试必须是同一次写入。
    requests: BTreeMap<String, (WordKey, bool)>,
    /// 写入之后有没有放掉租约就去联网。
    holding: bool,
}

impl LocalDictionary for Engine {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.words.clone())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        assert!(changes.len() <= super::sync::MAX_APPLY_BATCH);
        if self.busy {
            return Err(LocalError::Busy);
        }
        self.applies += 1;
        self.holding = true;
        for change in changes {
            assert!(change.request_id.starts_with("dsync-") && change.request_id.len() == 38);
            let previous = self
                .requests
                .insert(change.request_id.clone(), (change.key.clone(), change.add));
            assert!(
                previous.is_none_or(|previous| previous == (change.key.clone(), change.add)),
                "request id reused for a different write"
            );
        }
        let outcomes = changes
            .iter()
            .map(|change| {
                if self.refuse.contains(&change.key) {
                    return LocalOutcome::Rejected;
                }
                if change.add {
                    self.words.insert(change.key.clone());
                } else {
                    self.words.remove(&change.key);
                }
                LocalOutcome::Applied
            })
            .collect();
        if std::mem::take(&mut self.crash_after_apply) {
            return Err(LocalError::Unavailable);
        }
        Ok(outcomes)
    }

    fn release(&mut self) {
        self.holding = false;
    }
}

struct Env {
    now_ms: u64,
    counter: u64,
}

impl Environment for Env {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }

    fn random(&mut self, bytes: &mut [u8]) -> Result<(), SyncError> {
        for byte in bytes.iter_mut() {
            self.counter = self
                .counter
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *byte = (self.counter >> 56) as u8;
        }
        Ok(())
    }
}

/// 写盘：把状态序列化后存起来；`crash_at` 是第几次写盘时进程死掉（这次没写成，之后的步骤也不会发生）。
struct Disk2 {
    saved: Option<String>,
    saves: usize,
    crash_at: Option<usize>,
}

impl StateSink for Disk2 {
    fn save(&mut self, state: &SyncState) -> Result<(), SyncError> {
        self.saves += 1;
        if self.crash_at == Some(self.saves) {
            return Err(SyncError::Storage);
        }
        self.saved = Some(serde_json::to_string(state).unwrap());
        Ok(())
    }
}

struct Device {
    id: String,
    state: SyncState,
    sink: Disk2,
    engine: Engine,
    env: Env,
    ring: KeyRing,
    offline: bool,
}

impl Device {
    fn new(seed: u8, key: Option<&str>) -> Self {
        let id = format!("{seed:02x}").repeat(16);
        Self {
            state: SyncState::new(&id).unwrap(),
            id,
            sink: Disk2 {
                saved: None,
                saves: 0,
                crash_at: None,
            },
            engine: Engine::default(),
            env: Env {
                now_ms: START_MS,
                counter: u64::from(seed) << 32,
            },
            ring: KeyRing {
                current: key.map(data_key),
            },
            offline: false,
        }
    }

    fn run(&mut self, share: &Share, options: Options) -> Result<Report, SyncError> {
        let mut connection = Connection {
            share: share.clone(),
        };
        let offline = self.offline;
        let mut parts = Parts {
            remote: &mut connection,
            local: &mut self.engine,
            env: &mut self.env,
            sink: &mut self.sink,
        };
        let options = Options {
            offline: options.offline || offline,
            ..options
        };
        let result = run(&mut self.state, &mut parts, &self.ring, options);
        assert!(
            !self.engine.holding,
            "the engine lease outlived the apply phase"
        );
        result
    }

    fn sync(&mut self, share: &Share) -> Report {
        self.run(share, Options::default()).unwrap()
    }

    /// 只记录本机改动，不联网（宿主在用户改词之后马上做的那一步）。
    fn capture(&mut self, share: &Share) {
        let mut connection = Connection {
            share: share.clone(),
        };
        let mut parts = Parts {
            remote: &mut connection,
            local: &mut self.engine,
            env: &mut self.env,
            sink: &mut self.sink,
        };
        capture(&mut self.state, &mut parts).unwrap();
    }

    /// 进程退出后重启：内存里的状态换成最后一次写盘的。
    fn reboot(&mut self) {
        self.sink.crash_at = None;
        self.state = match &self.sink.saved {
            Some(saved) => serde_json::from_str(saved).unwrap(),
            None => SyncState::new(&self.id).unwrap(),
        };
    }

    fn advance(&mut self, ms: u64) {
        self.env.now_ms += ms;
    }

    fn add(&mut self, key: &WordKey) {
        self.engine.words.insert(key.clone());
    }

    fn remove(&mut self, key: &WordKey) {
        self.engine.words.remove(key);
    }

    fn shared(&self) -> BTreeSet<WordKey> {
        self.state
            .ledger()
            .map(|ledger| {
                ledger
                    .replica
                    .entries
                    .iter()
                    .filter(|(_, record)| record.live)
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// 让所有设备轮流同步到没有新东西为止，然后核对：各台的本机词库相同，并且都等于共享状态，没有待发送和待写入的。
fn settle(devices: &mut [&mut Device], share: &Share) -> BTreeSet<WordKey> {
    for _ in 0..4 {
        for device in devices.iter_mut() {
            device.advance(1_000);
            device.sync(share);
        }
    }
    let expected = devices[0].engine.words.clone();
    for device in devices.iter() {
        let status = device.state.status();
        assert_eq!(device.engine.words, expected, "device {}", device.id);
        assert_eq!(device.shared(), expected, "device {}", device.id);
        assert_eq!(
            (status.outbox, status.pending_apply),
            (0, 0),
            "device {}",
            device.id
        );
    }
    expected
}

// ---- 场景 ----

#[test]
fn three_devices_converge_after_adding_changing_codes_and_deleting() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let mut c = Device::new(0xc3, Some(K1));
    let shuishan = pinyin("shui'shan", "水杉");
    let ss = pinyin("ss", "水杉");
    let nihao = pinyin("ni'hao", "你好");
    let zaijian = pinyin("zai'jian", "再见");
    // A 先有两个词，第一次同步把它们带上去；B 已有的词也带上去。
    a.add(&shuishan);
    a.add(&nihao);
    b.add(&zaijian);
    a.sync(&share);
    b.sync(&share);
    c.sync(&share);
    let all = settle(&mut [&mut a, &mut b, &mut c], &share);
    assert_eq!(
        all,
        BTreeSet::from([shuishan.clone(), nihao.clone(), zaijian.clone()])
    );
    // B 把「水杉」的编码改成 ss（删旧加新），C 删掉「你好」。
    b.remove(&shuishan);
    b.add(&ss);
    b.advance(1_000);
    b.sync(&share);
    c.remove(&nihao);
    c.advance(1_000);
    c.sync(&share);
    let all = settle(&mut [&mut a, &mut b, &mut c], &share);
    assert_eq!(all, BTreeSet::from([ss, zaijian]));
}

#[test]
fn concurrent_edit_and_delete_resolve_by_timestamp_not_by_upload_order() {
    let shuishan = pinyin("shui'shan", "水杉");
    let ss = pinyin("ss", "水杉");
    // 两种上传顺序都试一遍：结果只看时间戳。
    for b_uploads_first in [false, true] {
        let share = Share::new();
        let mut a = Device::new(0xa1, Some(K1));
        let mut b = Device::new(0xb2, Some(K1));
        a.add(&shuishan);
        a.sync(&share);
        b.sync(&share);
        assert!(b.engine.words.contains(&shuishan));
        // 两台都断网。B 在 10:00 把编码改成 ss；A 在 10:05 删掉「水杉」，晚于 B 的改动。
        a.offline = true;
        b.offline = true;
        b.advance(10_000);
        b.remove(&shuishan);
        b.add(&ss);
        b.capture(&share);
        a.advance(15_000);
        a.remove(&shuishan);
        a.capture(&share);
        a.offline = false;
        b.offline = false;
        if b_uploads_first {
            b.sync(&share);
            a.sync(&share);
        } else {
            a.sync(&share);
            b.sync(&share);
        }
        // 改编码是删旧加新：旧编码的词条两边都删了；A 删的是旧词条，新编码 ss 的词条不受影响。
        let all = settle(&mut [&mut a, &mut b], &share);
        assert_eq!(
            all,
            BTreeSet::from([ss.clone()]),
            "b first: {b_uploads_first}"
        );
    }

    // 同一个词条：一台删除、另一台重新加入，晚的那个算数，与谁先上传无关。
    for (delete_later, b_uploads_first) in
        [(true, false), (true, true), (false, false), (false, true)]
    {
        let share = Share::new();
        let mut a = Device::new(0xa1, Some(K1));
        let mut b = Device::new(0xb2, Some(K1));
        a.add(&shuishan);
        a.sync(&share);
        b.sync(&share);
        a.offline = true;
        b.offline = true;
        // B 先删掉，稍后再加回来；A 删掉它，早于或晚于 B 加回来的那一刻。
        b.remove(&shuishan);
        b.capture(&share);
        if delete_later {
            b.advance(10_000);
            b.add(&shuishan);
            b.capture(&share);
            a.advance(20_000);
            a.remove(&shuishan);
            a.capture(&share);
        } else {
            a.advance(10_000);
            a.remove(&shuishan);
            a.capture(&share);
            b.advance(20_000);
            b.add(&shuishan);
            b.capture(&share);
        }
        a.offline = false;
        b.offline = false;
        if b_uploads_first {
            b.sync(&share);
            a.sync(&share);
        } else {
            a.sync(&share);
            b.sync(&share);
        }
        let all = settle(&mut [&mut a, &mut b], &share);
        let expected = if delete_later {
            BTreeSet::new()
        } else {
            BTreeSet::from([shuishan.clone()])
        };
        assert_eq!(
            all, expected,
            "delete later: {delete_later}, b first: {b_uploads_first}"
        );
    }
}

/// 一个确定的伪随机数，测试里不引入随机数 crate。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

#[test]
fn random_offline_interleavings_with_crashes_and_replays_converge() {
    let universe: Vec<WordKey> = (0..24)
        .map(|index| {
            let code = ["ma", "ba", "da", "ta", "na", "la"][index % 6];
            let word = ["妈", "爸", "大", "他"][index / 6];
            pinyin(code, word)
        })
        .collect();
    for seed in 0..40u64 {
        let share = Share::new();
        let mut devices: Vec<Device> = (0..3)
            .map(|index| Device::new(0xa0 + index, Some(K1)))
            .collect();
        let mut random = Lcg(seed);
        for _ in 0..80 {
            let index = random.next(3) as usize;
            let device = &mut devices[index];
            device.advance(1_000 + random.next(90_000));
            match random.next(10) {
                0..=3 => {
                    let key = &universe[random.next(universe.len() as u64) as usize];
                    if device.engine.words.contains(key) {
                        device.remove(key);
                    } else {
                        device.add(key);
                    }
                    device.capture(&share);
                }
                4 => {
                    // 改编码：删掉一个已有的词，换一个编码加回来。
                    if let Some(old) = device.engine.words.iter().next().cloned() {
                        let new = universe[random.next(universe.len() as u64) as usize].clone();
                        device.remove(&old);
                        device.add(&new);
                        device.capture(&share);
                    }
                }
                5 => device.offline = !device.offline,
                6 => {
                    // 在某一次写盘时崩溃，然后重启。
                    device.sink.crash_at = Some(device.sink.saves + 1 + random.next(4) as usize);
                    let _ = device.run(&share, Options::default());
                    device.reboot();
                }
                7 => {
                    // 网盘写成之后才断网。
                    share.with(|disk| disk.faults.fail_after_create = true);
                    let _ = device.run(&share, Options::default());
                    share.with(|disk| disk.faults.fail_after_create = false);
                    device.reboot();
                }
                _ => {
                    let _ = device.run(&share, Options::default());
                }
            }
        }
        for device in devices.iter_mut() {
            device.offline = false;
        }
        let [a, b, c] = &mut devices[..] else {
            unreachable!()
        };
        settle(&mut [a, b, c], &share);
        // 每台设备对同一个词条的同一个时间戳最多发出一次（崩溃重试没有重复发）。
        let published = share.published(&[K1]);
        let unique: BTreeSet<_> = published.iter().collect();
        assert_eq!(unique.len(), published.len(), "seed {seed}");
    }
}

#[test]
fn crashing_between_any_two_persisted_steps_neither_loses_nor_duplicates() {
    let words: Vec<WordKey> = ["yi", "er", "san", "si", "wu"]
        .iter()
        .map(|code| pinyin(code, "数"))
        .collect();
    // 一次完整的同步要写盘很多次；在每一次写盘时都崩溃一遍，重启后再同步。
    for crash_at in 1..=12 {
        let share = Share::new();
        let mut a = Device::new(0xa1, Some(K1));
        let mut b = Device::new(0xb2, Some(K1));
        for word in &words[..3] {
            a.add(word);
        }
        a.sync(&share);
        b.sync(&share);
        a.remove(&words[0]);
        a.add(&words[3]);
        b.add(&words[4]);
        a.capture(&share);
        b.sink.crash_at = Some(b.sink.saves + crash_at);
        let _ = b.run(&share, Options::default());
        b.reboot();
        // B 写进本机词库之后、写盘之前崩溃。
        b.engine.crash_after_apply = true;
        let _ = b.run(&share, Options::default());
        b.engine.crash_after_apply = false;
        b.reboot();
        a.sink.crash_at = Some(a.sink.saves + crash_at);
        let _ = a.run(&share, Options::default());
        a.reboot();
        let all = settle(&mut [&mut a, &mut b], &share);
        assert_eq!(
            all,
            BTreeSet::from([
                words[1].clone(),
                words[2].clone(),
                words[3].clone(),
                words[4].clone()
            ]),
            "crash at {crash_at}"
        );
        let published = share.published(&[K1]);
        for (device, key, live) in [
            (&a.id, &words[0], false),
            (&a.id, &words[3], true),
            (&b.id, &words[4], true),
        ] {
            let count = published
                .iter()
                .filter(|(writer, word, alive, _)| {
                    writer == device && word == key && *alive == live
                })
                .count();
            assert_eq!(
                count, 1,
                "crash at {crash_at}: {key:?} published {count} times"
            );
        }
    }
}

#[test]
fn a_publish_that_reached_the_share_before_the_crash_is_recognised_not_resent() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let word = pinyin("shui'shan", "水杉");
    a.sync(&share);
    a.add(&word);
    share.with(|disk| disk.faults.fail_after_create = true);
    assert_eq!(
        a.run(&share, Options::default()),
        Err(SyncError::Unavailable)
    );
    a.reboot();
    assert_eq!(
        a.state.outbox_len(),
        1,
        "the device does not know the event landed"
    );
    a.sync(&share);
    assert_eq!(a.state.outbox_len(), 0);
    let published = share.published(&[K1]);
    assert_eq!(
        published
            .iter()
            .filter(|(_, key, _, _)| *key == word)
            .count(),
        1
    );
}

#[test]
fn the_clock_never_runs_backwards_across_restarts_and_wall_clock_jumps() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let word = pinyin("shui'shan", "水杉");
    a.sync(&share);
    b.sync(&share);
    // B 的时钟快一个小时，先加了这个词。
    b.advance(3_600_000);
    b.add(&word);
    b.sync(&share);
    a.sync(&share);
    assert!(a.engine.words.contains(&word));
    // A 在本机时间更早的时候删掉它：A 已经见过 B 的时间戳，删除仍然更晚，算数。
    a.remove(&word);
    a.reboot();
    a.sync(&share);
    // A 的系统时间往回调了一天，再加回来、再删：每一步都比上一步晚。
    a.env.now_ms -= 86_400_000;
    a.add(&word);
    a.sync(&share);
    a.reboot();
    a.remove(&word);
    a.sync(&share);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert!(all.is_empty());
    let stamps: Vec<u64> = share
        .published(&[K1])
        .into_iter()
        .filter(|(device, _, _, _)| *device == a.id)
        .map(|(_, _, _, hlc)| hlc)
        .collect();
    assert!(
        stamps.windows(2).all(|pair| pair[0] < pair[1]),
        "{stamps:?}"
    );
    assert!(stamps
        .iter()
        .all(|stamp| *stamp > (START_MS + 3_600_000) << 10));
}

#[test]
fn a_reinstalled_device_rejoins_with_a_new_identity_and_keeps_its_words() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let shared = pinyin("gong'xiang", "共享");
    let local = pinyin("ben'ji", "本机");
    a.add(&shared);
    a.sync(&share);
    b.sync(&share);
    // B 重装：同步状态和设备身份都没了，词库从备份里恢复出一个它独有的词。
    let mut b = Device::new(0xb3, Some(K1));
    b.add(&local);
    b.advance(5_000);
    b.sync(&share);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all, BTreeSet::from([shared, local]));
}

#[test]
fn a_device_appears_once_in_the_device_table_however_many_processes_and_runs() {
    let share = Share::new();
    let mut settings = Device::new(0xa1, Some(K1));
    settings.add(&pinyin("ma", "妈"));
    settings.sync(&share);
    // 同一个安装的另一个进程（输入法）读同一份写盘的状态、用同一个设备编号。
    let mut keyboard = Device::new(0xa1, Some(K1));
    keyboard.sink.saved = settings.sink.saved.clone();
    keyboard.reboot();
    keyboard.engine.words = settings.engine.words.clone();
    for round in 0..5 {
        keyboard.advance(HEARTBEAT_MS * 2);
        keyboard.add(&pinyin("ba", if round % 2 == 0 { "爸" } else { "吧" }));
        keyboard.sync(&share);
        settings.sink.saved = keyboard.sink.saved.clone();
        settings.reboot();
        settings.engine.words = keyboard.engine.words.clone();
        settings.advance(HEARTBEAT_MS * 2);
        settings.sync(&share);
        keyboard.sink.saved = settings.sink.saved.clone();
        keyboard.reboot();
    }
    let devices = &settings.state.ledger().unwrap().replica.devices;
    assert_eq!(devices.len(), 1);
    assert!(devices.contains_key(&settings.id));
}

const HEARTBEAT_MS: u64 = super::sync::HEARTBEAT_SECS as u64 * 1000;

#[test]
fn removing_a_device_rekeys_the_log_and_locks_the_removed_device_out() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let mut c = Device::new(0xc3, Some(K1));
    let before = pinyin("yi'qian", "以前");
    let after = pinyin("yi'hou", "以后");
    a.add(&before);
    a.sync(&share);
    b.sync(&share);
    c.sync(&share);
    settle(&mut [&mut a, &mut b, &mut c], &share);
    // A 移除 C：密钥串换钥，新密钥只给 A；B 要等 A 再批准一次。
    a.state.retire(&c.id);
    a.ring = ring(K2);
    b.ring = KeyRing::default();
    a.add(&after);
    let report = a.sync(&share);
    assert!(report.compacted);
    let ledger = a.state.ledger().unwrap();
    assert_eq!((ledger.key_id.as_str(), ledger.generation), (K2, 1));
    // 宣布移除的事件在新 epoch 里，下一次封口时 C 就从设备表里去掉了；在那之前它已经不拖住墓碑回收。
    assert!(ledger.replica.devices[&c.id].retired);
    // C 还拿着旧密钥：读到封口为止，进不了新 epoch，也写不进去。
    c.add(&pinyin("tou'kui", "偷窥"));
    let report = c.sync(&share);
    assert!(report.pending_key);
    assert!(!c.engine.words.contains(&after));
    // B 在等待批准：同样进不了新 epoch，改动留在本机。
    let local = pinyin("deng'dai", "等待");
    b.add(&local);
    assert!(b.sync(&share).pending_key);
    assert_eq!(b.state.outbox_len(), 1);
    // A 批准了 B：B 读完旧 epoch 剩下的部分（用缓存的旧密钥），进入新 epoch，发出改动。
    b.ring = ring(K2);
    b.sync(&share);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all, BTreeSet::from([before, after, local]));
    let c_ops: Vec<_> = share
        .published(&[K1, K2])
        .into_iter()
        .filter(|(device, _, _, _)| *device == c.id)
        .collect();
    assert!(
        c_ops.is_empty(),
        "the removed device published nothing after removal"
    );
}

#[test]
fn an_old_device_rejoins_after_compaction_deleted_its_epoch() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let doomed = pinyin("shan'chu", "删除");
    a.add(&doomed);
    a.sync(&share);
    b.sync(&share);
    settle(&mut [&mut a, &mut b], &share);
    // B 离线很久；这期间 A 删掉了 doomed，又写了足够多的事件，日志被压缩了好几次，旧 epoch 都已删除。
    b.offline = true;
    let offline_word = pinyin("li'xian", "离线");
    b.add(&offline_word);
    b.capture(&share);
    a.remove(&doomed);
    for index in 0..150u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
        a.advance(60_000);
        a.sync(&share);
    }
    // 过了 90 天，A 又写满一个 epoch：这次封口时 B 已经算是长期离线，墓碑不再等它。
    a.advance(LAPSE_SECS as u64 * 1000 + OLD_EPOCH_GRACE_SECS as u64 * 1000);
    for index in 0..70u32 {
        a.add(&pinyin("ba", &format!("字{index}")));
        a.advance(1_000);
        a.sync(&share);
    }
    a.advance(OLD_EPOCH_GRACE_SECS as u64 * 1000 + 1_000);
    a.sync(&share);
    let current = a.state.ledger().unwrap().epoch;
    assert!(current >= 2, "compacted {current} times");
    assert!(
        share.with(|disk| !disk.dirs.contains(&epoch_dir(0))),
        "the epoch B last read is gone"
    );
    // 墓碑已经回收（B 太久没出现，不再等它确认）。
    assert!(!a
        .state
        .ledger()
        .unwrap()
        .replica
        .entries
        .contains_key(&doomed));
    b.offline = false;
    b.advance(LAPSE_SECS as u64 * 1000 + 3 * OLD_EPOCH_GRACE_SECS as u64 * 1000);
    b.sync(&share);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert!(
        !all.contains(&doomed),
        "the deletion reached B through the checkpoint"
    );
    assert!(
        all.contains(&offline_word),
        "B's offline word was published"
    );
}

#[test]
fn tombstones_wait_for_an_active_device_that_has_not_applied_them() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let doomed = pinyin("shan'chu", "删除");
    a.add(&doomed);
    a.sync(&share);
    b.sync(&share);
    settle(&mut [&mut a, &mut b], &share);
    // B 的输入法一直占着词库：收到了删除，却写不进本机词库，也就不确认。
    b.engine.busy = true;
    a.remove(&doomed);
    a.sync(&share);
    let report = b.sync(&share);
    assert!(report.deferred);
    let acked = b.state.acked();
    for index in 0..70u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
        a.advance(1_000);
        a.sync(&share);
    }
    assert!(a.state.ledger().unwrap().epoch >= 1);
    assert!(
        a.state
            .ledger()
            .unwrap()
            .replica
            .entries
            .contains_key(&doomed),
        "kept for B"
    );
    b.advance(1_000);
    assert!(b.sync(&share).deferred);
    assert_eq!(
        b.state.acked(),
        acked,
        "nothing acknowledged while deferred"
    );
    b.engine.busy = false;
    settle(&mut [&mut a, &mut b], &share);
    assert!(!b.engine.words.contains(&doomed));
    assert!(b.state.acked() > acked);
}

#[test]
fn a_rolled_back_or_deleted_share_is_refused() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    a.add(&pinyin("ma", "妈"));
    a.sync(&share);
    let old = share.snapshot();
    a.add(&pinyin("ba", "爸"));
    a.sync(&share);
    // 有人把网盘恢复成了更早的样子（比如删掉最近的事件，想让删掉的词复活）。
    share.restore(old);
    a.add(&pinyin("da", "大"));
    assert_eq!(a.run(&share, Options::default()), Err(SyncError::Rollback));
    // 改写了本机读过的事件、再接一个新的：接不上链头。
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    a.add(&pinyin("ma", "妈"));
    a.sync(&share);
    b.sync(&share);
    // 删掉 B 读过的最后一个事件（B 的登记），A 忘掉位置后在同一个序号上写进别的内容。
    let last = share.with(|disk| {
        disk.files
            .keys()
            .filter(|path| path.contains("/e-"))
            .max()
            .cloned()
            .unwrap()
    });
    share.with(|disk| disk.files.remove(&last));
    a.state.forget_remote();
    a.sync(&share);
    a.add(&pinyin("ba", "爸"));
    a.sync(&share);
    a.add(&pinyin("da", "大"));
    a.sync(&share);
    assert_eq!(b.run(&share, Options::default()), Err(SyncError::Rollback));
    // 整个文件夹被删掉：读到过事件的设备不会从头再建一份。
    share.with(|disk| {
        disk.files.clear();
        disk.dirs.retain(String::is_empty);
    });
    assert_eq!(a.run(&share, Options::default()), Err(SyncError::Rollback));
    // 用户在设置里重新测试连接并保存，就从网盘现在的样子重新开始，本机的词都保留。
    a.state.forget_remote();
    a.sync(&share);
    assert!(a.engine.words.contains(&pinyin("ba", "爸")));
    assert!(share.with(|disk| disk.dirs.contains(V1)));
}

#[test]
fn a_mass_deletion_is_held_back_until_the_user_decides() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    for index in 0..60u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
    }
    a.sync(&share);
    b.sync(&share);
    settle(&mut [&mut a, &mut b], &share);
    // B 的词库被重置了：60 个词都不见了。
    b.engine.words.clear();
    b.sync(&share);
    assert_eq!(b.state.guard().map(|guard| guard.deletions), Some(60));
    a.sync(&share);
    assert_eq!(a.engine.words.len(), 60, "nothing deleted elsewhere");
    // 用户选「从网盘恢复」：词写回 B。
    b.run(
        &share,
        Options {
            restore_deletions: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(b.engine.words.len(), 60);
    assert_eq!(b.state.guard(), None);
    // 再来一次，这次用户确认要删：删除同步到 A。
    b.engine.words.clear();
    b.sync(&share);
    assert!(b.state.guard().is_some());
    b.run(
        &share,
        Options {
            confirm_deletions: true,
            ..Options::default()
        },
    )
    .unwrap();
    settle(&mut [&mut a, &mut b], &share);
    assert!(a.engine.words.is_empty());
}

#[test]
fn words_the_engine_refuses_are_not_retried_until_their_value_changes() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let odd = pinyin("xx", "怪");
    a.add(&odd);
    a.sync(&share);
    b.engine.refuse.insert(odd.clone());
    b.sync(&share);
    let applies = b.engine.applies;
    b.advance(1_000);
    b.sync(&share);
    assert_eq!(b.engine.applies, applies, "not retried");
    assert_eq!(b.state.status().pending_apply, 0);
    assert_eq!(b.state.acked(), b.state.ledger().unwrap().seq);
    // B 没有这个词也不会把它当成「本机删掉了」发出去。
    assert!(a.engine.words.contains(&odd));
    a.sync(&share);
    assert!(a.engine.words.contains(&odd));
}

#[test]
fn garbage_at_the_next_sequence_number_is_skipped_by_everyone() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    a.sync(&share);
    b.sync(&share);
    a.sync(&share);
    let seq = a.state.ledger().unwrap().seq;
    let path = super::sync::event_path(0, seq + 1);
    share.with(|disk| {
        assert!(!disk.files.contains_key(&path));
        disk.files
            .insert(path.clone(), b"{\"prev\": 1, \"junk\": true".to_vec());
    });
    let word = pinyin("ma", "妈");
    a.add(&word);
    a.sync(&share);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all, BTreeSet::from([word]));
}

#[test]
fn listing_lag_and_publish_contention_resolve_without_loss() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    a.sync(&share);
    b.sync(&share);
    // 新写的文件过三次列目录才看得见：B 不知道 A 刚写的序号，独占创建撞上之后重读再写。
    share.with(|disk| disk.faults.hide_new_for = 3);
    a.add(&pinyin("ma", "妈"));
    a.sync(&share);
    b.add(&pinyin("ba", "爸"));
    let _ = b.run(&share, Options::default());
    share.with(|disk| disk.faults.hide_new_for = 0);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all.len(), 2);
}

#[test]
fn network_drops_mid_sync_keep_all_persisted_progress() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    for index in 0..30u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
    }
    a.sync(&share);
    b.sync(&share);
    for index in 0..30u32 {
        b.add(&pinyin("ba", &format!("字{index}")));
        b.capture(&share);
    }
    for budget in [0usize, 1, 2, 3, 5, 8] {
        share.with(|disk| disk.faults.fail_after = Some(budget));
        let _ = b.run(&share, Options::default());
        share.with(|disk| {
            disk.faults.fail_after = None;
            disk.faults.offline = false;
        });
    }
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all.len(), 60);
}

#[test]
fn an_oversized_response_or_rejected_credentials_stop_the_sync_with_their_codes() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    a.sync(&share);
    let seq = a.state.ledger().unwrap().seq;
    share.with(|disk| {
        disk.files.insert(
            super::sync::event_path(0, seq + 1),
            vec![b' '; super::log::MAX_EVENT_BYTES as usize + 1],
        );
    });
    let error = a.run(&share, Options::default()).unwrap_err();
    assert_eq!(
        (error, error.code()),
        (SyncError::TooLarge, "dictionary_sync_too_large")
    );
    share.with(|disk| disk.faults.unauthorized = true);
    let error = a.run(&share, Options::default()).unwrap_err();
    assert_eq!(error.code(), "dictionary_sync_unauthorized");
}

#[test]
fn an_unknown_format_or_an_upgrade_seal_stops_reading_and_writing() {
    let share = Share::new();
    share.with(|disk| {
        disk.dirs.insert(super::sync::ROOT.to_owned());
        disk.dirs.insert(V1.to_owned());
        disk.files.insert(
            super::sync::FORMAT_FILE.to_owned(),
            br#"{"format":"msime-dictionary","version":2}"#.to_vec(),
        );
    });
    let mut a = Device::new(0xa1, Some(K1));
    assert_eq!(
        a.run(&share, Options::default()),
        Err(SyncError::Unsupported)
    );

    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    a.sync(&share);
    let ledger = a.state.ledger().unwrap().clone();
    let mut seal: serde_json::Value = serde_json::from_slice(&ledger.seal(1)).unwrap();
    seal["upgrade"] = serde_json::json!(2);
    share.with(|disk| {
        disk.files.insert(
            super::sync::event_path(ledger.epoch, ledger.seq + 1),
            serde_json::to_vec(&seal).unwrap(),
        );
    });
    a.add(&pinyin("ma", "妈"));
    assert_eq!(
        a.run(&share, Options::default()),
        Err(SyncError::Unsupported)
    );
    assert_eq!(a.state.outbox_len(), 1, "nothing written past the upgrade");
}

#[test]
fn a_pending_device_can_start_the_shared_dictionary_over_without_losing_anyones_words() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let mut c = Device::new(0xc3, None);
    let old = pinyin("jiu", "旧");
    let b_word = pinyin("bi", "笔");
    let c_word = pinyin("xin", "新");
    a.add(&old);
    a.sync(&share);
    b.sync(&share);
    b.add(&b_word);
    b.capture(&share);
    b.offline = true;
    // 只有 A 和 B 有旧密钥，A 丢了；C 在密钥串里重置，得到只属于自己的新密钥 K3。
    c.add(&c_word);
    assert!(c.sync(&share).pending_key);
    c.ring = ring(K3);
    assert!(c.sync(&share).pending_key, "the log is still under K1");
    {
        let mut connection = Connection {
            share: share.clone(),
        };
        let mut parts = Parts {
            remote: &mut connection,
            local: &mut c.engine,
            env: &mut c.env,
            sink: &mut c.sink,
        };
        restart(&mut c.state, &mut parts, &c.ring).unwrap();
    }
    c.sync(&share);
    // B 回来时被 C 批准，拿到 K3：认出共享词库重建过，把自己的词合并进来，什么也不删。
    b.offline = false;
    b.ring = ring(K3);
    let report = b.sync(&share);
    assert!(report.rejoined);
    let all = settle(&mut [&mut b, &mut c], &share);
    assert_eq!(all, BTreeSet::from([old, b_word, c_word]));
}

#[test]
fn nothing_on_the_share_or_in_debug_output_reveals_words_codes_or_keys() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let secret = pinyin("mi'mi", "秘密词条");
    a.add(&secret);
    a.sync(&share);
    let material = B64.encode([b'1'; 32]);
    for (path, bytes) in share.with(|disk| disk.files.clone()) {
        let text = String::from_utf8_lossy(&bytes);
        for needle in [
            "秘密词条",
            "mi'mi",
            "mi\\u0027mi",
            &material,
            &hex::encode([b'1'; 32]),
        ] {
            assert!(!text.contains(needle), "{path} contains {needle}");
        }
    }
    let debug = format!("{:?}", a.state);
    assert!(
        !debug.contains("秘密词条") && !debug.contains("mi'mi"),
        "{debug}"
    );
}

#[test]
fn the_scheduler_keeps_automatic_syncs_inside_the_request_budget() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let drive = |device: &mut Device, trigger: Trigger| {
        let mut connection = Connection {
            share: share.clone(),
        };
        let mut parts = Parts {
            remote: &mut connection,
            local: &mut device.engine,
            env: &mut device.env,
            sink: &mut device.sink,
        };
        schedule::drive(
            &mut device.state,
            &mut parts,
            &device.ring,
            trigger,
            Options::default(),
        )
    };
    let outcome = drive(&mut a, Trigger::Opened);
    assert!(outcome.network && outcome.result.is_ok());
    // 改了一个词：马上记下，但不联网；一分钟防抖之后才发。
    a.add(&pinyin("ma", "妈"));
    let requests = share.with(|disk| disk.requests);
    let outcome = drive(&mut a, Trigger::LocalChange);
    assert!(!outcome.network);
    assert_eq!(share.with(|disk| disk.requests), requests);
    assert_eq!(a.state.outbox_len(), 1);
    assert!(outcome.next_check_ms <= schedule::PUSH_DEBOUNCE_MS);
    a.advance(outcome.next_check_ms);
    let outcome = drive(&mut a, Trigger::Periodic);
    assert!(outcome.network);
    assert_eq!(a.state.outbox_len(), 0);
    // 一直改词、一直触发：30 分钟里的请求数不超过自动额度。
    let start = share.with(|disk| disk.requests);
    for index in 0..200u32 {
        a.add(&pinyin("ba", &format!("词{index}")));
        drive(&mut a, Trigger::LocalChange);
        a.advance(9_000);
        drive(&mut a, Trigger::Periodic);
    }
    let used = share.with(|disk| disk.requests) - start;
    assert!(
        used <= schedule::AUTOMATIC_REQUESTS,
        "{used} requests in 30 minutes"
    );
    // 手动同步还有额度可用；用完之后报限流。
    let outcome = drive(&mut a, Trigger::Manual);
    assert!(
        outcome.network && outcome.result.is_ok(),
        "{:?}",
        outcome.result
    );
    for _ in 0..schedule::MANUAL_REQUESTS {
        a.add(&pinyin("da", &format!("词{}", a.env.now_ms)));
        a.advance(1);
        drive(&mut a, Trigger::Manual);
    }
    let outcome = drive(&mut a, Trigger::Manual);
    assert_eq!(outcome.result.unwrap_err(), SyncError::RateLimited);
}

#[test]
fn a_large_first_upload_is_split_into_bounded_events() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    for index in 0..3_000u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
    }
    // 很长的快捷短语：按字节数而不是条数分批。
    for index in 0..100u32 {
        a.add(&WordKey::new(WordKind::Quick, &format!("q{index}"), &"长".repeat(1_300)).unwrap());
    }
    let report = a.sync(&share);
    assert_eq!(report.published_ops, 3_100);
    assert!(report.published_events >= 4, "{report:?}");
    for (path, bytes) in share.with(|disk| disk.history.clone()) {
        assert!(bytes.len() as u64 <= super::log::MAX_EVENT_BYTES, "{path}");
    }
    let report = b.sync(&share);
    assert_eq!(report.applied, 3_100);
    let all = settle(&mut [&mut a, &mut b], &share);
    assert_eq!(all.len(), 3_100);
}

#[test]
fn a_first_join_from_a_checkpoint_never_deletes_local_words_even_against_later_tombstones() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let word = pinyin("shui'shan", "水杉");
    a.add(&word);
    a.sync(&share);
    // C 登记之后就没再同步：它没确认那次删除，墓碑因此留在检查点里。
    let mut c = Device::new(0xc3, Some(K1));
    c.sync(&share);
    a.sync(&share);
    // A 的时钟快了一天，删掉这个词，然后写满一个 epoch，让新设备只能从检查点开始。
    a.advance(86_400_000);
    a.remove(&word);
    a.sync(&share);
    // 前面是 A 加词和登记、C 登记、A 删除这 3 个事件，再写 61 个正好攒满 64 个：封口后新 epoch 里没有事件，新设备只能从检查点得知那个墓碑。
    for index in 0..61u32 {
        a.add(&pinyin("ma", &format!("词{index}")));
        a.advance(1_000);
        a.sync(&share);
    }
    let ledger = a.state.ledger().unwrap();
    assert!(
        ledger.epoch >= 1 && ledger.events == 0,
        "{} {}",
        ledger.epoch,
        ledger.events
    );
    assert!(!ledger.replica.is_live(&word) && ledger.replica.entries.contains_key(&word));
    // 新设备本机就有这个词，时钟是准的：比墓碑早一天。
    let mut b = Device::new(0xb2, Some(K1));
    b.add(&word);
    let report = b.sync(&share);
    assert_eq!(report.received_events, 0, "joined from the checkpoint");
    assert!(b.engine.words.contains(&word));
    let all = settle(&mut [&mut a, &mut b], &share);
    assert!(
        all.contains(&word),
        "the joining device's word is kept and shared"
    );
}

#[test]
fn a_deletion_the_engine_refuses_is_not_turned_into_a_resurrection() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let word = pinyin("shan'bu'diao", "删不掉");
    a.add(&word);
    a.sync(&share);
    b.sync(&share);
    assert!(b.engine.words.contains(&word));
    b.engine.refuse.insert(word.clone());
    a.remove(&word);
    a.sync(&share);
    b.sync(&share);
    // B 的词库里还有它，但 B 不会把它当成本机新加的词发出去。
    assert!(b.engine.words.contains(&word));
    for _ in 0..3 {
        b.advance(1_000);
        b.sync(&share);
        a.advance(1_000);
        a.sync(&share);
    }
    assert!(!a.engine.words.contains(&word));
    assert_eq!(b.state.status().pending_apply, 0);
    assert_eq!(b.state.acked(), b.state.ledger().unwrap().seq);
    let b_adds = share
        .published(&[K1])
        .into_iter()
        .filter(|(device, key, live, _)| *device == b.id && *key == word && *live)
        .count();
    assert_eq!(b_adds, 0);
}

#[test]
fn deleting_and_re_adding_a_word_uses_a_new_request_id_each_time() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    let mut b = Device::new(0xb2, Some(K1));
    let word = pinyin("lai'hui", "来回");
    a.sync(&share);
    b.sync(&share);
    for _ in 0..3 {
        a.add(&word);
        a.advance(1_000);
        a.sync(&share);
        b.advance(1_000);
        b.sync(&share);
        assert!(b.engine.words.contains(&word));
        a.remove(&word);
        a.advance(1_000);
        a.sync(&share);
        b.advance(1_000);
        b.sync(&share);
        assert!(!b.engine.words.contains(&word));
    }
    // 三次加入、三次删除，各用一个编号。
    assert_eq!(b.engine.requests.len(), 6);
}

/// 合法长短语也会把检查点撑过读取上限：不能先封口、再发布一个自己读不了的文件；用户删减词条后应能继续同步。
#[test]
fn an_oversized_checkpoint_keeps_the_log_open_for_corrective_deletions() {
    // 崩溃与持久化由其他模拟覆盖；这里仅观察远端文件与封口，不在每批上传时重复编码几十 MiB 的本机快照。
    struct VolatileSink;
    impl StateSink for VolatileSink {
        fn save(&mut self, _: &SyncState) -> Result<(), SyncError> {
            Ok(())
        }
    }
    let share = Share::new();
    let synchronize = |device: &mut Device, options: Options| {
        let mut remote = Connection {
            share: share.clone(),
        };
        let mut sink = VolatileSink;
        let mut parts = Parts {
            remote: &mut remote,
            local: &mut device.engine,
            env: &mut device.env,
            sink: &mut sink,
        };
        run(&mut device.state, &mut parts, &device.ring, options)
    };
    let mut a = Device::new(0xa1, Some(K1));
    for index in 0..6_500 {
        a.add(&WordKey::new(WordKind::Quick, &format!("q{index}"), &"s".repeat(4_096)).unwrap());
    }
    assert!(synchronize(&mut a, Options::default()).is_ok());
    share.with(|disk| {
        for (path, bytes) in &disk.files {
            if path.ends_with("checkpoint.json") {
                assert!(
                    bytes.len() as u64 <= super::log::MAX_CHECKPOINT_BYTES,
                    "{path}: {}",
                    bytes.len()
                );
            }
        }
    });
    assert!(a.state.ledger().unwrap().sealed.is_none());
    let oversized_epoch = a.state.ledger().unwrap().epoch;
    let before_rekey = share.with(|disk| disk.history.len());
    a.ring.current = Some(data_key(K2));
    assert_eq!(
        synchronize(&mut a, Options::default()),
        Err(SyncError::TooLarge)
    );
    assert_eq!(share.with(|disk| disk.history.len()), before_rekey);
    assert!(a.state.ledger().unwrap().sealed.is_none());
    a.ring.current = Some(data_key(K1));
    a.engine.words.clear();
    for _ in 0..3 {
        synchronize(
            &mut a,
            Options {
                confirm_deletions: true,
                ..Options::default()
            },
        )
        .unwrap();
    }
    assert!(a.state.ledger().unwrap().replica.live_count() == 0);
    assert_eq!(a.state.status().outbox, 0);
    // 下一次设备确认把删除纳入回收范围后，检查点恢复为有界文件。
    a.advance(86_400_000);
    synchronize(&mut a, Options::default()).unwrap();
    assert!(a.state.ledger().unwrap().epoch > oversized_epoch);
    let mut b = Device::new(0xb2, Some(K1));
    synchronize(&mut b, Options::default()).unwrap();
    assert!(b.engine.words.is_empty());
}

/// 重建也必须在封口前检查本机词库能否装进检查点。
#[test]
fn an_oversized_restart_leaves_the_existing_log_writable() {
    let share = Share::new();
    let mut a = Device::new(0xa1, Some(K1));
    a.sync(&share);
    let before = share.with(|disk| (disk.history.clone(), disk.dirs.clone()));
    let mut c = Device::new(0xc3, Some(K2));
    for index in 0..6_500 {
        c.add(&WordKey::new(WordKind::Quick, &format!("q{index}"), &"s".repeat(4_096)).unwrap());
    }
    let mut connection = Connection {
        share: share.clone(),
    };
    let mut parts = Parts {
        remote: &mut connection,
        local: &mut c.engine,
        env: &mut c.env,
        sink: &mut c.sink,
    };
    assert_eq!(
        restart(&mut c.state, &mut parts, &c.ring),
        Err(SyncError::TooLarge)
    );
    assert_eq!(
        share.with(|disk| (disk.history.clone(), disk.dirs.clone())),
        before
    );
    a.add(&pinyin("shui", "合成词"));
    a.sync(&share);
}
