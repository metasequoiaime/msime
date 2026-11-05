//! 设置进程只管理同一安装配置的后台项；配置工作与剪贴板轮询分开执行。
use std::process::Child;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub(crate) const BACKGROUND_SETUP_TIMEOUT: Duration = Duration::from_secs(25);

pub(crate) fn owns_installed_host(
    packaged: bool,
    default_options: bool,
    default_state: bool,
    native_directory: bool,
) -> bool {
    packaged && default_options && default_state && native_directory
}

struct GlobalVoiceSetup {
    managed: bool,
    desired: Option<bool>,
    applied: Option<bool>,
    failures: u32,
    retry_at: Duration,
}
impl GlobalVoiceSetup {
    fn new(managed: bool) -> Self {
        Self {
            managed,
            desired: None,
            applied: None,
            failures: 0,
            retry_at: Duration::ZERO,
        }
    }
    fn observe(&mut self, enabled: bool) {
        if self.desired != Some(enabled) {
            // 首次关闭不触碰系统；尝试过启用后，关闭必须撤销可能已经登记的后台项。
            if self.desired.is_none() && !enabled {
                self.applied = Some(false);
            }
            self.desired = Some(enabled);
            self.failures = 0;
            self.retry_at = Duration::ZERO;
        }
    }
    fn due(&self, now: Duration) -> bool {
        self.managed && self.applied != self.desired && now >= self.retry_at
    }
    fn complete(
        &mut self,
        enabled: bool,
        now: Duration,
        result: Result<(), String>,
    ) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.applied = Some(enabled);
                self.failures = 0;
                Ok(())
            }
            Err(error) => {
                self.applied = None;
                // 旧目标的失败不延迟已经排队的新目标，尤其不能延迟关闭。
                if self.desired == Some(enabled) {
                    self.failures = self.failures.saturating_add(1);
                    self.retry_at =
                        now + Duration::from_secs((1u64 << self.failures.min(6)).min(60));
                }
                Err(error)
            }
        }
    }
    #[cfg(test)]
    fn poll(
        &mut self,
        enabled: bool,
        now: Duration,
        mut configure: impl FnMut(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.managed {
            return Ok(());
        }
        self.observe(enabled);
        if !self.due(now) {
            return Ok(());
        }
        self.complete(enabled, now, configure(enabled))
    }
}

pub(crate) struct GlobalVoiceSetupWorker {
    setup: GlobalVoiceSetup,
    pending: Option<(bool, Receiver<Result<(), String>>)>,
}
impl GlobalVoiceSetupWorker {
    pub(crate) fn new(managed: bool) -> Self {
        Self {
            setup: GlobalVoiceSetup::new(managed),
            pending: None,
        }
    }
    pub(crate) fn poll(
        &mut self,
        enabled: bool,
        now: Duration,
        configure: impl FnOnce(bool) -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        if !self.setup.managed {
            return Ok(());
        }
        self.setup.observe(enabled);
        let mut completed = Ok(());
        if let Some((attempt, receiver)) = self.pending.take() {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => {
                    self.pending = Some((attempt, receiver));
                    return Ok(());
                }
                Err(TryRecvError::Disconnected) => Err("background_setup_worker_failed".into()),
            };
            completed = self.setup.complete(attempt, now, result);
        }
        if self.setup.due(now) {
            let (sender, receiver) = mpsc::channel();
            match std::thread::Builder::new()
                .name("msime-global-voice-setup".into())
                .spawn(move || {
                    let _ = sender.send(configure(enabled));
                }) {
                Ok(_) => self.pending = Some((enabled, receiver)),
                Err(_) => {
                    return self.setup.complete(
                        enabled,
                        now,
                        Err("background_setup_worker_failed".into()),
                    )
                }
            }
        }
        completed
    }
}

pub(crate) fn wait_for_background_setup(mut child: Child, budget: Duration) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("background_setup_failed".into())
                }
            }
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("background_setup_failed".into());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("background_setup_timeout".into());
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_development_and_locator_overrides_never_manage_installed_service() {
        for bits in 0..16 {
            let values: Vec<bool> = (0..4).map(|i| bits & (1 << i) != 0).collect();
            assert_eq!(
                owns_installed_host(values[0], values[1], values[2], values[3]),
                bits == 15
            );
            let mut setup = GlobalVoiceSetup::new(owns_installed_host(
                values[0], values[1], values[2], values[3],
            ));
            let mut calls = 0;
            setup
                .poll(true, Duration::ZERO, |_| {
                    calls += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(calls, if bits == 15 { 1 } else { 0 });
        }
    }
    #[test]
    fn failures_retry_after_backoff_and_only_success_is_cached() {
        let mut setup = GlobalVoiceSetup::new(true);
        assert!(setup
            .poll(true, Duration::ZERO, |_| Err("synthetic".into()))
            .is_err());
        setup
            .poll(true, Duration::from_secs(1), |_| {
                panic!("退避期间不能重复启动")
            })
            .unwrap();
        let mut calls = 0;
        setup
            .poll(true, Duration::from_secs(2), |_| {
                calls += 1;
                Ok(())
            })
            .unwrap();
        setup
            .poll(true, Duration::from_secs(70), |_| {
                panic!("成功后不重复登记")
            })
            .unwrap();
        assert_eq!(calls, 1);
    }
    #[test]
    fn disable_after_failed_enable_is_not_lost_or_delayed() {
        let mut setup = GlobalVoiceSetup::new(true);
        setup
            .poll(false, Duration::ZERO, |_| panic!("首次关闭不触碰系统"))
            .unwrap();
        assert!(setup
            .poll(true, Duration::ZERO, |_| Err("synthetic".into()))
            .is_err());
        let mut disabled = false;
        setup
            .poll(false, Duration::ZERO, |enabled| {
                disabled = !enabled;
                Ok(())
            })
            .unwrap();
        assert!(disabled);
    }
    #[test]
    fn persistent_failure_is_rate_limited_but_recovers_after_installation() {
        let mut setup = GlobalVoiceSetup::new(true);
        let mut calls = 0;
        for second in 0..600 {
            let _ = setup.poll(true, Duration::from_secs(second), |_| {
                calls += 1;
                Err("synthetic".into())
            });
        }
        assert!(calls < 16);
        setup
            .poll(true, Duration::from_secs(660), |_| Ok(()))
            .unwrap();
        setup
            .poll(true, Duration::from_secs(720), |_| {
                panic!("恢复后不重复登记")
            })
            .unwrap();
    }
    #[test]
    fn blocked_setup_does_not_block_polling_and_latest_disable_runs_after_it() {
        let mut worker = GlobalVoiceSetupWorker::new(true);
        let (started, start) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        worker
            .poll(true, Duration::ZERO, move |enabled| {
                started.send(enabled).unwrap();
                wait.recv_timeout(Duration::from_secs(3)).unwrap();
                Ok(())
            })
            .unwrap();
        assert!(start.recv_timeout(Duration::from_secs(1)).unwrap());
        let began = Instant::now();
        for _ in 0..5 {
            worker
                .poll(false, Duration::ZERO, |_| {
                    panic!("前一轮未完成，不能并发配置")
                })
                .unwrap();
        }
        assert!(began.elapsed() < Duration::from_secs(1));
        assert_eq!(worker.setup.applied, None);
        release.send(()).unwrap();
        let (disabled, observed) = mpsc::channel();
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.setup.applied != Some(false) && Instant::now() < deadline {
            let disabled = disabled.clone();
            worker
                .poll(false, Duration::from_secs(1), move |enabled| {
                    disabled.send(enabled).unwrap();
                    Ok(())
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!observed.recv_timeout(Duration::from_secs(1)).unwrap());
        assert_eq!(worker.setup.applied, Some(false));
        assert!(observed.try_recv().is_err());
    }

    #[test]
    fn worker_failure_does_not_cache_success_and_retry_keeps_backoff() {
        let mut worker = GlobalVoiceSetupWorker::new(true);
        worker
            .poll(true, Duration::ZERO, |_| Err("synthetic".into()))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut failure = None;
        while Instant::now() < deadline {
            if let Err(error) = worker.poll(true, Duration::ZERO, |_| panic!("失败结果尚未消费"))
            {
                failure = Some(error);
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(failure.as_deref(), Some("synthetic"));
        assert_eq!(worker.setup.applied, None);
        worker
            .poll(true, Duration::from_secs(1), |_| panic!("退避期间不能重试"))
            .unwrap();
        worker
            .poll(true, Duration::from_secs(2), |_| Ok(()))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.setup.applied != Some(true) && Instant::now() < deadline {
            worker
                .poll(true, Duration::from_secs(2), |_| panic!("成功只执行一次"))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(worker.setup.applied, Some(true));
        worker
            .poll(true, Duration::from_secs(100), |_| {
                panic!("已经成功，不再启动")
            })
            .unwrap();
    }

    #[test]
    fn stale_failed_enable_does_not_delay_disable() {
        let mut worker = GlobalVoiceSetupWorker::new(true);
        let (release, wait) = mpsc::channel();
        worker
            .poll(true, Duration::ZERO, move |_| {
                wait.recv_timeout(Duration::from_secs(3)).unwrap();
                Err("synthetic".into())
            })
            .unwrap();
        worker
            .poll(false, Duration::ZERO, |_| panic!("不能并发配置"))
            .unwrap();
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.setup.applied != Some(false) && Instant::now() < deadline {
            let _ = worker.poll(false, Duration::ZERO, |enabled| {
                assert!(!enabled);
                Ok(())
            });
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(worker.setup.applied, Some(false));
    }

    #[test]
    fn isolated_worker_never_starts_a_configuration() {
        let mut worker = GlobalVoiceSetupWorker::new(false);
        worker
            .poll(true, Duration::ZERO, |_| panic!("隔离配置不管理正式宿主"))
            .unwrap();
        assert!(worker.pending.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn helper_wait_accepts_two_phases_and_reaps_a_timed_out_child() {
        use std::process::{Command, Stdio};
        let child = Command::new("/bin/sh")
            .args(["-c", "/bin/sleep 0.02; /bin/sleep 0.02"])
            .spawn()
            .unwrap();
        assert!(wait_for_background_setup(child, BACKGROUND_SETUP_TIMEOUT).is_ok());
        let child = Command::new("/bin/sh")
            .args(["-c", "exit 3"])
            .spawn()
            .unwrap();
        assert_eq!(
            wait_for_background_setup(child, Duration::from_secs(2)),
            Err("background_setup_failed".into())
        );
        let child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let pid = child.id();
        let began = Instant::now();
        assert_eq!(
            wait_for_background_setup(child, Duration::from_millis(20)),
            Err("background_setup_timeout".into())
        );
        assert!(began.elapsed() < Duration::from_secs(2));
        assert!(!Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }
    #[test]
    fn pending_setup_coalesces_back_to_its_original_value() {
        let mut worker = GlobalVoiceSetupWorker::new(true);
        let (release, wait) = mpsc::channel();
        worker
            .poll(true, Duration::ZERO, move |_| {
                wait.recv_timeout(Duration::from_secs(3)).unwrap();
                Ok(())
            })
            .unwrap();
        worker
            .poll(false, Duration::ZERO, |_| panic!("不能并发关闭"))
            .unwrap();
        worker
            .poll(true, Duration::ZERO, |_| panic!("不能并发重新开启"))
            .unwrap();
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while worker.setup.applied != Some(true) && Instant::now() < deadline {
            worker
                .poll(true, Duration::ZERO, |_| panic!("旧结果已经满足最新目标"))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(worker.setup.applied, Some(true));
        assert!(worker.pending.is_none());
    }
}
