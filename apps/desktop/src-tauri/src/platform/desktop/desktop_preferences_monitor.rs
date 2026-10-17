use msime_client_core::clipboard::{ClipboardHistoryEntry, ClipboardHistoryStore};
use msime_client_core::preferences::PreferencesStore;
#[cfg(target_os = "linux")]
use std::mem::MaybeUninit;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::sync::Mutex;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

#[cfg(target_os = "macos")]
pub(crate) fn record_macos_clipboard_change(
    snapshot: msime_host_macos::ClipboardSnapshot,
    last_change_count: &mut Option<i64>,
    enabled: bool,
    preferences: &PreferencesStore,
    history: &Mutex<ClipboardHistoryStore>,
) {
    if *last_change_count == Some(snapshot.change_count) {
        return;
    }
    *last_change_count = Some(snapshot.change_count);
    if !enabled {
        return;
    }
    let Some(text) = snapshot.text else { return };
    let already_current = history
        .lock()
        .ok()
        .and_then(|mut history| {
            history.load().ok()?;
            Some(
                history
                    .entries()
                    .first()
                    .is_some_and(|entry| entry.text == text),
            )
        })
        .unwrap_or(false);
    if already_current {
        return;
    }
    let _ = preferences.capture_clipboard_text(text);
    if let Ok(mut history) = history.lock() {
        let _ = history.load();
    }
}

/// Wait for a filesystem notification without making the desktop clipboard
/// panel depend on a fixed polling interval. The IBus host has its own GIO
/// monitor, but the Tauri shell runs in a separate process and must observe
/// writes made by that host (or by the standalone capture service) too.
///
/// Watching the parent directory keeps atomic replacement and delete/recreate
/// writes visible. Any event is sufficient: `poll` still compares the loaded
/// revision and history contents before emitting an invalidation. If inotify
/// is unavailable, retain the bounded timeout as the portable fallback.
#[cfg(target_os = "linux")]
pub(crate) fn wait_for_filesystem_change(path: &Path, timeout: Duration) -> bool {
    use rustix::fs::inotify;
    use rustix::io::Errno;

    let Some(parent) = path.parent() else {
        std::thread::sleep(timeout);
        return false;
    };
    let Ok(inotify) = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
    else {
        std::thread::sleep(timeout);
        return false;
    };
    if inotify::add_watch(
        &inotify,
        parent,
        inotify::WatchFlags::ATTRIB
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::CREATE
            | inotify::WatchFlags::DELETE
            | inotify::WatchFlags::MOVED_FROM
            | inotify::WatchFlags::MOVED_TO,
    )
    .is_err()
    {
        std::thread::sleep(timeout);
        return false;
    }

    let deadline = Instant::now() + timeout;
    let mut buffer = [MaybeUninit::uninit(); 4096];
    let mut reader = inotify::Reader::new(inotify, &mut buffer);
    loop {
        match reader.next() {
            Ok(_) => return true,
            Err(Errno::AGAIN) => {}
            Err(_) => return false,
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        std::thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod inotify_tests {
    use super::wait_for_filesystem_change;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn observes_atomic_history_replacement() {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("clipboard_history.json");
        let writer = history.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            let temporary = writer.with_extension("next");
            fs::write(&temporary, b"[]").unwrap();
            fs::rename(temporary, writer).unwrap();
        });
        assert!(wait_for_filesystem_change(
            &history,
            Duration::from_millis(500)
        ));
    }

    #[test]
    fn bounded_timeout_is_preserved_without_events() {
        let root = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        assert!(!wait_for_filesystem_change(
            &root.path().join("clipboard_history.json"),
            Duration::from_millis(25)
        ));
        assert!(started.elapsed() >= Duration::from_millis(20));
    }
}

/// One shared polling state for all desktop shells. Input capture remains in the host.
pub(crate) struct Monitor {
    revision: Option<u64>,
    history: Option<Vec<ClipboardHistoryEntry>>,
}

impl Monitor {
    pub(crate) fn new(store: &PreferencesStore) -> Self {
        Self {
            revision: store.load().ok().map(|snapshot| snapshot.revision),
            history: None,
        }
    }

    pub(crate) fn poll<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        store: &PreferencesStore,
        history: &Mutex<ClipboardHistoryStore>,
    ) {
        let Ok(snapshot) = store.load() else { return };
        if let Ok(mut history) = history.lock() {
            history.set_limit(snapshot.preferences.clipboard_history_limit);
        }
        let entries = if snapshot.preferences.clipboard_history {
            history
                .lock()
                .ok()
                .and_then(|mut history| history.load().ok().map(|_| history.entries().to_vec()))
        } else {
            Some(Vec::new())
        };
        if let Some(entries) = entries {
            if self.history.as_ref() != Some(&entries) {
                self.history = Some(entries);
                // Clipboard contents never cross the event channel.
                let _ = app.emit("clipboard-history-changed", ());
            }
        }
        if self.revision != Some(snapshot.revision) {
            self.revision = Some(snapshot.revision);
            let _ = app.emit("preferences-changed", snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::mpsc;
    use tauri::Listener;

    #[test]
    fn external_preferences_notify_once_and_corrupt_files_preserve_the_last_revision() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        let writer = PreferencesStore::new(root.path());
        let history = Mutex::new(ClipboardHistoryStore::open(
            root.path().join("clipboard_history.json"),
        ));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (send, received) = mpsc::channel::<Value>();
        app.listen("preferences-changed", move |event| {
            send.send(serde_json::from_str(event.payload()).unwrap())
                .unwrap();
        });
        let mut monitor = Monitor::new(&store);
        monitor.poll(app.handle(), &store, &history);
        assert!(received.try_recv().is_err());
        let initial = writer.load().unwrap();
        let mut preferences = initial.preferences;
        preferences.candidate_page_size = 7;
        let saved = writer.save(initial.revision, preferences).unwrap();
        monitor.poll(app.handle(), &store, &history);
        let event = received.try_recv().unwrap();
        assert_eq!(event["revision"], saved.revision);
        assert_eq!(event["preferences"]["candidate_page_size"], 7);
        monitor.poll(app.handle(), &store, &history);
        assert!(received.try_recv().is_err());

        let path = root.path().join("preferences.json");
        let valid = std::fs::read(&path).unwrap();
        std::fs::write(&path, "synthetic-corrupt-preferences").unwrap();
        monitor.poll(app.handle(), &store, &history);
        assert!(received.try_recv().is_err());
        assert_eq!(monitor.revision, Some(saved.revision));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "synthetic-corrupt-preferences"
        );
        std::fs::write(&path, valid).unwrap();
        let mut preferences = saved.preferences;
        preferences.candidate_page_size = 9;
        let repaired = writer.save(saved.revision, preferences).unwrap();
        monitor.poll(app.handle(), &store, &history);
        assert_eq!(received.try_recv().unwrap()["revision"], repaired.revision);
    }

    #[test]
    fn native_clipboard_changes_only_emit_invalidation_and_disable_clears_cached_text() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        let initial = store.load().unwrap();
        let mut preferences = initial.preferences;
        preferences.clipboard_history = true;
        let enabled = store.save(initial.revision, preferences).unwrap();
        let path = root.path().join("clipboard_history.json");
        let history = Mutex::new(ClipboardHistoryStore::open(&path));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (send, received) = mpsc::channel::<Value>();
        app.listen("clipboard-history-changed", move |event| {
            send.send(serde_json::from_str(event.payload()).unwrap())
                .unwrap();
        });
        let mut monitor = Monitor::new(&store);
        monitor.poll(app.handle(), &store, &history);
        assert_eq!(received.try_recv().unwrap(), Value::Null);
        assert!(store
            .capture_clipboard_text("synthetic-clipboard-text".into())
            .unwrap());
        monitor.poll(app.handle(), &store, &history);
        assert_eq!(received.try_recv().unwrap(), Value::Null);
        assert_eq!(monitor.history.as_ref().unwrap().len(), 1);
        assert_eq!(
            monitor.history.as_ref().unwrap()[0].text,
            "synthetic-clipboard-text"
        );
        monitor.poll(app.handle(), &store, &history);
        assert!(received.try_recv().is_err());
        std::fs::write(&path, "synthetic-corrupt-history").unwrap();
        monitor.poll(app.handle(), &store, &history);
        assert!(received.try_recv().is_err());
        let mut preferences = enabled.preferences;
        preferences.clipboard_history = false;
        store.save(enabled.revision, preferences).unwrap();
        monitor.poll(app.handle(), &store, &history);
        assert_eq!(received.try_recv().unwrap(), Value::Null);
        assert!(monitor.history.as_ref().unwrap().is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_clipboard_changes_are_bounded_by_change_count_and_history_state() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        let initial = store.load().unwrap();
        let mut preferences = initial.preferences;
        preferences.clipboard_history = true;
        store.save(initial.revision, preferences).unwrap();
        let history = Mutex::new(ClipboardHistoryStore::open(
            root.path().join("clipboard_history.json"),
        ));
        let mut last_change_count = None;
        record_macos_clipboard_change(
            msime_host_macos::ClipboardSnapshot {
                change_count: 1,
                text: Some("synthetic-external".into()),
            },
            &mut last_change_count,
            true,
            &store,
            &history,
        );
        let first = history.lock().unwrap().entries().to_vec();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].text, "synthetic-external");
        record_macos_clipboard_change(
            msime_host_macos::ClipboardSnapshot {
                change_count: 1,
                text: Some("synthetic-ignored".into()),
            },
            &mut last_change_count,
            true,
            &store,
            &history,
        );
        assert_eq!(history.lock().unwrap().entries(), first.as_slice());
        record_macos_clipboard_change(
            msime_host_macos::ClipboardSnapshot {
                change_count: 2,
                text: Some("synthetic-external".into()),
            },
            &mut last_change_count,
            true,
            &store,
            &history,
        );
        assert_eq!(history.lock().unwrap().entries(), first.as_slice());
        // The native reader reports excluded (concealed, transient, password-manager) or text-less changes as a new count with no text.
        record_macos_clipboard_change(
            msime_host_macos::ClipboardSnapshot {
                change_count: 5,
                text: None,
            },
            &mut last_change_count,
            true,
            &store,
            &history,
        );
        assert_eq!(history.lock().unwrap().entries(), first.as_slice());
        assert_eq!(last_change_count, Some(5));
        record_macos_clipboard_change(
            msime_host_macos::ClipboardSnapshot {
                change_count: 3,
                text: Some("synthetic-second".into()),
            },
            &mut last_change_count,
            false,
            &store,
            &history,
        );
        assert_eq!(history.lock().unwrap().entries(), first.as_slice());
    }
}
