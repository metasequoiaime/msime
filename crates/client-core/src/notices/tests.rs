use super::*;
use std::cell::Cell;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;

fn notice(id: &str) -> Notice {
    Notice {
        id: id.to_owned(),
        title: format!("公告 {id}"),
        body: "**维护**通知".to_owned(),
        targets: vec!["all".to_owned()],
        channels: vec!["app".to_owned()],
        published_at: "2026-10-01T03:00:00Z".to_owned(),
    }
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_790_856_000 + seconds)
}

#[test]
fn markdown_renders_with_raw_html_escaped() {
    assert_eq!(
        markdown_to_html("# 标题\n\n**粗体** 和 *斜体*\n\n- 一\n- 二"),
        "<h1>标题</h1>\n<p><strong>粗体</strong> 和 <em>斜体</em></p>\n<ul>\n<li>一</li>\n<li>二</li>\n</ul>\n"
    );
    let html =
        markdown_to_html("<script>alert(1)</script>\n\n文字 <img src=x onerror=alert(1)> 结束");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
}

#[test]
fn only_web_and_mail_links_survive() {
    assert_eq!(
        markdown_to_html("[下载](https://msime.app/download)"),
        "<p><a href=\"https://msime.app/download\">下载</a></p>\n"
    );
    assert_eq!(
        markdown_to_html("[写信](mailto:support@msime.app)"),
        "<p><a href=\"mailto:support@msime.app\">写信</a></p>\n"
    );
    for unsafe_link in [
        "[点我](javascript:alert(1))",
        "[点我](JAVASCRIPT:alert(1))",
        "[点我](data:text/html,x)",
        "[点我](file:///etc/passwd)",
        "[点我](/relative)",
        "<javascript:alert(1)>",
    ] {
        let html = markdown_to_html(unsafe_link);
        assert!(!html.contains("<a"), "{unsafe_link} -> {html}");
    }
    assert_eq!(
        markdown_to_html("[点我](javascript:alert(1))"),
        "<p>点我</p>\n"
    );
}

#[test]
fn images_become_links_and_are_never_loaded() {
    assert_eq!(
        markdown_to_html("![截图](https://msime.app/a.png)"),
        "<p><a href=\"https://msime.app/a.png\">截图</a></p>\n"
    );
    assert_eq!(markdown_to_html("![截图](javascript:x)"), "<p>截图</p>\n");
}

#[test]
fn the_cache_is_used_within_a_minute_and_refreshed_after() {
    let directory = tempfile::tempdir().unwrap();
    let store = NoticeStore::new(directory.path().join("notices"));
    let calls = Cell::new(0);
    let fetch = |items: Vec<Notice>| {
        let calls = &calls;
        move || {
            calls.set(calls.get() + 1);
            Ok(items)
        }
    };
    let first = store
        .current(
            NoticeChannel::App,
            "win",
            at(0),
            fetch(vec![notice("2"), notice("1")]),
        )
        .unwrap();
    assert_eq!(first, [notice("2"), notice("1")]);
    let cached = store
        .current(NoticeChannel::App, "windows", at(59), fetch(vec![]))
        .unwrap();
    assert_eq!(cached, first);
    assert_eq!(calls.get(), 1);

    // A failed refresh keeps showing the cached feed, and is not retried within the minute.
    let failed = store
        .current(NoticeChannel::App, "windows", at(61), || {
            Err(AccountError::Unavailable)
        })
        .unwrap();
    assert_eq!(failed, first);
    store
        .current(NoticeChannel::App, "windows", at(100), fetch(vec![]))
        .unwrap();
    assert_eq!(calls.get(), 1);

    let refreshed = store
        .current(
            NoticeChannel::App,
            "windows",
            at(122),
            fetch(vec![notice("3")]),
        )
        .unwrap();
    assert_eq!(refreshed, [notice("3")]);
    assert_eq!(calls.get(), 2);

    // Another platform's feed is never served from this one's cache.
    let other = store
        .current(
            NoticeChannel::App,
            "linux",
            at(123),
            fetch(vec![notice("9")]),
        )
        .unwrap();
    assert_eq!(other, [notice("9")]);
}

#[test]
fn dismissed_notices_stay_hidden_until_they_leave_the_feed() {
    let directory = tempfile::tempdir().unwrap();
    let store = NoticeStore::new(directory.path());
    store
        .current(NoticeChannel::App, "android", at(0), || {
            Ok(vec![notice("2"), notice("1")])
        })
        .unwrap();
    store.dismiss("2").unwrap();
    store.dismiss("2").unwrap();
    assert!(store.dismiss("../x").is_err());
    let shown = store
        .current(NoticeChannel::App, "android", at(1), || Ok(vec![]))
        .unwrap();
    assert_eq!(shown, [notice("1")]);
    let refreshed = store
        .current(NoticeChannel::App, "android", at(120), || {
            Ok(vec![notice("2"), notice("1")])
        })
        .unwrap();
    assert_eq!(refreshed, [notice("1")]);
    // Once "2" has left the feed its dismissal is forgotten, so the list cannot grow without bound.
    store
        .current(NoticeChannel::App, "android", at(240), || {
            Ok(vec![notice("1")])
        })
        .unwrap();
    let back = store
        .current(NoticeChannel::App, "android", at(360), || {
            Ok(vec![notice("2"), notice("1")])
        })
        .unwrap();
    assert_eq!(back, [notice("2"), notice("1")]);
}

#[test]
fn an_unknown_platform_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let store = NoticeStore::new(directory.path());
    assert_eq!(
        store.current(NoticeChannel::App, "beos", at(0), || Ok(vec![])),
        Err(NoticeError::Invalid)
    );
}

#[cfg(unix)]
#[test]
fn cache_read_ignores_a_symlinked_cache_file() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    let cached = NoticeCache {
        feed: "app/windows".into(),
        attempted_at_unix_ms: 1,
        items: vec![notice("external")],
        dismissed: Vec::new(),
    };
    std::fs::write(external.path(), serde_json::to_vec(&cached).unwrap()).unwrap();
    symlink(external.path(), directory.path().join(NOTICES_FILE)).unwrap();

    let store = NoticeStore::new(directory.path());
    assert!(store.read().items.is_empty());
}

#[cfg(unix)]
#[test]
fn cache_read_ignores_a_symlinked_cache_directory() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let cached = NoticeCache {
        feed: "app/windows".into(),
        attempted_at_unix_ms: 1,
        items: vec![notice("external")],
        dismissed: Vec::new(),
    };
    std::fs::write(
        external.path().join(NOTICES_FILE),
        serde_json::to_vec(&cached).unwrap(),
    )
    .unwrap();
    let linked = directory.path().join("linked");
    symlink(external.path(), &linked).unwrap();

    let store = NoticeStore::new(linked);
    assert!(store.read().items.is_empty());
}

#[test]
fn the_feed_is_fetched_anonymously_and_bad_items_are_left_out() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let body = serde_json::json!({"items": [
            {"id": "7", "title": "新版本", "body": "见 [更新日志](https://msime.app)", "targets": ["harmony"], "channels": ["app"], "published_at": "2026-10-01T03:00:00Z", "future_field": 1},
            {"id": "6", "title": "", "body": "", "targets": [], "channels": [], "published_at": "2026-09-30T03:00:00Z"}
        ]})
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: public, max-age=60\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::io::Write::write_all(reader.get_mut(), response.as_bytes()).unwrap();
        head
    });
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let items = fetch_notices(&client, NoticeChannel::App, "ohos").unwrap();
    let head = server.join().unwrap();
    assert!(head.starts_with("GET /v1/notices?channel=app&platform=harmony HTTP/1.1"));
    assert!(!head.to_ascii_lowercase().contains("authorization"));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "7");
    assert_eq!(items[0].targets, ["harmony"]);
}
