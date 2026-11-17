//! 真实账号驱动、MSL1 密文、学习流、SQLite 与持久进度串起来的合成行为测试。

use super::*;
use crate::account::learning_sync::round::{
    DownloadOutcome, RoundContext, RoundError, RoundReport,
};
use crate::account::learning_sync::state::{RemoteProgress, StateStore};
use msime_engine::{LearningSyncRecord, LearningSyncState, RuntimePaths};

#[derive(Default)]
pub(super) struct RoundServer {
    blobs: BTreeMap<String, (u64, Vec<u8>)>,
    trace: Vec<String>,
    upload_status: Option<u16>,
    download_status: Option<u16>,
    metadata_patch: Option<fn(&mut Value)>,
    download_override: Option<Vec<u8>>,
}

fn blob_response(status: u16, body: Vec<u8>) -> BlobResponse {
    BlobResponse {
        status,
        body,
        retry_after_seconds: Some(17),
    }
}

pub(super) fn handle(server: &mut Server, request: &BlobRequest<'_>) -> BlobResponse {
    let round = server.round.as_mut().unwrap();
    match request {
        BlobRequest::Metadata { device_id } => {
            round.trace.push("metadata".into());
            let devices: Vec<_> = server.devices.iter().map(|(id, device)| json!({
                "device_id": id, "public_key": device["public_key"], "approved": server.wraps.contains_key(id),
            })).collect();
            let blobs: Vec<_> = round.blobs.iter().map(|(id, (revision, bytes))| json!({
                "device_id": id, "key_id": server.key, "revision": revision, "bytes": bytes.len(),
            })).collect();
            let mut value = json!({"key_id": server.key, "devices": devices, "wrap": server.wraps.get(*device_id), "blobs": blobs});
            if let Some(patch) = round.metadata_patch {
                patch(&mut value);
            }
            blob_response(200, value.to_string().into_bytes())
        }
        BlobRequest::Upload {
            device_id,
            generation,
            envelope,
        } => {
            round.trace.push(format!("upload:{device_id}:{generation}"));
            if let Some(status) = round.upload_status {
                return blob_response(status, vec![]);
            }
            if round
                .blobs
                .get(*device_id)
                .is_some_and(|(old, _)| old >= generation)
            {
                return blob_response(409, vec![]);
            }
            round
                .blobs
                .insert((*device_id).into(), (*generation, envelope.to_vec()));
            blob_response(204, vec![])
        }
        BlobRequest::Download { device_id } => {
            round.trace.push(format!("download:{device_id}"));
            if let Some(status) = round.download_status {
                return blob_response(status, vec![]);
            }
            let body = round
                .download_override
                .clone()
                .unwrap_or_else(|| round.blobs[*device_id].1.clone());
            blob_response(200, body)
        }
        BlobRequest::Delete { device_id } => {
            round.trace.push(format!("delete:{device_id}"));
            round.blobs.remove(*device_id);
            blob_response(204, vec![])
        }
    }
}

struct Pair {
    _root: tempfile::TempDir,
    server: Arc<Mutex<Server>>,
    a: BackendAccountSession<Api, FileAccountSessionStorage>,
    storage_a: FileAccountSessionStorage,
    storage_b: FileAccountSessionStorage,
    login: AccountLoginBinding,
    state: std::path::PathBuf,
    paths: RuntimePaths,
}

impl Pair {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let server = Arc::new(Mutex::new(Server::default()));
        let storage_a = store(&root.path().join("a"));
        let storage_b = store(&root.path().join("b"));
        let a = session(&server, &storage_a);
        let b = session(&server, &storage_b);
        let login = sign_in(&a);
        sign_in(&b);
        assert_eq!(status(&a)["status"], "ready");
        let pending = status(&b);
        a.learning_operation(
            Operation::Approve {
                device_id: pending["device_id"].as_str().unwrap().into(),
            },
            "合成 A",
            None,
        )
        .unwrap();
        let confirm = status(&b);
        b.learning_operation(
            Operation::Confirm {
                key_id: confirm["confirm"]["key_id"].as_str().unwrap().into(),
                approver_device_id: confirm["confirm"]["approver_device_id"]
                    .as_str()
                    .unwrap()
                    .into(),
            },
            "合成 B",
            None,
        )
        .unwrap();
        let paths = RuntimePaths {
            resources: root.path().join("resources"),
            user_data: root.path().join("user"),
            cache: root.path().join("cache"),
            dictionaries: root.path().join("dictionaries"),
        };
        for path in [
            &paths.resources,
            &paths.user_data,
            &paths.cache,
            &paths.dictionaries,
        ] {
            std::fs::create_dir(path).unwrap();
        }
        msime_engine::ensure_english_schema(&paths.dictionary("msime-english.db")).unwrap();
        let state = root.path().join("a");
        server.lock().unwrap().round = Some(RoundServer::default());
        Self {
            _root: root,
            server,
            a,
            storage_a,
            storage_b,
            login,
            state,
            paths,
        }
    }

    fn context(&self) -> RoundContext<'_> {
        RoundContext {
            paths: &self.paths,
            state_directory: &self.state,
            content_id: "synthetic-v1",
            main_dictionary: false,
        }
    }

    fn keys(&self, storage: &FileAccountSessionStorage) -> Keystore {
        let value: Value =
            serde_json::from_str(&storage.load_learning_keystore().unwrap().unwrap()).unwrap();
        Keystore::parse(value["keystore"].as_str().unwrap()).unwrap()
    }

    fn edit_round(&self, edit: impl FnOnce(&mut RoundServer)) {
        edit(self.server.lock().unwrap().round.as_mut().unwrap());
    }

    fn remote_stream(&self, generation: u64, stream: &[u8]) {
        let keys = self.keys(&self.storage_b);
        let envelope = keys.seal_blob(generation, stream).unwrap();
        self.edit_round(|round| {
            round.blobs.insert(keys.device_id(), (generation, envelope));
        });
    }

    fn remote(&self, generation: u64, records: &[LearningSyncRecord], content: &str) {
        let mut stream = Vec::new();
        msime_engine::write_learning_sync(&state(records), content, &mut stream).unwrap();
        self.remote_stream(generation, &stream);
    }

    fn run(&self) -> Result<RoundReport, RoundError> {
        self.a
            .learning_sync_once(&self.login, "合成 A", &self.context())
    }

    fn progress(&self) -> RemoteProgress {
        let progress = StateStore::new(&self.state)
            .read(&self.login.user_id)
            .unwrap();
        let keys = self.keys(&self.storage_b);
        progress
            .sources
            .get(keys.ready_key_id().unwrap())
            .and_then(|sources| sources.0.get(&keys.device_id()))
            .copied()
            .unwrap_or_default()
    }

    fn exported(&self) -> LearningSyncState {
        let mut actual = LearningSyncState::default();
        msime_engine::stream_learning_sync_state(&self.paths, &mut |record| {
            actual.insert(record.clone()).unwrap();
            true
        })
        .unwrap();
        actual
    }

    fn after_blob(
        &self,
        kind: &'static str,
        action: impl Fn() + Send + Sync + 'static,
    ) -> BackendAccountSession<Api, FileAccountSessionStorage> {
        let server = self.server.clone();
        let used = std::sync::atomic::AtomicBool::new(false);
        BackendAccountSession::new(
            Api {
                server: server.clone(),
                after_request: Some(Arc::new(move || {
                    let matches = server
                        .lock()
                        .unwrap()
                        .round
                        .as_ref()
                        .unwrap()
                        .trace
                        .last()
                        .is_some_and(|line| line.starts_with(kind));
                    if matches && !used.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        action();
                    }
                })),
            },
            self.storage_a.clone(),
        )
    }
}

fn state(records: &[LearningSyncRecord]) -> LearningSyncState {
    let mut result = LearningSyncState::default();
    for record in records {
        result.insert(record.clone()).unwrap();
    }
    result
}

fn weight(frequency: u64, time: u64) -> LearningSyncRecord {
    LearningSyncRecord::Weight {
        dictionary: "english".into(),
        key: "sample".into(),
        value: "Sample".into(),
        display: "Sample".into(),
        weight: frequency,
        updated_at: time,
    }
}

#[test]
fn first_upload_precedes_download_and_real_merge_then_skips_unchanged() {
    let pair = Pair::new();
    msime_engine::merge_learning_sync_state(&pair.paths, false, &state(&[weight(4, 10)]), 0)
        .unwrap();
    pair.remote(3, &[weight(7, 20)], "synthetic-v2");
    let report = pair.run().unwrap();
    assert_eq!(report.uploaded, Some(1));
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Applied);
    assert!(report.downloads[0].dictionary_mismatch);
    assert_eq!(pair.exported(), state(&[weight(7, 20)]));
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 3,
            applied: 3
        }
    );
    let keys = pair.keys(&pair.storage_a);
    pair.edit_round(|round| {
        assert_eq!(round.trace[0], "metadata");
        assert!(round.trace[1].starts_with("upload:"));
        assert!(round.trace[2].starts_with("download:"));
        let opened = keys
            .open_blob(&keys.device_id(), &round.blobs[&keys.device_id()].1)
            .unwrap();
        let uploaded =
            msime_engine::read_learning_sync(std::io::Cursor::new(opened.stream)).unwrap();
        assert_eq!(uploaded.state, state(&[weight(4, 10)]));
    });
    // 合并改变摘要，下一轮上传一次；第三轮摘要与服务端代次都未变。
    assert_eq!(pair.run().unwrap().uploaded, Some(2));
    let unchanged = pair.run().unwrap();
    assert!(unchanged.upload_unchanged);
    assert_eq!(
        unchanged.downloads[0].outcome,
        DownloadOutcome::AlreadyApplied
    );
}

#[test]
fn lost_or_rolled_back_server_replica_is_restored_above_local_uploaded() {
    let pair = Pair::new();
    assert_eq!(pair.run().unwrap().uploaded, Some(1));
    let own = pair.keys(&pair.storage_a).device_id();
    pair.edit_round(|round| {
        round.blobs.remove(&own);
    });
    assert_eq!(pair.run().unwrap().uploaded, Some(2));
    pair.edit_round(|round| {
        round.blobs.get_mut(&own).unwrap().0 = 1;
    });
    assert_eq!(pair.run().unwrap().uploaded, Some(3));
}

#[test]
fn exhausted_remote_generation_is_deleted_before_upload_without_resetting_local_counter() {
    let pair = Pair::new();
    pair.run().unwrap();
    let own = pair.keys(&pair.storage_a).device_id();
    pair.edit_round(|round| {
        round.blobs.get_mut(&own).unwrap().0 = learning_e2e::MAX_GENERATION;
        round.trace.clear();
    });
    assert_eq!(pair.run().unwrap().uploaded, Some(2));
    pair.edit_round(|round| {
        assert_eq!(
            round.trace,
            vec![
                "metadata".to_owned(),
                format!("delete:{own}"),
                format!("upload:{own}:2")
            ]
        );
    });
}

#[test]
fn quota_failure_does_not_prevent_remote_merge_or_record_failed_upload() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 40)], "synthetic-v1");
    pair.edit_round(|round| round.upload_status = Some(413));
    let report = pair.run().unwrap();
    assert!(matches!(
        report.upload_error,
        Some(RoundError::Http { status: 413, .. })
    ));
    assert_eq!(report.uploaded, None);
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Applied);
    assert!(StateStore::new(&pair.state)
        .read(&pair.login.user_id)
        .unwrap()
        .uploads
        .is_empty());
    assert_eq!(pair.exported(), state(&[weight(9, 40)]));
}

#[test]
fn upload_rate_limit_returns_retry_after_without_recording_success() {
    let pair = Pair::new();
    pair.edit_round(|round| round.upload_status = Some(429));
    assert!(matches!(
        pair.run(),
        Err(RoundError::Http {
            status: 429,
            retry_after_seconds: Some(17)
        })
    ));
    assert!(StateStore::new(&pair.state)
        .read(&pair.login.user_id)
        .unwrap()
        .uploads
        .is_empty());
}

#[test]
fn full_stream_validation_is_required_even_after_successful_aead() {
    let pair = Pair::new();
    pair.remote_stream(100, b"synthetic invalid stream\n");
    let report = pair.run().unwrap();
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Undecryptable);
    assert_eq!(pair.progress(), RemoteProgress::default());
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        0
    );
    assert_eq!(pair.exported().records().len(), 0);

    // 合法前缀中的未来时间也不能绕过页尾校验提前推进观察值或写入本机。
    let mut stream = Vec::new();
    msime_engine::write_learning_sync(&state(&[weight(9, 900)]), "synthetic-v1", &mut stream)
        .unwrap();
    let footer_marker = b"{\"type\":\"footer\"";
    let footer = stream
        .windows(footer_marker.len())
        .position(|bytes| bytes == footer_marker)
        .unwrap();
    stream.truncate(footer);
    pair.remote_stream(101, &stream);
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(pair.progress(), RemoteProgress::default());
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        0
    );
}

#[test]
fn tampered_ciphertext_and_wrong_source_do_not_advance_progress() {
    let pair = Pair::new();
    pair.remote(1, &[weight(10, 60)], "synthetic-v1");
    pair.edit_round(|round| {
        round.blobs.values_mut().next().unwrap().1[70] ^= 1;
    });
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(pair.progress(), RemoteProgress::default());
    let a = pair.keys(&pair.storage_a);
    pair.edit_round(|round| {
        round.download_override = Some(round.blobs[&a.device_id()].1.clone());
    });
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(pair.progress(), RemoteProgress::default());
}

#[test]
fn authenticated_generation_may_advance_since_metadata_but_cannot_fall_below_it() {
    let pair = Pair::new();
    pair.remote(5, &[weight(10, 60)], "synthetic-v1");
    pair.edit_round(|round| {
        round.metadata_patch = Some(|value| {
            value["blobs"][0]["revision"] = json!(4);
        })
    });
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Applied
    );
    assert_eq!(pair.progress().verified, 5);
    pair.edit_round(|round| {
        round.metadata_patch = Some(|value| {
            // 只提高远端记录；本机上传记录保留原代次。
            for blob in value["blobs"].as_array_mut().unwrap() {
                blob["revision"] = json!(100);
            }
        })
    });
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(pair.progress().verified, 5);
}

#[test]
fn authenticated_rollback_is_rejected_without_observing_its_time() {
    let pair = Pair::new();
    pair.remote(5, &[weight(10, 60)], "synthetic-v1");
    pair.run().unwrap();
    pair.remote(4, &[weight(99, 900)], "synthetic-v1");
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Rollback
    );
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 5,
            applied: 5
        }
    );
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        60
    );
}

#[test]
fn merge_failure_keeps_verified_and_retries_the_same_generation_after_repair() {
    let pair = Pair::new();
    pair.remote(2, &[weight(7, 70)], "synthetic-v1");
    let english = pair.paths.dictionary("msime-english.db");
    std::fs::remove_file(&english).unwrap();
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::MergeFailed
    );
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 0
        }
    );
    msime_engine::ensure_english_schema(&english).unwrap();
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Applied
    );
    assert_eq!(pair.exported(), state(&[weight(7, 70)]));
    assert_eq!(pair.progress().applied, 2);
}

#[test]
fn reset_during_download_is_reread_before_applying_remote_rows() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    let directory = pair.state.clone();
    let account = pair.login.user_id.clone();
    let paths = pair.paths.clone();
    let session = pair.after_blob("download:", move || {
        let _maintenance = crate::dictionary::access::DictionaryAccess::try_maintenance(
            &paths.user_data,
            &paths.dictionaries,
        )
        .unwrap()
        .unwrap();
        StateStore::new(&directory)
            .prepare_reset(&account, 100, 0)
            .unwrap();
    });
    let report = session
        .learning_sync_once(&pair.login, "合成 A", &pair.context())
        .unwrap();
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Applied);
    assert_eq!(pair.exported().records().len(), 0);
    assert_eq!(pair.progress().applied, 1);
}

#[test]
fn maintenance_busy_during_download_persists_verified_ciphertext_until_idle() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    let paths = pair.paths.clone();
    let held = Arc::new(Mutex::new(None));
    let captured = held.clone();
    let session = pair.after_blob("download:", move || {
        *captured.lock().unwrap() = crate::dictionary::access::DictionaryAccess::try_session(
            &paths.user_data,
            &paths.dictionaries,
        )
        .unwrap();
    });
    let report = session
        .learning_sync_once(&pair.login, "合成 A", &pair.context())
        .unwrap();
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Pending);
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 1,
            applied: 0
        }
    );
    assert_eq!(pair.exported().records().len(), 0);
    held.lock().unwrap().take();
    let before = pair
        .server
        .lock()
        .unwrap()
        .round
        .as_ref()
        .unwrap()
        .trace
        .clone();
    // 模拟持有词库的进程重启；服务端不再提供该文件，认领不应重新下载。
    pair.edit_round(|round| {
        round.blobs.clear();
        round.download_status = Some(503);
    });
    let restarted = super::session(&pair.server, &pair.storage_a);
    let merged = restarted
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap();
    assert_eq!(merged[0].outcome, DownloadOutcome::Applied);
    assert_eq!(
        pair.server.lock().unwrap().round.as_ref().unwrap().trace,
        before
    );
    assert_eq!(pair.exported(), state(&[weight(9, 100)]));
    assert_eq!(pair.progress().applied, 1);
    assert!(restarted
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap()
        .is_empty());
}

#[path = "round/pending.rs"]
mod pending;

#[path = "round/rate_limit.rs"]
mod rate_limit;

#[test]
fn relogin_during_upload_cannot_record_uploaded_or_start_downloading() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    let storage = pair.storage_a.clone();
    let server = pair.server.clone();
    let session = pair.after_blob("upload:", move || {
        sign_in(&super::session(&server, &storage));
    });
    assert!(matches!(
        session.learning_sync_once(&pair.login, "合成 A", &pair.context()),
        Err(RoundError::Account(AccountError::Cancelled))
    ));
    assert!(StateStore::new(&pair.state)
        .read(&pair.login.user_id)
        .unwrap()
        .uploads
        .is_empty());
    assert_eq!(pair.progress(), RemoteProgress::default());
    pair.edit_round(|round| {
        assert!(!round
            .trace
            .iter()
            .any(|entry| entry.starts_with("download:")))
    });
}

#[test]
fn key_change_in_same_login_during_download_cannot_accept_or_merge_old_data() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    let storage = pair.storage_a.clone();
    let server = pair.server.clone();
    let session = pair.after_blob("download:", move || {
        super::session(&server, &storage)
            .learning_operation(Operation::Reset, "合成 A", None)
            .unwrap();
    });
    assert!(matches!(
        session.learning_sync_once(&pair.login, "合成 A", &pair.context()),
        Err(RoundError::Account(AccountError::Cancelled))
    ));
    assert_eq!(pair.progress(), RemoteProgress::default());
    assert_eq!(pair.exported().records().len(), 0);
}

#[test]
fn malformed_or_ambiguous_metadata_stops_before_upload() {
    for patch in [
        (|value: &mut Value| {
            value["devices"][0]["public_key"] = json!("invalid");
        }) as fn(&mut Value),
        |value: &mut Value| {
            let duplicate = value["devices"][0].clone();
            value["devices"].as_array_mut().unwrap().push(duplicate);
        },
        |value: &mut Value| {
            let duplicate = value["blobs"][0].clone();
            value["blobs"].as_array_mut().unwrap().push(duplicate);
        },
        |value: &mut Value| {
            value["blobs"][0]["device_id"] = json!("0".repeat(32));
        },
        |value: &mut Value| {
            value["blobs"][0]["revision"] = json!(0);
        },
        |value: &mut Value| {
            value["blobs"][0]["key_id"] = json!("0".repeat(32));
        },
    ] {
        let pair = Pair::new();
        pair.remote(1, &[weight(9, 100)], "synthetic-v1");
        pair.edit_round(|round| round.metadata_patch = Some(patch));
        assert!(matches!(pair.run(), Err(RoundError::InvalidMetadata)));
        pair.edit_round(|round| assert_eq!(round.trace, vec!["metadata"]));
        assert_eq!(pair.progress(), RemoteProgress::default());
    }
}

#[test]
fn concurrent_round_is_rejected_before_network_and_releases_its_lock() {
    let pair = Pair::new();
    let store = StateStore::new(&pair.state);
    let held = store.try_round().unwrap().unwrap();
    assert!(matches!(pair.run(), Err(RoundError::Busy)));
    pair.edit_round(|round| assert!(round.trace.is_empty()));
    drop(held);
    assert_eq!(pair.run().unwrap().uploaded, Some(1));
}

#[test]
fn waiting_approval_never_starts_data_requests() {
    let pair = Pair::new();
    let own = pair.keys(&pair.storage_a).device_id();
    pair.server.lock().unwrap().wraps.remove(&own);
    assert!(matches!(pair.run(), Err(RoundError::NotReady)));
    pair.edit_round(|round| assert!(round.trace.is_empty()));
}

#[test]
fn corrupt_progress_is_preserved_and_no_upload_is_attempted() {
    let pair = Pair::new();
    let cloud = pair.state.join("cloud-sync");
    std::fs::create_dir(&cloud).unwrap();
    let file = cloud.join("learning.json");
    std::fs::write(&file, b"synthetic corrupt state").unwrap();
    assert!(matches!(
        pair.run(),
        Err(RoundError::State(
            crate::account::learning_sync::state::StateError::Invalid
        ))
    ));
    assert_eq!(std::fs::read(&file).unwrap(), b"synthetic corrupt state");
    pair.edit_round(|round| assert_eq!(round.trace, vec!["metadata"]));
}

#[test]
fn remote_http_failure_preserves_retry_after_and_does_not_mark_verified() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    pair.edit_round(|round| round.download_status = Some(503));
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Http {
            status: 503,
            retry_after_seconds: Some(17)
        }
    );
    assert_eq!(pair.progress(), RemoteProgress::default());
}
