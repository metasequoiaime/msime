//! `--latency`：给模拟用户的每一次派发计时，量一段真实打字流里每次按键花多少毫秒。
//!
//! 和 `rerank_latency` 不同，这里每块打完就上屏、上文一路变长，选词、锁拼写、退格都照真实的顺序发生，所以上屏后第一次按键要重读上文的那笔开销也在样本里。两臂（挂不挂键盘模型）选的词不全相同，样本不按键配对，比较的是两次运行的分布；同一台机器上要交替跑几轮再看。

use std::time::{Duration, Instant};

/// 派发的种类。按键和退格是用户每打一下都要等的，选词和锁拼写是点候选栏。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    Backspace,
    Select,
    Lock,
    Cancel,
}

impl Kind {
    const ALL: [Kind; 5] = [
        Kind::Key,
        Kind::Backspace,
        Kind::Select,
        Kind::Lock,
        Kind::Cancel,
    ];

    fn name(self) -> &'static str {
        match self {
            Kind::Key => "key",
            Kind::Backspace => "backspace",
            Kind::Select => "select",
            Kind::Lock => "lock",
            Kind::Cancel => "cancel",
        }
    }
}

struct Sample {
    kind: Kind,
    /// 一块的第一键：上屏或换了上文之后的第一次按键。
    opens_chunk: bool,
    /// 这一键之前已经打了几个键（这一块里），量长串的尾巴。
    depth: usize,
    ms: f64,
}

#[derive(Default)]
pub struct Latency {
    samples: Vec<Sample>,
}

impl Latency {
    pub fn start() -> Instant {
        Instant::now()
    }

    pub fn record(&mut self, kind: Kind, opens_chunk: bool, depth: usize, elapsed: Duration) {
        self.samples.push(Sample {
            kind,
            opens_chunk,
            depth,
            ms: elapsed.as_secs_f64() * 1000.0,
        });
    }

    /// 汇总成 JSON。`skip` 条最早的样本不计：进程里第一次按键要打开词库、加载模型，那是启动的开销，单独列在 `first`。
    pub fn to_json(&self, skip: usize, budget_ms: f64) -> serde_json::Value {
        let kept = &self.samples[skip.min(self.samples.len())..];
        let summarize = |values: Vec<f64>| -> serde_json::Value {
            let mut sorted = values;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = sorted.len();
            let pick = |p: f64| -> f64 {
                if n == 0 {
                    return 0.0;
                }
                let rank = (p / 100.0 * n as f64).ceil().max(1.0) as usize;
                sorted[rank.min(n) - 1]
            };
            let mean = if n == 0 {
                0.0
            } else {
                sorted.iter().sum::<f64>() / n as f64
            };
            let over = sorted.iter().filter(|ms| **ms > budget_ms).count();
            let over_double = sorted.iter().filter(|ms| **ms > 2.0 * budget_ms).count();
            let round = |value: f64| (value * 1000.0).round() / 1000.0;
            serde_json::json!({
                "n": n,
                "mean": round(mean),
                "p50": round(pick(50.0)),
                "p90": round(pick(90.0)),
                "p95": round(pick(95.0)),
                "p99": round(pick(99.0)),
                "max": round(pick(100.0)),
                "over_budget": round(if n == 0 { 0.0 } else { over as f64 / n as f64 }),
                "over_two_frames": round(if n == 0 { 0.0 } else { over_double as f64 / n as f64 }),
            })
        };
        let mut by_kind = serde_json::Map::new();
        for kind in Kind::ALL {
            by_kind.insert(
                kind.name().to_owned(),
                summarize(
                    kept.iter()
                        .filter(|s| s.kind == kind)
                        .map(|s| s.ms)
                        .collect(),
                ),
            );
        }
        let keys = |filter: &dyn Fn(&Sample) -> bool| {
            summarize(
                kept.iter()
                    .filter(|s| s.kind == Kind::Key && filter(s))
                    .map(|s| s.ms)
                    .collect(),
            )
        };
        let typing = summarize(
            kept.iter()
                .filter(|s| matches!(s.kind, Kind::Key | Kind::Backspace))
                .map(|s| s.ms)
                .collect(),
        );
        let all = summarize(kept.iter().map(|s| s.ms).collect());
        serde_json::json!({
            "budget_ms": budget_ms,
            "skipped_first": skip.min(self.samples.len()),
            "first": self.samples.iter().take(skip).map(|s| (s.ms * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "all_dispatches": all,
            "keystrokes": typing,
            "by_kind": by_kind,
            "key_opening_chunk": keys(&|s| s.opens_chunk),
            "key_inside_chunk": keys(&|s| !s.opens_chunk),
            "key_depth_1_4": keys(&|s| s.depth < 4),
            "key_depth_5_9": keys(&|s| (4..9).contains(&s.depth)),
            "key_depth_10_plus": keys(&|s| s.depth >= 9),
        })
    }
}
