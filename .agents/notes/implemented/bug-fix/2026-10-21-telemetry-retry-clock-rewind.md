# Agent Note: 遥测退避拒绝超出上限的未来截止时间

Status: implemented

## Problem

遥测发送把服务端 `Retry-After` 限制在 24 小时内，但读回持久化的 `retry_after_unix_ms` 时只检查它是否晚于当前时间。在错误的未来时间收到限流、随后系统校时回拨后，短短两分钟的限流会挡住队列一周，重建 `TelemetryStore` 也不会恢复。合成回归已复现两种结果：成功发送没有发生，新限流请求也没有机会发出。

## Decision

与 `account::cloud_sync::state::SyncState::backoff_until` 一样，只有剩余时间大于零且不超过既有最大退避的截止时间才有效。进入发送前检查这个窗口，并记住当时的旧截止时间；发送后只丢弃仍等于该旧值的截止时间，保留在途期间其他请求写入的未来截止时间。后者可能基于稍后的时钟，不能再按本次较早的时间检查最大窗口；成功或普通网络失败可以清掉旧异常时间，新的限流从修正后的时间计时，并发请求留下的有效更长退避继续保留。不修改状态格式或文件访问入口。

## Alternatives considered

- 在发送后再按本次开始时间校验最大窗口可以共用一个判断，但并发请求可能晚一秒才写入正常的 24 小时退避，被较早的时间误算为超限；只清除已观察的旧值。

- 保存发起限流时的时间可识别更小幅度的回拨，但要增加持久化字段和旧状态迁移；现有上限已足以识别不可能由正常请求产生的远期截止时间。
- 把异常时间改为当前时间加 24 小时可以继续保守退避，但每次时钟变化都可能延长等待，原本短暂限流也会被强制扩到一天；沿用仓库云同步的失效规则。
- 改存 `Instant` 能在进程内避开校时，但截止时间要跨进程和重启共享，没有可持久化的统一单调纪元。

## Verification

`cargo test -p msime-client-services --lib telemetry --locked --quiet`：31 项通过。两项回拨回归先在旧实现因 `deferred` 仍为真而失败，修复后跨 store 重建恢复发送、成功清掉旧截止时间及新限流的两分钟等待均通过。`Duration::MAX` 仍被限制为 24 小时，恰好上限和到期前一毫秒仍退避，到期后才发送；既有并发回归也通过。独立审查发现较早请求可能把稍后写入的正常 24 小时退避当成异常；新增 Barrier 回归先复现清零，再验证合并保留新截止时间。时间是传入的合成值，不修改系统时钟，不访问线上服务。

## Consequences

只有超出最大退避窗口的回拨能被无新增状态地识别；剩余时间仍在 24 小时内时继续保守等待。参考 [Rust SystemTime 文档](https://doc.rust-lang.org/std/time/struct.SystemTime.html) 的非单调保证；不承诺识别每次校时。

## Related notes

检索活跃笔记的遥测、退避、Retry-After 和时钟回拨：[遥测目录句柄](../../implemented/bug-fix/2026-10-09-telemetry-cache-directory-handle.md)只持有存储访问边界，不改动；[词库更新](../../implemented/feature/2026-10-18-dictionary-auto-update.md)和[统计上传](../../implemented/feature/2026-10-17-typing-statistics-cloud-upload.md)的节流也处理回拨，但不是服务端退避，属于部分重叠。云同步编排提案持有独立状态机的退避，本规则只对齐它现有实现；其余 Android 重试笔记与此处无关，没有被本改动吸收的旧提案。
