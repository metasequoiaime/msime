//! 输入线程的可选工作降级。
//!
//! 输入法的基础候选必须留在当前线程并及时返回；神经重排、停顿模型这类可选工作可以
//! 暂时让路。商业输入法通常会同时看系统负载、推理耗时和任务队列长度，本模块保留同
//! 一条边界：只要观察到调度压力或输入动作超出一帧，就在一个很短的窗口内停掉可选
//! 工作，基础词典路径不受影响。

use msime_engine::time::Instant;
use std::time::Duration;

const INPUT_FRAME_BUDGET: Duration = Duration::from_millis(16);
const SHED_WINDOW: Duration = Duration::from_millis(300);
const PRESSURE_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
pub(crate) struct LoadMonitor {
    shed_until: Option<Instant>,
    external_pressure: bool,
    last_sample: Option<Instant>,
}

impl LoadMonitor {
    /// 在一个输入动作开始时取样。系统负载探针有节流，不会为每个按键打开文件。
    pub(crate) fn begin(&mut self) {
        let now = Instant::now();
        if self
            .last_sample
            .is_none_or(|sampled| now.duration_since(sampled) >= PRESSURE_SAMPLE_INTERVAL)
        {
            self.external_pressure = system_pressure();
            self.last_sample = Some(now);
        }
        if self.external_pressure {
            self.shed_until = Some(now + SHED_WINDOW);
        }
    }

    /// 记录输入动作耗时。超过一帧时，下一小段时间让可选模型让路。
    pub(crate) fn finish(&mut self, elapsed: Duration) {
        if elapsed > INPUT_FRAME_BUDGET {
            self.shed_until = Some(Instant::now() + SHED_WINDOW);
        }
    }

    pub(crate) fn should_shed(&self) -> bool {
        self.external_pressure || self.shed_until.is_some_and(|until| Instant::now() < until)
    }

    #[cfg(test)]
    pub(crate) fn force_shed(&mut self) {
        self.shed_until = Some(Instant::now() + SHED_WINDOW);
        self.external_pressure = false;
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn external_pressure_for_test(&mut self) -> bool {
        self.last_sample = None;
        self.external_pressure = system_pressure_probe();
        self.last_sample = Some(Instant::now());
        self.external_pressure
    }
}

/// Linux/Android 的 cgroup CPU pressure 和 `/proc/loadavg` 是轻量且不需要额外依赖的
/// 系统负载来源。
///
/// 只把明显超过可用 CPU 容量的情况当作压力；短暂的一次高负载不会改变基础候选
/// 语义，只会让可选模型暂缓。其他宿主没有统一的无权限系统接口，继续由输入耗时
/// 自适应降级。
#[cfg(any(target_os = "linux", all(target_os = "android", not(test))))]
fn system_pressure_probe() -> bool {
    // Docker、systemd 和 Android 的 cgroup v2 会把当前进程组的 CPU 等待写在
    // `cpu.pressure`；它比宿主机共享的 loadavg 更能反映输入法自己是否被限流。
    if std::fs::read_to_string("/sys/fs/cgroup/cpu.pressure")
        .ok()
        .is_some_and(|pressure| {
            pressure.lines().any(|line| {
                line.split_ascii_whitespace()
                    .filter_map(|field| field.strip_prefix("avg10="))
                    .filter_map(|value| value.parse::<f64>().ok())
                    .any(|value| value >= 5.0)
            })
        })
    {
        return true;
    }
    let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") else {
        return false;
    };
    let Some(load) = loadavg
        .split_ascii_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
    else {
        return false;
    };
    let cpus = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1) as f64;
    load > (cpus * 0.85).max(1.5)
}

#[cfg(all(not(test), not(any(target_os = "linux", target_os = "android"))))]
fn system_pressure_probe() -> bool {
    false
}

// 普通单测隔离外部负载，避免共享测试主机改变候选次序；
// Docker 压力专项通过上面的测试入口直接运行真实探针。
#[cfg(test)]
fn system_pressure() -> bool {
    false
}

#[cfg(not(test))]
fn system_pressure() -> bool {
    system_pressure_probe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_input_sheds_optional_work_for_a_short_window() {
        let mut monitor = LoadMonitor::default();
        assert!(!monitor.should_shed());
        monitor.finish(INPUT_FRAME_BUDGET + Duration::from_millis(1));
        assert!(monitor.should_shed());
    }

    #[test]
    fn forced_shedding_does_not_change_the_external_pressure_state() {
        let mut monitor = LoadMonitor::default();
        monitor.force_shed();
        assert!(monitor.should_shed());
        assert!(!monitor.external_pressure);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "需要 Docker 或其他外部负载才能验证系统压力探针"]
    fn linux_external_pressure_probe_observes_a_loaded_container() {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let mut monitor = LoadMonitor::default();
            if monitor.external_pressure_for_test() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("system pressure probe did not report external pressure");
    }
}
