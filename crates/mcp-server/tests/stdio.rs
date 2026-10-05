//! The server as an agent sees it: the built binary spoken to over stdio by the SDK's own client, against a synthetic dictionary and state directory.

use rmcp::model::{CallToolRequestParams, CallToolResult, GetPromptRequestParams};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};

/// A data root the Engine opens: the smallest dictionary it accepts, as the host-api tests build it, with one bundled wubi word, and a runtime-options document pointing at it.
fn fixture(root: &Path) -> std::path::PathBuf {
    let tables = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                  CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                  CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                  CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                  CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
                  INSERT INTO wubi86 VALUES('aaaa','合成工',500);";
    for name in ["resources", "dictionaries"] {
        let path = root.join(name);
        std::fs::create_dir(&path).unwrap();
        rusqlite::Connection::open(path.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(tables)
            .unwrap();
    }
    std::fs::create_dir(root.join("user")).unwrap();
    let text = root.to_str().unwrap();
    let options = root.join("runtime-options.json");
    let document = json!({
        "api_version": 1,
        "resources": format!("{text}/resources"),
        "user_data": format!("{text}/user"),
        "cache": format!("{text}/cache"),
        "dictionaries": format!("{text}/dictionaries"),
        "preferences": msime_client_core::preferences::Preferences::default(),
        "preferences_directory": text,
    });
    std::fs::write(&options, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    options
}

async fn start(options: &Path, flags: &[&str]) -> (RunningService<RoleClient, ()>, Child) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_msime-mcp"));
    command.arg("--options").arg(options).args(flags);
    // The test machine's own input method must not leak in.
    for name in [
        "MSIME_CLIENT_HOST_OPTIONS",
        "MSIME_IBUS_OPTIONS",
        "MSIME_CLIENT_STATE_DIR",
    ] {
        command.env_remove(name);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let transport = (child.stdout.take().unwrap(), child.stdin.take().unwrap());
    (().serve(transport).await.unwrap(), child)
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    arguments: Value,
) -> CallToolResult {
    let Value::Object(arguments) = arguments else {
        panic!("arguments must be an object");
    };
    client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments))
        .await
        .unwrap()
}

/// The structured result of a call that succeeded.
async fn ok(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    arguments: Value,
) -> Value {
    let result = call(client, name, arguments).await;
    assert_ne!(
        result.is_error,
        Some(true),
        "{name} failed: {:?}",
        result.content
    );
    result.structured_content.expect("a structured result")
}

/// The text of a call that the server refused.
async fn refused(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    arguments: Value,
) -> String {
    let result = call(client, name, arguments).await;
    assert_eq!(result.is_error, Some(true), "{name} should have failed");
    serde_json::to_string(&result.content).unwrap()
}

/// Writes are spaced a second apart by the server.
async fn after_write_interval() {
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

async fn tool_names(client: &RunningService<RoleClient, ()>) -> Vec<String> {
    let mut names: Vec<String> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn read_only_by_default() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let (client, _child) = start(&options, &[]).await;

    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, "msime");
    assert_eq!(
        tool_names(&client).await,
        [
            "get_preferences",
            "get_typing_statistics",
            "list_candidate_skins",
            "list_quick_phrases",
            "read_diagnostic_log",
            "set_diagnostic_log"
        ]
    );
    let page = ok(&client, "list_quick_phrases", json!({})).await;
    assert_eq!(page, json!({ "phrases": [], "has_more": false }));
    // A write tool that is not offered cannot be called either.
    assert!(client
        .call_tool(
            CallToolRequestParams::new("edit_quick_phrases")
                .with_arguments(json!({ "edits": [] }).as_object().unwrap().clone())
        )
        .await
        .is_err());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_agent_manages_quick_phrases_and_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let (client, _child) = start(&options, &["--allow-write"]).await;
    assert_eq!(tool_names(&client).await.len(), 9);

    // Quick phrases: add, list, replace, remove, with a failure in the middle of a batch.
    let outcome = ok(
        &client,
        "edit_quick_phrases",
        json!({ "edits": [
            { "op": "add", "code": "yx", "text": "someone@example.com" },
            { "op": "add", "code": "dz", "text": "合成地址" },
        ]}),
    )
    .await;
    assert_eq!(outcome, json!({ "applied": 2 }));
    let page = ok(&client, "list_quick_phrases", json!({ "code_prefix": "y" })).await;
    assert_eq!(
        page,
        json!({ "phrases": [{ "code": "yx", "text": "someone@example.com" }], "has_more": false })
    );
    // Too soon after the last write.
    assert!(refused(
        &client,
        "edit_quick_phrases",
        json!({ "edits": [{ "op": "remove", "code": "dz", "text": "合成地址" }] })
    )
    .await
    .contains("one a second"));

    after_write_interval().await;
    let outcome = ok(
        &client,
        "edit_quick_phrases",
        json!({ "edits": [
            { "op": "replace", "previous": { "code": "yx", "text": "someone@example.com" }, "replacement": { "code": "yx", "text": "other@example.com" } },
            { "op": "remove", "code": "zz", "text": "不存在" },
            { "op": "remove", "code": "dz", "text": "合成地址" },
        ]}),
    )
    .await;
    assert_eq!(outcome["applied"], 1);
    assert_eq!(outcome["failed_index"], 1);
    assert!(outcome["error"].as_str().unwrap().contains("not found"));
    let page = ok(&client, "list_quick_phrases", json!({})).await;
    assert_eq!(page["phrases"].as_array().unwrap().len(), 2);
    assert!(page["phrases"]
        .as_array()
        .unwrap()
        .contains(&json!({ "code": "yx", "text": "other@example.com" })));

    // Preferences: read, change with the revision, refuse a stale one.
    let before = ok(&client, "get_preferences", json!({})).await;
    let revision = before["revision"].as_u64().unwrap();
    after_write_interval().await;
    let after = ok(
        &client,
        "update_preferences",
        json!({ "expected_revision": revision, "candidate_page_size": 7, "scheme": "wubi", "character_width": "fullwidth" }),
    )
    .await;
    assert_eq!(after["candidate_page_size"], 7);
    assert_eq!(after["scheme"], "wubi");
    assert_eq!(after["character_width"], "fullwidth");
    assert_eq!(after["revision"], revision + 1);
    assert_eq!(ok(&client, "get_preferences", json!({})).await, after);
    after_write_interval().await;
    assert!(refused(
        &client,
        "update_preferences",
        json!({ "expected_revision": revision, "candidate_page_size": 5 })
    )
    .await
    .contains("changed since"));
    // Only the allowlist can be named; the SDK reports anything else as a failed call the agent can read and correct.
    assert!(refused(
        &client,
        "update_preferences",
        json!({ "expected_revision": revision + 1, "learning": false })
    )
    .await
    .contains("unknown field `learning`"));
    assert_eq!(ok(&client, "get_preferences", json!({})).await, after);
    assert_eq!(ok(&client, "get_preferences", json!({})).await, after);

    // Skins: install one from a manifest and a base64 preview, then see it listed; an existing one is replaced only when asked.
    let skin = json!({
        "package_id": "sunset",
        "manifest": "schema_version = 1\nid = 'sunset'\nname = 'Sunset'\nversion = '1.0'\nbase = 'night'\npreview = 'preview.png'\n[supports]\nlayouts = ['vertical']\nthemes = ['dark']\n[candidate_window]\nmin_width_dip = 200\n",
        "images": { "preview.png": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==" }
    });
    after_write_interval().await;
    assert_eq!(
        ok(&client, "create_candidate_skin", skin.clone()).await,
        json!({ "id": "sunset", "replaced": false })
    );
    let listed = ok(&client, "list_candidate_skins", json!({})).await;
    assert_eq!(listed["skins"][0]["id"], "sunset");
    assert_eq!(listed["skins"][0]["synced"], false);
    after_write_interval().await;
    assert!(refused(&client, "create_candidate_skin", skin.clone())
        .await
        .contains("replace: true"));
    let mut replacement = skin;
    replacement["replace"] = json!(true);
    after_write_interval().await;
    assert_eq!(
        ok(&client, "create_candidate_skin", replacement).await,
        json!({ "id": "sunset", "replaced": true })
    );

    // Statistics: nothing recorded yet, and off until the user turns them on.
    let statistics = ok(&client, "get_typing_statistics", json!({ "days": 3 })).await;
    assert_eq!(statistics["enabled"], false);
    assert_eq!(statistics["days"], json!([]));
    assert_eq!(statistics["active_seconds"], 0);
    assert!(statistics.get("hours").is_none());
    assert!(
        !refused(&client, "get_typing_statistics", json!({ "days": 0 }))
            .await
            .is_empty()
    );

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_agent_imports_reweighs_and_explains_dictionary_words() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let (client, _child) = start(&options, &["--allow-dictionary-read"]).await;
    // Reading alone offers no way to change a word.
    assert!(!tool_names(&client)
        .await
        .iter()
        .any(|name| name.starts_with("edit_") || name.starts_with("import_")));
    client.cancel().await.unwrap();

    let (client, _child) = start(&options, &["--allow-write", "--allow-dictionary-read"]).await;
    assert_eq!(tool_names(&client).await.len(), 13);

    let outcome = ok(
        &client,
        "import_dictionary_words",
        json!({ "dictionary": "pinyin", "words": [
            { "code": "hecheng", "word": "合成" },
            { "code": "has space!", "word": "合成" },
        ]}),
    )
    .await;
    assert_eq!(outcome["added"], 1);
    assert_eq!(outcome["existing"], 0);
    assert_eq!(outcome["rejected"].as_array().unwrap().len(), 1);
    assert_eq!(outcome["rejected"][0]["index"], 1);
    let page = ok(
        &client,
        "list_dictionary_words",
        json!({ "dictionary": "pinyin" }),
    )
    .await;
    let words = page["words"].as_array().unwrap();
    assert_eq!(words.len(), 1);
    assert_eq!(words[0]["word"], "合成");
    assert_eq!(words[0]["bundled"], false);
    let stored_code = words[0]["code"].clone();
    // The bundled words need a code to look under.
    assert!(refused(
        &client,
        "list_dictionary_words",
        json!({ "dictionary": "wubi", "include_bundled": true })
    )
    .await
    .contains("needs a code_prefix"));

    // Sending the same words again adds nothing.
    after_write_interval().await;
    let replay = ok(
        &client,
        "import_dictionary_words",
        json!({ "dictionary": "pinyin", "words": [{ "code": "hecheng", "word": "合成" }] }),
    )
    .await;
    assert_eq!(
        (replay["added"].clone(), replay["existing"].clone()),
        (json!(0), json!(1))
    );

    after_write_interval().await;
    let outcome = ok(
        &client,
        "edit_dictionary_words",
        json!({ "edits": [
            { "op": "set_weight", "dictionary": "wubi", "code": "aaaa", "word": "合成工", "weight": 900 },
            { "op": "add", "dictionary": "wubi", "code": "aaaa", "word": "合成字", "weight": 100 },
            { "op": "remove", "dictionary": "pinyin", "code": stored_code, "word": "合成" },
            { "op": "remove", "dictionary": "wubi", "code": "aaaa", "word": "不存在" },
        ]}),
    )
    .await;
    assert_eq!(outcome["applied"], 3);
    assert_eq!(outcome["failed_index"], 3);
    assert!(outcome["error"].as_str().unwrap().contains("not found"));
    let page = ok(
        &client,
        "list_dictionary_words",
        json!({ "dictionary": "wubi", "code_prefix": "aaaa", "include_bundled": true }),
    )
    .await;
    assert_eq!(
        page["words"],
        json!([
            { "code": "aaaa", "word": "合成字", "weight": 100, "bundled": false },
            { "code": "aaaa", "word": "合成工", "weight": 900, "bundled": true },
        ])
    );
    let pinyin = ok(
        &client,
        "list_dictionary_words",
        json!({ "dictionary": "pinyin" }),
    )
    .await;
    assert_eq!(pinyin["words"], json!([]));

    // The ranking explained: the reweighted bundled word leads the user's own.
    let lookup = ok(
        &client,
        "lookup_candidates",
        json!({ "code": "aaaa", "scheme": "wubi", "limit": 2 }),
    )
    .await;
    assert_eq!(
        lookup["candidates"],
        json!([
            { "text": "合成工", "code": "aaaa", "origin": "dictionary", "weight": 900 },
            { "text": "合成字", "code": "aaaa", "origin": "user_word", "weight": 100 },
        ])
    );
    assert!(
        !refused(&client, "lookup_candidates", json!({ "code": "AAAA" }))
            .await
            .is_empty()
    );

    client.cancel().await.unwrap();
}

async fn prompt_names(client: &RunningService<RoleClient, ()>) -> Vec<String> {
    let mut names: Vec<String> = client
        .list_all_prompts()
        .await
        .unwrap()
        .into_iter()
        .map(|prompt| prompt.name)
        .collect();
    names.sort();
    names
}

/// The text of the single message a prompt expands to.
async fn prompt_text(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> String {
    let result = client
        .get_prompt(
            GetPromptRequestParams::new(name)
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();
    let messages = result["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    messages[0]["content"]["text"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn prompts_follow_the_tools_they_need() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());

    // Without --allow-write there is no create_candidate_skin, so no make_skin either.
    let (client, _child) = start(&options, &[]).await;
    assert_eq!(prompt_names(&client).await, ["diagnose"]);
    let text = prompt_text(&client, "diagnose", json!({ "problem": "候选窗不见了" })).await;
    assert!(text.contains("The user reports: 候选窗不见了"));
    assert!(text.contains("read_diagnostic_log"));
    // A skipped argument arrives as nothing or as an empty string; either way the assistant asks.
    for arguments in [json!({}), json!({ "problem": "  " })] {
        assert!(prompt_text(&client, "diagnose", arguments)
            .await
            .contains("Ask the user"));
    }
    assert!(client
        .get_prompt(GetPromptRequestParams::new("make_skin"))
        .await
        .is_err());
    client.cancel().await.unwrap();

    let (client, _child) = start(&options, &["--allow-write"]).await;
    assert_eq!(prompt_names(&client).await, ["diagnose", "make_skin"]);
    let text = prompt_text(&client, "make_skin", json!({ "style": "dark teal, calm" })).await;
    assert!(text.contains("The user wants this skin: dark teal, calm"));
    assert!(text.contains("create_candidate_skin"));
    assert!(prompt_text(&client, "make_skin", json!({}))
        .await
        .contains("Ask the user"));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn an_agent_turns_on_and_reads_the_diagnostic_log() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    // No flags: the settings page registers the server this way, and someone who cannot edit a configuration file must still get from a description of the problem to the log.
    let (client, _child) = start(&options, &[]).await;

    // Off by default: nothing to read, and the answer says how to get something.
    let view = ok(&client, "read_diagnostic_log", json!({})).await;
    assert_eq!(view["server_enabled"], false);
    assert_eq!(view["lines"], json!([]));
    // The local time to measure "a few minutes ago" from, in the log's own form.
    let now = view["now"].as_str().unwrap();
    assert_eq!(now.len(), 19);
    assert!(now.starts_with("20") && now.as_bytes()[10] == b' ');
    assert!(view["hint"]
        .as_str()
        .unwrap()
        .contains("set_diagnostic_log"));

    let switched = ok(&client, "set_diagnostic_log", json!({ "enabled": true })).await;
    assert_eq!(switched["server_enabled"], true);
    assert_eq!(switched["tsf_enabled"], cfg!(windows));
    assert_eq!(
        ok(&client, "get_preferences", json!({})).await["diagnostic_log_server"],
        true
    );
    let view = ok(&client, "read_diagnostic_log", json!({})).await;
    assert!(view["hint"]
        .as_str()
        .unwrap()
        .contains("nothing has been written"));

    // What a host writes once the user reproduces the problem, in the place this platform's host writes it.
    let file = if cfg!(windows) {
        directory.path().join("logs").join("server.log")
    } else {
        directory.path().join("diagnostic.log")
    };
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        "2026-09-25 09:00:00 [p1] focus_in\n\
         2026-09-25 10:00:00 [p1] candidate_window_slow stage=layout elapsed_ms=180\n\
         2026-09-25 10:00:01 [p1:t2] [msime][issue47] seq=1 stage=key-down vk=0x41 wch=U+0061 key=a key_class=letter\n\
         2026-09-25 10:00:02 [p1] focus_out\n",
    )
    .unwrap();

    let view = ok(
        &client,
        "read_diagnostic_log",
        json!({ "since": "2026-09-25 10:00", "lines": 2 }),
    )
    .await;
    assert_eq!(view["server_enabled"], true);
    assert!(view.get("hint").is_none());
    assert_eq!(view["has_more"], true);
    assert_eq!(
        view["lines"],
        json!([
            "2026-09-25 10:00:01 [p1:t2] [msime][issue47] seq=1 stage=key-down vk=- wch=- key=- key_class=letter",
            "2026-09-25 10:00:02 [p1] focus_out"
        ])
    );
    let view = ok(
        &client,
        "read_diagnostic_log",
        json!({ "contains": "SLOW" }),
    )
    .await;
    assert_eq!(view["lines"].as_array().unwrap().len(), 1);
    assert!(!refused(
        &client,
        "read_diagnostic_log",
        json!({ "since": "earlier" })
    )
    .await
    .is_empty());

    // Done: the log goes off again, and what was written stays readable.
    after_write_interval().await;
    let switched = ok(&client, "set_diagnostic_log", json!({ "enabled": false })).await;
    assert_eq!(
        switched,
        json!({ "server_enabled": false, "tsf_enabled": false })
    );
    let view = ok(&client, "read_diagnostic_log", json!({})).await;
    assert_eq!(view["lines"].as_array().unwrap().len(), 4);

    client.cancel().await.unwrap();
}

/// The command line: one tool per run, under the same flags and with the same answers as the server.
fn run_cli(options: &Path, args: &[&str], stdin: Option<&str>) -> (i32, Value, String) {
    let (code, stdout, stderr) = run_cli_text(options, args, stdin);
    let stdout = if stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&stdout).unwrap()
    };
    (code, stdout, stderr)
}

fn run_cli_text(options: &Path, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    use std::io::Write;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_msime-mcp"));
    command.arg("--options").arg(options).args(args);
    for name in [
        "MSIME_CLIENT_HOST_OPTIONS",
        "MSIME_IBUS_OPTIONS",
        "MSIME_CLIENT_STATE_DIR",
    ] {
        command.env_remove(name);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.unwrap_or("").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn the_command_line_runs_the_same_tools() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());

    let (code, tools, _) = run_cli(&options, &["tools"], None);
    assert_eq!(code, 0);
    let mut names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "get_preferences",
            "get_typing_statistics",
            "list_candidate_skins",
            "list_quick_phrases",
            "read_diagnostic_log",
            "set_diagnostic_log"
        ]
    );
    assert!(tools[0]["inputSchema"].is_object());

    let edit = r#"{"edits":[{"op":"add","code":"yx","text":"someone@example.com"}]}"#;
    let (code, _, error) = run_cli(&options, &["call", "edit_quick_phrases", edit], None);
    assert_eq!(code, 1);
    assert!(error.contains("--allow-write"), "{error}");

    let (code, outcome, _) = run_cli(
        &options,
        &["--allow-write", "call", "edit-quick-phrases", "-"],
        Some(edit),
    );
    assert_eq!(code, 0);
    assert_eq!(outcome, json!({ "applied": 1 }));

    let (code, page, _) = run_cli(
        &options,
        &["call", "list_quick_phrases", r#"{"code_prefix":"y"}"#],
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(
        page,
        json!({ "phrases": [{ "code": "yx", "text": "someone@example.com" }], "has_more": false })
    );

    let (code, _, error) = run_cli(
        &options,
        &["call", "list_quick_phrases", r#"{"limit":0}"#],
        None,
    );
    assert_eq!(code, 1);
    assert!(
        error.contains("limit must be between 1 and 1000"),
        "{error}"
    );

    let (code, _, error) = run_cli(&options, &["call", "list_quick_phrases", "[]"], None);
    assert_eq!(code, 2);
    assert!(error.contains("JSON object"), "{error}");
}

#[test]
fn expand_and_config_print_lines_for_testing_by_hand() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());

    // expand 不需要另加 --allow-dictionary-read；方案默认是用户当前的（这里是全拼），五笔词要指定 --scheme。
    let (code, lines, error) =
        run_cli_text(&options, &["expand", "aaaa", "--scheme", "wubi"], None);
    assert_eq!(code, 0, "{error}");
    assert_eq!(lines, "1\t合成工\taaaa\tdictionary\t500\n");
    let (code, view, error) = run_cli(
        &options,
        &["expand", "aaaa", "--scheme", "wubi", "--json"],
        None,
    );
    assert_eq!(code, 0, "{error}");
    assert_eq!(view["candidates"][0]["text"], "合成工");
    let (code, _, error) = run_cli_text(&options, &["expand", "AAAA"], None);
    assert_eq!(code, 1);
    assert!(error.contains("lowercase"), "{error}");

    let (code, before, error) = run_cli_text(&options, &["config"], None);
    assert_eq!(code, 0, "{error}");
    assert!(
        before.lines().any(|line| line == "scheme = quanpin"),
        "{before}"
    );

    // config set 自己读 revision，并且不需要另加 --allow-write。
    let (code, after, error) = run_cli_text(
        &options,
        &["config", "set", "scheme=wubi", "candidate_page_size=9"],
        None,
    );
    assert_eq!(code, 0, "{error}");
    assert!(after.lines().any(|line| line == "scheme = wubi"), "{after}");
    assert!(
        after.lines().any(|line| line == "candidate_page_size = 9"),
        "{after}"
    );
    // 下一次运行读到的就是新的偏好：expand 不指定方案也按五笔查。
    let (code, lines, error) = run_cli_text(&options, &["expand", "aaaa"], None);
    assert_eq!(code, 0, "{error}");
    assert_eq!(lines, "1\t合成工\taaaa\tdictionary\t500\n");

    let (code, _, error) = run_cli_text(&options, &["config", "set", "no_such_key=1"], None);
    assert_eq!(code, 1);
    assert!(error.contains("no_such_key"), "{error}");
}

#[test]
fn writes_from_separate_runs_are_spaced_out_too() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let add = |text: &str| format!(r#"{{"edits":[{{"op":"add","code":"yx","text":"{text}"}}]}}"#);
    let (code, _, error) = run_cli(
        &options,
        &[
            "--allow-write",
            "call",
            "edit_quick_phrases",
            &add("one@example.com"),
        ],
        None,
    );
    assert_eq!(code, 0, "{error}");
    let (code, _, error) = run_cli(
        &options,
        &[
            "--allow-write",
            "call",
            "edit_quick_phrases",
            &add("two@example.com"),
        ],
        None,
    );
    assert_eq!(code, 1);
    assert!(error.contains("one a second"), "{error}");
    std::thread::sleep(Duration::from_millis(1100));
    let (code, _, error) = run_cli(
        &options,
        &[
            "--allow-write",
            "call",
            "edit_quick_phrases",
            &add("two@example.com"),
        ],
        None,
    );
    assert_eq!(code, 0, "{error}");
}

#[test]
fn the_command_line_prints_the_prompts() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let (code, prompts, _) = run_cli(&options, &["prompts"], None);
    assert_eq!(code, 0);
    let names: Vec<&str> = prompts
        .as_array()
        .unwrap()
        .iter()
        .map(|prompt| prompt["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["diagnose"]);

    let (code, text, _) = run_cli_text(
        &options,
        &["prompt", "diagnose", r#"{"problem":"候选窗不见了"}"#],
        None,
    );
    assert_eq!(code, 0);
    assert!(text.contains("候选窗不见了"), "{text}");
    assert!(text.contains("read_diagnostic_log"), "{text}");
    assert!(text.contains("msime-mcp <the same flags> call"), "{text}");

    let (code, _, error) = run_cli_text(&options, &["prompt", "make-skin"], None);
    assert_eq!(code, 1);
    assert!(error.contains("--allow-write"), "{error}");
    let (code, text, _) = run_cli_text(&options, &["--allow-write", "prompt", "make-skin"], None);
    assert_eq!(code, 0);
    assert!(text.contains("create_candidate_skin"), "{text}");
}

#[test]
fn arguments_can_come_from_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let file = directory.path().join("edit.json");
    std::fs::write(
        &file,
        r#"{"edits":[{"op":"add","code":"dz","text":"北京市海淀区"}]}"#,
    )
    .unwrap();
    let argument = format!("@{}", file.display());
    let (code, _, error) = run_cli(
        &options,
        &["--allow-write", "call", "edit_quick_phrases", &argument],
        None,
    );
    assert_eq!(code, 0, "{error}");
    let (_, page, _) = run_cli(&options, &["call", "list_quick_phrases"], None);
    assert_eq!(page["phrases"][0]["text"], "北京市海淀区");
    let (code, _, error) = run_cli(
        &options,
        &["call", "list_quick_phrases", "@/nonexistent/args.json"],
        None,
    );
    assert_eq!(code, 2);
    assert!(error.contains("cannot read the arguments"), "{error}");
}
