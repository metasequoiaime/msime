//! 输入宿主只启动线程；偏好读取、变化检测和同步始终留在该线程。

use super::INVALID;
use msime_client_core::preferences::PreferencesStore;
use msime_client_services::settings_webdav::{SettingsSyncTrigger, SettingsWebDav};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_secs(5);
const RETRY: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    preferences_directory: String,
}

#[derive(Default)]
struct Signal {
    stopped: Mutex<bool>,
    wake: Condvar,
}

impl Signal {
    fn stop(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.wake.notify_all();
    }

    fn wait(&self, duration: Duration) -> bool {
        let stopped = self.stopped.lock().unwrap_or_else(PoisonError::into_inner);
        let (stopped, _) = self
            .wake
            .wait_timeout_while(stopped, duration, |stopped| !*stopped)
            .unwrap_or_else(PoisonError::into_inner);
        *stopped
    }
}

struct Worker {
    directory: PathBuf,
    signal: Arc<Signal>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn start(
        directory: PathBuf,
        mut tick: impl FnMut() -> Duration + Send + 'static,
    ) -> std::io::Result<Self> {
        let signal = Arc::new(Signal::default());
        let flag = signal.clone();
        let (finished, done) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("msime-settings-sync".into())
            .spawn(move || {
                while !flag.wait(Duration::ZERO) {
                    let delay = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut tick))
                        .unwrap_or(RETRY);
                    if flag.wait(delay) {
                        break;
                    }
                }
                let _ = finished.send(());
            })?;
        Ok(Self {
            directory,
            signal,
            done,
            thread: Some(thread),
        })
    }

    fn stop(&mut self, wait: Option<Duration>) -> bool {
        self.signal.stop();
        let exited = match wait {
            Some(wait) => !matches!(
                self.done.recv_timeout(wait),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            None => {
                let _ = self.done.recv();
                true
            }
        };
        if exited {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
        exited
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.signal.stop();
    }
}

struct Local {
    directory: PathBuf,
    revision: Option<u64>,
    due: Instant,
}

impl Local {
    fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            revision: None,
            due: Instant::now(),
        }
    }

    fn tick(&mut self) -> Duration {
        let now = Instant::now();
        let store = PreferencesStore::new(&self.directory);
        let snapshot = match super::configuration::load(&store) {
            Ok(snapshot) => snapshot,
            Err(_) => return RETRY,
        };
        let changed = self
            .revision
            .is_some_and(|previous| previous != snapshot.revision);
        let first = self.revision.is_none();
        self.revision = Some(snapshot.revision);
        if !first && !changed && now < self.due {
            return POLL.min(self.due.saturating_duration_since(now));
        }
        let config = snapshot.preferences.settings_sync_webdav;
        if !config.is_configured() {
            return POLL;
        }
        let trigger = if changed {
            SettingsSyncTrigger::LocalChange
        } else {
            SettingsSyncTrigger::Periodic
        };
        let result = SettingsWebDav::new(
            &config.url,
            &config.username,
            &config.password,
            self.directory.join("settings-sync"),
            &msime_client_services::account::cloud_clipboard_device_name(),
        )
        .map(|service| service.existing_identity_only())
        .and_then(|service| service.run_scheduled(&store, trigger));
        let delay = result.map_or(RETRY, |run| {
            Duration::from_millis(run.next_check_ms.clamp(1_000, 30 * 60 * 1_000))
        });
        self.due = Instant::now() + delay;
        POLL.min(delay)
    }
}

static BACKGROUND: Mutex<Option<Worker>> = Mutex::new(None);

pub(crate) fn start_json(bytes: &[u8]) -> Result<Value, String> {
    let request: Request = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let directory = Path::new(&request.preferences_directory);
    if !directory.is_absolute()
        || request.preferences_directory.len() > 16_384
        || request.preferences_directory.contains('\0')
    {
        return Err(INVALID.into());
    }
    let mut background = BACKGROUND.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(previous) = background.as_mut() {
        if previous.directory == directory && !previous.signal.wait(Duration::ZERO) {
            return Ok(json!({}));
        }
        if !previous.stop(Some(Duration::ZERO)) {
            return Err("settings_sync_busy".into());
        }
    }
    let mut local = Local::new(directory.to_owned());
    let worker = Worker::start(directory.to_owned(), move || local.tick())
        .map_err(|_| "settings_sync_unavailable")?;
    *background = Some(worker);
    Ok(json!({}))
}

/// 未等到的工作线程仍由本进程持有，后续 stop 可以继续等待，不能误报已经退出。
pub(crate) fn stop(timeout_ms: u32) -> bool {
    let mut background = BACKGROUND.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(worker) = background.as_mut() else {
        return true;
    };
    let wait = (timeout_ms != u32::MAX).then(|| Duration::from_millis(timeout_ms.into()));
    if !worker.stop(wait) {
        return false;
    }
    *background = None;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_returns_before_a_round_finishes_and_timeout_keeps_the_live_handle() {
        let (entered, entry) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let mut worker = Worker::start(PathBuf::from("synthetic"), move || {
            entered.send(()).unwrap();
            gate.recv().unwrap();
            Duration::from_secs(3600)
        })
        .unwrap();
        entry.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(!worker.stop(Some(Duration::ZERO)));
        assert!(worker.thread.is_some());
        release.send(()).unwrap();
        assert!(worker.stop(Some(Duration::from_secs(10))));
        assert!(worker.thread.is_none());
    }

    #[test]
    fn stopping_a_sleeping_worker_wakes_it_without_another_round() {
        let (entered, entry) = mpsc::channel();
        let mut worker = Worker::start(PathBuf::from("synthetic"), move || {
            entered.send(()).unwrap();
            Duration::from_secs(3600)
        })
        .unwrap();
        entry.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(worker.stop(Some(Duration::from_secs(10))));
        assert!(entry.try_recv().is_err());
    }

    #[test]
    fn unconfigured_or_unpaired_hosts_do_not_create_sync_state() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("not-created");
        let mut local = Local::new(directory.clone());
        assert_eq!(local.tick(), POLL);
        assert!(!directory.exists());
        let store = PreferencesStore::new(&directory);
        store.apply_patch(0, &json!({"settings_sync_webdav":{"url":"http://127.0.0.1:9/dav", "username":"synthetic-user", "password":"synthetic-app-password"}})).unwrap();
        local.tick();
        assert!(!directory.join("settings-sync").exists());
        assert_eq!(store.load().unwrap().revision, 1);
    }

    #[test]
    fn invalid_start_paths_are_rejected_before_a_thread_is_created() {
        for path in ["relative", "/synthetic\0path"] {
            assert_eq!(
                start_json(&serde_json::to_vec(&json!({"preferences_directory":path})).unwrap())
                    .unwrap_err(),
                INVALID
            );
        }
    }
}
