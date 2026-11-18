# Agent Note: Android 旧偏好中的快捷短语混输默认值

Status: implemented

## Problem

Android 在接入 `quick_phrase_candidates` 后，旧 Android 偏好里的 `mixed_input.quick_phrase` 可能缺字段。共享 Rust 为了保持 macOS、Linux 旧行为会把这个字段读作开启，但 Android 之前没有这项能力，因此旧用户会突然看到快捷短语混进普通候选。

## Decision

Android 原生层把 `mixed_input` 中缺少的 `quick_phrase` 补成显式 `false`。建会话前的初始偏好、实时偏好和后续偏好快照统一经过这个归一化，设置页读取缺字段时显示关闭，保存其他混输设置时也把关闭值写回。共享 Rust 的通用旧文档规则保持不变，macOS 和 Linux 继续按缺字段开启处理。

## Alternatives considered

- **修改共享 `MixedInputPreferences` 的缺省值**：会改变 macOS、Linux 升级用户的既有候选行为，不采用。
- **只改设置页显示**：引擎仍会按缺字段开启，页面显示与实际候选不一致，不采用。
- **只在首次建会话时补字段**：偏好轮询或保存后仍可能恢复开启，不足以覆盖 Android 的全部运行时入口，不采用。

## Consequences

Android 旧用户的快捷短语候选保持此前不可见，用户显式开启后才生效。显式保存为 `true` 的 Android 偏好仍正常工作，macOS、Linux 和共享默认值不受影响。

## Verification

- JVM 冒烟覆盖缺字段、显式开启和显式关闭的 Android 规则。
- Android 建会话和偏好快照路径都调用同一归一化策略，设置页保存路径补写显式关闭。
