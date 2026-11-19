//! 下载限流必须结束整轮，不能继续消耗其他设备的读取额度。

use super::*;

#[test]
fn download_rate_limit_stops_before_the_next_device_and_can_resume_next_round() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    let storage_c = store(&pair._root.path().join("c"));
    let c = session(&pair.server, &storage_c);
    sign_in(&c);
    let pending = status(&c);
    pair.a
        .learning_operation(
            Operation::Approve {
                device_id: pending["device_id"].as_str().unwrap().into(),
            },
            "合成 A",
            None,
        )
        .unwrap();
    let confirm = status(&c);
    c.learning_operation(
        Operation::Confirm {
            key_id: confirm["confirm"]["key_id"].as_str().unwrap().into(),
            approver_device_id: confirm["confirm"]["approver_device_id"]
                .as_str()
                .unwrap()
                .into(),
        },
        "合成 C",
        None,
    )
    .unwrap();
    let keys_c = pair.keys(&storage_c);
    let mut stream = Vec::new();
    msime_engine::write_learning_sync(&state(&[weight(12, 200)]), "synthetic-v1", &mut stream)
        .unwrap();
    let envelope = keys_c.seal_blob(1, &stream).unwrap();
    pair.edit_round(|round| {
        round.blobs.insert(keys_c.device_id(), (1, envelope));
        round.download_status = Some(429);
    });

    assert!(matches!(
        pair.run(),
        Err(RoundError::Http {
            status: 429,
            retry_after_seconds: Some(17)
        })
    ));
    pair.edit_round(|round| {
        assert_eq!(
            round
                .trace
                .iter()
                .filter(|entry| entry.starts_with("download:"))
                .count(),
            1
        );
    });
    let progress = StateStore::new(&pair.state)
        .read(&pair.login.user_id)
        .unwrap();
    assert!(progress.sources.is_empty());
    assert_eq!(progress.uploads.len(), 1);
    assert_eq!(pair.exported().records().len(), 0);

    // 后一轮可以重读两台设备；此前限流不能把它们标成已经合并。
    pair.edit_round(|round| {
        round.download_status = None;
        round.trace.clear();
    });
    let report = pair.run().unwrap();
    assert!(report.upload_unchanged);
    assert_eq!(report.downloads.len(), 2);
    assert!(report.downloads.iter().all(|download| {
        download.generation == Some(1) && download.outcome == DownloadOutcome::Applied
    }));
    assert_eq!(pair.exported(), state(&[weight(12, 200)]));
}
