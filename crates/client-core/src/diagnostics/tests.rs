use super::*;
use crate::preferences::{Preferences, PreferencesSnapshot};
use std::fs::File;

fn request(root: &Path, include: DiagnosticInclude) -> DiagnosticBundleRequest {
    DiagnosticBundleRequest {
        state_root: root.to_string_lossy().into_owned(),
        include,
        sources: DiagnosticSources::default(),
        destination: None,
    }
}

fn save_preferences(root: &Path, preferences: Preferences) {
    let snapshot = PreferencesSnapshot {
        format_version: 1,
        revision: 0,
        preferences,
    };
    std::fs::write(
        root.join("preferences.json"),
        serde_json::to_vec(&snapshot).unwrap(),
    )
    .unwrap();
}

#[test]
fn input_events_drop_every_line_that_carries_text_or_unknown_kinds() {
    let directory = tempfile::tempdir().unwrap();
    let events = directory.path().join("input-events.ndjson");
    std::fs::write(
        &events,
        concat!(
            "{\"t_ms\":1,\"kind\":\"key_down\"}\n",
            "{\"t_ms\":2,\"kind\":\"commit\",\"duration_ms\":3}\n",
            "{\"t_ms\":3,\"kind\":\"commit\",\"text\":\"你好\"}\n",
            "{\"t_ms\":4,\"kind\":\"typed_letter\"}\n",
            "{\"t_ms\":\"5\",\"kind\":\"key_up\"}\n",
            "not json\n",
            "[1,2]\n",
            "\n",
            "{\"t_ms\":6,\"kind\":\"backspace\",\"key\":\"a\"}\n",
        ),
    )
    .unwrap();
    let mut request = request(
        directory.path(),
        DiagnosticInclude {
            input_events: true,
            ..DiagnosticInclude::default()
        },
    );
    request.sources.input_events = Some(events.to_string_lossy().into_owned());

    let bundle = build_bundle(&request).unwrap();

    let count = bundle.counts.input_events.unwrap();
    assert_eq!(count.kept, 2);
    assert_eq!(count.dropped, 6);
    let sections = bundle.sections.unwrap();
    let text = serde_json::to_string(&sections).unwrap();
    assert!(!text.contains("你好"));
    assert!(!text.contains("\"key\""));
    assert_eq!(
        sections["input_events"],
        serde_json::json!([
            {"t_ms": 1, "kind": "key_down"},
            {"t_ms": 2, "kind": "commit", "duration_ms": 3},
        ])
    );
}

#[test]
fn performance_records_need_exactly_the_three_whitelisted_keys() {
    assert!(performance_record(br#"{"t_ms":1,"kind":"commit","duration_ms":4}"#).is_some());
    assert!(performance_record(br#"{"t_ms":1,"kind":"commit"}"#).is_none());
    assert!(
        performance_record(br#"{"t_ms":1,"kind":"commit","duration_ms":4,"text":"a"}"#).is_none()
    );
    assert!(crash_record(br#"{"at":"2026-10-05T00:00:00Z","message":"m","stack":"s"}"#).is_some());
    assert!(crash_record(br#"{"at":"x","message":"m","stack":"s","extra":1}"#).is_none());
}

// 异常说明可能带着正在输入的文字，崩溃记录进诊断包时只留异常类型和栈帧，路径只留文件名。
#[test]
fn crash_records_keep_the_exception_type_and_frames_but_not_the_reason() {
    let line = serde_json::json!({
        "at": "2026-10-05T00:00:00Z",
        "message": "java.lang.IllegalStateException: 合成文字 typed text",
        "stack": "java.lang.IllegalStateException: 合成文字 typed text\n\tat app.msime.android.Foo.bar(Foo.java:12)\nCaused by: org.json.JSONException: Unterminated string at 合成\n\t... 3 more\n#00 pc 0001 /data/app/abc/lib/arm64/libmsime_host_api.so (crash+4)",
    })
    .to_string();
    let record = crash_record(line.as_bytes()).unwrap();
    assert_eq!(record["message"], "java.lang.IllegalStateException");
    let stack = record["stack"].as_str().unwrap();
    assert!(
        !stack.contains("合成") && !stack.contains("typed text"),
        "{stack}"
    );
    assert!(
        stack.contains("at app.msime.android.Foo.bar(Foo.java:12)"),
        "{stack}"
    );
    assert!(
        stack.contains("Caused by: org.json.JSONException"),
        "{stack}"
    );
    assert!(
        stack.contains("libmsime_host_api.so (crash+4)") && !stack.contains("/data/app"),
        "{stack}"
    );
}

#[test]
fn a_missing_source_is_an_empty_section() {
    let directory = tempfile::tempdir().unwrap();
    let mut request = request(
        directory.path(),
        DiagnosticInclude {
            crash_logs: true,
            performance_logs: true,
            ..DiagnosticInclude::default()
        },
    );
    request.sources.crash_logs = Some(
        directory
            .path()
            .join("missing.ndjson")
            .to_string_lossy()
            .into_owned(),
    );
    let bundle = build_bundle(&request).unwrap();
    assert_eq!(bundle.counts.crash_logs.unwrap().kept, 0);
    assert_eq!(bundle.counts.performance_logs.unwrap().kept, 0);
    assert!(bundle.counts.input_events.is_none());
}

#[test]
fn the_configuration_snapshot_is_redacted_inside_the_zip() {
    let directory = tempfile::tempdir().unwrap();
    let mut preferences = Preferences::default();
    preferences.voice_input.asr_token = "voice-secret".into();
    preferences.ai_assistant.token = "ai-secret".into();
    preferences.tencent_tmt.secret_key = "tencent-secret".into();
    save_preferences(directory.path(), preferences);
    let destination = directory.path().join("bundle.zip");
    let mut request = request(
        directory.path(),
        DiagnosticInclude {
            config_snapshot: true,
            input_events: true,
            ..DiagnosticInclude::default()
        },
    );
    request.destination = Some(destination.to_string_lossy().into_owned());

    let bundle = build_bundle(&request).unwrap();

    assert!(bundle.counts.config_snapshot);
    assert!(bundle.bytes.unwrap() > 0);
    let mut archive = zip::ZipArchive::new(File::open(&destination).unwrap()).unwrap();
    let mut names: Vec<_> = archive.file_names().map(str::to_owned).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "config_snapshot.json",
            "input_events.ndjson",
            "manifest.json"
        ]
    );
    let mut config = String::new();
    archive
        .by_name("config_snapshot.json")
        .unwrap()
        .read_to_string(&mut config)
        .unwrap();
    assert!(!config.contains("secret\""));
    assert!(!config.contains("voice-secret"));
    assert!(!config.contains("ai-secret"));
    assert!(!config.contains("tencent-secret"));
    assert!(config.contains(crate::preferences::REDACTED));
}

#[test]
fn relative_paths_are_refused() {
    let mut request = request(Path::new("relative"), DiagnosticInclude::default());
    assert_eq!(
        build_bundle(&request).unwrap_err().code(),
        "diagnostics_invalid"
    );
    let directory = tempfile::tempdir().unwrap();
    request.state_root = directory.path().to_string_lossy().into_owned();
    request.destination = Some("bundle.zip".into());
    assert_eq!(
        build_bundle(&request).unwrap_err().code(),
        "diagnostics_invalid"
    );
}

#[test]
fn only_the_most_recent_lines_are_kept() {
    let directory = tempfile::tempdir().unwrap();
    let events = directory.path().join("events.ndjson");
    let mut text = String::new();
    for index in 0..(MAX_INPUT_EVENTS + 3) {
        text.push_str(&format!("{{\"t_ms\":{index},\"kind\":\"key_up\"}}\n"));
    }
    std::fs::write(&events, text).unwrap();
    let mut request = request(
        directory.path(),
        DiagnosticInclude {
            input_events: true,
            ..DiagnosticInclude::default()
        },
    );
    request.sources.input_events = Some(events.to_string_lossy().into_owned());
    let bundle = build_bundle(&request).unwrap();
    let count = bundle.counts.input_events.unwrap();
    assert_eq!(count.kept, MAX_INPUT_EVENTS);
    assert_eq!(count.truncated, 3);
    assert_eq!(bundle.sections.unwrap()["input_events"][0]["t_ms"], 3);
}
