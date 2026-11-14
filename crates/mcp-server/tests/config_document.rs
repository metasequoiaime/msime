//! 用独立进程验证完整配置命令，不启动 GUI，也不访问真实用户目录。

use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Output};

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_msime-mcp"))
        .env_remove("MSIME_CLIENT_HOST_OPTIONS")
        .env_remove("MSIME_IBUS_OPTIONS")
        .arg("--state-dir")
        .arg(root.join("state"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn script_validates_applies_and_detects_a_stale_revision_without_exposing_values() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("patch.json");
    std::fs::write(&file, br#"{"candidate_page_size":7}"#).unwrap();
    let path = file.to_str().unwrap();
    let checked = run(root.path(), &["config", "validate", path]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&checked.stdout).unwrap(),
        json!({"valid":true,"revision":0})
    );
    assert!(!root.path().join("state").exists());
    let denied = run(root.path(), &["config", "apply", path, "0"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("--allow-write"));
    assert!(!root.path().join("state").exists());
    let applied = run(
        root.path(),
        &["--allow-write", "config", "apply", path, "0"],
    );
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let revision = serde_json::from_slice::<Value>(&applied.stdout).unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert!(revision > 0);
    let store_file = root.path().join("state/preferences.json");
    let before = std::fs::read(&store_file).unwrap();
    let conflict = run(
        root.path(),
        &["--allow-write", "config", "apply", path, "0"],
    );
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("revision conflict"));
    assert_eq!(std::fs::read(&store_file).unwrap(), before);
    std::fs::write(
        &file,
        br#"{"sentence_association":{"neural_keybord":"synthetic-private-value"}}"#,
    )
    .unwrap();
    for args in [
        vec!["config", "validate", path],
        vec!["--allow-write", "config", "apply", path, "1"],
    ] {
        let result = run(root.path(), &args);
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("synthetic-private-value"));
        assert!(result.stdout.is_empty());
        assert_eq!(std::fs::read(&store_file).unwrap(), before);
    }
    let current = run(root.path(), &["config", "revision"]);
    assert!(current.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&current.stdout).unwrap(),
        json!({"revision":revision})
    );
}

#[test]
fn schema_works_without_runtime_options_and_includes_omitted_fields() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["config", "schema"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(schema["properties"]["usage_reporting_consent"].is_object());
    assert!(schema["properties"]["vim_mode_apps"]["items"].is_object());
    assert!(!root.path().join("state").exists());
}

#[test]
fn invalid_shapes_and_large_input_do_not_write() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("patch.json");
    for data in [
        "[]".to_owned(),
        "{\"candidate_page_size\":100}".to_owned(),
        "{\"scheme\":\"synthetic-unknown-scheme\"}".to_owned(),
        "{".to_owned(),
        " ".repeat(1024 * 1024 + 1),
    ] {
        std::fs::write(&file, data).unwrap();
        let output = run(root.path(), &["config", "validate", file.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(!root.path().join("state").exists());
    }
}

#[test]
fn a_broken_existing_document_is_reported_without_recovery_or_lock_creation() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let preferences = state.join("preferences.json");
    let broken = b"{synthetic-truncated";
    std::fs::write(&preferences, broken).unwrap();
    let patch = root.path().join("patch.json");
    std::fs::write(&patch, br#"{"candidate_page_size":7}"#).unwrap();
    let output = run(
        root.path(),
        &["config", "validate", patch.to_str().unwrap()],
    );
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&preferences).unwrap(), broken);
    assert_eq!(std::fs::read_dir(state).unwrap().count(), 1);
}
