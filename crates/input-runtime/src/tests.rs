//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::runtime::empty_result;
use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
#[derive(Default)]
struct Fixture {
    scheme: u8,
    dedicated_english: bool,
    nine_key: bool,
    nine_key_spellings: Vec<String>,
    local_mode: String,
    words: Vec<String>,
    codes: Vec<String>,
    text: String,
    snapshot_fails: bool,
    balanced_openings: Vec<u8>,
    cache_resets: usize,
    context_resets: usize,
    /// Candidates this engine holds back until asked, standing in for the Engine's cap on a
    /// single-letter query. Empty means an engine that already returns everything it has.
    withheld: Vec<String>,
    /// Where each candidate came from, parallel to `words`. Empty means an engine answering from
    /// the local dictionary alone, which is what most of these tests are about.
    sources: Vec<u8>,
    /// What is left to compose after a candidate is picked, standing in for an Engine that answered
    /// with a candidate covering only part of the input. `None` is an engine that finishes.
    remaining_after_select: Option<String>,
    /// The seat each candidate is fixed to, parallel to `words`: 1-based, 0 for unfixed. Empty means nothing is fixed.
    positions: Vec<u8>,
    /// What the Engine reports as the reading, which the Japanese scheme sets to the converted kana. Empty means no reading.
    reading: String,
    /// What the Engine reports it takes as characters rather than punctuation.
    spelling_symbols: String,
}

#[cfg(unix)]
fn private_tempdir() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

#[cfg(unix)]
#[test]
fn provider_connect_rejects_untrusted_filesystem_endpoints() {
    let root = private_tempdir();
    let socket = root.path().join("provider.sock");
    std::fs::write(&socket, b"synthetic").unwrap();
    assert!(UnixSocketProvider::new(&socket).connect().is_none());
    std::fs::remove_file(&socket).unwrap();

    let target = root.path().join("target.sock");
    let listener = UnixListener::bind(&target).unwrap();
    let alias = root.path().join("alias.sock");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    assert!(UnixSocketProvider::new(&alias).connect().is_none());
    drop(listener);

    let outside = root.path().join("outside");
    let outside_nested = outside.join("nested");
    std::fs::create_dir_all(&outside_nested).unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&outside_nested, std::fs::Permissions::from_mode(0o700)).unwrap();
    let outside_socket = outside_nested.join("provider.sock");
    let listener = UnixListener::bind(&outside_socket).unwrap();
    let inside = root.path().join("inside");
    std::fs::create_dir(&inside).unwrap();
    std::os::unix::fs::symlink(&outside, inside.join("linked")).unwrap();
    assert!(
        UnixSocketProvider::new(inside.join("linked/nested/provider.sock"))
            .connect()
            .is_none()
    );
    drop(listener);

    let socket = root.path().join("private.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(UnixSocketProvider::new(&socket).connect().is_none());
    drop(listener);
}

#[cfg(unix)]
#[test]
fn cloud_dictionary_provider_forwards_bounded_request() {
    let directory = private_tempdir();
    let socket = directory.path().join("cloud-dictionary.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["version"], 1);
        assert_eq!(request["kind"], "cloud_dictionary");
        assert_eq!(request["request"]["operation"], "changes");
        let mut stream = stream;
        std::io::Write::write_all(&mut stream, br#"{"changes":[],"next":0}"#).unwrap();
        std::io::Write::write_all(&mut stream, b"\n").unwrap();
    });
    let request = json!({"operation":"changes","after":0,"limit":1});
    let response = UnixSocketProvider::new(socket)
        .cloud_dictionary(request)
        .unwrap();
    assert_eq!(response["next"], 0);
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn credential_test_provider_keeps_request_and_response_bounded() {
    let directory = private_tempdir();
    let socket = directory.path().join("online.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["version"], 1);
        assert_eq!(request["kind"], "credential_test");
        assert_eq!(request["query"]["service"], "ai.assistant");
        assert_eq!(request["query"]["config"]["provider"], "deepseek");
        let mut stream = stream;
        std::io::Write::write_all(
            &mut stream,
            br#"{"ok":true,"message":"configuration accepted"}"#,
        )
        .unwrap();
        std::io::Write::write_all(&mut stream, b"\n").unwrap();
    });
    let response = UnixSocketProvider::new(socket)
        .test_credential("ai.assistant", &json!({"provider":"deepseek"}))
        .unwrap();
    assert!(response.ok);
    assert_eq!(response.message, "configuration accepted");
    server.join().unwrap();

    assert!(
        UnixSocketProvider::new(directory.path().join("missing.sock"))
            .test_credential("unknown", &json!({}))
            .is_none()
    );
    assert!(
        UnixSocketProvider::new(directory.path().join("missing.sock"))
            .test_credential("voice.asr", &json!({"value":"x".repeat(16_384)}))
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn online_provider_forwards_the_ai_cache_probe_flag() {
    let mut query: OnlineQuery = serde_json::from_value(json!({
        "scheme": 0, "generation": 3, "identity": "identity", "query_text": "nihao",
        "cache_key": "cache", "pinyin_segments": ["ni", "hao"], "cloud_eligible": true,
        "ai_eligible": true, "cloud_candidates": false, "session_id": 5,
        "ai_assistant": {"enabled": true, "provider": "synthetic", "model": "synthetic-model",
                         "endpoint": "https://ai.invalid/v1/chat/completions"},
    }))
    .unwrap();
    // Absent in every document a host already produces, so only the Linux probe carries it.
    assert!(!query.ai_cache_only);
    assert!(serde_json::to_value(&query)
        .unwrap()
        .get("ai_cache_only")
        .is_none());
    query.ai_cache_only = true;

    let directory = private_tempdir();
    let socket = directory.path().join("online.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["kind"], "online");
        assert_eq!(request["query"]["ai_cache_only"], true);
        let mut stream = stream;
        std::io::Write::write_all(
            &mut stream,
            "{\"candidates\":[{\"text\":\"你好\",\"source\":1}]}\n".as_bytes(),
        )
        .unwrap();
    });
    assert_eq!(
        UnixSocketProvider::new(socket).query_candidates(query),
        Some(vec![("你好".to_owned(), 1)])
    );
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn online_provider_deduplicates_before_enforcing_source_quota() {
    let directory = private_tempdir();
    let socket = directory.path().join("online.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        std::io::BufRead::read_line(
            &mut std::io::BufReader::new(stream.try_clone().unwrap()),
            &mut line,
        )
        .unwrap();
        std::io::Write::write_all(
            &mut stream,
            "{\"candidates\":[{\"text\":\"重复\",\"source\":1},{\"text\":\"重复\",\"source\":1},{\"text\":\"甲\",\"source\":1},{\"text\":\"乙\",\"source\":1}]}\n".as_bytes(),
        )
        .unwrap();
    });
    let query: OnlineQuery = serde_json::from_value(json!({
        "scheme": 0,
        "generation": 1,
        "identity": "identity",
        "query_text": "nihao",
        "cache_key": "cache",
        "pinyin_segments": ["ni", "hao"],
        "cloud_eligible": true,
        "cloud_candidates": false,
        "ai_eligible": true,
        "session_id": 5,
        "ai_assistant": {
            "enabled": true,
            "provider": "synthetic",
            "model": "synthetic-model",
            "endpoint": "https://ai.invalid/v1/chat/completions",
            "candidate_limit": 3
        }
    }))
    .unwrap();
    assert_eq!(
        UnixSocketProvider::new(socket).query_candidates(query),
        Some(vec![
            ("重复".to_owned(), 1),
            ("甲".to_owned(), 1),
            ("乙".to_owned(), 1),
        ])
    );
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn translation_provider_rejects_controls_at_the_socket_boundary() {
    let directory = private_tempdir();
    let request_socket = directory.path().join("translation-request.sock");
    let listener = UnixListener::bind(&request_socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server_done = done.clone();
    let server_accepted = accepted.clone();
    let server = std::thread::spawn(move || loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                server_accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                std::io::Write::write_all(
                    &mut stream,
                    br#"{"translations":[{"text":"safe","translation":"safe"}]}"#,
                )
                .unwrap();
                std::io::Write::write_all(&mut stream, b"\n").unwrap();
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if server_done.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("translation fixture failed: {error}"),
        }
    });
    let provider = UnixSocketProvider::new(&request_socket);
    for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
        let control = char::from_u32(codepoint).unwrap();
        assert!(provider
            .translate(TranslationQuery {
                generation: 1,
                target_language: "en".into(),
                candidates: vec![format!("safe{control}")],
                sentence: false,
                provider: TranslationService::Tencent,
                translation_account: false,
                custom_translation: None,
                niutrans: None,
            })
            .is_none());
    }
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    server.join().unwrap();
    assert_eq!(accepted.load(std::sync::atomic::Ordering::Relaxed), 0);

    let response_socket = directory.path().join("translation-response.sock");
    let listener = UnixListener::bind(&response_socket).unwrap();
    let server = std::thread::spawn(move || {
        let reply = |mut stream: std::os::unix::net::UnixStream, response: &str| {
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            std::io::BufRead::read_line(&mut reader, &mut request).unwrap();
            std::io::Write::write_all(&mut stream, response.as_bytes()).unwrap();
            std::io::Write::write_all(&mut stream, b"\n").unwrap();
        };
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let (stream, _) = listener.accept().unwrap();
            let control = char::from_u32(codepoint).unwrap();
            let response = json!({
                "translations": [{"text":"safe","translation":format!("before{control}after")}]
            })
            .to_string();
            reply(stream, &response);
        }
        let (stream, _) = listener.accept().unwrap();
        reply(
            stream,
            r#"{"translations":[{"text":"safe","translation":"translated"}]}"#,
        );
    });
    let provider = UnixSocketProvider::new(response_socket);
    let query = TranslationQuery {
        generation: 1,
        target_language: "en".into(),
        candidates: vec!["safe".into()],
        sentence: false,
        provider: TranslationService::Tencent,
        translation_account: false,
        custom_translation: None,
        niutrans: None,
    };
    for _ in (0..=0x1f).chain(0x7f..=0x9f) {
        assert!(provider.translate(query.clone()).is_none());
    }
    assert_eq!(
        provider.translate(query).unwrap(),
        vec![TranslationResult {
            text: "safe".into(),
            translation: "translated".into(),
        }]
    );
    server.join().unwrap();
}

#[test]
fn translation_query_carries_the_selected_service() {
    for (service, name) in [
        (TranslationService::Off, "none"),
        (TranslationService::Account, "account"),
        (TranslationService::Tencent, "tencent"),
        (TranslationService::NiuTrans, "niutrans"),
        (TranslationService::Custom, "custom"),
    ] {
        let document = json!({"generation": 1, "candidates": ["中"], "provider": name});
        let query: TranslationQuery = serde_json::from_value(document).unwrap();
        assert_eq!(query.provider, service);
        assert_eq!(serde_json::to_value(&query).unwrap()["provider"], name);
    }
    assert!(serde_json::from_value::<TranslationQuery>(
        json!({"generation": 1, "candidates": ["中"]})
    )
    .is_err());
    assert!(serde_json::from_value::<TranslationQuery>(
        json!({"generation": 1, "candidates": ["中"], "provider": "deepl"})
    )
    .is_err());
    let sentence: TranslationQuery = serde_json::from_value(json!({
        "generation": 1,
        "candidates": ["这是一个手动触发的整句翻译请求"],
        "sentence": true,
        "provider": "tencent"
    }))
    .unwrap();
    assert!(sentence.sentence);
    assert_eq!(serde_json::to_value(sentence).unwrap()["sentence"], true);
    let account: TranslationQuery = serde_json::from_value(json!({
        "generation": 1,
        "candidates": ["中"],
        "provider": "none",
        "translation_account": true
    }))
    .unwrap();
    assert!(account.translation_account);
    assert_eq!(
        serde_json::to_value(account).unwrap()["translation_account"],
        true
    );
}

#[cfg(unix)]
#[test]
fn sentence_translation_is_single_item_and_bounded() {
    let provider = UnixSocketProvider::new("/this/provider-does-not-exist");
    let too_long = TranslationQuery {
        generation: 1,
        target_language: "en".into(),
        candidates: vec!["中".repeat(513)],
        sentence: true,
        provider: TranslationService::Tencent,
        translation_account: false,
        custom_translation: None,
        niutrans: None,
    };
    assert!(provider.translate(too_long).is_none());
    let two_items = TranslationQuery {
        generation: 1,
        target_language: "en".into(),
        candidates: vec!["第一句".into(), "第二句".into()],
        sentence: true,
        provider: TranslationService::Tencent,
        translation_account: false,
        custom_translation: None,
        niutrans: None,
    };
    assert!(provider.translate(two_items).is_none());
}

#[cfg(unix)]
#[test]
fn translation_switched_off_never_reaches_the_provider() {
    let directory = private_tempdir();
    let socket = directory.path().join("translation-off.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let query = TranslationQuery {
        generation: 1,
        target_language: "en".into(),
        candidates: vec!["中".into()],
        sentence: false,
        provider: TranslationService::Off,
        translation_account: false,
        custom_translation: None,
        niutrans: None,
    };
    assert_eq!(
        UnixSocketProvider::new(&socket).translate(query),
        Some(Vec::new())
    );
    assert!(
        listener.accept().is_err(),
        "a switched-off query connected to the provider"
    );
}

#[cfg(unix)]
#[test]
fn voice_provider_rejects_events_without_generation_binding() {
    let directory = private_tempdir();
    let socket = directory.path().join("voice.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        std::io::BufRead::read_line(
            &mut std::io::BufReader::new(stream.try_clone().unwrap()),
            &mut request,
        )
        .unwrap();
        std::io::Write::write_all(&mut stream, br#"{"text":"stale","type":"final"}"#).unwrap();
        std::io::Write::write_all(&mut stream, b"\n").unwrap();
    });
    let provider = UnixSocketProvider::new(socket);
    assert!(provider
        .voice_stream_with_options_feedback(
            "zh-cn",
            7,
            &Value::Null,
            None,
            &mut |_, _| {},
            None,
            None,
        )
        .is_none());
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn voice_provider_names_only_known_missing_dependencies() {
    for (reply, expected) in [
        (
            r#"{"generation":7,"type":"final","text":"","ok":false,"error":"voice_dependency_missing","detail":"websockets"}"#,
            Err(Some("websockets")),
        ),
        (
            r#"{"generation":7,"type":"final","text":"","ok":false,"error":"voice_dependency_missing","detail":"recorder"}"#,
            Err(Some("recorder")),
        ),
        (
            r#"{"generation":7,"type":"final","text":"","ok":false,"error":"voice_dependency_missing","detail":"local_asr"}"#,
            Err(Some("local_asr")),
        ),
        (
            r#"{"generation":7,"type":"final","text":"","ok":false,"error":"voice_dependency_missing","detail":"token=secret"}"#,
            Err(None),
        ),
        (
            r#"{"generation":7,"type":"final","text":"","ok":false}"#,
            Err(None),
        ),
        (
            r#"{"generation":7,"type":"final","text":"水杉","ok":true}"#,
            Ok("水杉".to_owned()),
        ),
    ] {
        let directory = private_tempdir();
        let socket = directory.path().join("voice.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            std::io::BufRead::read_line(
                &mut std::io::BufReader::new(stream.try_clone().unwrap()),
                &mut request,
            )
            .unwrap();
            std::io::Write::write_all(&mut stream, reply.as_bytes()).unwrap();
            std::io::Write::write_all(&mut stream, b"\n").unwrap();
        });
        let result = UnixSocketProvider::new(socket).voice_stream_with_options_diagnosed(
            "zh-cn",
            7,
            &Value::Null,
            None,
            &mut |_, _| {},
            None,
            None,
        );
        assert_eq!(result, expected, "{reply}");
        server.join().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn dangling_segment_delimiter_cleanup_matches_windows_policy() {
    assert!(needs_dangling_segment_delimiter_backspace("ni'", 3));
    assert!(needs_dangling_segment_delimiter_backspace("ni''ma", 3));
    assert!(!needs_dangling_segment_delimiter_backspace("ni", 2));
    assert!(!needs_dangling_segment_delimiter_backspace("'ma", 0));
    assert!(!needs_dangling_segment_delimiter_backspace("ni'", 2));
    assert!(!needs_dangling_segment_delimiter_backspace("ni'", 9));
}

#[test]
fn voice_control_rejects_zero_generation_without_connecting() {
    let directory = private_tempdir();
    let socket = directory.path().join("voice-control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let provider = UnixSocketProvider::new(&socket);
    assert!(!provider.voice_cancel(0));
    assert!(!provider.voice_stop(0));
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}
impl InputEngine for Fixture {
    fn reset_context(&mut self) {
        self.context_resets += 1;
    }
    fn reset_cache(&mut self) -> Result<(), RuntimeError> {
        self.cache_resets += 1;
        Ok(())
    }
    fn balance_paired_punctuation_after_auto_close(
        &mut self,
        opening: u8,
    ) -> Result<(), RuntimeError> {
        self.balanced_openings.push(opening);
        Ok(())
    }
    /// Model what the engine does: a real change to the mode resets the composition.
    ///
    /// `InputSession::set_dedicated_english_mode` calls `reset_composition()` when the flag
    /// actually flips, which is the source's `SetEnglishInputMode` followed by `ClearState`. The
    /// default here was a no-op, so nothing on this side held the rule and a change in the engine
    /// -- pinned 467 commits ahead of the reference -- would have gone unnoticed.
    fn set_dedicated_english(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        if self.dedicated_english == enabled {
            return Ok(());
        }
        self.dedicated_english = enabled;
        self.text.clear();
        Ok(())
    }
    fn expand_initial_candidates(&mut self) -> Result<bool, RuntimeError> {
        if self.withheld.is_empty() {
            return Ok(false);
        }
        self.words.append(&mut self.withheld);
        Ok(true)
    }
    fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.nine_key = enabled;
        self.nine_key_spellings.clear();
        Ok(())
    }
    fn choose_nine_key_spelling(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if !self.nine_key || index >= self.nine_key_spellings.len() {
            return Ok(empty_result(false));
        }
        self.text = self.nine_key_spellings[index].clone();
        self.nine_key_spellings = vec![self.text.clone()];
        Ok(empty_result(true))
    }
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        if value == b'!' {
            return Err(RuntimeError::Engine("injected punctuation failure".into()));
        }
        if value != b',' {
            return Ok(empty_result(false));
        }
        Ok(EngineResult {
            handled: true,
            has_commit: true,
            commit: "，".into(),
            diagnostic: String::new(),
        })
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if self.text.is_empty() {
            return Ok(empty_result(false));
        }
        let mut result = self.select(index)?;
        result.commit.push_str("-remaining-segments");
        Ok(result)
    }
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        if self.snapshot_fails {
            return Err(RuntimeError::Engine("injected snapshot failure".into()));
        }
        Ok(EngineSnapshot {
            scheme: self.scheme,
            nine_key: self.nine_key,
            nine_key_spellings: self.nine_key_spellings.clone(),
            candidate_codes: self.codes.clone(),
            candidate_annotations: self
                .words
                .iter()
                .enumerate()
                .map(|(index, _)| format!("({index})"))
                .collect(),
            candidate_sources: if self.sources.len() == self.words.len() {
                self.sources.clone()
            } else {
                vec![0; self.words.len()]
            },
            candidate_positions: if self.positions.len() == self.words.len() {
                self.positions.clone()
            } else {
                vec![0; self.words.len()]
            },
            candidate_corrected: vec![false; self.words.len()],
            candidate_answers_key: vec![true; self.words.len()],
            candidate_list_open: false,
            microsoft_shuangpin: false,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: self.scheme == 2
                && self.text.len() == 4
                && self.words.len() == 1,
            local_mode: self.local_mode.clone(),
            spelling_symbols: self.spelling_symbols.clone(),
            dedicated_english: self.dedicated_english,
            preedit: self.text.clone(),
            reading: self.reading.clone(),
            editing_text: self.text.clone(),
            caret_position: self.text.len(),
            segment_raw_boundaries: vec![],
            candidates: if self.text.is_empty() {
                vec![]
            } else {
                self.words.clone()
            },
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        if self.nine_key && (b'2'..=b'9').contains(&value) {
            self.text.push(value as char);
            self.nine_key_spellings = vec!["ni".into(), "mi".into()];
            return Ok(empty_result(true));
        }
        if value.is_ascii_digit() || value.is_ascii_punctuation() {
            return Ok(empty_result(false));
        }
        self.text.push(value as char);
        Ok(empty_result(true))
    }
    fn command(&mut self, _command: Command) -> Result<EngineResult, RuntimeError> {
        self.text.clear();
        self.nine_key_spellings.clear();
        Ok(empty_result(true))
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if let Some(remaining) = self.remaining_after_select.take() {
            self.text = remaining;
            self.local_mode = "none".into();
            return Ok(EngineResult {
                handled: true,
                has_commit: true,
                commit: self.words[index].clone(),
                diagnostic: String::new(),
            });
        }
        self.text.clear();
        self.nine_key_spellings.clear();
        self.local_mode = "none".into();
        Ok(EngineResult {
            handled: true,
            has_commit: true,
            commit: self.words[index].clone(),
            diagnostic: String::new(),
        })
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        let mut result = self.select(index)?;
        result.commit.push_str(match edge {
            CandidateEdge::FirstHan => "-first",
            CandidateEdge::LastHan => "-last",
        });
        Ok(result)
    }
}

#[test]
fn unique_complete_wubi_code_auto_commits_unless_a_phrase_is_being_built() {
    let create = |fixture| {
        let mut runtime = Runtime::new(fixture, 5).unwrap();
        runtime.focus(true).unwrap();
        runtime
    };
    let mut unique = create(Fixture {
        scheme: 2,
        words: vec!["合成候选".into()],
        ..Fixture::default()
    });
    let mut last = None;
    for value in b"wqaa" {
        last = Some(
            unique
                .dispatch(Action::Character {
                    value: *value,
                    shift: false,
                })
                .unwrap(),
        );
    }
    let last = last.unwrap();
    assert_eq!(last.commit.as_deref(), Some("合成候选"));
    assert!(last.view.editing_text.is_empty());

    // The next physical key belongs to a new composition. Windows carries the
    // committed prefix through its TSF continuation payload, then replays this
    // key into the fresh composition instead of dropping it with the automatic
    // four-code commit.
    let fifth = unique
        .dispatch(Action::Character {
            value: b'b',
            shift: false,
        })
        .unwrap();
    assert!(fifth.commit.is_none());
    assert_eq!(fifth.view.editing_text, "b");

    let mut ambiguous = create(Fixture {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        ..Fixture::default()
    });
    for value in b"wqab" {
        ambiguous
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    assert_eq!(ambiguous.view().editing_text, "wqab");

    let mut phrase = create(Fixture {
        scheme: 2,
        words: vec!["合成候选".into()],
        ..Fixture::default()
    });
    phrase.phrase_prefix = "合成前缀".into();
    for value in b"wqaa" {
        phrase
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    assert_eq!(phrase.view().editing_text, "wqaa");
    assert_eq!(phrase.view().phrase_prefix, "合成前缀");
}
#[test]
fn a_letter_after_a_complete_wubi_code_commits_the_first_candidate_and_starts_the_next() {
    let create = |fixture| {
        let mut runtime = Runtime::new(fixture, 5).unwrap();
        runtime.focus(true).unwrap();
        runtime
    };
    let type_all = |runtime: &mut Runtime<Fixture>, keys: &[u8]| {
        let mut last = None;
        for value in keys {
            last = Some(
                runtime
                    .dispatch(Action::Character {
                        value: *value,
                        shift: false,
                    })
                    .unwrap(),
            );
        }
        last.unwrap()
    };

    // An ambiguous four-letter code stays open on its fourth key, and the fifth letter commits
    // the first candidate and becomes the next composition instead of being dropped.
    let mut ambiguous = create(Fixture {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        local_mode: "none".into(),
        ..Fixture::default()
    });
    let fourth = type_all(&mut ambiguous, b"wqab");
    assert!(fourth.commit.is_none());
    assert_eq!(fourth.view.editing_text, "wqab");
    let fifth = type_all(&mut ambiguous, b"x");
    assert_eq!(fifth.commit.as_deref(), Some("合成甲"));
    assert_eq!(
        fifth.commit_context.as_ref().map(|context| context.scheme),
        Some(2)
    );
    assert_eq!(fifth.view.editing_text, "x");

    // Shorter codes, other schemes, local modes, dedicated English and a held phrase keep the
    // letter in the composition.
    let mut short = create(Fixture {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        local_mode: "none".into(),
        ..Fixture::default()
    });
    let shorter = type_all(&mut short, b"wqa");
    assert_eq!(shorter.view.editing_text, "wqa");
    for (case, fixture) in [
        Fixture {
            scheme: 0,
            words: vec!["候选".into(), "后续".into()],
            local_mode: "none".into(),
            ..Fixture::default()
        },
        Fixture {
            scheme: 2,
            local_mode: "unicode".into(),
            words: vec!["合成甲".into(), "合成乙".into()],
            ..Fixture::default()
        },
        Fixture {
            scheme: 2,
            dedicated_english: true,
            words: vec!["wqab".into(), "wqabx".into()],
            local_mode: "none".into(),
            ..Fixture::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut other = create(fixture);
        let last = type_all(&mut other, b"wqabx");
        assert!(last.commit.is_none(), "case {case}");
        assert_eq!(last.view.editing_text, "wqabx", "case {case}");
    }
    let mut phrase = create(Fixture {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        local_mode: "none".into(),
        ..Fixture::default()
    });
    phrase.phrase_prefix = "合成前缀".into();
    let held = type_all(&mut phrase, b"wqabx");
    assert!(held.commit.is_none());
    assert_eq!(held.view.editing_text, "wqabx");

    // A four-letter code with nothing to commit is not a complete code.
    let mut unanswered = create(Fixture {
        scheme: 2,
        words: Vec::new(),
        local_mode: "none".into(),
        ..Fixture::default()
    });
    let empty = type_all(&mut unanswered, b"wqabx");
    assert!(empty.commit.is_none());
    assert_eq!(empty.view.editing_text, "wqabx");
}
fn runtime() -> Runtime<Fixture> {
    Runtime::new(
        Fixture {
            scheme: 0,
            dedicated_english: false,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            local_mode: "none".into(),
            words: (0..12).map(|n| format!("candidate-{n}")).collect(),
            codes: Vec::new(),
            text: String::new(),
            snapshot_fails: false,
            balanced_openings: Vec::new(),
            cache_resets: 0,
            context_resets: 0,
            withheld: Vec::new(),
            sources: Vec::new(),
            remaining_after_select: None,
            positions: Vec::new(),
            reading: String::new(),
            spelling_symbols: String::new(),
        },
        5,
    )
    .unwrap()
}

#[test]
fn auto_close_balance_accepts_only_the_book_title_opening() {
    let mut runtime = runtime();
    for invalid in [b'(', b'>', b'a', b' ', 0, 128, 255] {
        assert!(matches!(
            runtime.balance_paired_punctuation_after_auto_close(invalid),
            Err(RuntimeError::InvalidPunctuation)
        ));
    }
    assert!(runtime.engine.balanced_openings.is_empty());
    runtime
        .balance_paired_punctuation_after_auto_close(b'<')
        .unwrap();
    assert_eq!(runtime.engine.balanced_openings, vec![b'<']);
}

// The AI context accumulator. Every host but Linux sent an empty context,
// so AI suggestions had to guess from the pinyin alone.
// The seating table in candidate_selection_policy.h places one candidate per provider, because the
// reference has one of each. A provider here answers with several - the AI limit reaches ten - and
// the first attempt at this treated everything past the first as a local candidate. That is not a
// cosmetic mistake: a local candidate is what takes the first seat, so the second AI suggestion was
// promoted over the first and landed on the space bar.
#[test]
fn several_candidates_from_one_provider_take_their_seat_as_a_group() {
    let seated = |words: &[&str], sources: Vec<u8>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme: 0,
                dedicated_english: false,
                nine_key: false,
                nine_key_spellings: Vec::new(),
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).into()).collect(),
                // The seating only runs on a snapshot whose parallel arrays all match, so the
                // codes have to be as long as the words for this to exercise anything.
                codes: (0..words.len()).map(|n| format!("code-{n}")).collect(),
                text: String::new(),
                snapshot_fails: false,
                balanced_openings: Vec::new(),
                cache_resets: 0,
                context_resets: 0,
                withheld: Vec::new(),
                sources,
                remaining_after_select: None,
                positions: Vec::new(),
                reading: String::new(),
                spelling_symbols: String::new(),
            },
            9,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        type_key(&mut runtime)
            .view
            .candidates
            .into_iter()
            .map(|candidate| (candidate.text, candidate.annotation))
            .collect::<Vec<_>>()
    };

    // Chinese first, then the whole AI group in the order it arrived. The annotation travels with
    // its candidate, so it also says the parallel arrays were rotated together rather than the text
    // alone: 本地 arrived third and keeps "(2)".
    assert_eq!(
        seated(&["AI 一", "AI 二", "本地"], vec![3, 3, 0]),
        vec![
            ("本地".to_string(), "(2)".to_string()),
            ("AI 一".to_string(), "(0)".to_string()),
            ("AI 二".to_string(), "(1)".to_string()),
        ]
    );
    // Same for a cloud reply of more than one, and the AI group still follows the cloud group.
    assert_eq!(
        seated(&["云一", "云二", "AI", "本地"], vec![2, 2, 3, 0])
            .into_iter()
            .map(|(text, _)| text)
            .collect::<Vec<_>>(),
        vec!["本地", "云一", "云二", "AI"]
    );
    // With a cloud candidate present English sits after AI, and a second English candidate waits
    // behind the seated ones rather than displacing anything.
    assert_eq!(
        seated(
            &["AI 一", "AI 二", "英一", "英二", "云", "本地"],
            vec![3, 3, 4, 4, 2, 0]
        )
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>(),
        vec!["本地", "云", "AI 一", "AI 二", "英一", "英二"]
    );
}

// A single complete kana in Japanese romaji offers its hiragana and katakana as the first two local candidates, and the reference keeps that pair ahead of the cloud word (`JapaneseSingleKanaPairStaysAheadOfCloudCandidate`, which expects か, カ, then the cloud candidate). Anything else keeps the one-seat local prefix.
#[test]
fn japanese_single_kana_pair_stays_ahead_of_the_cloud_candidate() {
    let seated = |scheme: u8, reading: &str, words: &[&str], sources: Vec<u8>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme,
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).into()).collect(),
                codes: (0..words.len()).map(|n| format!("code-{n}")).collect(),
                sources,
                reading: reading.into(),
                ..Fixture::default()
            },
            9,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        type_key(&mut runtime)
            .view
            .candidates
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<_>>()
    };
    let words = ["か", "カ", "蚊", "科"];

    assert_eq!(
        seated(3, "か", &words, vec![0, 0, 2, 0]),
        vec!["か", "カ", "蚊", "科"]
    );
    // Two kana are not a single-kana conversion, so the cloud word takes the second seat.
    assert_eq!(
        seated(3, "かき", &words, vec![0, 0, 2, 0]),
        vec!["か", "蚊", "カ", "科"]
    );
    // Pending romaji is an incomplete conversion.
    assert_eq!(
        seated(3, "k", &words, vec![0, 0, 2, 0]),
        vec!["か", "蚊", "カ", "科"]
    );
    // The rule belongs to the Japanese scheme only.
    assert_eq!(
        seated(0, "か", &words, vec![0, 0, 2, 0]),
        vec!["か", "蚊", "カ", "科"]
    );
    // With a single local candidate the two-seat prefix takes what there is.
    assert_eq!(seated(3, "か", &["か", "蚊"], vec![0, 2]), vec!["か", "蚊"]);
    assert_eq!(seated(3, "か", &["蚊", "か"], vec![2, 0]), vec!["か", "蚊"]);
}

// An English word the user fixed at position 1 keeps the first seat when online candidates arrive. That fixed position is the only way the Engine seats English ahead of a Chinese candidate, so Space commits the English word rather than the Chinese one.
#[test]
fn promoted_english_candidate_keeps_the_first_seat_with_cloud_and_ai() {
    let seated = |words: &[&str], sources: Vec<u8>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme: 0,
                dedicated_english: false,
                nine_key: false,
                nine_key_spellings: Vec::new(),
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).into()).collect(),
                codes: (0..words.len()).map(|n| format!("code-{n}")).collect(),
                text: String::new(),
                snapshot_fails: false,
                balanced_openings: Vec::new(),
                cache_resets: 0,
                context_resets: 0,
                withheld: Vec::new(),
                sources,
                remaining_after_select: None,
                positions: Vec::new(),
                reading: String::new(),
                spelling_symbols: String::new(),
            },
            9,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        type_key(&mut runtime)
            .view
            .candidates
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        seated(
            &["GitHub", "个", "给", "云候选", "AI联想"],
            vec![4, 0, 0, 2, 3]
        ),
        vec!["GitHub", "个", "云候选", "AI联想", "给"]
    );
    // Only the Engine's first seat signals a promotion. An English candidate it placed after the leading Chinese one is seated behind cloud and AI as usual.
    assert_eq!(
        seated(
            &["个", "GitHub", "给", "云候选", "AI联想"],
            vec![0, 4, 0, 2, 3]
        ),
        vec!["个", "云候选", "AI联想", "GitHub", "给"]
    );
    // A promoted English word does not pull a second English candidate into the leading English seat; the rest wait behind the Chinese candidates.
    assert_eq!(
        seated(
            &["GitHub", "个", "Gitter", "给", "云候选"],
            vec![4, 0, 4, 0, 2]
        ),
        vec!["GitHub", "个", "云候选", "给", "Gitter"]
    );
}

// An English word the user fixed to a seat stays there when a cloud or AI reply arrives, as the reference's `FixedEnglishCandidateKeepsItsMixedCandidatePosition` expects. Without the fixed-English pass the seating would put it behind the online candidates.
#[test]
fn fixed_english_candidate_keeps_its_seat_when_online_candidates_arrive() {
    let runtime = |words: &[&str], sources: Vec<u8>, positions: Vec<u8>| {
        let mut runtime = Runtime::new(
            Fixture {
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).into()).collect(),
                codes: (0..words.len()).map(|n| format!("code-{n}")).collect(),
                sources,
                positions,
                reading: String::new(),
                ..Fixture::default()
            },
            9,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        let page = type_key(&mut runtime).view.candidates;
        (runtime, page)
    };
    let texts = |page: &[Candidate]| {
        page.iter()
            .map(|candidate| candidate.text.clone())
            .collect::<Vec<_>>()
    };

    // Fixed to the first seat, with a cloud candidate.
    let (mut cloud_only, cloud_page) = runtime(
        &["个", "GitHub", "给", "云候选"],
        vec![0, 4, 0, 2],
        vec![0, 1, 0, 0],
    );
    assert_eq!(texts(&cloud_page), vec!["GitHub", "个", "云候选", "给"]);

    // Still first once an AI candidate joins the cloud one.
    let (_, page) = runtime(
        &["个", "GitHub", "给", "云候选", "AI联想"],
        vec![0, 4, 0, 2, 3],
        vec![0, 1, 0, 0, 0],
    );
    assert_eq!(texts(&page), vec!["GitHub", "个", "云候选", "AI联想", "给"]);

    // Fixed to the third seat among local, AI and cloud candidates.
    let (_, page) = runtime(
        &["个", "GitHub", "AI联想", "云候选", "给"],
        vec![0, 4, 3, 2, 0],
        vec![0, 3, 0, 0, 0],
    );
    assert_eq!(texts(&page)[2], "GitHub");
    assert_eq!(texts(&page), vec!["个", "云候选", "GitHub", "AI联想", "给"]);

    // Picking the first seat commits the re-seated English word, not the Engine's first candidate.
    let done = cloud_only
        .dispatch(Action::Select(cloud_page[0].id))
        .unwrap();
    assert_eq!(done.commit.as_deref(), Some("GitHub"));
}

// Half a phrase belongs in the composition, not in the document. Picking a candidate that covers
// only part of the input leaves the Engine composing the rest and hands back the piece that was
// picked; sending that piece straight out puts half a phrase into the application - a search box
// searches for it, an editor records an undo step for it - while the user is still typing.
#[test]
fn a_chosen_phrase_piece_waits_for_the_rest_of_the_phrase() {
    let start = |remaining: Option<&str>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme: 0,
                dedicated_english: false,
                nine_key: false,
                nine_key_spellings: Vec::new(),
                local_mode: "none".into(),
                words: vec!["海滩".into(), "跑步".into()],
                codes: Vec::new(),
                text: String::new(),
                snapshot_fails: false,
                balanced_openings: Vec::new(),
                cache_resets: 0,
                context_resets: 0,
                withheld: Vec::new(),
                sources: Vec::new(),
                remaining_after_select: remaining.map(str::to_owned),
                positions: Vec::new(),
                reading: String::new(),
                spelling_symbols: String::new(),
            },
            5,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        runtime
    };
    let pick = |runtime: &mut Runtime<Fixture>| {
        let id = runtime.view().candidates[0].id;
        runtime.dispatch(Action::Select(id)).unwrap()
    };

    // Off, which is what a host that cannot draw the piece gets: unchanged behaviour.
    let mut runtime = start(Some("paobu"));
    type_key(&mut runtime);
    let held = pick(&mut runtime);
    assert_eq!(held.commit.as_deref(), Some("海滩"));
    assert!(held.view.phrase_prefix.is_empty());

    // On: the piece is held, shown to the host separately from the editing text, and the whole
    // phrase goes out as one commit when the composition ends.
    let mut runtime = start(Some("paobu"));
    runtime.set_phrase_preedit(true);
    type_key(&mut runtime);
    let held = pick(&mut runtime);
    assert_eq!(held.commit, None);
    assert_eq!(held.view.phrase_prefix, "海滩");
    assert_eq!(held.view.editing_text, "paobu");
    let rest = runtime.view().candidates[1].id;
    let done = runtime.dispatch(Action::Select(rest)).unwrap();
    assert_eq!(done.commit.as_deref(), Some("海滩跑步"));
    assert!(done.view.phrase_prefix.is_empty());
    assert!(done.view.editing_text.is_empty());

    // Escape throws away what was chosen along with what was typed, as the reference's _HandleCancel
    // does - it clears word_for_creating_word in the same breath as terminating the composition.
    let mut runtime = start(Some("paobu"));
    runtime.set_phrase_preedit(true);
    type_key(&mut runtime);
    pick(&mut runtime);
    let cancelled = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert_eq!(cancelled.commit, None);
    assert!(cancelled.view.phrase_prefix.is_empty());

    // Leaving the client cancels the composition too, but there the piece goes to the document:
    // before it was ever held back it would already be there, and a click into another window is
    // not the user throwing the phrase away.
    let mut runtime = start(Some("paobu"));
    runtime.set_phrase_preedit(true);
    type_key(&mut runtime);
    pick(&mut runtime);
    let blurred = runtime.focus(false).unwrap();
    assert_eq!(blurred.commit.as_deref(), Some("海滩"));
    assert!(blurred.view.phrase_prefix.is_empty());

    // A commit that no candidate was picked for is not part of a phrase. Punctuation finishes the
    // composition and sends the mark out with it; that commit has to read the same either way.
    let mut plain = start(None);
    type_key(&mut plain);
    let expected = plain.dispatch(Action::Punctuation(b',')).unwrap().commit;
    assert!(expected.is_some());
    let mut runtime = start(None);
    runtime.set_phrase_preedit(true);
    type_key(&mut runtime);
    let punctuated = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(punctuated.commit, expected);
    assert!(punctuated.view.phrase_prefix.is_empty());

    // Turning it off with a piece in hand hands the piece back rather than dropping it.
    let mut runtime = start(Some("paobu"));
    runtime.set_phrase_preedit(true);
    type_key(&mut runtime);
    pick(&mut runtime);
    assert_eq!(runtime.set_phrase_preedit(false).as_deref(), Some("海滩"));
    assert!(runtime.view().phrase_prefix.is_empty());
}

// The one place this leaves the reference: there, backspacing the remaining reading away keeps the
// chosen piece on screen with nothing after it. Holding text with no composition under it would
// make every host's "is there a composition" test lie, so the piece is committed instead.
#[test]
fn a_phrase_piece_survives_the_reading_being_deleted() {
    let mut runtime = Runtime::new(
        Fixture {
            scheme: 0,
            dedicated_english: false,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            local_mode: "none".into(),
            words: vec!["海滩".into(), "跑步".into()],
            codes: Vec::new(),
            text: String::new(),
            snapshot_fails: false,
            balanced_openings: Vec::new(),
            cache_resets: 0,
            context_resets: 0,
            withheld: Vec::new(),
            sources: Vec::new(),
            remaining_after_select: Some("p".into()),
            positions: Vec::new(),
            reading: String::new(),
            spelling_symbols: String::new(),
        },
        5,
    )
    .unwrap();
    runtime.set_phrase_preedit(true);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let id = runtime.view().candidates[0].id;
    let held = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(held.view.phrase_prefix, "海滩");

    let emptied = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert!(emptied.view.editing_text.is_empty());
    assert_eq!(emptied.commit.as_deref(), Some("海滩"));
    assert!(emptied.view.phrase_prefix.is_empty());
}

#[test]
fn ai_context_keeps_the_recent_tail_on_a_character_boundary() {
    let mut runtime = runtime();
    runtime.focused = true;
    runtime.remember_commit("你好");
    runtime.remember_commit("世界");
    assert_eq!(runtime.ai_context, "你好世界");

    // Bounded at 1024 bytes, because query_candidates refuses anything
    // longer outright rather than trimming it.
    for _ in 0..400 {
        runtime.remember_commit("字");
    }
    assert!(runtime.ai_context.len() <= 1024);
    // The cut lands on a character boundary, so the context is still valid
    // UTF-8 and does not start with half a character.
    assert!(runtime.ai_context.is_char_boundary(0));
    assert!(std::str::from_utf8(runtime.ai_context.as_bytes()).is_ok());
    assert!(runtime.ai_context.ends_with('字'));
    // It is the tail that is kept, not the head.
    assert!(!runtime.ai_context.starts_with("你好"));
}

#[test]
fn ai_context_does_not_leak_between_clients() {
    let mut runtime = runtime();
    runtime.focused = true;
    runtime.remember_commit("上一个应用里的句子");
    assert!(!runtime.ai_context.is_empty());

    // A commit while unfocused is not context at all, and clears what was
    // there: the user has left.
    runtime.focused = false;
    runtime.remember_commit("anything");
    assert!(runtime.ai_context.is_empty());
}
// An engine that models the one thing the phrase rules turn on: a selection takes its reading off
// the front and leaves the rest, and the caret can sit somewhere other than the end.
//
// `Fixture` cannot express either - its `select` replaces the whole reading and its caret is always
// at the end - and a rule about what is left in front of the caret cannot be tested against an
// engine that has no such thing.
struct PhraseEngine {
    reading: String,
    caret: usize,
    /// How much of the reading each selection takes off the front, oldest first. A selection past
    /// the end of this list finishes the composition.
    consumes: Vec<usize>,
    words: Vec<String>,
    /// Published with nothing composed, as the Engine publishes `/` and `@` when their modes are on.
    idle_symbols: String,
}

impl PhraseEngine {
    fn new(consumes: Vec<usize>) -> Self {
        Self {
            reading: String::new(),
            caret: 0,
            consumes,
            words: vec!["海滩".into(), "跑步".into()],
            idle_symbols: String::new(),
        }
    }
}

impl InputEngine for PhraseEngine {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        Ok(EngineSnapshot {
            scheme: 0,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            candidate_codes: Vec::new(),
            candidate_annotations: vec![String::new(); self.words.len()],
            candidate_sources: vec![0; self.words.len()],
            candidate_positions: vec![0; self.words.len()],
            candidate_corrected: vec![false; self.words.len()],
            candidate_answers_key: vec![true; self.words.len()],
            candidate_list_open: false,
            microsoft_shuangpin: false,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: false,
            local_mode: "none".into(),
            spelling_symbols: if self.reading.is_empty() {
                self.idle_symbols.clone()
            } else {
                String::new()
            },
            dedicated_english: false,
            preedit: self.reading.clone(),
            reading: String::new(),
            editing_text: self.reading.clone(),
            caret_position: self.caret.min(self.reading.len()),
            segment_raw_boundaries: Vec::new(),
            candidates: if self.reading.is_empty() {
                Vec::new()
            } else {
                self.words.clone()
            },
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        self.reading.push(value as char);
        self.caret = self.reading.len();
        Ok(empty_result(true))
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        match command {
            Command::MoveHome => self.caret = 0,
            Command::MoveEnd => self.caret = self.reading.len(),
            Command::Backspace => {
                if self.caret > 0 {
                    self.reading.remove(self.caret - 1);
                    self.caret -= 1;
                }
            }
            _ => {
                // Like the real Engine, a key that ends a composition is not wanted when there is no reading to end.
                let composing = !self.reading.is_empty();
                self.reading.clear();
                self.caret = 0;
                return Ok(empty_result(composing));
            }
        }
        Ok(empty_result(true))
    }
    // The whole reading before the caret is one segment, so a segment Backspace takes all of it.
    fn segment_command(&mut self, command: SegmentCommand) -> Result<EngineResult, RuntimeError> {
        if !matches!(command, SegmentCommand::Backspace) || self.caret == 0 {
            return Ok(empty_result(!self.reading.is_empty()));
        }
        self.reading = self.reading.split_off(self.caret);
        self.caret = 0;
        Ok(empty_result(true))
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        let commit = self.words[index].clone();
        if self.consumes.is_empty() {
            self.reading.clear();
        } else {
            let consumed = self.consumes.remove(0).min(self.reading.len());
            self.reading = self.reading.split_off(consumed);
        }
        self.caret = self.reading.len();
        Ok(EngineResult {
            handled: true,
            has_commit: true,
            commit,
            diagnostic: String::new(),
        })
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        // Like the real Engine, there is nothing to finish without a reading.
        if self.reading.is_empty() {
            return Ok(empty_result(false));
        }
        self.select(index)
    }
    // Every mark is one without a Chinese form, such as `/`.
    fn punctuation(&mut self, _value: u8) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn select_edge(
        &mut self,
        index: usize,
        _edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
}

// A held piece over an emptied reading is a composition: `/` ends it as a mark on every route instead of opening a mode behind it, and a host that reads the View's symbols (Harmony) sees none to compose or pick with.
#[test]
fn a_mode_symbol_behind_a_held_phrase_ends_the_phrase() {
    let emptied = || {
        let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
        runtime.engine.idle_symbols = "/@".into();
        let id = runtime.view().candidates[0].id;
        runtime.dispatch(Action::Select(id)).unwrap();
        let kept = runtime.dispatch(Action::SegmentBackspace).unwrap();
        assert_eq!(kept.view.phrase_prefix, "海滩");
        assert!(kept.view.editing_text.is_empty());
        assert!(kept.view.spelling_symbols.is_empty());
        runtime
    };
    for action in [
        Action::Character {
            value: b'/',
            shift: false,
        },
        Action::Punctuation(b'/'),
        Action::PunctuationAscii(b'/'),
    ] {
        let mut runtime = emptied();
        let ended = runtime.dispatch(action).unwrap();
        assert!(ended.handled);
        assert_eq!(ended.commit.as_deref(), Some("海滩/"));
        assert!(ended.view.phrase_prefix.is_empty());
        assert!(ended.view.editing_text.is_empty());
        assert_eq!(ended.view.spelling_symbols, "/@");
    }
}

// The reading is typed rather than seeded: taking focus cancels the composition, so an engine that
// started with one would lose it before the first key of the test.
fn phrase_runtime(reading: &str, consumes: Vec<usize>) -> Runtime<PhraseEngine> {
    let mut runtime = Runtime::new(PhraseEngine::new(consumes), 5).unwrap();
    runtime.set_phrase_preedit(true);
    runtime.focus(true).unwrap();
    for byte in reading.bytes() {
        runtime
            .dispatch(Action::Character {
                value: byte,
                shift: false,
            })
            .unwrap();
    }
    runtime
}

// A Ctrl+Backspace that empties the reading does not end the phrase: the chosen piece stays in the composition with its selection, as the reference's `keep_creating_word_after_empty_raw` keeps it (MSIME-Windows server/src/ipc/event_listener.cpp, pinned by `ShouldRetreatCreatingWordSelection(true, false, true, 0, 0, 1)` and `ShouldDropCreatingWordSegment(true, false, true, 0, 1)` in test_input_key_policy.cpp). Every follow-up key then acts on that phrase.
#[test]
fn a_segment_backspace_that_empties_the_reading_keeps_the_phrase() {
    let emptied = || {
        let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
        let id = runtime.view().candidates[0].id;
        let held = runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(held.view.phrase_prefix, "海滩");
        assert_eq!(held.view.editing_text, "paobu");
        let kept = runtime.dispatch(Action::SegmentBackspace).unwrap();
        assert_eq!(kept.commit, None);
        assert!(kept.handled);
        assert_eq!(kept.view.phrase_prefix, "海滩");
        assert!(kept.view.editing_text.is_empty());
        runtime
    };

    // Backspace takes the selection back: the reading it consumed returns.
    let mut runtime = emptied();
    let back = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(back.commit, None);
    assert!(back.view.phrase_prefix.is_empty());
    assert_eq!(back.view.editing_text, "haitan");

    // A second Ctrl+Backspace deletes the chosen piece, which ends the composition with nothing sent.
    let mut runtime = emptied();
    let dropped = runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(dropped.commit, None);
    assert!(dropped.handled);
    assert!(dropped.view.phrase_prefix.is_empty());
    assert!(dropped.view.editing_text.is_empty());

    // Enter sends the phrase, and the key does not also reach the application.
    let mut runtime = emptied();
    let committed = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(committed.commit.as_deref(), Some("海滩"));
    assert!(committed.handled);
    assert!(committed.view.phrase_prefix.is_empty());

    // Escape throws it away.
    let mut runtime = emptied();
    let cancelled = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert_eq!(cancelled.commit, None);
    assert!(cancelled.handled);
    assert!(cancelled.view.phrase_prefix.is_empty());

    // Leaving the client still sends the phrase.
    let mut runtime = emptied();
    let blurred = runtime.focus(false).unwrap();
    assert_eq!(blurred.commit.as_deref(), Some("海滩"));
}

// Going back into a phrase that is half chosen.
//
// The reference has two rules for it, both in `input_key_policy.h`, and this host had neither: the
// piece the user picked could only be finished or thrown away whole. Picking the wrong word for the
// first half of a phrase is ordinary, and the way out of it was to cancel the composition and type
// the whole thing again.
#[test]
fn the_last_selection_of_a_phrase_can_be_taken_back() {
    // Backspace on the last character of the reading: the selection comes back instead of the
    // composition ending. The reading it consumed is what is on screen afterwards, so the user can
    // pick a different word for it.
    let mut runtime = phrase_runtime("haitanp", vec![6]);
    let id = runtime.view().candidates[0].id;
    let held = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(held.view.phrase_prefix, "海滩");
    assert_eq!(held.view.editing_text, "p");

    let back = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(back.commit, None);
    assert!(back.view.phrase_prefix.is_empty());
    assert_eq!(back.view.editing_text, "haitan");
    assert!(!back.view.candidates.is_empty());

    // And it is a stack: only the newest selection comes back, the ones before it stay.
    let mut runtime = phrase_runtime("haitanpaobux", vec![6, 5]);
    let first = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(first)).unwrap();
    let second = runtime.view().candidates[1].id;
    let held = runtime.dispatch(Action::Select(second)).unwrap();
    assert_eq!(held.view.phrase_prefix, "海滩跑步");
    assert_eq!(held.view.editing_text, "x");
    let back = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(back.view.phrase_prefix, "海滩");
    assert_eq!(back.view.editing_text, "paobu");

    // With more than one character left the key is an ordinary Backspace: the user is editing the
    // reading, not leaving it.
    let mut runtime = phrase_runtime("haitanpa", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let edited = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(edited.view.phrase_prefix, "海滩");
    assert_eq!(edited.view.editing_text, "p");

    // Nothing was ever selected, so there is nothing to go back to and Backspace stays Backspace.
    let mut runtime = phrase_runtime("p", Vec::new());
    let plain = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert!(plain.view.editing_text.is_empty());
    assert!(plain.view.phrase_prefix.is_empty());
}

// Ctrl+Backspace with nothing before the caret deletes the selection itself, and unlike Backspace
// it does not hand the reading back: the user asked to remove that piece of the phrase, not to
// spell it again (the reference's PRD R3).
#[test]
fn a_segment_backspace_with_nothing_before_the_caret_drops_the_selection() {
    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    let held = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(held.view.phrase_prefix, "海滩");
    assert_eq!(held.view.editing_text, "paobu");

    runtime
        .dispatch(Action::Command(Command::MoveHome))
        .unwrap();
    let dropped = runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(dropped.commit, None);
    assert!(dropped.view.phrase_prefix.is_empty());
    // The reading it consumed is gone for good; what the user typed after it is untouched.
    assert_eq!(dropped.view.editing_text, "paobu");

    // With the caret anywhere else the key is the ordinary segment Backspace and reaches the
    // Engine, which owns the unit boundaries.
    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let edited = runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(edited.view.phrase_prefix, "海滩");
}

fn type_key(runtime: &mut Runtime<Fixture>) -> Transition {
    runtime
        .dispatch(Action::Character {
            value: b'a',
            shift: false,
        })
        .unwrap()
}

#[test]
fn candidate_codes_follow_candidates_in_page_and_complete_snapshots() {
    let mut runtime = Runtime::new(
        Fixture {
            scheme: 2,
            dedicated_english: false,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            local_mode: "none".into(),
            words: vec!["甲".into(), "乙".into()],
            codes: vec!["ab".into(), "ac".into()],
            text: String::new(),
            snapshot_fails: false,
            balanced_openings: Vec::new(),
            cache_resets: 0,
            context_resets: 0,
            withheld: Vec::new(),
            sources: Vec::new(),
            remaining_after_select: None,
            positions: Vec::new(),
            reading: String::new(),
            spelling_symbols: String::new(),
        },
        2,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view;
    assert_eq!(page.candidates[0].code, "ab");
    assert_eq!(page.candidates[1].code, "ac");
    let snapshot = runtime.all_candidates();
    assert_eq!(snapshot.candidates[0].code, "ab");
    assert_eq!(snapshot.candidates[1].code, "ac");
    assert!(snapshot.reading.is_empty());
    let serialized = serde_json::to_value(snapshot).unwrap();
    assert_eq!(serialized["candidates"][1]["code"], "ac");
    assert_eq!(serialized["reading"], "");
}

#[test]
fn online_provider_worker_is_bounded_and_filters_invalid_results() {
    let query = OnlineQuery {
        scheme: 0,
        generation: 4,
        identity: "identity".into(),
        query_text: "nihao".into(),
        cache_key: "cache".into(),
        pinyin_segments: vec!["ni".into(), "hao".into()],
        cloud_eligible: true,
        ai_eligible: true,
        cloud_candidates: true,
        session_id: 9,
        ai_context: String::new(),
        ai_assistant: None,
        ai_cache_only: false,
    };
    let worker = OnlineProviderWorker::spawn(1, |query| {
        if query.query_text == "nihao" {
            Some(("你好".into(), 0))
        } else {
            Some((String::new(), 7))
        }
    })
    .unwrap();
    assert!(worker.submit(query.clone()));
    let mut result = None;
    for _ in 0..100 {
        result = worker.try_recv();
        if result.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let result = result.expect("provider result");
    assert_eq!(result.query, query);
    assert_eq!(result.text, "你好");
    assert_eq!(result.source, 0);
    worker.shutdown();
}

#[test]
fn online_provider_worker_keeps_only_the_latest_completed_result() {
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&calls);
    let worker = OnlineProviderWorker::spawn(1, move |query| {
        observed.fetch_add(1, Ordering::SeqCst);
        Some((query.query_text.clone(), 0))
    })
    .unwrap();
    let query = |text: &str| OnlineQuery {
        scheme: 0,
        generation: 1,
        identity: "identity".into(),
        query_text: text.into(),
        cache_key: text.into(),
        pinyin_segments: vec![],
        cloud_eligible: true,
        ai_eligible: false,
        cloud_candidates: true,
        session_id: 9,
        ai_context: String::new(),
        ai_assistant: None,
        ai_cache_only: false,
    };
    assert!(worker.submit(query("first")));
    for _ in 0..100 {
        if calls.load(Ordering::SeqCst) >= 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(worker.submit(query("second")));
    for _ in 0..100 {
        if calls.load(Ordering::SeqCst) >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        worker.try_recv().map(|result| result.text),
        Some("second".into())
    );
    assert!(worker.try_recv().is_none());
    worker.shutdown();
}

#[test]
fn cloud_request_requires_eligible_query() {
    assert_eq!(
        WINDOWS_CLOUD_DEBOUNCE,
        std::time::Duration::from_millis(500)
    );
    let mut query = OnlineQuery {
        scheme: 0,
        generation: 1,
        identity: "x".into(),
        query_text: "ni".into(),
        cache_key: "x".into(),
        pinyin_segments: vec![],
        cloud_eligible: false,
        ai_eligible: false,
        cloud_candidates: true,
        session_id: 1,
        ai_context: String::new(),
        ai_assistant: None,
        ai_cache_only: false,
    };
    assert!(cloud_request_url(&query).is_none());
    query.cloud_eligible = true;
    assert!(cloud_request_url(&query)
        .unwrap()
        .contains("inputtools.google.com"));
    let response = serde_json::json!(["SUCCESS", [["ni", ["你"]]]]).to_string();
    // Korean never goes to a cloud provider, even with a query claiming eligibility.
    let korean = OnlineQuery {
        scheme: KOREAN_SCHEME,
        ..query.clone()
    };
    assert!(cloud_request_url(&korean).is_none());
    assert!(cloud_candidate_from_response(korean, response.as_bytes()).is_none());
    let result = cloud_candidate_from_response(query, response.as_bytes()).unwrap();
    assert_eq!(result.text, "你");
    assert_eq!(result.source, 0);
}

#[test]
fn online_provider_worker_rejects_zero_capacity_and_shutdowns_idle() {
    assert!(OnlineProviderWorker::spawn(0, |_| None).is_err());
    let worker = OnlineProviderWorker::spawn(1, |_| None).unwrap();
    worker.shutdown();
}
#[test]
fn replacement_requires_verified_idle_and_preserves_session_focus() {
    let mut active = runtime();
    active.focus(true).unwrap();
    let old = type_key(&mut active).view;
    assert!(matches!(
        active.replace_engine(runtime().engine, 2),
        Err(RuntimeError::CompositionActive)
    ));
    assert_eq!(active.view().editing_text, old.editing_text);
    active.dispatch(Action::Command(Command::Cancel)).unwrap();
    active.replace_engine(runtime().engine, 2).unwrap();
    let updated = type_key(&mut active).view;
    assert_eq!(updated.session, old.session);
    assert!(updated.focused && updated.generation > old.generation);
    assert_eq!(updated.candidates.len(), 2);
    assert!(matches!(
        active.dispatch(Action::Select(old.candidates[0].id)),
        Err(RuntimeError::StaleCandidate)
    ));
    active.engine.snapshot_fails = true;
    assert!(active.refresh().is_err());
    assert!(active.view().editing_text.is_empty());
    assert!(
        !active.is_idle(),
        "missing snapshot is not proof of idle Engine"
    );
    assert!(matches!(
        active.replace_engine(runtime().engine, 2),
        Err(RuntimeError::CompositionActive)
    ));
}

#[test]
fn touch_layout_changes_atomically_with_engine_replacement() {
    let mut active = runtime();
    assert_eq!(
        active.view().touch_keyboard_layout,
        TouchKeyboardLayout::TwentySixKey
    );
    active.focus(true).unwrap();
    type_key(&mut active);
    assert!(matches!(
        active.replace_engine_with_touch_layout(runtime().engine, 2, TouchKeyboardLayout::NineKey),
        Err(RuntimeError::CompositionActive)
    ));
    assert_eq!(
        active.view().touch_keyboard_layout,
        TouchKeyboardLayout::TwentySixKey
    );
    active.dispatch(Action::Command(Command::Cancel)).unwrap();
    active
        .replace_engine_with_touch_layout(runtime().engine, 2, TouchKeyboardLayout::NineKey)
        .unwrap();
    assert_eq!(
        active.view().touch_keyboard_layout,
        TouchKeyboardLayout::NineKey
    );
    active
        .replace_engine_with_touch_layout(runtime().engine, 2, TouchKeyboardLayout::Handwriting)
        .unwrap();
    assert_eq!(
        active.view().touch_keyboard_layout,
        TouchKeyboardLayout::Handwriting
    );
}

#[test]
fn translations_are_generation_scoped_and_exposed_on_candidates() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let view = type_key(&mut runtime).view;
    assert!(
        !runtime.apply_translations(view.generation - 1, [("candidate-0".into(), "old".into())])
    );
    assert!(runtime.apply_translations(
        view.generation,
        [("candidate-0".into(), "translated".into())]
    ));
    assert_eq!(
        runtime.view().candidates[0].translation.as_deref(),
        Some("translated")
    );
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(runtime
        .view()
        .candidates
        .iter()
        .all(|candidate| candidate.translation.is_none()));
}

#[test]
fn replacement_snapshot_failure_keeps_the_original_engine() {
    let mut active = runtime();
    active.focus(true).unwrap();
    let generation = active.view().generation;
    let mut replacement = runtime().engine;
    replacement.snapshot_fails = true;
    assert!(active.replace_engine(replacement, 2).is_err());
    assert_eq!(active.view().generation, generation);
    assert_eq!(type_key(&mut active).view.candidates.len(), 5);
}

#[test]
fn paging_and_selection_use_global_engine_indices() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let page = runtime.dispatch(Action::NextPage).unwrap().view;
    assert_eq!(page.page, 1);
    assert_eq!(page.page_count, 3);
    assert_eq!(page.candidates[0].annotation, "(5)");
    assert_eq!(page.candidates[0].text, "candidate-5");
    let result = runtime
        .dispatch(Action::Select(page.candidates[2].id))
        .unwrap();
    assert_eq!(result.commit.as_deref(), Some("candidate-7"));
    assert!(result.view.candidates.is_empty());
}

#[test]
fn complete_candidate_snapshot_is_on_demand_and_preserves_global_identity() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view;
    assert_eq!(page.candidates.len(), 5);
    assert!(runtime.apply_translations(
        page.generation,
        [("candidate-10".into(), "translated".into())]
    ));

    let snapshot = runtime.all_candidates();
    assert_eq!(snapshot.session, page.session);
    assert_eq!(snapshot.generation, page.generation);
    assert_eq!(snapshot.preedit, "a");
    assert_eq!(snapshot.candidates.len(), 12);
    assert_eq!(snapshot.candidates[10].id.index, 10);
    assert_eq!(snapshot.candidates[10].annotation, "(10)");
    assert_eq!(snapshot.candidates[10].source, 0);
    assert_eq!(snapshot.candidates[10].fixed_position, 0);
    assert_eq!(
        snapshot.candidates[10].translation.as_deref(),
        Some("translated")
    );
    assert!(snapshot.candidates[0].highlighted);
}

#[test]
fn expanded_panel_selection_accepts_only_any_candidate_from_current_generation() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view;
    let outside_page = runtime.all_candidates().candidates[10].id;
    let generation = page.generation;

    assert!(matches!(
        runtime.dispatch(Action::Select(outside_page)),
        Err(RuntimeError::StaleCandidate)
    ));
    for invalid in [
        CandidateId {
            session: outside_page.session + 1,
            ..outside_page
        },
        CandidateId {
            generation: outside_page.generation + 1,
            ..outside_page
        },
        CandidateId {
            index: 12,
            ..outside_page
        },
    ] {
        assert!(matches!(
            runtime.dispatch(Action::SelectAnyCandidate(invalid)),
            Err(RuntimeError::StaleCandidate)
        ));
        assert_eq!(runtime.view().generation, generation);
    }

    let selected = runtime
        .dispatch(Action::SelectAnyCandidate(outside_page))
        .unwrap();
    assert_eq!(selected.commit.as_deref(), Some("candidate-10"));
    assert!(selected.view.candidates.is_empty());
}

#[test]
fn candidate_list_edges_reach_the_ends_of_the_whole_list() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let highlighted = |view: &View| {
        view.candidates
            .iter()
            .find(|candidate| candidate.highlighted)
            .unwrap()
            .text
            .clone()
    };

    // From the second page, Home goes back to the very first candidate and takes the page with it -
    // the reference answers its Home with SetSelection(0), which readjusts the page. Stopping at the
    // top of the page the user is already looking at is a keystroke that changes almost nothing.
    runtime.dispatch(Action::NextPage).unwrap();
    let first = runtime.dispatch(Action::FirstCandidate).unwrap().view;
    assert_eq!(highlighted(&first), "candidate-0");
    assert_eq!(first.page, 0);

    // End reaches the last candidate there is, page and all. The fixture holds twelve at a page of
    // five, so that is the third page rather than the end of the first.
    let last = runtime.dispatch(Action::LastCandidate).unwrap().view;
    assert_eq!(highlighted(&last), "candidate-11");
    assert_eq!(last.page, 2);
    assert_eq!(last.page_count, 3);

    // Pressing it again stays put rather than walking further.
    let again = runtime.dispatch(Action::LastCandidate).unwrap().view;
    assert_eq!(highlighted(&again), "candidate-11");
}

// The Engine caps what it returns to a short query and hands the rest over when asked. End has to
// ask, or it lands on the last candidate that happened to be cached - and a second press would then
// move further, which is not what an End key does.
#[test]
fn the_last_candidate_is_the_last_one_the_engine_has() {
    let mut runtime = withholding_runtime(5, 4, 5);
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view;
    assert_eq!(page.page_count, 1);

    let last = runtime.dispatch(Action::LastCandidate).unwrap().view;
    assert_eq!(last.page_count, 2);
    assert_eq!(
        last.candidates
            .iter()
            .find(|candidate| candidate.highlighted)
            .unwrap()
            .text,
        "candidate-8"
    );
}

#[test]
fn edge_selection_checks_identity_and_routes_global_index() {
    for edge in [CandidateEdge::FirstHan, CandidateEdge::LastHan] {
        let mut active = runtime();
        active.focus(true).unwrap();
        let first = type_key(&mut active).view.candidates[0].id;
        let page = active.dispatch(Action::NextPage).unwrap().view;
        let id = page.candidates[1].id;
        assert_eq!(id.index, 6);
        let generation = active.view().generation;
        for invalid in [
            first,
            CandidateId {
                session: id.session + 1,
                ..id
            },
            CandidateId { index: 0, ..id },
            CandidateId { index: 10, ..id },
        ] {
            assert!(matches!(
                active.dispatch(Action::SelectEdge(invalid, edge)),
                Err(RuntimeError::StaleCandidate)
            ));
            assert_eq!(active.view().generation, generation);
        }
        let selected = active.dispatch(Action::SelectEdge(id, edge)).unwrap();
        assert_eq!(
            selected.commit.as_deref(),
            Some(match edge {
                CandidateEdge::FirstHan => "candidate-6-first",
                CandidateEdge::LastHan => "candidate-6-last",
            })
        );
        assert!(selected.view.editing_text.is_empty());
    }
}

#[test]
fn punctuation_finishes_highlighted_candidate_and_remaining_segments() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    let result = runtime
        .dispatch(Action::Character {
            value: b',',
            shift: false,
        })
        .unwrap();
    assert_eq!(
        result.commit.as_deref(),
        Some("candidate-5-remaining-segments，")
    );
    assert!(result.handled && result.view.editing_text.is_empty());
}

#[test]
fn ascii_punctuation_finishes_highlighted_candidate_for_keypad_marks() {
    for mark in *b".-+/*" {
        let mut runtime = runtime();
        runtime.focus(true).unwrap();
        type_key(&mut runtime);
        let result = runtime.dispatch(Action::PunctuationAscii(mark)).unwrap();
        let expected = format!("candidate-0-remaining-segments{}", mark as char);
        assert_eq!(result.commit.as_deref(), Some(expected.as_str()));
        assert!(result.handled && result.view.editing_text.is_empty());
    }
}

#[test]
fn unsupported_punctuation_is_appended_only_after_a_composition() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let idle = runtime
        .dispatch(Action::Character {
            value: b'@',
            shift: false,
        })
        .unwrap();
    assert!(!idle.handled && idle.commit.is_none());
    type_key(&mut runtime);
    let result = runtime
        .dispatch(Action::Character {
            value: b'@',
            shift: false,
        })
        .unwrap();
    assert_eq!(
        result.commit.as_deref(),
        Some("candidate-0-remaining-segments@")
    );
}

#[test]
fn punctuation_failure_does_not_lose_an_already_finished_commit() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let result = runtime
        .dispatch(Action::Character {
            value: b'!',
            shift: false,
        })
        .unwrap();
    assert_eq!(
        result.commit.as_deref(),
        Some("candidate-0-remaining-segments!")
    );
    assert!(result
        .diagnostic
        .unwrap()
        .contains("injected punctuation failure"));
}

#[test]
fn number_keys_select_the_visible_page_and_pass_through_when_idle() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    assert!(
        !runtime
            .dispatch(Action::Character {
                value: b'2',
                shift: false
            })
            .unwrap()
            .handled
    );
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    let result = runtime
        .dispatch(Action::Character {
            value: b'2',
            shift: false,
        })
        .unwrap();
    assert_eq!(result.commit.as_deref(), Some("candidate-6"));
}

#[test]
fn nine_key_mode_owns_digits_and_spelling_choices_are_generation_scoped() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let original_generation = runtime.view().generation;
    runtime.set_nine_key_enabled(true).unwrap();
    assert!(runtime.view().nine_key && runtime.view().generation > original_generation);
    let typed = runtime
        .dispatch(Action::Character {
            value: b'6',
            shift: false,
        })
        .unwrap();
    assert!(typed.handled && typed.commit.is_none());
    assert_eq!(
        typed.view.nine_key_spellings,
        vec!["ni".to_owned(), "mi".to_owned()]
    );
    let invalid_digit = runtime
        .dispatch(Action::Character {
            value: b'1',
            shift: false,
        })
        .unwrap();
    assert!(!invalid_digit.handled && invalid_digit.commit.is_none());
    let separator = runtime
        .dispatch(Action::Character {
            value: b'\'',
            shift: false,
        })
        .unwrap();
    assert!(!separator.handled && separator.commit.is_none());
    let generation = separator.view.generation;
    let stale = NineKeySpellingId {
        session: separator.view.session,
        generation: generation - 1,
        index: 0,
    };
    assert!(matches!(
        runtime.dispatch(Action::ChooseNineKeySpelling(stale)),
        Err(RuntimeError::StaleNineKeySpelling)
    ));
    let invalid = NineKeySpellingId {
        session: separator.view.session,
        generation,
        index: 2,
    };
    assert!(matches!(
        runtime.dispatch(Action::ChooseNineKeySpelling(invalid)),
        Err(RuntimeError::StaleNineKeySpelling)
    ));
    let selected = runtime
        .dispatch(Action::ChooseNineKeySpelling(NineKeySpellingId {
            session: separator.view.session,
            generation,
            index: 1,
        }))
        .unwrap();
    assert!(selected.handled && selected.view.editing_text == "mi");
    assert!(matches!(
        runtime.set_nine_key_enabled(false),
        Err(RuntimeError::CompositionActive)
    ));
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    runtime.set_nine_key_enabled(false).unwrap();
    assert!(!runtime.view().nine_key);
    runtime.engine.scheme = 1;
    runtime.refresh().unwrap();
    assert!(matches!(
        runtime.set_nine_key_enabled(true),
        Err(RuntimeError::InvalidNineKeyScheme)
    ));
}

#[test]
fn unavailable_numeric_slot_does_not_jump_back_to_first_page() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    runtime.dispatch(Action::NextPage).unwrap();
    let result = runtime
        .dispatch(Action::Character {
            value: b'9',
            shift: false,
        })
        .unwrap();
    assert!(result.handled && result.commit.is_none());
    assert_eq!(result.view.page, 2);
}

#[test]
fn engine_mode_is_authoritative_and_resets_old_highlight() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    runtime.engine.local_mode = "unicode".into();
    let result = runtime
        .dispatch(Action::Character {
            value: b'0',
            shift: false,
        })
        .unwrap();
    assert_eq!(result.view.local_mode, "unicode");
    assert_eq!(result.view.page, 0);
    assert!(!result.view.editing_text.starts_with('U'));
}

#[test]
fn dedicated_english_state_resets_highlight_without_guessing_from_text() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    let text = runtime.view().editing_text;
    assert!(!runtime.view().dedicated_english);
    assert_eq!(runtime.view().page, 1);
    runtime.engine.dedicated_english = true;
    runtime.refresh().unwrap();
    assert!(runtime.view().dedicated_english);
    assert_eq!(runtime.view().page, 0);
    assert_eq!(runtime.view().editing_text, text);
    assert_eq!(runtime.view().local_mode, "none");
    runtime.engine.dedicated_english = false;
    runtime.refresh().unwrap();
    assert!(!runtime.view().dedicated_english);
}

#[test]
fn punctuation_host_context_uses_the_applied_runtime_state() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    assert!(runtime.punctuation_host_context_available(false));
    assert!(!runtime.punctuation_host_context_available(true));

    runtime.engine.dedicated_english = true;
    runtime.refresh().unwrap();
    assert!(!runtime.punctuation_host_context_available(false));

    runtime.engine.dedicated_english = false;
    runtime.engine.local_mode = "unicode".into();
    runtime.refresh().unwrap();
    assert!(!runtime.punctuation_host_context_available(false));
}

#[test]
fn switching_the_language_drops_the_composition_being_spelled() {
    // The source pairs `SetEnglishInputMode` with `ClearState`, and the engine does the same inside
    // `set_dedicated_english_mode`: letters spelled for Chinese are not what the user wants sitting
    // in an English composition. The hosts reach this through `msime_client_set_english_mode`, which
    // is what every mode-switch chord ends up calling -- Shift, a Ctrl tap, Ctrl+Alt+Space and
    // Ctrl+Shift+E alike.
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert!(!runtime.view().editing_text.is_empty());
    runtime.set_dedicated_english(true).unwrap();
    assert!(runtime.view().dedicated_english);
    assert_eq!(runtime.view().editing_text, "");
    // Setting the same mode again is not a change, so there is nothing to reset and nothing to lose.
    type_key(&mut runtime);
    let spelled = runtime.view().editing_text.clone();
    assert!(!spelled.is_empty());
    runtime.set_dedicated_english(true).unwrap();
    assert_eq!(runtime.view().editing_text, spelled);
}

#[test]
fn commit_context_precedes_mode_reset_for_every_selection_route() {
    for route in 0..5 {
        let mut runtime = runtime();
        runtime.focus(true).unwrap();
        runtime.engine.local_mode = "unicode".into();
        let view = type_key(&mut runtime).view;
        let id = view.candidates[0].id;
        let action = match route {
            0 => Action::Select(id),
            1 => Action::SelectEdge(id, CandidateEdge::FirstHan),
            2 => Action::SelectHighlighted,
            3 => Action::Finish,
            _ => Action::Character {
                value: b'1',
                shift: false,
            },
        };
        let committed = runtime.dispatch(action).unwrap();
        assert!(committed.commit.is_some());
        assert_eq!(committed.commit_context.unwrap().local_mode, "unicode");
        assert_eq!(committed.view.local_mode, "none");
    }
}

#[test]
fn finish_preserves_engine_completion_of_remaining_segments() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.dispatch(Action::NextPage).unwrap();
    let result = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(
        result.commit.as_deref(),
        Some("candidate-5-remaining-segments")
    );
    assert!(result.view.preedit.is_empty());
}
#[test]
fn stale_views_and_other_sessions_cannot_select() {
    let mut a = runtime();
    let mut b = runtime();
    a.focus(true).unwrap();
    b.focus(true).unwrap();
    let id = type_key(&mut a).view.candidates[0].id;
    type_key(&mut b);
    assert!(matches!(
        b.dispatch(Action::Select(id)),
        Err(RuntimeError::StaleCandidate)
    ));
    a.dispatch(Action::NextCandidate).unwrap();
    assert!(matches!(
        a.dispatch(Action::Select(id)),
        Err(RuntimeError::StaleCandidate)
    ));
}
#[test]
fn focus_changes_end_the_engine_context() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    assert_eq!(runtime.engine.context_resets, 1);
    type_key(&mut runtime);
    runtime.focus(false).unwrap();
    assert_eq!(runtime.engine.context_resets, 2);
    runtime.focus(true).unwrap();
    assert_eq!(runtime.engine.context_resets, 3);
}
#[test]
fn cache_maintenance_reaches_engine_without_acquiring_focus() {
    let mut runtime = runtime();
    let idle = runtime.dispatch(Action::ResetCache).unwrap();
    assert!(idle.handled);
    assert!(idle.commit.is_none());
    assert_eq!(runtime.engine.cache_resets, 1);
    assert!(!runtime.focused);
    assert!(!type_key(&mut runtime).handled);

    runtime.focus(true).unwrap();
    let composed = type_key(&mut runtime);
    let refreshed = runtime.dispatch(Action::ResetCache).unwrap();
    assert_eq!(runtime.engine.cache_resets, 2);
    assert_eq!(refreshed.view.preedit, composed.view.preedit);
    assert!(refreshed.commit.is_none());

    runtime.focus(false).unwrap();
    assert!(runtime.dispatch(Action::ResetCache).unwrap().handled);
    assert_eq!(runtime.engine.cache_resets, 3);
    assert!(!runtime.focused);
    assert!(!type_key(&mut runtime).handled);
}
#[test]
fn unfocused_keys_pass_through_and_blur_cancels_composition() {
    let mut runtime = runtime();
    assert!(!type_key(&mut runtime).handled);
    runtime.focus(true).unwrap();
    let id = type_key(&mut runtime).view.candidates[0].id;
    assert!(runtime.focus(false).unwrap().view.preedit.is_empty());
    assert!(!type_key(&mut runtime).handled);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert!(matches!(
        runtime.dispatch(Action::Select(id)),
        Err(RuntimeError::StaleCandidate)
    ));
}
#[test]
fn character_width_conversion_preserves_non_ascii_and_roundtrips_ascii() {
    let full = crate::character_width::to_fullwidth("A 1!");
    assert_eq!(full, "Ａ　１！");
    assert_eq!(crate::character_width::to_halfwidth(&full), "A 1!");
    assert_eq!(crate::character_width::to_fullwidth("中文"), "中文");
}

#[test]
fn rerank_context_that_fits_is_handed_over_whole() {
    // Short contexts are what the sentence eval measured, so they must reach the model untouched.
    assert_eq!(crate::rerank_context("你好世界", 64, 10), "你好世界");
    assert_eq!(crate::rerank_context("", 64, 10), "");
}

#[test]
fn rerank_context_holds_still_while_a_candidate_grows() {
    // A long context used to slide by one character per keystroke, which made the reranker rerun its prefix every time.
    let context: String = "今天天气很好我们一起去公园散步".repeat(8);
    let windows: Vec<&str> = (1..=40)
        .map(|longest| crate::rerank_context(&context, 64, longest))
        .collect();
    let mut distinct = windows.clone();
    distinct.dedup();
    assert!(
        distinct.len() <= 64 / crate::RERANK_CONTEXT_STEP + 1,
        "{}",
        distinct.len()
    );
    for (longest, window) in (1..=40).zip(&windows) {
        let count = window.chars().count();
        assert!(count + longest < 64, "longest {longest} kept {count}");
        assert_eq!(count % crate::RERANK_CONTEXT_STEP, 0);
        // Always the most recent text, never the start of it.
        assert!(context.ends_with(window));
    }
}

#[test]
fn rerank_context_cuts_on_a_character_boundary_and_can_empty() {
    let context = "a中b文".repeat(40);
    let window = crate::rerank_context(&context, 64, 20);
    assert_eq!(window.chars().count(), 32);
    assert!(context.ends_with(window));
    // A candidate that fills the window leaves no room, and the answer is an empty context rather than a panic.
    assert_eq!(crate::rerank_context(&context, 64, 70), "");
}

/// `move_to_back` is the whole of the demotion rule that can be tested without an engine, and
/// the version this replaced shipped with no test at all — which is how it reached `develop`
/// dropping Japanese katakana and, separately, the model's own runner-up choices.
#[test]
fn demotion_moves_flagged_items_to_the_end_and_keeps_both_orders() {
    let mut items = vec!["a", "b", "c", "d", "e"];
    crate::move_to_back(&mut items, &[false, true, false, true, false]);
    assert_eq!(items, vec!["a", "c", "e", "b", "d"]);
}

#[test]
fn in_place_order_applies_candidate_permutations() {
    let mut values = vec!["zero", "one", "two", "three", "four"];
    apply_order(&mut values, &[2, 4, 1, 0, 3]);
    assert_eq!(values, vec!["two", "four", "one", "zero", "three"]);
}

#[test]
fn demotion_loses_nothing() {
    // The point of moving rather than removing: every candidate is still reachable by paging.
    let mut items: Vec<u32> = (0..9).collect();
    crate::move_to_back(
        &mut items,
        &[false, true, true, false, true, false, false, true, true],
    );
    let mut sorted = items.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..9).collect::<Vec<u32>>());
    assert_eq!(items.len(), 9);
}

#[test]
fn demotion_with_no_flags_is_identity() {
    let mut items = vec![1, 2, 3];
    crate::move_to_back(&mut items, &[false, false, false]);
    assert_eq!(items, vec![1, 2, 3]);
}

#[test]
fn a_short_flag_list_leaves_the_tail_in_place() {
    // Defensive: the parallel arrays are length-checked before this runs, but a mismatch must
    // not reorder anything it was not told about.
    let mut items = vec![1, 2, 3, 4];
    crate::move_to_back(&mut items, &[true]);
    assert_eq!(items, vec![2, 3, 4, 1]);
}

/// Only `CandidateSource::Generated` names alternative readings of one key. Every other source
/// is plural by design — English words, emoji, kaomoji, quick phrases, AI suggestions — and an
/// earlier version of this rule kept one of each and dropped the rest.
#[test]
fn only_the_lattice_source_is_treated_as_alternative_readings() {
    assert_eq!(crate::LATTICE_SOURCE, 8);
    for plural in [2u8, 3, 4, 5, 6, 7] {
        assert_ne!(crate::LATTICE_SOURCE, plural);
    }
}

/// A runtime over an engine answering `rows` (text, source) in that order, with one key typed.
fn lattice_runtime(rows: &[(&str, u8)]) -> Runtime<Fixture> {
    let mut runtime = Runtime::new(
        Fixture {
            local_mode: "none".into(),
            words: rows.iter().map(|(text, _)| (*text).to_owned()).collect(),
            sources: rows.iter().map(|(_, source)| *source).collect(),
            codes: vec![String::new(); rows.len()],
            ..Fixture::default()
        },
        9,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime
}

fn candidate_texts(runtime: &mut Runtime<Fixture>) -> Vec<String> {
    runtime
        .all_candidates()
        .candidates
        .into_iter()
        .map(|candidate| candidate.text)
        .collect()
}

#[test]
fn a_long_sentence_keeps_three_readings_together_on_the_first_page() {
    let mut runtime = lattice_runtime(&[
        ("根据官方新闻稿", 8),
        ("根据", 0),
        ("跟", 0),
        ("根据官房新闻稿", 8),
        ("根据关防新闻稿", 8),
        ("根据官方新闻高", 8),
        ("根据官方新闻告", 8),
    ]);
    assert_eq!(
        candidate_texts(&mut runtime),
        [
            "根据官方新闻稿",
            "根据官房新闻稿",
            "根据关防新闻稿",
            "根据",
            "跟",
            "根据官方新闻高",
            "根据官方新闻告",
        ]
    );
}

#[test]
fn the_kept_readings_follow_the_first_one_wherever_it_sits() {
    let mut runtime = lattice_runtime(&[
        ("根据", 0),
        ("通过虚开发票", 8),
        ("跟", 0),
        ("通过需开发票", 8),
        ("通过虚开发飘", 8),
        ("通过需开发飘", 8),
    ]);
    assert_eq!(
        candidate_texts(&mut runtime),
        [
            "根据",
            "通过虚开发票",
            "通过需开发票",
            "通过虚开发飘",
            "跟",
            "通过需开发飘",
        ]
    );
}

#[test]
fn a_two_character_reading_keeps_only_the_first() {
    let mut runtime =
        lattice_runtime(&[("你好", 8), ("倪好", 8), ("你号", 8), ("你", 0), ("尼", 0)]);
    assert_eq!(
        candidate_texts(&mut runtime),
        ["你好", "你", "尼", "倪好", "你号"]
    );
}

#[test]
fn only_readings_as_wide_as_the_first_are_counted() {
    // A shorter lattice row is a reading of part of the key, not a runner-up for the whole of it.
    let mut runtime = lattice_runtime(&[
        ("根据官方新闻稿", 8),
        ("根据官方", 8),
        ("根据", 0),
        ("根据官房新闻稿", 8),
        ("根据关防新闻稿", 8),
        ("根据官方新闻高", 8),
    ]);
    assert_eq!(
        candidate_texts(&mut runtime),
        [
            "根据官方新闻稿",
            "根据官房新闻稿",
            "根据关防新闻稿",
            "根据官方",
            "根据",
            "根据官方新闻高",
        ]
    );
}

#[test]
fn single_character_lattice_rows_are_left_alone() {
    // Japanese kana: あ and ア are both Generated and both one character.
    let mut runtime = lattice_runtime(&[("あ", 8), ("ア", 8), ("阿", 0)]);
    assert_eq!(candidate_texts(&mut runtime), ["あ", "ア", "阿"]);
}

#[test]
fn a_regrouped_reading_commits_what_its_seat_shows() {
    let mut runtime = lattice_runtime(&[
        ("根据官方新闻稿", 8),
        ("根据", 0),
        ("根据官房新闻稿", 8),
        ("根据关防新闻稿", 8),
        ("根据官方新闻高", 8),
    ]);
    let view = runtime.view();
    assert_eq!(view.candidates[1].text, "根据官房新闻稿");
    assert_eq!(view.candidates[3].text, "根据");
    let transition = runtime
        .dispatch(Action::Select(view.candidates[1].id))
        .unwrap();
    assert_eq!(transition.commit.as_deref(), Some("根据官房新闻稿"));

    let mut runtime = lattice_runtime(&[
        ("根据官方新闻稿", 8),
        ("根据", 0),
        ("根据官房新闻稿", 8),
        ("根据关防新闻稿", 8),
        ("根据官方新闻高", 8),
    ]);
    let view = runtime.view();
    let transition = runtime
        .dispatch(Action::Select(view.candidates[3].id))
        .unwrap();
    assert_eq!(transition.commit.as_deref(), Some("根据"));
}

#[test]
fn regrouping_twice_changes_nothing() {
    // The settle pass runs the same regrouping over a list that already went through it.
    let mut runtime = lattice_runtime(&[
        ("根据官方新闻稿", 8),
        ("根据", 0),
        ("根据官房新闻稿", 8),
        ("根据关防新闻稿", 8),
        ("根据官方新闻高", 8),
    ]);
    let before = candidate_texts(&mut runtime);
    assert!(!runtime.demote_runner_up_readings());
    assert_eq!(candidate_texts(&mut runtime), before);
}

/// Records every rescoring context the runtime hands over.
struct RecordsContext {
    inner: Fixture,
    contexts: Vec<String>,
}

impl InputEngine for RecordsContext {
    fn set_rescoring_context(&mut self, context: &str) {
        self.contexts.push(context.to_owned());
    }
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        self.inner.snapshot()
    }
    fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult, RuntimeError> {
        self.inner.character(value, shift)
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        self.inner.command(command)
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.inner.select(index)
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.inner.select_edge(index, edge)
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.inner.finish(index)
    }
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        self.inner.punctuation(value)
    }
}

fn recording_engine() -> RecordsContext {
    RecordsContext {
        inner: Fixture {
            local_mode: "none".into(),
            words: vec!["会议".into(), "回忆".into()],
            ..Fixture::default()
        },
        contexts: Vec::new(),
    }
}

#[test]
fn committed_text_reaches_the_engine_rescoring_context() {
    let mut runtime = Runtime::new(recording_engine(), 5).unwrap();
    runtime.focus(true).unwrap();
    assert_eq!(runtime.engine.contexts.last().map(String::as_str), Some(""));

    runtime.seed_context("明天开");
    assert_eq!(
        runtime.engine.contexts.last().map(String::as_str),
        Some("明天开")
    );

    runtime
        .dispatch(Action::Character {
            value: b'h',
            shift: false,
        })
        .unwrap();
    let transition = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(transition.commit.as_deref(), Some("会议"));
    assert_eq!(
        runtime.engine.contexts.last().map(String::as_str),
        Some("明天开会议")
    );
    assert_eq!(runtime.ai_context, "明天开会议");

    // Leaving the client ends the sentence for the Engine's models as it does for the AI provider.
    runtime.focus(false).unwrap();
    assert_eq!(runtime.engine.contexts.last().map(String::as_str), Some(""));
}

#[test]
fn clearing_the_context_starts_the_next_seed_from_nothing() {
    let mut runtime = Runtime::new(recording_engine(), 5).unwrap();
    runtime.focus(true).unwrap();
    runtime.seed_context("第一句");
    // Cancel keeps what was committed, which is why a per-case seed needs the explicit clear.
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert_eq!(runtime.ai_context, "第一句");

    runtime.clear_context();
    assert!(runtime.ai_context.is_empty());
    assert_eq!(runtime.engine.contexts.last().map(String::as_str), Some(""));

    runtime.seed_context("第二句");
    assert_eq!(runtime.ai_context, "第二句");
    assert_eq!(
        runtime.engine.contexts.last().map(String::as_str),
        Some("第二句")
    );
}

#[test]
fn a_replacement_engine_inherits_the_committed_text() {
    let mut runtime = Runtime::new(recording_engine(), 5).unwrap();
    runtime.focus(true).unwrap();
    runtime.seed_context("上一句");
    runtime.replace_engine(recording_engine(), 5).unwrap();
    assert_eq!(runtime.engine.contexts, ["上一句"]);
}

// The real Engine, not the fixture. Unicode mode is the one place where a bare
// digit is input rather than a candidate index, and the two layers decide that
// separately: the Engine reports the digit as handled, and the runtime only
// falls through to selection for a digit the Engine refused. A regression in
// either one silently turns "U4e2d" into a candidate pick, and the Windows and
// macOS suites that would notice both need their own host to run.
fn real_engine_options(root: &std::path::Path) -> msime_engine::host::EngineOptions {
    let path = |name: &str| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path.to_str().unwrap().to_owned()
    };
    msime_engine::host::EngineOptions {
        resources: path("resources"),
        user_data: path("user"),
        cache: path("cache"),
        dictionaries: path("dictionaries"),
        scheme: 0,
        shuangpin_profile: 0,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: true,
        autocorrect_neighbor: true,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_profile: 0,
        helpcode: false,
        show_helpcode: true,
        helpcode_schema: "ziranma".into(),
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        frequency_mode: "promote".into(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_english: true,
        english_minimum_prefix: 2,
        mixed_emoji: false,
        mixed_kaomoji: false,
        local_unicode: true,
        local_date_time: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        local_expression: false,
        local_command: false,
        local_mention: false,
        command_table: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        helpcode_table: None,
        sentence_association: msime_engine::host::SentenceAssociationOptions {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        },
        rescoring_context: String::new(),
        sentence_alternatives: true,
        vietnamese_input_method: 0,
        vietnamese_tone_style: 0,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
    }
}

#[test]
fn unicode_mode_digits_compose_a_code_point_rather_than_picking_a_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let session = msime_engine::host::Session::new(&real_engine_options(directory.path())).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();

    let shift_u = runtime
        .dispatch(Action::Character {
            value: b'U',
            shift: true,
        })
        .unwrap();
    assert_eq!(shift_u.view.local_mode, "unicode");

    for value in *b"4e2d" {
        let transition = runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
        // A digit read as a candidate index would commit here and leave the mode.
        assert!(
            transition.commit.is_none(),
            "{} committed instead of extending the code point",
            value as char
        );
        assert_eq!(transition.view.local_mode, "unicode");
    }
    assert_eq!(runtime.view().editing_text, "U4e2d");
    assert!(runtime
        .view()
        .candidates
        .iter()
        .any(|candidate| candidate.text == "中"));
}

/// Korean syllables commit themselves as the next one starts, and every way out of a syllable - Space, Enter, a digit, punctuation, leaving the client - commits it rather than dropping it. With no candidates, the navigation keys have nothing to do and go back to the host.
#[test]
fn korean_syllables_commit_through_the_runtime_without_candidates() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = real_engine_options(directory.path());
    options.scheme = KOREAN_SCHEME;
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    let character = |runtime: &mut Runtime, value: u8| {
        runtime
            .dispatch(Action::Character {
                value,
                shift: value.is_ascii_uppercase(),
            })
            .unwrap()
    };

    let mut committed = String::new();
    for value in *b"dkssud" {
        let transition = character(&mut runtime, value);
        assert!(transition.handled);
        committed.push_str(transition.commit.as_deref().unwrap_or_default());
    }
    assert_eq!(committed, "안");
    let view = runtime.view();
    assert_eq!(view.scheme, KOREAN_SCHEME);
    assert_eq!(runtime.scheme(), KOREAN_SCHEME);
    assert_eq!(view.preedit, "녕");
    assert_eq!(view.reading, "녕");
    assert_eq!(view.editing_text, "sud");
    assert!(view.candidates.is_empty());
    assert_eq!(view.page_count, 0);
    assert!(runtime.online_query().unwrap().is_none());
    assert!(!runtime.punctuation_host_context_available(false));

    // Candidate navigation has no list to move through and does not eat the key.
    for action in [
        Action::NextPage,
        Action::PreviousPage,
        Action::NextCandidate,
        Action::PreviousCandidate,
        Action::FirstCandidate,
        Action::LastCandidate,
    ] {
        let transition = runtime.dispatch(action).unwrap();
        assert!(!transition.handled);
        assert!(transition.commit.is_none());
        assert_eq!(transition.view.preedit, "녕");
    }

    // Space picks the highlighted candidate elsewhere; here it commits the syllable and passes through.
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(!space.handled);
    assert_eq!(space.commit.as_deref(), Some("녕"));
    assert_eq!(
        space.commit_context.as_ref().map(|context| context.scheme),
        Some(KOREAN_SCHEME)
    );
    assert_eq!(space.view.editing_text, "");

    // A digit is not a candidate key: it commits the syllable and the host inserts it.
    character(&mut runtime, b'r');
    character(&mut runtime, b'k');
    let digit = character(&mut runtime, b'1');
    assert!(!digit.handled);
    assert_eq!(digit.commit.as_deref(), Some("가"));

    // Punctuation typed as a character goes the punctuation route and stays ASCII.
    character(&mut runtime, b'r');
    character(&mut runtime, b'k');
    let period = character(&mut runtime, b'.');
    assert!(period.handled);
    assert_eq!(period.commit.as_deref(), Some("가."));
    character(&mut runtime, b'r');
    character(&mut runtime, b'k');
    let comma = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(comma.commit.as_deref(), Some("가,"));
    let idle = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert!(!idle.handled && idle.commit.is_none());

    // Enter commits and passes through; Escape discards.
    character(&mut runtime, b'R');
    character(&mut runtime, b'k');
    let enter = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert!(!enter.handled);
    assert_eq!(enter.commit.as_deref(), Some("까"));
    character(&mut runtime, b'r');
    let escape = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(escape.handled && escape.commit.is_none());

    // Leaving the client commits the open syllable.
    character(&mut runtime, b'g');
    character(&mut runtime, b'k');
    let left = runtime.focus(false).unwrap();
    assert_eq!(left.commit.as_deref(), Some("하"));
    assert_eq!(left.view.editing_text, "");
    // Nothing composing: leaving commits nothing.
    runtime.focus(true).unwrap();
    assert!(runtime.focus(false).unwrap().commit.is_none());
    // Attaching a new client discards a syllable left open in the previous one instead of writing it into the new client.
    runtime.focus(true).unwrap();
    character(&mut runtime, b'g');
    character(&mut runtime, b'k');
    let attached = runtime.focus(true).unwrap();
    assert!(attached.commit.is_none());
    assert_eq!(attached.view.editing_text, "");
}

/// Tab and Shift+Tab page the candidate list, so with no candidates on screen the paging actions must not eat the key: the host has to insert the Tab or move focus as it would without the input method.
#[test]
fn idle_paging_passes_through_on_pinyin() {
    let assert_passes_through = |runtime: &mut Runtime<Fixture>| {
        for action in [Action::NextPage, Action::PreviousPage] {
            let transition = runtime.dispatch(action).unwrap();
            assert!(!transition.handled);
            assert!(transition.commit.is_none());
            assert!(transition.view.candidates.is_empty());
            assert_eq!(transition.view.preedit, "");
            assert_eq!(transition.view.reading, "");
            assert_eq!(transition.view.editing_text, "");
        }
    };

    // Focused with nothing typed.
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    assert_eq!(runtime.view().scheme, 0);
    assert_passes_through(&mut runtime);

    // Right after a commit the list is gone again, and paging must not reach back into it.
    let typed = type_key(&mut runtime);
    assert!(typed.handled);
    assert!(!typed.view.candidates.is_empty());
    let committed = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("candidate-0"));
    assert!(committed.view.candidates.is_empty());
    assert_eq!(committed.view.reading, "");
    assert_passes_through(&mut runtime);
}

/// A host that draws half-composed phrases itself still receives every finished Korean syllable as a commit: the syllable is text, not a piece of a phrase.
#[test]
fn korean_syllables_are_never_held_as_a_phrase_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = real_engine_options(directory.path());
    options.scheme = KOREAN_SCHEME;
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    assert_eq!(runtime.set_phrase_preedit(true), None);
    runtime.focus(true).unwrap();
    let mut commits = Vec::new();
    for value in *b"rksk" {
        let transition = runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
        assert!(transition.handled);
        assert_eq!(transition.view.phrase_prefix, "");
        commits.extend(transition.commit);
    }
    assert_eq!(commits, ["가"]);
    assert_eq!(runtime.view().preedit, "나");
}

fn korean_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
    options.scheme = KOREAN_SCHEME;
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// Types `keys` and opens the Hanja list of the syllable they leave composing.
fn open_korean_hanja(runtime: &mut Runtime, keys: &str) -> Transition {
    for value in keys.bytes() {
        runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
    }
    let opened = runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    assert!(opened.handled && opened.commit.is_none(), "{keys}");
    opened
}

fn texts(view: &View) -> Vec<String> {
    view.candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

/// The Hanja list pages and navigates like any candidate list: Space takes the highlighted row, a digit the row on the visible page, and the arrows move the highlight. The table's order is kept.
#[test]
fn a_korean_hanja_list_pages_and_selects_through_the_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = korean_runtime(directory.path());
    let opened = open_korean_hanja(&mut runtime, "gks");
    assert_eq!(texts(&opened.view)[..3], ["韓", "漢", "寒"]);
    assert_eq!(
        opened.view.candidates[0].annotation,
        "나라 이름 한, 한나라 한"
    );
    assert_eq!(opened.view.preedit, "한");
    assert!(opened.view.page_count > 2);
    assert!(opened.view.candidates[0].highlighted);

    // Next and previous candidate move the highlight; Space commits it.
    runtime.dispatch(Action::NextCandidate).unwrap();
    let moved = runtime.dispatch(Action::NextCandidate).unwrap();
    assert!(moved.handled);
    assert!(moved.view.candidates[2].highlighted);
    runtime.dispatch(Action::PreviousCandidate).unwrap();
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(space.handled);
    assert_eq!(space.commit.as_deref(), Some("漢"));
    assert_eq!(
        space.commit_context.as_ref().map(|context| context.scheme),
        Some(KOREAN_SCHEME)
    );
    assert_eq!(space.view.editing_text, "");
    assert!(space.view.candidates.is_empty());

    // A digit picks from the page on screen, so the Hangul is not committed first and the choice is not lost.
    open_korean_hanja(&mut runtime, "gks");
    let paged = runtime.dispatch(Action::NextPage).unwrap();
    assert_eq!(paged.view.page, 1);
    let second_on_page = paged.view.candidates[1].text.clone();
    let digit = runtime
        .dispatch(Action::Character {
            value: b'2',
            shift: false,
        })
        .unwrap();
    assert!(digit.handled);
    assert_eq!(digit.commit.as_deref(), Some(second_on_page.as_str()));
    assert_eq!(digit.view.editing_text, "");

    // A digit past the end of the page is swallowed and the syllable keeps composing.
    open_korean_hanja(&mut runtime, "rmf");
    let outside = runtime
        .dispatch(Action::Character {
            value: b'9',
            shift: false,
        })
        .unwrap();
    assert!(outside.handled && outside.commit.is_none());
    assert_eq!(outside.view.preedit, "글");
    // 0 is not a selection: it commits the Hangul and goes to the host.
    let zero = runtime
        .dispatch(Action::Character {
            value: b'0',
            shift: false,
        })
        .unwrap();
    assert!(!zero.handled);
    assert_eq!(zero.commit.as_deref(), Some("글"));
}

/// Escape and the trigger close the list and keep the syllable; a letter closes it and keeps composing. Every way of ending without a choice - punctuation, the host's finish key, leaving the client - commits the Hangul, whatever row is highlighted.
#[test]
fn closing_or_finishing_a_korean_hanja_list_keeps_the_hangul() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = korean_runtime(directory.path());

    open_korean_hanja(&mut runtime, "gks");
    let escape = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(escape.handled && escape.commit.is_none());
    assert_eq!(escape.view.preedit, "한");
    assert!(escape.view.candidates.is_empty());
    let toggled = runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    assert!(!toggled.view.candidates.is_empty());
    let closed = runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    assert!(closed.handled && closed.view.candidates.is_empty());
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    open_korean_hanja(&mut runtime, "gk");
    let letter = runtime
        .dispatch(Action::Character {
            value: b'r',
            shift: false,
        })
        .unwrap();
    assert!(letter.handled && letter.commit.is_none());
    assert_eq!(letter.view.preedit, "학");
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert_eq!(runtime.view().editing_text, "");

    // Punctuation with the second row highlighted.
    open_korean_hanja(&mut runtime, "gks");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let period = runtime.dispatch(Action::Punctuation(b'.')).unwrap();
    assert!(period.handled);
    assert_eq!(period.commit.as_deref(), Some("한."));
    assert!(period.view.candidates.is_empty());

    // The host's finish key with the second row highlighted.
    open_korean_hanja(&mut runtime, "gks");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let finished = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(finished.commit.as_deref(), Some("한"));
    assert!(finished.view.candidates.is_empty());

    // Leaving the client.
    open_korean_hanja(&mut runtime, "gks");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let left = runtime.focus(false).unwrap();
    assert_eq!(left.commit.as_deref(), Some("한"));
    assert_eq!(left.view.editing_text, "");
    assert!(left.view.candidates.is_empty());
}

/// Attaching a client discards what was composing in the previous one. A Cancel with the Hanja list open only closes the list, so the discard must not stop there and carry the syllable into the new client.
#[test]
fn attaching_a_client_discards_a_syllable_whose_hanja_list_is_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = korean_runtime(directory.path());
    open_korean_hanja(&mut runtime, "gks");
    let attached = runtime.focus(true).unwrap();
    assert!(attached.commit.is_none());
    assert_eq!(attached.view.editing_text, "");
    assert_eq!(attached.view.preedit, "");
    assert!(attached.view.candidates.is_empty());
    // The next key starts from nothing rather than finishing 한 into the new client.
    let typed = runtime
        .dispatch(Action::Character {
            value: b'r',
            shift: false,
        })
        .unwrap();
    assert!(typed.commit.is_none());
    assert_eq!(typed.view.preedit, "ㄱ");
}

/// The model reorders Chinese candidates; a Hanja list is a table in frequency order for one syllable and keeps that order, as does the runner-up demotion that only lattice readings are for.
#[test]
fn korean_lists_are_never_reranked_or_demoted() {
    let reordered = |scheme: u8, words: &[&str], sources: Vec<u8>, model: Option<SentenceModel>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme,
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).to_owned()).collect(),
                codes: vec!["gks".into(); words.len()],
                sources,
                ..Fixture::default()
            },
            5,
        )
        .unwrap();
        if let Some(model) = model {
            runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
        }
        runtime.focus(true).unwrap();
        runtime
            .dispatch(Action::Character {
                value: b'g',
                shift: false,
            })
            .unwrap();
        texts(&runtime.view())
    };
    let hanja = ["韓", "漢", "寒"];
    let favours_cold = || Some(favouring_model(&['韓', '漢', '寒'], &['寒']));
    // The same rows under a pinyin scheme are reranked, so the model would move 寒 up if Korean let it.
    assert_eq!(
        reordered(0, &hanja, vec![LATTICE_SOURCE; 3], favours_cold()),
        ["寒", "韓", "漢"]
    );
    assert_eq!(
        reordered(
            KOREAN_SCHEME,
            &hanja,
            vec![LATTICE_SOURCE; 3],
            favours_cold()
        ),
        hanja
    );
    // Lattice-sourced sentence rows are demoted behind the first under a pinyin scheme and left alone under Korean.
    // The runtime pages five rows, so the view is the first page.
    let sentences = ["韓國語", "漢國語", "寒國語", "閑國語", "限國語", "國", "語"];
    let sources = || {
        let mut sources = vec![LATTICE_SOURCE; 5];
        sources.extend([0, 0]);
        sources
    };
    assert_eq!(
        reordered(0, &sentences, sources(), None),
        ["韓國語", "漢國語", "寒國語", "國", "語"]
    );
    assert_eq!(
        reordered(KOREAN_SCHEME, &sentences, sources(), None),
        sentences[..5]
    );
}

/// An Engine whose digits end the composition with a commit and are left to the host, as Korean digits are with the Hanja list closed, while it still shows candidates.
struct DigitCommitsEngine {
    reading: String,
}

impl InputEngine for DigitCommitsEngine {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        let words: Vec<String> = if self.reading.is_empty() {
            Vec::new()
        } else {
            vec!["甲".into(), "乙".into()]
        };
        let count = words.len();
        Ok(EngineSnapshot {
            scheme: KOREAN_SCHEME,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            candidate_codes: vec![self.reading.clone(); count],
            candidate_annotations: vec![String::new(); count],
            candidate_sources: vec![0; count],
            candidate_positions: vec![0; count],
            candidate_corrected: vec![false; count],
            candidate_answers_key: vec![true; count],
            candidate_list_open: false,
            microsoft_shuangpin: false,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: false,
            local_mode: "none".into(),
            spelling_symbols: String::new(),
            dedicated_english: false,
            preedit: self.reading.clone(),
            reading: self.reading.clone(),
            editing_text: self.reading.clone(),
            caret_position: self.reading.len(),
            segment_raw_boundaries: Vec::new(),
            candidates: words,
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        if value.is_ascii_digit() {
            return Ok(EngineResult {
                handled: false,
                has_commit: true,
                commit: std::mem::take(&mut self.reading),
                diagnostic: String::new(),
            });
        }
        self.reading.push(value as char);
        Ok(empty_result(true))
    }
    fn command(&mut self, _command: Command) -> Result<EngineResult, RuntimeError> {
        self.reading.clear();
        Ok(empty_result(true))
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.reading.clear();
        Ok(EngineResult {
            handled: true,
            has_commit: true,
            commit: ["甲", "乙"][index].to_owned(),
            diagnostic: String::new(),
        })
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
    fn punctuation(&mut self, _value: u8) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn select_edge(
        &mut self,
        index: usize,
        _edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
}

/// A digit the Engine already answered with a commit is not also a page selection: selecting would replace the commit, and the text it carried - a Korean syllable - would be lost.
#[test]
fn a_digit_that_already_committed_is_not_also_a_selection() {
    let mut runtime = Runtime::new(
        DigitCommitsEngine {
            reading: String::new(),
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    runtime
        .dispatch(Action::Character {
            value: b'x',
            shift: false,
        })
        .unwrap();
    assert_eq!(runtime.view().candidates.len(), 2);
    let digit = runtime
        .dispatch(Action::Character {
            value: b'1',
            shift: false,
        })
        .unwrap();
    assert!(!digit.handled);
    assert_eq!(digit.commit.as_deref(), Some("x"));
    assert!(digit.view.candidates.is_empty());
}

/// A digit the scheme spells with is never a page selection, even when the Engine lets it go with candidates showing; outside a local mode the spelling symbols are the scheme's own keys (a Zhuyin tone or phonetic key, a VNI mark).
#[test]
fn a_spelling_digit_the_engine_let_go_is_not_a_selection() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    runtime.engine.spelling_symbols = "0123456789".into();
    for value in *b"ab" {
        runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
    }
    assert!(!runtime.view().candidates.is_empty());
    let digit = runtime
        .dispatch(Action::Character {
            value: b'2',
            shift: false,
        })
        .unwrap();
    assert!(!digit.handled);
    assert!(digit.commit.is_none());
    assert_eq!(digit.view.editing_text, "ab");

    // The same digit picks the row once the scheme no longer spells with it; the next key's snapshot is where the runtime learns that.
    runtime.engine.spelling_symbols.clear();
    runtime
        .dispatch(Action::Character {
            value: b'c',
            shift: false,
        })
        .unwrap();
    let picked = runtime
        .dispatch(Action::Character {
            value: b'2',
            shift: false,
        })
        .unwrap();
    assert_eq!(picked.commit.as_deref(), Some("candidate-1"));
}

fn generated_mode_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
    options.local_expression = true;
    options.local_command = true;
    options.local_mention = true;
    options.mention_entries = vec![msime_engine::host::MentionEntry {
        text: "张三".into(),
        key: "zhang'san".into(),
    }];
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

fn character(runtime: &mut Runtime, value: u8) -> Transition {
    runtime
        .dispatch(Action::Character {
            value,
            shift: value.is_ascii_uppercase(),
        })
        .unwrap()
}

#[test]
fn mention_places_switch_on_live_and_carry_their_parent() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    character(&mut runtime, b'@');
    for value in *b"shenzhen" {
        character(&mut runtime, value);
    }
    assert!(runtime
        .view()
        .candidates
        .iter()
        .all(|candidate| candidate.text != "深圳市"));
    runtime.set_mention_places(true).unwrap();
    let view = runtime.view();
    assert_eq!(view.candidates[0].text, "深圳市");
    assert_eq!(view.candidates[0].annotation, "广东省");
    assert_eq!(view.candidates[0].code, "shen'zhen'shi");
    runtime.set_mention_places(false).unwrap();
    assert!(runtime
        .view()
        .candidates
        .iter()
        .all(|candidate| candidate.text != "深圳市"));
}

// The operators of the expression mode arrive as punctuation on hosts that classify Shift+= and Shift+8 that way. The runtime finishes a composition before it translates punctuation, which here would commit the half-typed expression; the Engine's `spelling_symbols` is what routes them back to the Engine as characters.
#[test]
fn expression_operators_sent_as_punctuation_extend_the_expression() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    let entered = character(&mut runtime, b'V');
    assert_eq!(entered.view.local_mode, "expression");
    assert_eq!(entered.view.spelling_symbols, "0123456789+-*/.()%^");
    character(&mut runtime, b'1');
    for (action, value) in [
        (Action::Punctuation(b'+'), b'2'),
        (Action::PunctuationAscii(b'*'), b'3'),
        (Action::Punctuation(b'('), b'4'),
    ] {
        let transition = runtime.dispatch(action).unwrap();
        assert!(transition.handled && transition.commit.is_none());
        assert_eq!(transition.view.local_mode, "expression");
        // A digit is input here, never a candidate shortcut.
        let transition = character(&mut runtime, value);
        assert!(transition.commit.is_none(), "{value} picked a candidate");
    }
    runtime.dispatch(Action::Punctuation(b')')).unwrap();
    assert_eq!(runtime.view().editing_text, "V1+2*3(4)");
    assert!(runtime.online_query().unwrap().is_none());

    // A mark the mode does not spell with still ends the composition.
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    character(&mut runtime, b'V');
    for value in *b"2*3" {
        character(&mut runtime, value);
    }
    assert_eq!(runtime.view().candidates[0].text, "6");
    let committed = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("6，"));
    let context = committed.commit_context.unwrap();
    assert_eq!(context.local_mode, "expression");
    assert!(!context.typing_statistics);
    assert_eq!(committed.view.local_mode, "none");
}

// The apostrophe is no spelling symbol, so a host may send it as punctuation; after a unit it separates the target instead of finishing the expression.
#[test]
fn an_apostrophe_after_a_unit_separates_the_target_on_every_route() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    for action in [
        Action::Punctuation(b'\''),
        Action::PunctuationAscii(b'\''),
        Action::Character {
            value: b'\'',
            shift: false,
        },
    ] {
        character(&mut runtime, b'V');
        for value in *b"3jin" {
            character(&mut runtime, value);
        }
        let separated = runtime.dispatch(action).unwrap();
        assert!(separated.handled && separated.commit.is_none());
        character(&mut runtime, b'g');
        assert_eq!(runtime.view().editing_text, "V3jin'g");
        assert_eq!(runtime.view().candidates[0].text, "1500克");
        assert!(runtime.online_query().unwrap().is_none());
        assert!(runtime.command_translation().is_none());
        runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    }
    // Before any unit the apostrophe is still the mark that ends the expression.
    character(&mut runtime, b'V');
    character(&mut runtime, b'3');
    let highlighted = runtime.view().candidates[0].text.clone();
    let committed = runtime.dispatch(Action::PunctuationAscii(b'\'')).unwrap();
    assert_eq!(committed.commit, Some(format!("{highlighted}'")));
    assert_eq!(committed.view.local_mode, "none");
}

// `/fy` hands its English to the host as a request of its own and takes the answer back as a row that commits it, under the runtime's generation guard.
#[test]
fn the_translate_command_round_trips_through_the_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    character(&mut runtime, b'/');
    assert!(runtime.command_translation().is_none());
    for value in *b"fyhello" {
        character(&mut runtime, value);
    }
    runtime.dispatch(Action::Punctuation(b'\'')).unwrap();
    for value in *b"world" {
        character(&mut runtime, value);
    }
    assert!(runtime.online_query().unwrap().is_none());
    let query = runtime
        .command_translation()
        .expect("a translation request");
    assert_eq!(query.text, "hello world");
    assert_eq!(query.generation, runtime.generation());

    let mut stale = query.clone();
    stale.generation -= 1;
    assert!(!runtime
        .apply_command_translation(&stale, "你好世界")
        .unwrap());
    assert!(!runtime.apply_command_translation(&query, "").unwrap());
    let before = runtime.view().candidates[0].id;
    assert!(runtime
        .apply_command_translation(&query, "你好世界")
        .unwrap());
    let view = runtime.view();
    assert_eq!(view.candidates[0].text, "你好世界");
    assert_eq!(view.candidates[0].annotation, "翻译");
    assert_eq!(view.candidates[1].text, "hello world");
    assert!(view.candidates[0].translation.is_none());
    // The identity moved on, so the old page's first ID cannot pick the new first row.
    assert_ne!(view.candidates[0].id, before);
    // A second answer for the same request is stale.
    assert!(!runtime.apply_command_translation(&query, "你好").unwrap());
    // Answered, the text is not asked for again when the host plans requests for the new view.
    assert!(runtime.command_translation().is_none());
    let picked = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(picked.commit.as_deref(), Some("你好世界"));
    assert_eq!(picked.view.local_mode, "none");

    // A host that forwards the request through its candidate translation path hands the answer back the same way, and it still becomes the row rather than a gloss.
    character(&mut runtime, b'/');
    for value in *b"fycat" {
        character(&mut runtime, value);
    }
    let query = runtime
        .command_translation()
        .expect("a translation request");
    assert!(runtime.apply_translations(query.generation, [("cat".to_owned(), "猫".to_owned())]));
    let view = runtime.view();
    assert_eq!(view.candidates[0].text, "猫");
    assert!(view.candidates.iter().all(|row| row.translation.is_none()));
    // Glosses for other text stay glosses.
    let generation = runtime.generation();
    assert!(runtime.apply_translations(generation, [("猫".to_owned(), "cat".to_owned())]));
    assert_eq!(
        runtime.view().candidates[0].translation.as_deref(),
        Some("cat")
    );
}

// A mark on a bare `/` or `@` is punctuation on every route: the mode ends and nothing from its list is committed.
#[test]
fn a_mark_on_a_bare_slash_or_at_is_not_a_pick() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    for (open, action, expected) in [
        (b'/', Action::Punctuation(b'/'), "//"),
        (b'/', Action::Punctuation(b','), "/，"),
        (b'/', Action::PunctuationAscii(b','), "/,"),
        (
            b'/',
            Action::Character {
                value: b'/',
                shift: false,
            },
            "//",
        ),
        (b'@', Action::Punctuation(b'@'), "@@"),
    ] {
        assert_eq!(character(&mut runtime, open).view.editing_text.len(), 1);
        let ended = runtime.dispatch(action).unwrap();
        assert!(ended.handled);
        assert_eq!(ended.commit.as_deref(), Some(expected));
        assert_eq!(ended.view.local_mode, "none");
    }
    // Space still takes the first row, and Enter the literal prefix.
    character(&mut runtime, b'/');
    let first = runtime.view().candidates[0].text.clone();
    let picked = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(picked.commit, Some(first));
    character(&mut runtime, b'/');
    let raw = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(raw.commit.as_deref(), Some("/"));
    // A host flush (focus moving, the host inserting text) keeps the literal prefix too.
    for open in *b"/@" {
        character(&mut runtime, open);
        let flushed = runtime.dispatch(Action::Finish).unwrap();
        assert_eq!(
            flushed.commit.as_deref(),
            Some(&*char::from(open).to_string())
        );
        assert_eq!(flushed.view.local_mode, "none");
    }
}

// Whether `/` and `@` open a mode follows the punctuation mode, so the view the host and the punctuation route read changes with it at once rather than at the next key.
#[test]
fn punctuation_mode_changes_refresh_the_mode_symbols() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    assert_eq!(runtime.view().spelling_symbols, "/@");
    runtime.set_chinese_punctuation_enabled(false).unwrap();
    assert!(runtime.view().spelling_symbols.is_empty());
    runtime.set_chinese_punctuation_enabled(true).unwrap();
    assert_eq!(runtime.view().spelling_symbols, "/@");
    runtime.set_punctuation_lock(2).unwrap();
    assert!(runtime.view().spelling_symbols.is_empty());
    runtime.set_punctuation_lock(0).unwrap();
    let opened = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    assert_eq!(opened.view.local_mode, "command");
}

#[test]
fn slash_and_at_open_their_modes_only_with_nothing_composed() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = generated_mode_runtime(directory.path());
    assert_eq!(runtime.view().spelling_symbols, "/@");

    // The explicit punctuation route opens the mode as the character route does.
    let opened = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    assert!(opened.handled && opened.commit.is_none());
    assert_eq!(opened.view.local_mode, "command");
    assert!(opened.view.spelling_symbols.is_empty());
    assert!(!opened.view.candidates.is_empty());
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();

    let opened = character(&mut runtime, b'@');
    assert_eq!(opened.view.local_mode, "mention");
    assert_eq!(opened.view.candidates[0].text, "张三");
    let committed = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("张三"));
    assert!(!committed.commit_context.unwrap().typing_statistics);

    // A literal ASCII mark the host chose after weighing the surrounding text never opens a mode.
    let literal = runtime.dispatch(Action::PunctuationAscii(b'/')).unwrap();
    assert_eq!(literal.view.local_mode, "none");

    // With a composition `/` is punctuation: the composition is committed with the mark after it.
    for value in *b"ab" {
        character(&mut runtime, value);
    }
    assert!(runtime.view().spelling_symbols.is_empty());
    let finished = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    let commit = finished.commit.unwrap();
    assert!(commit.ends_with('/'), "{commit:?}");
    assert_eq!(finished.view.local_mode, "none");
    assert!(finished.commit_context.unwrap().typing_statistics);
}

/// Without a settled model attached, the settle call is inert.
///
/// This is the shape every installation that ships one model is in, and the one where a mistake
/// would be invisible: a settle pass that quietly reordered candidates using the fast model would
/// look like the candidate window moving on its own after the user stopped typing.
#[test]
fn settling_without_a_second_model_changes_nothing() {
    let mut runtime = runtime();
    runtime.focus(true).expect("focus");
    for byte in b"nihao" {
        runtime
            .dispatch(Action::Character {
                value: *byte,
                shift: false,
            })
            .expect("type");
    }
    let before: Vec<String> = runtime
        .view()
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect();
    assert!(!runtime.rerank_settled(), "no settled model, nothing to do");
    let after: Vec<String> = runtime
        .view()
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect();
    assert_eq!(after, before);
}

/// An idle session has no candidates to settle on, and asking is not an error.
#[test]
fn settling_while_idle_is_inert() {
    let mut runtime = runtime();
    runtime.focus(true).expect("focus");
    assert!(!runtime.rerank_settled());
}

/// The desktop sentence model switch gates the settled rerank without detaching the model: off, a settle leaves the list and its generation alone; back on, the same attached model reorders. Needs a shipped sentence model in `MSIME_NEURAL_MODEL_DIR` or `MSIME_EVAL_RESOURCES` (the desktop one, else the keyboard one standing in).
#[test]
fn the_desktop_switch_gates_the_settled_rerank() {
    let directories: Vec<std::path::PathBuf> = ["MSIME_NEURAL_MODEL_DIR", "MSIME_EVAL_RESOURCES"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(Into::into)
        .collect();
    let Some(model) = [
        "sentence-model-desktop.safetensors",
        "sentence-model.safetensors",
    ]
    .into_iter()
    .flat_map(|name| {
        directories
            .iter()
            .map(move |directory| directory.join(name))
    })
    .find(|path| path.is_file()) else {
        eprintln!("skipping the_desktop_switch_gates_the_settled_rerank: no sentence model in MSIME_NEURAL_MODEL_DIR or MSIME_EVAL_RESOURCES");
        return;
    };
    let model = SentenceModel::load(&std::fs::read(&model).unwrap()).unwrap();
    // Two lattice readings the model tells apart: the lattice put the misspelt one first.
    let mut runtime = Runtime::new(
        Fixture {
            local_mode: "none".into(),
            words: vec!["输入发".into(), "输入法".into()],
            codes: vec!["shu'ru'fa".into(), "shu'ru'fa".into()],
            sources: vec![LATTICE_SOURCE, LATTICE_SOURCE],
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
    runtime.focus(true).unwrap();
    runtime
        .dispatch(Action::Character {
            value: b's',
            shift: false,
        })
        .unwrap();
    let texts = |runtime: &Runtime<Fixture>| -> Vec<String> {
        runtime
            .view()
            .candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect()
    };
    let before = texts(&runtime);
    assert_eq!(before, ["输入发", "输入法"]);
    let generation = runtime.view().generation;

    runtime.set_settled_rerank_enabled(false);
    assert!(!runtime.settled_rerank_enabled());
    assert!(!runtime.rerank_settled(), "switched off, nothing runs");
    assert_eq!(texts(&runtime), before);
    assert_eq!(runtime.view().generation, generation);

    runtime.set_settled_rerank_enabled(true);
    assert!(runtime.rerank_settled(), "switched back on, the model runs");
    assert_eq!(texts(&runtime), ["输入法", "输入发"]);
    assert!(runtime.view().generation > generation);
}

/// A sentence model whose next-character distribution ignores the context: the final layer norm has zero gain, so every position's hidden state is its bias, and the tied embedding turns that into the same logits each time. Characters listed in `favoured` get a high logit and the rest of `characters` a low one, so the model prefers any candidate spelled with the favoured characters and nothing else about it is left to chance.
fn favouring_model(characters: &[char], favoured: &[char]) -> SentenceModel {
    const CONTEXT: usize = 16;
    let vocabulary: Vec<String> = ["<pad>", "<unk>", "<bos>"]
        .into_iter()
        .map(str::to_owned)
        .chain(characters.iter().map(char::to_string))
        .collect();
    let logits: Vec<f32> = [0.0, -8.0, 0.0]
        .into_iter()
        .chain(
            characters
                .iter()
                .map(|c| if favoured.contains(c) { 8.0 } else { -8.0 }),
        )
        .collect();
    let config = format!(
        r#"{{"vocab":{},"n_layer":0,"n_head":1,"n_embd":1,"context":{CONTEXT},"dropout":0.0}}"#,
        vocabulary.len()
    );
    let tensors: [(&str, usize, Vec<f32>); 4] = [
        ("tok.weight", vocabulary.len(), logits),
        ("pos.weight", CONTEXT, vec![0.0; CONTEXT]),
        ("ln_f.weight", 1, vec![0.0]),
        ("ln_f.bias", 1, vec![1.0]),
    ];
    let mut header = serde_json::Map::new();
    header.insert(
        "__metadata__".into(),
        serde_json::json!({
            "format": "chinese-ime-lm",
            "version": "1",
            "precision": "f32",
            "config": config,
            "vocab": serde_json::to_string(&vocabulary).unwrap(),
        }),
    );
    let mut data = Vec::new();
    for (name, rows, values) in tensors {
        let start = data.len();
        data.extend(values.iter().flat_map(|value| value.to_le_bytes()));
        header.insert(
            name.into(),
            serde_json::json!({"dtype": "F32", "shape": [rows, 1], "data_offsets": [start, data.len()]}),
        );
    }
    // The 1-D tensors are declared with their real shape.
    header["ln_f.weight"]["shape"] = serde_json::json!([1]);
    header["ln_f.bias"]["shape"] = serde_json::json!([1]);
    let header = serde_json::to_vec(&serde_json::Value::Object(header)).unwrap();
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&data);
    SentenceModel::load(&bytes).unwrap()
}

/// The Wubi mixed-pinyin list for `dyn`: the exact code hit 态, a longer code's row 太快 (dynn), and a pinyin fallback row that corrected dyn to dun. `answered_by_pinyin_fallback` stands in for a list with no Wubi rows at all.
struct WubiMixedEngine {
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    reading: String,
}

impl WubiMixedEngine {
    const WORDS: [&'static str; 3] = ["态", "太快", "顿"];
}

impl InputEngine for WubiMixedEngine {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        let words: Vec<String> = if self.reading.is_empty() {
            Vec::new()
        } else {
            Self::WORDS.iter().map(|word| (*word).to_owned()).collect()
        };
        let count = words.len();
        Ok(EngineSnapshot {
            scheme: self.scheme,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            candidate_codes: ["dyn", "dynn", "dun"]
                .into_iter()
                .take(count)
                .map(str::to_owned)
                .collect(),
            candidate_annotations: vec![String::new(); count],
            candidate_sources: vec![0; count],
            candidate_positions: vec![0; count],
            candidate_corrected: [false, false, true].into_iter().take(count).collect(),
            candidate_answers_key: vec![true; count],
            candidate_list_open: false,
            microsoft_shuangpin: false,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: self.answered_by_pinyin_fallback && count > 0,
            wubi_unique_four_code: false,
            local_mode: "none".into(),
            spelling_symbols: String::new(),
            dedicated_english: false,
            preedit: self.reading.clone(),
            reading: String::new(),
            editing_text: self.reading.clone(),
            caret_position: self.reading.len(),
            segment_raw_boundaries: Vec::new(),
            candidates: words,
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        self.reading.push(value as char);
        Ok(empty_result(true))
    }
    fn command(&mut self, _command: Command) -> Result<EngineResult, RuntimeError> {
        let composing = !self.reading.is_empty();
        self.reading.clear();
        Ok(empty_result(composing))
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.reading.clear();
        Ok(EngineResult {
            handled: true,
            has_commit: true,
            commit: Self::WORDS[index].to_owned(),
            diagnostic: String::new(),
        })
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
    fn punctuation(&mut self, _value: u8) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn select_edge(
        &mut self,
        index: usize,
        _edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
}

fn typed_dyn_with_reranker(scheme: u8, answered_by_pinyin_fallback: bool) -> Vec<String> {
    let mut runtime = Runtime::new(
        WubiMixedEngine {
            scheme,
            answered_by_pinyin_fallback,
            reading: String::new(),
        },
        5,
    )
    .unwrap();
    let model = favouring_model(&['态', '太', '快', '顿'], &['太', '快']);
    runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
    runtime.focus(true).unwrap();
    for value in *b"dyn" {
        runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
    }
    runtime
        .view()
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

/// Typing `dyn` under Wubi with mixed pinyin showed 太快 above 态: the corrected pinyin row took the dictionary exemption away from the whole list and the model promoted the longer code's row. A list the Wubi table answered keeps the Engine's order.
#[test]
fn a_wubi_list_keeps_the_exact_code_hit_first_under_the_reranker() {
    assert_eq!(typed_dyn_with_reranker(2, false), ["态", "太快", "顿"]);
    // The same list under a pinyin scheme is reranked, so the model would promote 太快 if Wubi let it.
    assert_eq!(typed_dyn_with_reranker(0, false), ["太快", "态", "顿"]);
    // A Wubi code only the pinyin fallback answered is pinyin, and is reranked like pinyin.
    assert_eq!(typed_dyn_with_reranker(2, true), ["太快", "态", "顿"]);
}

fn withholding_runtime(offered: usize, withheld: usize, page_size: u8) -> Runtime<Fixture> {
    Runtime::new(
        Fixture {
            scheme: 0,
            dedicated_english: false,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            local_mode: "none".into(),
            words: (0..offered).map(|n| format!("candidate-{n}")).collect(),
            codes: Vec::new(),
            text: String::new(),
            snapshot_fails: false,
            balanced_openings: Vec::new(),
            cache_resets: 0,
            context_resets: 0,
            withheld: (offered..offered + withheld)
                .map(|n| format!("candidate-{n}"))
                .collect(),
            sources: Vec::new(),
            remaining_after_select: None,
            positions: Vec::new(),
            reading: String::new(),
            spelling_symbols: String::new(),
        },
        page_size,
    )
    .unwrap()
}

#[test]
fn paging_reaches_candidates_the_engine_withheld() {
    // Twelve offered at five a page is two full pages and a short third. Without the expansion the
    // third page is the end of the road, and the eight held back are unreachable by any key.
    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert_eq!(runtime.view().page_count, 3);
    let second = runtime.dispatch(Action::NextPage).unwrap().view;
    assert_eq!(second.page, 1);
    // Entering the short last page is where the expansion belongs: the page is filled before it is
    // shown, rather than appearing short and then growing under the user.
    let third = runtime.dispatch(Action::NextPage).unwrap().view;
    assert_eq!(third.page, 2);
    assert_eq!(third.page_count, 4);
    assert_eq!(third.candidates.len(), 5);
    let fourth = runtime.dispatch(Action::NextPage).unwrap().view;
    assert_eq!(fourth.page, 3);
    assert_eq!(
        fourth.candidates.first().map(|c| c.text.as_str()),
        Some("candidate-15")
    );
}

#[test]
fn candidate_page_len_matches_the_published_page_without_building_rows() {
    let mut short = withholding_runtime(3, 4, 5);
    short.focus(true).unwrap();
    type_key(&mut short);
    assert_eq!(short.candidate_page_len(), 3);
    short.dispatch(Action::NextPage).unwrap();
    assert_eq!(short.candidate_page_len(), 5);

    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert_eq!(runtime.candidate_page_len(), 5);
    runtime.dispatch(Action::NextPage).unwrap();
    assert_eq!(runtime.candidate_page_len(), 5);
    runtime.dispatch(Action::NextPage).unwrap();
    assert_eq!(runtime.candidate_page_len(), 5);
}

#[test]
fn translation_candidates_match_the_visible_page_without_full_rows() {
    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let light = runtime.translation_candidates().unwrap();
    let view = runtime.view();
    assert_eq!(light.generation, view.generation);
    assert_eq!(light.scheme, view.scheme);
    assert_eq!(light.local_mode, view.local_mode);
    assert_eq!(light.candidates.len(), view.candidates.len());
    for (candidate, visible) in light.candidates.iter().zip(&view.candidates) {
        assert_eq!(candidate.text, visible.text);
        assert_eq!(candidate.source, visible.source);
    }
}

#[test]
fn the_full_list_holds_what_paging_would_have_reached() {
    // Twelve offered and eight held back. Paging to the last page releases the eight; a host that
    // opens the whole list instead never paged, so it used to see only the first twelve and the
    // panel that promises everything was short by the tail of the answer.
    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert_eq!(runtime.all_candidates().candidates.len(), 20);
    // Asking again is stable: there is nothing left to release and the list does not shift.
    let repeated = runtime.all_candidates();
    assert_eq!(repeated.candidates.len(), 20);
    assert_eq!(
        repeated.candidates.last().map(|c| c.text.as_str()),
        Some("candidate-19")
    );
    // And it agrees with what paging reaches, which is the behaviour it is standing in for.
    let mut paged = withholding_runtime(12, 8, 5);
    paged.focus(true).unwrap();
    type_key(&mut paged);
    for _ in 0..3 {
        paged.dispatch(Action::NextPage).unwrap();
    }
    assert_eq!(paged.all_candidates().candidates.len(), 20);
}

#[test]
fn full_list_does_not_reorder_after_generation_identity_is_exhausted() {
    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.generation = u64::MAX;
    let before = runtime.view();

    // Releasing the withheld tail would reorder seats but cannot allocate a new
    // generation. Keep the published IDs and their seat mapping stable instead.
    let full = runtime.all_candidates();
    assert_eq!(full.generation, u64::MAX);
    assert_eq!(full.candidates.len(), 12);
    assert_eq!(
        full.candidates
            .iter()
            .take(before.candidates.len())
            .map(|candidate| candidate.text.as_str())
            .collect::<Vec<_>>(),
        before
            .candidates
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn expansion_that_fills_the_current_page_does_not_advance_past_it() {
    // Three offered is a single short page. Asking for the next one has nowhere to go, so the
    // arrivals fill this page instead - advancing would step straight over them.
    let mut runtime = withholding_runtime(3, 4, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    let filled = runtime.dispatch(Action::NextPage).unwrap().view;
    assert_eq!(filled.page, 0);
    assert_eq!(filled.page_count, 2);
    assert_eq!(filled.candidates.len(), 5);
    assert_eq!(
        filled.candidates.first().map(|c| c.text.as_str()),
        Some("candidate-0")
    );
    // The page is no longer short, so the next request moves on as usual.
    assert_eq!(runtime.dispatch(Action::NextPage).unwrap().view.page, 1);
}

#[test]
fn walking_the_highlight_off_the_end_reaches_candidates_the_engine_withheld() {
    // Ten offered at five a page is two full pages, so the partial-last-page rule below never fires
    // and this exercises the end of the list on its own. Walking down one candidate at a time - an
    // arrow key, a wheel notch - used to stop dead on the tenth, while page-down on the same query
    // walked past it. The cap is the Engine's single-letter limit, and selection has to release it
    // for the same reason paging does.
    let mut runtime = withholding_runtime(10, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    for _ in 0..9 {
        runtime.dispatch(Action::NextCandidate).unwrap();
    }
    let last_offered = runtime.view();
    assert_eq!(last_offered.page_count, 2);
    let expanded = runtime.dispatch(Action::NextCandidate).unwrap().view;
    assert_eq!(expanded.page, 2);
    assert_eq!(expanded.page_count, 4);
    assert_eq!(
        expanded
            .candidates
            .iter()
            .find(|candidate| candidate.highlighted)
            .map(|candidate| candidate.text.as_str()),
        Some("candidate-10")
    );
}

#[test]
fn stepping_into_the_partial_last_page_fills_it_first() {
    // The page-down path already fills the short last page before entering it. Arriving at the same
    // page by selection has to look the same, or the page appears short and then grows under a
    // highlight that is already sitting in it.
    let mut runtime = withholding_runtime(12, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    for _ in 0..9 {
        runtime.dispatch(Action::NextCandidate).unwrap();
    }
    let entered = runtime.dispatch(Action::NextCandidate).unwrap().view;
    assert_eq!(entered.page, 2);
    assert_eq!(entered.candidates.len(), 5);
    assert_eq!(
        entered.candidates.first().map(|c| c.text.as_str()),
        Some("candidate-10")
    );
}

#[test]
fn the_highlight_stops_at_the_last_candidate_once_nothing_is_withheld() {
    // Expansion is not a wrap: when the Engine has nothing left, the selection stays where it is
    // rather than moving or reordering the list under it.
    let mut runtime = withholding_runtime(3, 0, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    for _ in 0..2 {
        runtime.dispatch(Action::NextCandidate).unwrap();
    }
    let end = runtime.dispatch(Action::NextCandidate).unwrap().view;
    assert_eq!(end.candidates.len(), 3);
    assert_eq!(
        end.candidates
            .iter()
            .position(|candidate| candidate.highlighted),
        Some(2)
    );
}

#[test]
fn walking_the_highlight_backwards_never_asks_for_more() {
    // Only forward motion runs into the cap. Asking the Engine to expand while moving up would
    // reorder the list the user is reading back through.
    let mut runtime = withholding_runtime(10, 8, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    for _ in 0..8 {
        runtime.dispatch(Action::NextCandidate).unwrap();
    }
    runtime.dispatch(Action::PreviousCandidate).unwrap();
    assert_eq!(runtime.view().page_count, 2);
}

#[test]
fn an_engine_withholding_nothing_pages_exactly_as_before() {
    let mut runtime = withholding_runtime(12, 0, 5);
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    for expected in [1, 2, 2, 2] {
        assert_eq!(
            runtime.dispatch(Action::NextPage).unwrap().view.page,
            expected
        );
    }
    assert_eq!(runtime.view().page_count, 3);
}

// The runtime seats online candidates and reranks the Engine's list, so the page a host renders is not in the Engine's order. A selection names a seat on that page; the Engine has to be asked for the candidate sitting there, not for whatever it holds at the same number. fcitx5-native-ai caught this as an AI candidate shown in slot 1 committing the Engine's own second candidate.
#[test]
fn selecting_a_reseated_candidate_commits_that_candidate() {
    let mut runtime = Runtime::new(
        Fixture {
            local_mode: "none".into(),
            words: vec!["本地一".into(), "本地二".into(), "AI".into()],
            codes: (0..3).map(|n| format!("code-{n}")).collect(),
            sources: vec![0, 0, 3],
            ..Fixture::default()
        },
        9,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view.candidates;
    assert_eq!(
        page.iter()
            .map(|candidate| candidate.text.as_str())
            .collect::<Vec<_>>(),
        vec!["本地一", "AI", "本地二"]
    );

    let done = runtime.dispatch(Action::Select(page[1].id)).unwrap();
    assert_eq!(done.commit.as_deref(), Some("AI"));
}

// A phrase held over an emptied reading is still a composition: swapping the engine under it would
// retype the old scheme's reading into the new one on the next Backspace.
#[test]
fn a_held_phrase_over_an_emptied_reading_is_not_idle() {
    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let kept = runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(kept.view.phrase_prefix, "海滩");
    assert!(kept.view.editing_text.is_empty());
    assert!(!runtime.is_idle());
    let generation = runtime.view().generation;
    assert!(matches!(
        runtime.replace_engine(PhraseEngine::new(vec![6]), 5),
        Err(RuntimeError::CompositionActive)
    ));
    assert!(matches!(
        runtime.set_page_size(7),
        Err(RuntimeError::CompositionActive)
    ));
    assert!(matches!(
        runtime.set_nine_key_enabled(false),
        Err(RuntimeError::CompositionActive)
    ));
    assert_eq!(runtime.view().generation, generation);
    // Dropping the held piece ends the composition.
    runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert!(runtime.is_idle());
}

/// Settling that moves nothing keeps the generation, so a host that redraws on a new generation
/// does not flicker on every pause.
#[test]
fn settling_that_moves_nothing_keeps_the_generation() {
    let mut runtime = runtime();
    runtime.focus(true).unwrap();
    let page = type_key(&mut runtime).view;
    assert!(!runtime.rerank_settled());
    assert_eq!(runtime.view().generation, page.generation);
}

#[test]
fn the_full_list_seats_online_candidates_as_paging_does_and_retires_page_ids() {
    // The released tail carries a cloud candidate. Paging seats it second; the panel has to agree,
    // and the page drawn before the release must not select by the reordered seats.
    let with_cloud = || {
        let mut runtime = withholding_runtime(12, 8, 5);
        runtime.engine.sources = (0..20).map(|n| if n == 15 { 2 } else { 0 }).collect();
        runtime.engine.codes = vec![String::new(); 20];
        runtime.focus(true).unwrap();
        runtime
    };
    let mut runtime = with_cloud();
    let page = type_key(&mut runtime).view;
    let panel = runtime.all_candidates();
    assert_eq!(panel.candidates.len(), 20);
    assert_eq!(panel.candidates[1].text, "candidate-15");
    assert_eq!(panel.generation, page.generation + 1);
    assert!(matches!(
        runtime.dispatch(Action::Select(page.candidates[1].id)),
        Err(RuntimeError::StaleCandidate)
    ));

    let mut paged = with_cloud();
    type_key(&mut paged);
    for _ in 0..3 {
        paged.dispatch(Action::NextPage).unwrap();
    }
    let texts = |snapshot: CandidateSnapshot| -> Vec<String> {
        snapshot.candidates.into_iter().map(|c| c.text).collect()
    };
    assert_eq!(texts(paged.all_candidates()), texts(panel));
}

/// An engine whose snapshot fails once a candidate has been committed.
struct FailsAfterCommit {
    inner: Fixture,
    committed: bool,
}

impl InputEngine for FailsAfterCommit {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        if self.committed {
            return Err(RuntimeError::Engine("injected snapshot failure".into()));
        }
        self.inner.snapshot()
    }
    fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult, RuntimeError> {
        self.inner.character(value, shift)
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        self.inner.command(command)
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.committed = true;
        self.inner.select(index)
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.committed = true;
        self.inner.select_edge(index, edge)
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.inner.finish(index)
    }
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        self.inner.punctuation(value)
    }
}

#[test]
fn a_wubi_auto_commit_survives_a_failed_refresh() {
    let mut runtime = Runtime::new(
        FailsAfterCommit {
            inner: Fixture {
                scheme: 2,
                words: vec!["合成候选".into()],
                ..Fixture::default()
            },
            committed: false,
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let mut last = None;
    for value in b"wqaa" {
        last = Some(
            runtime
                .dispatch(Action::Character {
                    value: *value,
                    shift: false,
                })
                .unwrap(),
        );
    }
    let last = last.unwrap();
    assert_eq!(last.commit.as_deref(), Some("合成候选"));
    assert!(last
        .diagnostic
        .as_deref()
        .is_some_and(|diagnostic| diagnostic.starts_with("Candidate refresh failed")));
}

#[test]
fn a_busy_provider_keeps_only_the_newest_completed_result() {
    let query = |text: &str| OnlineQuery {
        scheme: 0,
        generation: 1,
        identity: text.into(),
        query_text: text.into(),
        cache_key: text.into(),
        pinyin_segments: vec![],
        cloud_eligible: true,
        ai_eligible: false,
        cloud_candidates: true,
        session_id: 1,
        ai_context: String::new(),
        ai_assistant: None,
        ai_cache_only: false,
    };
    let (started, first_running) = std::sync::mpsc::channel::<()>();
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let gate = std::sync::Mutex::new(gate);
    let worker = OnlineProviderWorker::spawn(1, move |query: &OnlineQuery| {
        if query.query_text == "ni" {
            started.send(()).unwrap();
            gate.lock().unwrap().recv().unwrap();
        }
        Some((query.query_text.clone(), 0))
    })
    .unwrap();
    assert!(worker.submit(query("ni")));
    first_running
        .recv_timeout(Duration::from_secs(5))
        .expect("first query running");
    for text in ["nih", "niha", "nihao"] {
        assert!(worker.submit(query(text)));
    }
    release.send(()).unwrap();
    let mut answered = None;
    for _ in 0..500 {
        if let Some(result) = worker.try_recv() {
            answered = Some(result.text);
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(answered, Some("nihao".to_owned()));
    assert!(worker.try_recv().is_none());
    worker.shutdown();
}

/// The scheme predicates the runtime reads give, for every existing scheme, exactly what the ordinal comparisons they replaced gave.
#[test]
fn scheme_predicates_reproduce_the_ordinal_rules_they_replace() {
    use super::runtime::{runtime_reorders_candidates, script_conversion};
    use msime_engine::SchemeType;
    for ordinal in 0..=4u8 {
        let scheme = SchemeType::from_u8(ordinal).unwrap();
        // Smart punctuation: neither Japanese nor Korean.
        assert_eq!(
            scheme.host_smart_punctuation(),
            ordinal != 3 && ordinal != KOREAN_SCHEME,
            "{ordinal}"
        );
        // Nine-key: quanpin only.
        assert_eq!(scheme.nine_key(), ordinal == 0, "{ordinal}");
        // Phrase holding, reranking and runner-up demotion: everything but Korean.
        assert_eq!(
            scheme.holds_phrase_progress(),
            ordinal != KOREAN_SCHEME,
            "{ordinal}"
        );
        assert_eq!(
            runtime_reorders_candidates(ordinal),
            ordinal != KOREAN_SCHEME,
            "{ordinal}"
        );
        // Committing on blur and the double Cancel of an open list: Korean only.
        assert_eq!(
            scheme.commits_on_blur(),
            ordinal == KOREAN_SCHEME,
            "{ordinal}"
        );
        assert_eq!(
            scheme.has_openable_candidate_list(),
            ordinal == KOREAN_SCHEME,
            "{ordinal}"
        );
        // The cloud gate refused Korean; Wubi never had a query the Engine called eligible.
        assert_eq!(
            scheme.cloud_eligible(),
            ordinal != 2 && ordinal != KOREAN_SCHEME,
            "{ordinal}"
        );
        // Only the schemes that open local modes listed idle spelling symbols (`/`, `@`).
        assert_eq!(scheme.opens_local_modes(), ordinal <= 1, "{ordinal}");
        assert_eq!(
            script_conversion(ordinal, "none"),
            ordinal <= 2,
            "{ordinal}"
        );
    }
    // The placeholder snapshot of a failed refresh names no scheme and keeps the treatment the ordinal comparisons gave it.
    assert!(runtime_reorders_candidates(255));
    assert!(!script_conversion(255, "none"));

    // The cloud gate reads the predicate: a Wubi query claiming eligibility is refused like a Korean one, and so is a query naming no scheme.
    let query = |scheme: u8| OnlineQuery {
        scheme,
        generation: 1,
        identity: "x".into(),
        query_text: "ni".into(),
        cache_key: "x".into(),
        pinyin_segments: vec![],
        cloud_eligible: true,
        ai_eligible: false,
        cloud_candidates: true,
        session_id: 1,
        ai_context: String::new(),
        ai_assistant: None,
        ai_cache_only: false,
    };
    for scheme in [0, 1, 3] {
        assert!(cloud_request_url(&query(scheme)).is_some(), "{scheme}");
    }
    for scheme in [2, KOREAN_SCHEME, 255] {
        assert!(cloud_request_url(&query(scheme)).is_none(), "{scheme}");
    }
}

fn scheme_runtime(scheme: u8, local_mode: &str) -> Runtime<Fixture> {
    let mut runtime = Runtime::new(
        Fixture {
            scheme,
            local_mode: local_mode.into(),
            words: vec!["甲".into(), "乙".into()],
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// `chinese_text`, `script_conversion` and `candidate_list_open` for the existing schemes, and the host-facing checks that now read predicates.
#[test]
fn views_carry_the_scheme_traits_of_existing_schemes() {
    for scheme in 0..=4u8 {
        let mut runtime = scheme_runtime(scheme, "none");
        let view = runtime.view();
        assert_eq!(view.chinese_text, scheme <= 2, "{scheme}");
        assert_eq!(view.script_conversion, scheme <= 2, "{scheme}");
        assert!(!view.candidate_list_open, "{scheme}");
        assert_eq!(
            runtime.punctuation_host_context_available(false),
            scheme <= 2,
            "{scheme}"
        );
        assert!(!runtime.punctuation_host_context_available(true));
        assert_eq!(
            matches!(
                runtime.set_nine_key_enabled(true),
                Err(RuntimeError::InvalidNineKeyScheme)
            ),
            scheme != 0,
            "{scheme}"
        );
        if scheme == 0 {
            runtime.set_nine_key_enabled(false).unwrap();
        }

        type_key(&mut runtime);
        let committed = runtime.dispatch(Action::SelectHighlighted).unwrap();
        assert_eq!(committed.commit.as_deref(), Some("甲"));
        let context = committed.commit_context.unwrap();
        assert_eq!(context.scheme, scheme);
        assert_eq!(context.script_conversion, scheme <= 2, "{scheme}");
    }
}

/// A Chinese scheme's text is not converted inside the modes whose text is not Chinese, and a commit carries the mode it was made in even when committing leaves that mode.
#[test]
fn script_conversion_stays_off_in_unicode_and_temporary_japanese_modes() {
    for local_mode in ["unicode", "temporary_japanese"] {
        let mut runtime = scheme_runtime(0, local_mode);
        let typed = type_key(&mut runtime);
        assert!(typed.view.chinese_text);
        assert!(!typed.view.script_conversion, "{local_mode}");
        let committed = runtime.dispatch(Action::SelectHighlighted).unwrap();
        assert!(
            !committed.commit_context.unwrap().script_conversion,
            "{local_mode}"
        );
        // Selecting returned the fixture to no local mode.
        assert!(committed.view.script_conversion);
    }
    let typed = type_key(&mut scheme_runtime(0, "emoji"));
    assert!(typed.view.script_conversion);
}

/// The view reports an open Hanja list from the Engine rather than leaving hosts to infer it, and discarding the composition with the list open still takes two Cancels: the first closes the list.
#[test]
fn an_open_korean_hanja_list_is_reported_and_discarded_with_two_cancels() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = korean_runtime(directory.path());
    let typed = character(&mut runtime, b'g');
    assert!(!typed.view.candidate_list_open);
    assert!(!typed.view.chinese_text && !typed.view.script_conversion);
    let opened = open_korean_hanja(&mut runtime, "ks");
    assert!(opened.view.candidate_list_open);
    assert!(!opened.view.candidates.is_empty());

    // Escape only closes the list.
    let closed = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(!closed.view.candidate_list_open);
    assert_eq!(closed.view.preedit, "한");

    // Leaving the client with the list open commits the Hangul (commits on blur), whatever is highlighted.
    open_korean_hanja(&mut runtime, "");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let left = runtime.focus(false).unwrap();
    assert_eq!(left.commit.as_deref(), Some("한"));
    assert!(!left.view.candidate_list_open);

    // Attaching a client discards: the first Cancel closes the list and the second takes the syllable.
    runtime.focus(true).unwrap();
    open_korean_hanja(&mut runtime, "gks");
    let attached = runtime.focus(true).unwrap();
    assert!(attached.commit.is_none());
    assert_eq!(attached.view.editing_text, "");
    assert!(!attached.view.candidate_list_open);
}

/// An Engine that spells with punctuation marks it lists in `spelling_symbols`, as Zhuyin's bopomofo keys are, recording every mark it is handed as a character.
struct SpellingMarksEngine {
    scheme: u8,
    symbols: String,
    text: String,
}

impl InputEngine for SpellingMarksEngine {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        Ok(EngineSnapshot {
            scheme: self.scheme,
            nine_key: false,
            nine_key_spellings: Vec::new(),
            candidate_codes: Vec::new(),
            candidate_annotations: Vec::new(),
            candidate_sources: Vec::new(),
            candidate_positions: Vec::new(),
            candidate_corrected: Vec::new(),
            candidate_answers_key: Vec::new(),
            candidate_list_open: false,
            microsoft_shuangpin: false,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: false,
            local_mode: "none".into(),
            spelling_symbols: self.symbols.clone(),
            dedicated_english: false,
            preedit: self.text.clone(),
            reading: String::new(),
            editing_text: self.text.clone(),
            caret_position: self.text.len(),
            segment_raw_boundaries: Vec::new(),
            candidates: Vec::new(),
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        if !self.symbols.as_bytes().contains(&value) {
            return Ok(empty_result(false));
        }
        self.text.push(char::from(value));
        Ok(empty_result(true))
    }
    fn command(&mut self, _command: Command) -> Result<EngineResult, RuntimeError> {
        self.text.clear();
        Ok(empty_result(true))
    }
    fn select(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn finish(&mut self, _index: usize) -> Result<EngineResult, RuntimeError> {
        Ok(EngineResult {
            handled: !self.text.is_empty(),
            has_commit: !self.text.is_empty(),
            commit: std::mem::take(&mut self.text),
            diagnostic: String::new(),
        })
    }
    fn punctuation(&mut self, _value: u8) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn select_edge(
        &mut self,
        index: usize,
        _edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.select(index)
    }
}

/// The literal-mark route gives a scheme the marks it spells with, as the punctuation route does, while the keys that open a local mode with nothing composed stay literal.
#[test]
fn ascii_punctuation_reaches_a_scheme_that_spells_with_marks() {
    // A scheme that opens no local mode: its listed marks are spelling, idle or composing.
    let mut runtime = Runtime::new(
        SpellingMarksEngine {
            scheme: 3,
            symbols: ",.".into(),
            text: String::new(),
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let idle = runtime.dispatch(Action::PunctuationAscii(b',')).unwrap();
    assert!(idle.handled && idle.commit.is_none());
    assert_eq!(idle.view.editing_text, ",");
    let composing = runtime.dispatch(Action::PunctuationAscii(b'.')).unwrap();
    assert!(composing.handled && composing.commit.is_none());
    assert_eq!(composing.view.editing_text, ",.");
    // A mark the scheme does not list still ends the composition with the literal mark.
    let ended = runtime.dispatch(Action::PunctuationAscii(b'!')).unwrap();
    assert_eq!(ended.commit.as_deref(), Some(",.!"));

    // A scheme whose idle symbols open modes: the literal route never opens one.
    let mut runtime = Runtime::new(
        SpellingMarksEngine {
            scheme: 0,
            symbols: "/".into(),
            text: String::new(),
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let literal = runtime.dispatch(Action::PunctuationAscii(b'/')).unwrap();
    assert!(literal.commit.is_none());
    assert_eq!(literal.view.editing_text, "");
}

const VIETNAMESE_SCHEME: u8 = 7;

/// A real Engine on the Vietnamese scheme with the given input method (0 Telex, 1 VNI), focused.
fn vietnamese_runtime(directory: &std::path::Path, input_method: u8) -> Runtime {
    let mut options = real_engine_options(directory);
    options.scheme = VIETNAMESE_SCHEME;
    options.vietnamese_input_method = input_method;
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// Types `keys` as plain characters, each one composing without a commit.
fn compose_vietnamese(runtime: &mut Runtime, keys: &str) -> Transition {
    let mut last = None;
    for value in keys.bytes() {
        let transition = character(runtime, value);
        assert!(
            transition.handled && transition.commit.is_none(),
            "{keys}: {}",
            value as char
        );
        last = Some(transition);
    }
    last.unwrap()
}

/// A Telex word is shown with its diacritics and no candidates, and Space commits the word and then leaves the space itself to the host, so the text reads `việt ` in that order. The scheme is not Chinese: nothing is script-converted and the host has no smart punctuation to apply.
#[test]
fn a_telex_word_commits_before_the_space_that_ends_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = vietnamese_runtime(directory.path(), 0);
    assert!(!runtime.punctuation_host_context_available(false));

    let typed = compose_vietnamese(&mut runtime, "vieejt");
    assert_eq!(typed.view.scheme, VIETNAMESE_SCHEME);
    assert_eq!(typed.view.editing_text, "việt");
    assert_eq!(typed.view.caret_position, "việt".len());
    assert_eq!(typed.view.reading, "");
    assert!(typed.view.candidates.is_empty());
    assert!(!typed.view.candidate_list_open);
    assert!(!typed.view.chinese_text);
    assert!(!typed.view.script_conversion);
    assert!(!runtime.punctuation_host_context_available(false));
    assert!(runtime.online_query().unwrap().is_none());

    // Space commits the word and passes through, so the host inserts the space after it.
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(!space.handled);
    assert_eq!(space.commit.as_deref(), Some("việt"));
    let context = space.commit_context.unwrap();
    assert_eq!(context.scheme, VIETNAMESE_SCHEME);
    assert!(!context.script_conversion);
    assert_eq!(space.view.editing_text, "");

    // Punctuation ends the word and stays ASCII after it.
    compose_vietnamese(&mut runtime, "nam");
    let period = character(&mut runtime, b'.');
    assert!(period.handled);
    assert_eq!(period.commit.as_deref(), Some("nam."));
    compose_vietnamese(&mut runtime, "nam");
    let comma = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(comma.commit.as_deref(), Some("nam,"));
    let idle = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert!(!idle.handled && idle.commit.is_none());

    // Enter commits the word and passes through.
    compose_vietnamese(&mut runtime, "xin");
    let enter = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert!(!enter.handled);
    assert_eq!(enter.commit.as_deref(), Some("xin"));
}

/// While a VNI word composes, a digit is a tone or vowel key and composes rather than picking a candidate; with nothing composing, a digit is the host's to type.
#[test]
fn vni_digits_compose_while_a_word_is_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = vietnamese_runtime(directory.path(), 1);

    let idle = character(&mut runtime, b'1');
    assert!(!idle.handled && idle.commit.is_none());
    assert_eq!(idle.view.editing_text, "");

    compose_vietnamese(&mut runtime, "a");
    assert_eq!(runtime.view().spelling_symbols, "0123456789");
    let digit = character(&mut runtime, b'1');
    assert!(digit.handled && digit.commit.is_none());
    assert_eq!(digit.view.editing_text, "á");

    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(!space.handled);
    assert_eq!(space.commit.as_deref(), Some("á"));

    let word = compose_vietnamese(&mut runtime, "vie65t");
    assert_eq!(word.view.editing_text, "việt");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("việt"));
    assert_eq!(runtime.view().spelling_symbols, "");
}

/// Leaving the client commits the word as shown. The first Escape shows the raw keys again and keeps composing; the second drops the composition and commits nothing.
#[test]
fn vietnamese_blur_commits_and_escape_restores_then_cancels() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = vietnamese_runtime(directory.path(), 0);

    compose_vietnamese(&mut runtime, "vieejt");
    let left = runtime.focus(false).unwrap();
    assert_eq!(left.commit.as_deref(), Some("việt"));
    assert_eq!(left.view.editing_text, "");
    runtime.focus(true).unwrap();
    assert!(runtime.focus(false).unwrap().commit.is_none());

    // Attaching a new client discards a word left open in the previous one.
    runtime.focus(true).unwrap();
    compose_vietnamese(&mut runtime, "vieejt");
    let attached = runtime.focus(true).unwrap();
    assert!(attached.commit.is_none());
    assert_eq!(attached.view.editing_text, "");

    let typed = compose_vietnamese(&mut runtime, "coffee");
    assert_ne!(typed.view.editing_text, "coffee");
    let restored = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(restored.handled && restored.commit.is_none());
    assert_eq!(restored.view.editing_text, "coffee");
    let cancelled = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(cancelled.handled && cancelled.commit.is_none());
    assert_eq!(cancelled.view.editing_text, "");
    assert_eq!(runtime.view().editing_text, "");
}

/// Caps Lock (uppercase without Shift) and Shift both reach the word as uppercase, with the tone still placed on the right vowel; there is no case folding as in Korean.
#[test]
fn vietnamese_uppercase_comes_through() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = vietnamese_runtime(directory.path(), 0);

    for value in *b"VIEEJT" {
        let transition = runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
        assert!(transition.handled && transition.commit.is_none());
    }
    assert_eq!(runtime.view().editing_text, "VIỆT");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("VIỆT"));

    let typed = compose_vietnamese(&mut runtime, "Vieejt");
    assert_eq!(typed.view.editing_text, "Việt");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("Việt"));
}

const CANTONESE_SCHEME: u8 = 5;

/// A `cantonese.db` with a few Jyutping rows, written with the shipped schema.
fn cantonese_dictionary(directory: &std::path::Path) -> String {
    use msime_engine::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let path = directory.join("cantonese.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('nei'),('hou'),('ngo'),('ngoi'),('oi'),('i');\
             INSERT INTO entries VALUES ('nei hou','你好',900),('nei hou','妳好',40),('nei','你',5000),('nei','妳',300),('hou','好',4000),('hou','號',500),('ngo','我',6000),('oi','愛',2500),('ngoi','外',1000);",
        )
        .unwrap();
    path.to_str().unwrap().to_owned()
}

/// A real Engine on the Cantonese scheme with learning on, so a learning path the scheme failed to skip would write. Focused.
fn cantonese_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
    options.scheme = CANTONESE_SCHEME;
    options.learning = true;
    options.cantonese_dictionary = cantonese_dictionary(directory);
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// Types `keys` as plain characters, each one composing without a commit.
fn compose_cantonese(runtime: &mut Runtime, keys: &str) -> Transition {
    let mut last = None;
    for value in keys.bytes() {
        let transition = character(runtime, value);
        assert!(
            transition.handled && transition.commit.is_none(),
            "{keys}: {}",
            value as char
        );
        last = Some(transition);
    }
    last.unwrap()
}

/// Every table of each SQLite file under `root`, with its row count, to show that nothing was written.
fn database_rows(root: &std::path::Path) -> Vec<(String, String, i64)> {
    let mut rows = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("db") {
                continue;
            }
            let connection = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let tables: Vec<String> = connection
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            for table in tables {
                let count = connection
                    .query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                rows.push((path.display().to_string(), table, count));
            }
        }
    }
    rows.sort();
    rows
}

/// Digits 1–9 pick from the visible page, and `'` is a syllable boundary the Engine takes while a word composes, on the character route and on both punctuation routes. The scheme is Chinese but its text is Traditional as stored, so nothing is script-converted.
#[test]
fn cantonese_digits_select_and_the_apostrophe_reaches_the_engine() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = cantonese_runtime(directory.path());

    let typed = compose_cantonese(&mut runtime, "neihou");
    assert_eq!(typed.view.scheme, CANTONESE_SCHEME);
    assert_eq!(typed.view.editing_text, "nei hou");
    assert_eq!(texts(&typed.view), ["你好", "妳好", "你", "妳"]);
    assert!(typed.view.chinese_text);
    assert!(!typed.view.script_conversion);
    assert!(!typed.view.candidate_list_open);
    assert_eq!(typed.view.spelling_symbols, "'");

    let picked = character(&mut runtime, b'2');
    assert!(picked.handled);
    assert_eq!(picked.commit.as_deref(), Some("妳好"));
    let context = picked.commit_context.unwrap();
    assert_eq!(context.scheme, CANTONESE_SCHEME);
    assert!(!context.script_conversion);
    assert_eq!(picked.view.editing_text, "");
    assert!(picked.view.chinese_text);
    assert!(!picked.view.script_conversion);

    // A digit past the end of the page is swallowed rather than typed into the document.
    compose_cantonese(&mut runtime, "ngo");
    let beyond = character(&mut runtime, b'9');
    assert!(beyond.handled && beyond.commit.is_none());
    assert_eq!(beyond.view.editing_text, "ngo");
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();

    // `ngo'oi` keeps two syllables where the letters alone would read `ngoi` (外) first.
    for (route, action) in [
        (
            "character",
            Action::Character {
                value: b'\'',
                shift: false,
            },
        ),
        ("punctuation", Action::Punctuation(b'\'')),
        ("ascii punctuation", Action::PunctuationAscii(b'\'')),
    ] {
        compose_cantonese(&mut runtime, "ngo");
        let boundary = runtime.dispatch(action).unwrap();
        assert!(boundary.handled && boundary.commit.is_none(), "{route}");
        let typed = compose_cantonese(&mut runtime, "oi");
        assert_eq!(typed.view.editing_text, "ngo oi", "{route}");
        assert_eq!(texts(&typed.view)[0], "我", "{route}");
        let picked = character(&mut runtime, b'1');
        assert_eq!(picked.commit.as_deref(), Some("我"), "{route}");
        let rest = character(&mut runtime, b'1');
        assert_eq!(rest.commit.as_deref(), Some("愛"), "{route}");
        assert_eq!(rest.view.editing_text, "", "{route}");
    }
}

/// A row covering only the leading syllables goes to the document at once, even for a host that draws held phrase pieces, and the rest keeps composing. Nothing the user picks is learned: the journal and the main dictionary keep their rows, and the same reading comes back in the same order.
#[test]
fn a_cantonese_partial_selection_commits_at_once_and_learns_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let dictionaries = directory.path().join("dictionaries");
    std::fs::create_dir_all(&dictionaries).unwrap();
    rusqlite::Connection::open(dictionaries.join(msime_engine::assets::MAIN_DICTIONARY))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',500);",
        )
        .unwrap();

    // The control: the same pick under Quanpin with the same options writes the journal, so the comparison below would see a Cantonese write. The baseline is taken after composing, because reading candidates can already create the journal with empty tables, so only the pick itself can change the counts.
    let mut options = real_engine_options(directory.path());
    options.learning = true;
    let mut quanpin = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 5).unwrap();
    quanpin.focus(true).unwrap();
    for value in *b"nihao" {
        character(&mut quanpin, value);
    }
    let initial = database_rows(directory.path());
    assert_eq!(
        character(&mut quanpin, b'2').commit.as_deref(),
        Some("拟好")
    );
    drop(quanpin);
    msime_engine::flush_personal_learning();
    // Some table must gain rows; a journal created with empty tables does not count as a write.
    let learned = database_rows(directory.path());
    assert!(
        learned.iter().any(|(file, table, count)| {
            let previous = initial
                .iter()
                .find(|(before_file, before_table, _)| before_file == file && before_table == table)
                .map_or(0, |(_, _, before)| *before);
            *count > previous
        }),
        "the Quanpin pick wrote no rows: {initial:?} -> {learned:?}"
    );

    let mut runtime = cantonese_runtime(directory.path());
    runtime.set_phrase_preedit(true);
    let before = database_rows(directory.path());
    compose_cantonese(&mut runtime, "neihou");
    let first = character(&mut runtime, b'3');
    assert!(first.handled);
    assert_eq!(first.commit.as_deref(), Some("你"));
    assert_eq!(first.view.phrase_prefix, "");
    assert_eq!(first.view.editing_text, "hou");
    assert_eq!(texts(&first.view), ["好", "號"]);
    let id = first.view.candidates[1].id;
    let second = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(second.commit.as_deref(), Some("號"));
    assert_eq!(second.view.phrase_prefix, "");
    assert_eq!(second.view.editing_text, "");

    // Picking the second row again and again does not lift it.
    for _ in 0..3 {
        compose_cantonese(&mut runtime, "neihou");
        assert_eq!(
            character(&mut runtime, b'2').commit.as_deref(),
            Some("妳好")
        );
    }
    let again = compose_cantonese(&mut runtime, "neihou");
    assert_eq!(texts(&again.view), ["你好", "妳好", "你", "妳"]);
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();

    drop(runtime);
    msime_engine::flush_personal_learning();
    assert_eq!(database_rows(directory.path()), before);
}

/// The sentence model and the runner-up demotion reorder Chinese lattice readings; a Cantonese or Stroke list comes from its own dictionary in its own order, so neither touches it.
#[test]
fn cantonese_lists_are_never_reranked_or_demoted() {
    let reordered = |scheme: u8, words: &[&str], sources: Vec<u8>, model: Option<SentenceModel>| {
        let mut runtime = Runtime::new(
            Fixture {
                scheme,
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).to_owned()).collect(),
                codes: vec!["neihou".into(); words.len()],
                sources,
                ..Fixture::default()
            },
            5,
        )
        .unwrap();
        if let Some(model) = model {
            runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
        }
        runtime.focus(true).unwrap();
        runtime
            .dispatch(Action::Character {
                value: b'n',
                shift: false,
            })
            .unwrap();
        texts(&runtime.view())
    };
    let rows = ["你", "妳", "尼"];
    let favours = || Some(favouring_model(&['你', '妳', '尼'], &['尼']));
    // The same rows under a pinyin scheme are reranked, so the model would move 尼 up if Cantonese let it.
    assert_eq!(
        reordered(0, &rows, vec![LATTICE_SOURCE; 3], favours()),
        ["尼", "你", "妳"]
    );
    assert_eq!(
        reordered(CANTONESE_SCHEME, &rows, vec![LATTICE_SOURCE; 3], favours()),
        rows
    );
    // Stroke lists come from stroke.db in its own order as well.
    assert_eq!(
        reordered(STROKE_SCHEME, &rows, vec![LATTICE_SOURCE; 3], favours()),
        rows
    );
    let sentences = ["你好嗎", "妳好嗎", "尼好嗎", "你號嗎", "妳號嗎", "你", "好"];
    let sources = || {
        let mut sources = vec![LATTICE_SOURCE; 5];
        sources.extend([0, 0]);
        sources
    };
    assert_eq!(
        reordered(0, &sentences, sources(), None),
        ["你好嗎", "妳好嗎", "尼好嗎", "你", "好"]
    );
    assert_eq!(
        reordered(CANTONESE_SCHEME, &sentences, sources(), None),
        sentences[..5]
    );
}

const ZHUYIN_SCHEME: u8 = 6;

/// A `zhuyin.db` with a few bopomofo rows, written with the shipped schema.
fn zhuyin_dictionary(directory: &std::path::Path) -> String {
    use msime_engine::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let path = directory.join("zhuyin.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('ㄋㄧˇ'),('ㄏㄠˇ'),('ㄊㄞˊ'),('ㄨㄢ'),('ㄇㄚ˙'),('ㄇㄚ'),('ㄝ');\
             INSERT INTO entries VALUES ('ㄋㄧˇ','你',1000),('ㄋㄧˇ','妳',300),('ㄏㄠˇ','好',2000),('ㄏㄠˇ','郝',10),('ㄋㄧˇ ㄏㄠˇ','你好',500),('ㄊㄞˊ','台',900),('ㄊㄞˊ','臺',400),('ㄨㄢ','彎',500),('ㄨㄢ','灣',300),('ㄊㄞˊ ㄨㄢ','臺灣',800),('ㄊㄞˊ ㄨㄢ','台灣',600),('ㄇㄚ˙','嗎',800),('ㄇㄚ','媽',700),('ㄝ','欸',50);",
        )
        .unwrap();
    path.to_str().unwrap().to_owned()
}

/// A real Engine on the Zhuyin scheme, focused.
fn zhuyin_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
    options.scheme = ZHUYIN_SCHEME;
    options.zhuyin_dictionary = zhuyin_dictionary(directory);
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// Types `keys` as plain characters, each one composing without a commit.
fn compose_zhuyin(runtime: &mut Runtime, keys: &str) -> Transition {
    let mut last = None;
    for value in keys.bytes() {
        let transition = character(runtime, value);
        assert!(
            transition.handled && transition.commit.is_none(),
            "{keys}: {}",
            value as char
        );
        last = Some(transition);
    }
    last.unwrap()
}

/// The Dachen keys that are digits and marks spell even with nothing composed, on the character route and on both punctuation routes, while a tone key with nothing to complete is the host's to type. The scheme is Chinese but writes Traditional as stored, and the bopomofo keys overlap the host's smart punctuation, so the host has none.
#[test]
fn zhuyin_idle_phonetic_keys_compose_and_idle_tone_keys_type_themselves() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());
    assert!(!runtime.punctuation_host_context_available(false));
    let idle = runtime.view();
    assert_eq!(idle.scheme, ZHUYIN_SCHEME);
    assert_eq!(idle.spelling_symbols, "125890,./;-");
    assert!(idle.chinese_text);
    assert!(!idle.script_conversion);

    for (route, action) in [
        (
            "character 1",
            Action::Character {
                value: b'1',
                shift: false,
            },
        ),
        (
            "character ,",
            Action::Character {
                value: b',',
                shift: false,
            },
        ),
        (
            "character -",
            Action::Character {
                value: b'-',
                shift: false,
            },
        ),
        ("punctuation ,", Action::Punctuation(b',')),
        ("punctuation -", Action::Punctuation(b'-')),
        ("ascii punctuation ,", Action::PunctuationAscii(b',')),
        ("ascii punctuation -", Action::PunctuationAscii(b'-')),
    ] {
        let typed = runtime.dispatch(action).unwrap();
        assert!(typed.handled && typed.commit.is_none(), "{route}");
        assert_eq!(typed.view.editing_text.len(), 1, "{route}");
        assert!(!typed.view.preedit.is_empty(), "{route}");
        assert!(typed.view.spelling_symbols.contains(' '), "{route}");
        let cleared = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
        assert!(cleared.handled && cleared.commit.is_none(), "{route}");
        assert_eq!(cleared.view.editing_text, "", "{route}");
    }

    for value in *b"3467" {
        let typed = character(&mut runtime, value);
        assert!(
            !typed.handled && typed.commit.is_none(),
            "{}",
            value as char
        );
        assert_eq!(typed.view.editing_text, "", "{}", value as char);
    }
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(!space.handled && space.commit.is_none());
}

/// `su3cl3` converts to 你好 with the caret held at the end, and Enter commits the conversion and keeps the key. Nothing is script-converted on the way out.
#[test]
fn zhuyin_enter_commits_the_conversion() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());

    let typed = compose_zhuyin(&mut runtime, "su3cl3");
    assert_eq!(typed.view.editing_text, "su3cl3");
    assert_eq!(typed.view.caret_position, "su3cl3".len());
    assert_eq!(typed.view.preedit, "你好");
    assert!(typed.view.candidates.is_empty());
    assert!(!typed.view.candidate_list_open);
    assert_eq!(typed.view.spelling_symbols, "1234567890,./;- ");

    let enter = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert!(enter.handled);
    assert_eq!(enter.commit.as_deref(), Some("你好"));
    let context = enter.commit_context.unwrap();
    assert_eq!(context.scheme, ZHUYIN_SCHEME);
    assert!(!context.script_conversion);
    assert_eq!(enter.view.editing_text, "");
    assert_eq!(enter.view.spelling_symbols, "125890,./;-");
}

/// Space is a spelling key while the Engine lists it: with a syllable pending it is the first tone, and with none pending it opens the list, whether the host sends it as a character or as its Space command. Once the list is open it takes the highlighted row on either route, and it never commits on its own.
#[test]
fn zhuyin_space_is_the_first_tone_and_opens_the_list() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());

    for (route, space) in [
        (
            "character",
            (|| Action::Character {
                value: b' ',
                shift: false,
            }) as fn() -> Action,
        ),
        ("command", || Action::SelectHighlighted),
    ] {
        compose_zhuyin(&mut runtime, "j0");
        let toned = runtime.dispatch(space()).unwrap();
        assert!(toned.handled && toned.commit.is_none(), "{route}");
        assert_eq!(toned.view.preedit, "彎", "{route}");
        assert!(!toned.view.candidate_list_open, "{route}");
        assert!(toned.view.candidates.is_empty(), "{route}");

        let opened = runtime.dispatch(space()).unwrap();
        assert!(opened.handled && opened.commit.is_none(), "{route}");
        assert!(opened.view.candidate_list_open, "{route}");
        assert_eq!(texts(&opened.view), ["彎", "灣"], "{route}");
        assert_eq!(opened.view.spelling_symbols, "0,./;-", "{route}");
        assert_eq!(opened.view.preedit, "彎", "{route}");

        // With the list open Space takes the highlighted row into the conversion and commits nothing.
        runtime.dispatch(Action::NextCandidate).unwrap();
        let picked = runtime.dispatch(space()).unwrap();
        assert!(picked.handled && picked.commit.is_none(), "{route}");
        assert!(!picked.view.candidate_list_open, "{route}");
        assert!(picked.view.candidates.is_empty(), "{route}");
        assert_eq!(picked.view.preedit, "灣", "{route}");
        let enter = runtime
            .dispatch(Action::Command(Command::CommitRaw))
            .unwrap();
        assert_eq!(enter.commit.as_deref(), Some("灣"), "{route}");
    }
}

/// The Down key's command opens the list over the conversion. A digit picks a row on the visible page without committing, `0` is the bopomofo ㄢ and closes the list rather than picking, and Escape closes the list before a second one clears the composition.
#[test]
fn zhuyin_list_digits_select_without_commit_and_escape_steps_back() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());

    compose_zhuyin(&mut runtime, "w96");
    let opened = runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    assert!(opened.handled && opened.commit.is_none());
    assert!(opened.view.candidate_list_open);
    assert_eq!(texts(&opened.view), ["台", "臺"]);

    let picked = character(&mut runtime, b'2');
    assert!(picked.handled && picked.commit.is_none());
    assert!(!picked.view.candidate_list_open);
    assert_eq!(picked.view.preedit, "臺");
    assert_eq!(picked.view.editing_text, "w96");

    // `0` with the list open spells ㄢ after the conversion instead of choosing a row.
    runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    let zero = character(&mut runtime, b'0');
    assert!(zero.handled && zero.commit.is_none());
    assert!(!zero.view.candidate_list_open);
    assert!(zero.view.candidates.is_empty());
    assert_eq!(zero.view.preedit, "臺ㄢ");

    // Space on a lone ㄢ is not a syllable: it is consumed and changes nothing.
    let toned = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(toned.handled && toned.commit.is_none());
    assert_eq!(toned.view.preedit, "臺ㄢ");

    // A digit past the end of the open page is swallowed and leaves the list as it was.
    let reopened = runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    assert!(reopened.view.candidate_list_open);
    let beyond = character(&mut runtime, b'9');
    assert!(beyond.handled && beyond.commit.is_none());
    assert!(beyond.view.candidate_list_open);
    assert_eq!(texts(&beyond.view), texts(&reopened.view));
    assert_eq!(beyond.view.preedit, reopened.view.preedit);

    let closed = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(closed.handled && closed.commit.is_none());
    assert!(!closed.view.candidate_list_open);
    assert!(!closed.view.editing_text.is_empty());
    let cleared = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(cleared.handled && cleared.commit.is_none());
    assert_eq!(cleared.view.editing_text, "");
    assert_eq!(runtime.view().preedit, "");
}

/// A Shift punctuation key commits the conversion followed by its full-width mark, through the character route and the punctuation route alike, whatever the host's punctuation context.
#[test]
fn zhuyin_shift_punctuation_commits_then_inserts_the_full_width_mark() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());

    compose_zhuyin(&mut runtime, "su3cl3");
    let comma = runtime
        .dispatch(Action::Character {
            value: b'<',
            shift: true,
        })
        .unwrap();
    assert!(comma.handled);
    assert_eq!(comma.commit.as_deref(), Some("你好，"));
    assert_eq!(comma.view.editing_text, "");
    assert!(!comma.commit_context.unwrap().script_conversion);

    compose_zhuyin(&mut runtime, "su3");
    let question = runtime.dispatch(Action::Punctuation(b'?')).unwrap();
    assert!(question.handled);
    assert_eq!(question.commit.as_deref(), Some("你？"));
    assert_eq!(question.view.editing_text, "");
}

/// Leaving the client commits the conversion, and attaching a new one discards a conversion left open in the previous one.
#[test]
fn zhuyin_blur_commits_the_conversion() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());

    compose_zhuyin(&mut runtime, "su3cl3");
    let left = runtime.focus(false).unwrap();
    assert_eq!(left.commit.as_deref(), Some("你好"));
    assert_eq!(left.view.editing_text, "");
    runtime.focus(true).unwrap();
    assert!(runtime.focus(false).unwrap().commit.is_none());

    runtime.focus(true).unwrap();
    compose_zhuyin(&mut runtime, "su3");
    runtime
        .dispatch(Action::Command(Command::ConvertHanja))
        .unwrap();
    let attached = runtime.focus(true).unwrap();
    assert!(attached.commit.is_none());
    assert_eq!(attached.view.editing_text, "");
    assert!(!attached.view.candidate_list_open);
}

/// A host that draws held phrase pieces still sees the Zhuyin spelling symbols: the View hides them only behind a held piece, and a Zhuyin pick from the open list joins the conversion rather than starting a phrase, so no piece is ever held.
#[test]
fn zhuyin_spelling_symbols_stay_visible_with_phrase_preedit() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());
    runtime.set_phrase_preedit(true);
    assert_eq!(runtime.view().spelling_symbols, "125890,./;-");

    let typed = compose_zhuyin(&mut runtime, "su3cl3");
    assert_eq!(typed.view.spelling_symbols, "1234567890,./;- ");
    let opened = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(opened.view.candidate_list_open);
    assert_eq!(opened.view.spelling_symbols, "0,./;-");
    assert_eq!(texts(&opened.view), ["你好", "好", "郝"]);

    let picked = character(&mut runtime, b'3');
    assert!(picked.handled && picked.commit.is_none());
    assert_eq!(picked.view.phrase_prefix, "");
    assert_eq!(picked.view.preedit, "你郝");
    assert_eq!(picked.view.spelling_symbols, "1234567890,./;- ");

    // A phonetic mark still spells on every route, where a held piece would have turned it into punctuation.
    let mark = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert!(mark.handled && mark.commit.is_none());
    assert_eq!(mark.view.phrase_prefix, "");
    let enter = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(enter.commit.as_deref(), Some("你郝"));
    assert_eq!(enter.view.phrase_prefix, "");
}

const STROKE_SCHEME: u8 = 8;

/// A `stroke.db` with a few single characters keyed by their stroke letters, written with the shipped schema. `土` has two codes, as characters with variant stroke orders do in the real data. The weights are made up.
fn stroke_dictionary(directory: &std::path::Path) -> String {
    use msime_engine::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let path = directory.join("stroke.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('h'),('s'),('p'),('n'),('z');\
             INSERT INTO entries VALUES ('h','一',9000),('hh','二',5000),('hhh','三',4000),('hs','十',4500),('hsh','土',2000),('hshh','土',10),('hhsh','王',2500),('hpn','大',5500),('pn','人',6000),('szh','口',3500);",
        )
        .unwrap();
    path.to_str().unwrap().to_owned()
}

/// A real Engine on the Stroke scheme with learning on, so a learning path the scheme failed to skip would write. Focused.
fn stroke_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
    options.scheme = STROKE_SCHEME;
    options.learning = true;
    options.stroke_dictionary = stroke_dictionary(directory);
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

/// Types `keys` as plain characters, each one composing without a commit.
fn compose_stroke(runtime: &mut Runtime, keys: &str) -> Transition {
    let mut last = None;
    for value in keys.bytes() {
        let transition = character(runtime, value);
        assert!(
            transition.handled && transition.commit.is_none(),
            "{keys}: {}",
            value as char
        );
        last = Some(transition);
    }
    last.unwrap()
}

/// The stroke letters compose on the character route and the preedit draws their glyphs while editing_text keeps the letters; exact matches lead and completions follow, each character once. Digits 1-9 pick from the visible page, since the scheme spells with no digit or symbol. The text is written as stored, so nothing is script-converted.
#[test]
fn stroke_letters_compose_and_digits_select() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());
    let idle = runtime.view();
    assert_eq!(idle.scheme, STROKE_SCHEME);
    assert_eq!(idle.spelling_symbols, "");
    assert!(idle.chinese_text);
    assert!(!idle.script_conversion);

    let typed = compose_stroke(&mut runtime, "hs");
    assert_eq!(typed.view.scheme, STROKE_SCHEME);
    assert_eq!(typed.view.editing_text, "hs");
    assert_eq!(typed.view.caret_position, 2);
    assert_eq!(typed.view.preedit, "一丨");
    assert_eq!(typed.view.reading, "一丨");
    assert_eq!(texts(&typed.view), ["十", "土"]);
    assert_eq!(typed.view.spelling_symbols, "");
    assert!(!typed.view.candidate_list_open);
    assert!(typed.view.chinese_text);
    assert!(!typed.view.script_conversion);

    let picked = character(&mut runtime, b'2');
    assert!(picked.handled);
    assert_eq!(picked.commit.as_deref(), Some("土"));
    let context = picked.commit_context.unwrap();
    assert_eq!(context.scheme, STROKE_SCHEME);
    assert!(!context.script_conversion);
    assert_eq!(picked.view.editing_text, "");
    assert_eq!(picked.view.preedit, "");

    // A digit past the end of the page is swallowed rather than typed into the document.
    compose_stroke(&mut runtime, "szh");
    let beyond = character(&mut runtime, b'9');
    assert!(beyond.handled && beyond.commit.is_none());
    assert_eq!(beyond.view.editing_text, "szh");
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
}

/// With nothing composed only h s p n z start a composition: the wildcard, the other letters and the digits go back to the host to type. While composing the wildcard appends a stroke that matches any one, other letters are swallowed without touching the composition, Backspace drops the last stroke and Escape clears it all.
#[test]
fn stroke_wildcard_and_other_letters_follow_the_composition() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());

    for value in *b"xa1" {
        let typed = character(&mut runtime, value);
        assert!(
            !typed.handled && typed.commit.is_none(),
            "{}",
            value as char
        );
        assert_eq!(typed.view.editing_text, "", "{}", value as char);
    }

    compose_stroke(&mut runtime, "hs");
    for value in *b"aqy" {
        let swallowed = character(&mut runtime, value);
        assert!(
            swallowed.handled && swallowed.commit.is_none(),
            "{}",
            value as char
        );
        assert_eq!(swallowed.view.editing_text, "hs", "{}", value as char);
        assert_eq!(swallowed.view.preedit, "一丨", "{}", value as char);
    }

    let wildcard = character(&mut runtime, b'x');
    assert!(wildcard.handled && wildcard.commit.is_none());
    assert_eq!(wildcard.view.editing_text, "hsx");
    assert_eq!(wildcard.view.preedit, "一丨＊");
    assert_eq!(texts(&wildcard.view), ["土"]);

    let back = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert!(back.handled && back.commit.is_none());
    assert_eq!(back.view.editing_text, "hs");
    assert_eq!(texts(&back.view), ["十", "土"]);

    // A wildcard in the middle matches any one stroke there.
    let middle = compose_stroke(&mut runtime, "xh");
    assert_eq!(middle.view.preedit, "一丨＊一");
    assert_eq!(texts(&middle.view), ["土"]);

    let cleared = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(cleared.handled && cleared.commit.is_none());
    assert_eq!(cleared.view.editing_text, "");
    assert!(cleared.view.candidates.is_empty());
}

/// Space takes the highlighted row, Enter commits the typed letters, and a composition with no match commits its letters on Space as well. Leaving the client commits nothing.
#[test]
fn stroke_space_picks_and_enter_commits_the_letters() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());

    compose_stroke(&mut runtime, "hh");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(space.handled);
    assert_eq!(space.commit.as_deref(), Some("三"));
    assert_eq!(space.commit_context.unwrap().scheme, STROKE_SCHEME);
    assert_eq!(space.view.editing_text, "");

    compose_stroke(&mut runtime, "pn");
    let enter = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert!(enter.handled);
    assert_eq!(enter.commit.as_deref(), Some("pn"));
    assert_eq!(enter.view.editing_text, "");

    let unmatched = compose_stroke(&mut runtime, "zzz");
    assert!(unmatched.view.candidates.is_empty());
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(space.handled);
    assert_eq!(space.commit.as_deref(), Some("zzz"));
    assert_eq!(space.view.editing_text, "");

    compose_stroke(&mut runtime, "hs");
    let left = runtime.focus(false).unwrap();
    assert!(left.commit.is_none());
}

/// Punctuation while composing commits the top row followed by the full-width mark.
#[test]
fn stroke_punctuation_commits_the_top_row_then_the_mark() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());

    compose_stroke(&mut runtime, "pn");
    let comma = runtime
        .dispatch(Action::Character {
            value: b',',
            shift: false,
        })
        .unwrap();
    assert!(comma.handled);
    assert_eq!(comma.commit.as_deref(), Some("人，"));
    assert_eq!(comma.view.editing_text, "");

    compose_stroke(&mut runtime, "hpn");
    let question = runtime.dispatch(Action::Punctuation(b'?')).unwrap();
    assert!(question.handled);
    assert_eq!(question.commit.as_deref(), Some("大？"));
    assert_eq!(question.view.editing_text, "");

    // The apostrophe separates no syllables under Stroke, so it is punctuation like the rest: the Windows Server and the Linux hosts send it here rather than as composition input.
    compose_stroke(&mut runtime, "pn");
    let apostrophe = runtime.dispatch(Action::Punctuation(b'\'')).unwrap();
    assert!(apostrophe.handled);
    let written = apostrophe.commit.unwrap();
    assert!(
        written.starts_with('人') && written.chars().count() == 2,
        "{written}"
    );
    assert_eq!(apostrophe.view.editing_text, "");
}

/// Picking a lower row again and again does not lift it, and nothing the user picks is written to any database.
#[test]
fn stroke_selections_learn_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());
    let first = compose_stroke(&mut runtime, "hh");
    assert_eq!(texts(&first.view), ["二", "三", "王"]);
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    let before = database_rows(directory.path());

    for _ in 0..3 {
        compose_stroke(&mut runtime, "hh");
        assert_eq!(character(&mut runtime, b'3').commit.as_deref(), Some("王"));
    }
    let again = compose_stroke(&mut runtime, "hh");
    assert_eq!(texts(&again.view), ["二", "三", "王"]);
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();

    drop(runtime);
    msime_engine::flush_personal_learning();
    assert_eq!(database_rows(directory.path()), before);
}
