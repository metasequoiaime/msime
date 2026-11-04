//! 设置进程只管理同一安装配置的后台项；失败按有上限的退避间隔重试。
use std::time::Duration;

pub(crate) fn owns_installed_host(
    packaged: bool,
    default_options: bool,
    default_state: bool,
    native_directory: bool,
) -> bool {
    packaged && default_options && default_state && native_directory
}

pub(crate) struct GlobalVoiceSetup {
    managed: bool,
    desired: Option<bool>,
    applied: Option<bool>,
    failures: u32,
    retry_at: Duration,
}
impl GlobalVoiceSetup {
    pub(crate) fn new(managed: bool) -> Self {
        Self {
            managed,
            desired: None,
            applied: None,
            failures: 0,
            retry_at: Duration::ZERO,
        }
    }
    pub(crate) fn poll(
        &mut self,
        enabled: bool,
        now: Duration,
        mut configure: impl FnMut(bool) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.managed {
            return Ok(());
        }
        if self.desired != Some(enabled) {
            // 尚未尝试过启用的首次关闭不触碰系统；尝试启用后即使返回失败，关闭也必须撤销可能已登记的服务。
            if self.desired.is_none() && !enabled {
                self.applied = Some(false);
            }
            self.desired = Some(enabled);
            self.failures = 0;
            self.retry_at = Duration::ZERO;
        }
        if self.applied == Some(enabled) || now < self.retry_at {
            return Ok(());
        }
        self.applied = None;
        match configure(enabled) {
            Ok(()) => {
                self.applied = Some(enabled);
                self.failures = 0;
                Ok(())
            }
            Err(error) => {
                self.failures = self.failures.saturating_add(1);
                self.retry_at = now + Duration::from_secs((1u64 << self.failures.min(6)).min(60));
                Err(error)
            }
        }
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
}
