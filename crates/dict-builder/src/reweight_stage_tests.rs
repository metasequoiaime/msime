//! `reweight` 阶段接在构建里的测试：checkout 没有登记万象时什么也不做，登记了就按万象换算、改坏了字节就失败，`curation/pinyin-reweight.txt` 打开第二步。

use super::*;

/// 一个最小的 msime-dictionary checkout 和读它的 `Build`。`registered` 时 `upstream.lock.json` 登记 `rime-wanxiang` 和 17 份万象词库，msime 的锁总是有这个 reference。
fn fixture(registered: bool) -> (tempfile::TempDir, PathBuf, PathBuf, Build) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("builder");
    let dictionary = temporary.path().join("dictionary");
    let out = temporary.path().join("out");
    std::fs::create_dir_all(root.join("resources/dictionary-sources")).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let write = |path: &str, bytes: &str| {
        let file = dictionary.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    };
    write(reweight::SINGLE_CHARS, "恩\ten\t299754\n");
    // 门的 是万象不收的碎片，压着 闷的；车检 是 万象 163 × 9.2。
    write(
        reweight::RIME_ICE,
        "门的\tmen'de\t435135\r\n闷的\tmen'de\t208546\r\n车检\tche'jian\t1500\r\n",
    );
    write(
        reweight::RIME_ICE_SUPPLEMENT,
        "# 补充表\n阿贝\ta'bei\t4351\n",
    );
    let header = "# Rime dictionary\n---\nname: fixture\n...\n";
    let mut upstream_files = vec![reweight::RIME_ICE, reweight::RIME_ICE_SUPPLEMENT];
    let mut wanxiang_paths = Vec::new();
    for name in reweight::IMPORTED
        .iter()
        .chain(reweight::RECORDED_ONLY.iter())
    {
        let body = match *name {
            "zi" => "恩\tēn\t31821\n",
            "jichu" => "闷的\tmèn de\t22668\n车检\tchē jiǎn\t163\n阿贝\tā bèi\t1052\n",
            _ => "",
        };
        let path = reweight::dictionary_path(name);
        write(&path, &format!("{header}{body}"));
        wanxiang_paths.push(path);
    }
    if registered {
        upstream_files.extend(wanxiang_paths.iter().map(String::as_str));
    }
    let reference = |name: &str| {
        (
            name.to_owned(),
            serde_json::json!({"repository": format!("https://example.invalid/{name}"), "commit": "a".repeat(40)}),
        )
    };
    let names = ["rime-ice", "rime-ice-supplement", reweight::REFERENCE];
    let references: serde_json::Map<String, serde_json::Value> =
        names.into_iter().map(reference).collect();
    let upstreams: serde_json::Map<String, serde_json::Value> = names
        .into_iter()
        .filter(|name| registered || *name != reweight::REFERENCE)
        .map(reference)
        .collect();
    let files: Vec<_> = upstream_files
        .iter()
        .map(|path| {
            serde_json::json!({
                "path": path,
                "upstream": sources::upstream_reference(path).unwrap(),
                "size": std::fs::metadata(dictionary.join(path)).unwrap().len(),
                "sha256": sources::sha256_file(&dictionary.join(path)).unwrap(),
            })
        })
        .collect();
    write(
        sources::UPSTREAM_LOCK,
        &serde_json::to_string(
            &serde_json::json!({"version": 1, "upstreams": upstreams, "files": files}),
        )
        .unwrap(),
    );
    let lock = serde_json::json!({"references": references, "mozc": {"repository": "https://example.invalid/mozc", "commit": "b".repeat(40)}, "files": []});
    std::fs::write(
        root.join("resources/dictionary-sources.lock.json"),
        serde_json::to_vec(&lock).unwrap(),
    )
    .unwrap();
    let lock = Lock::load(&root.join("resources/dictionary-sources.lock.json")).unwrap();
    let build = Build {
        sources: Sources {
            dictionary: Some(Dictionary::open(dictionary.clone(), &lock).unwrap()),
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache: temporary.path().join("cache"),
            offline: true,
        },
        out: out.clone(),
        complete: false,
    };
    (temporary, dictionary, out, build)
}

fn weight(out: &Path, value: &str, key: &str) -> i64 {
    let table = msime::pinyin_table(key).unwrap();
    sqlite::open(&out.join("msime-pinyin.db"))
        .unwrap()
        .query_row(
            &format!("select weight from {table} where key = ? and value = ?"),
            [key, value],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn the_stage_follows_quanpin_and_precedes_places() {
    let position = |stage| STAGES.iter().position(|listed| *listed == stage).unwrap();
    assert_eq!(position(Stage::Reweight), position(Stage::Quanpin) + 1);
    assert_eq!(
        position(Stage::PlacesSupplement),
        position(Stage::Reweight) + 1
    );
    assert_eq!(stage_name(Stage::Reweight), "reweight");
}

#[test]
fn a_checkout_that_does_not_record_wanxiang_is_left_alone() {
    let (_temporary, _dictionary, out, build) = fixture(false);
    build.run(Stage::Quanpin).unwrap();
    let before = sha256_file(&out.join("msime-pinyin.db")).unwrap();
    let summary = build.run(Stage::Reweight).unwrap();
    assert!(summary.starts_with("left out"), "{summary}");
    assert_eq!(sha256_file(&out.join("msime-pinyin.db")).unwrap(), before);
}

#[test]
fn the_first_step_leaves_aligned_sources_unchanged_and_realigns_a_regenerated_one() {
    let (_temporary, dictionary, out, build) = fixture(true);
    build.run(Stage::Quanpin).unwrap();
    let summary = build.run(Stage::Reweight).unwrap();
    assert_eq!(
        summary,
        "first step only: 4 rows shared with rime-wanxiang, 1 realigned, 0 rescaled, 0 capped"
    );
    // 补充表里的 阿贝 是上游原值 4351，换回 1052 × 9.28；其余已经对齐的行不变。
    assert_eq!(weight(&out, "阿贝", "a'bei"), 9763);
    assert_eq!(weight(&out, "车检", "che'jian"), 1500);
    assert_eq!(weight(&out, "门的", "men'de"), 435135);
    assert_eq!(weight(&out, "恩", "en"), 299754);
    // 再跑一次什么也不改。
    let before = sha256_file(&out.join("msime-pinyin.db")).unwrap();
    assert!(build.run(Stage::Reweight).unwrap().contains(" 0 realigned"));
    assert_eq!(sha256_file(&out.join("msime-pinyin.db")).unwrap(), before);
    // 打开第二步：门的 压到 闷的 下面。
    std::fs::create_dir_all(dictionary.join("curation")).unwrap();
    std::fs::write(
        dictionary.join(reweight::SETTINGS),
        "# 第二步\nunattested-cap\ton\n",
    )
    .unwrap();
    build.run(Stage::Quanpin).unwrap();
    let summary = build.run(Stage::Reweight).unwrap();
    assert!(
        summary.starts_with("second step: unattested-cap on"),
        "{summary}"
    );
    assert!(summary.ends_with("1 capped"), "{summary}");
    assert_eq!(weight(&out, "门的", "men'de"), 208545);
}

#[test]
fn a_changed_wanxiang_file_fails_the_stage() {
    let (_temporary, dictionary, _out, build) = fixture(true);
    build.run(Stage::Quanpin).unwrap();
    std::fs::write(
        dictionary.join(reweight::dictionary_path("jichu")),
        "# Rime dictionary\n...\n门的\tmén de\t99999\n",
    )
    .unwrap();
    let error = format!("{:#}", build.run(Stage::Reweight).unwrap_err());
    assert!(
        error.contains("differs from the size and SHA-256"),
        "{error}"
    );
}

#[test]
fn a_missing_recorded_only_file_fails_the_stage() {
    let (_temporary, dictionary, _out, build) = fixture(true);
    build.run(Stage::Quanpin).unwrap();
    std::fs::remove_file(dictionary.join(reweight::dictionary_path("cuoyin"))).unwrap();
    let error = format!("{:#}", build.run(Stage::Reweight).unwrap_err());
    assert!(error.contains("cuoyin.dict.yaml"), "{error}");
}
