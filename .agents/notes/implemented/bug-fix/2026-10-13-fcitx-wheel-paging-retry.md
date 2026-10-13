# Agent Note: Fcitx5 候选滚轮配置在 classicui 缺席后重试

Status: implemented

## Problem

Fcitx5 的候选滚轮开关由 classicui 的 `WheelForPaging` 配置控制。原来的 `CandidateWheelPagingSync::next()` 在返回待写值时就更新去重缓存。如果 classicui 暂时缺席，宿主没有写入配置，下一次读到相同偏好却会被当成已应用而跳过。偏好定时刷新原本只重试待写的候选字体，不会恢复滚轮配置。

## Decision

把滚轮同步的待写值和已应用值分开：`next()` 保留待写值，只有 `set_classicui_config()` 返回后才调用 `mark_applied()`。偏好刷新时，若字体或滚轮配置待写，分别调用其同步入口重试。用户在首次写入前改回默认值时取消待写，仍不接管桌面原有配置。

## Alternatives considered

- 在 `next()` 返回后仍立即更新缓存，再由宿主发现 classicui 缺席时回滚：缓存状态会取决于每个调用方是否记得回滚，且写配置抛错时也要重复处理。
- 每次偏好刷新都向 classicui 写入：会反复改写整个桌面共享的设置，也破坏相同偏好只写一次的现有语义。

## Consequences

classicui 暂时不可用时保留待写滚轮值，后续刷新可以重试；成功调用配置写入后，相同偏好继续去重。重试只在已有焦点会话、非受限上下文中进行，与候选字体的现有范围一致。

## Verification

`bash platforms/linux/build-container.sh` 退出 0，原生 IBus/Fcitx5 宿主和 provider 编译完成，ctest 83/83 通过。`bash scripts/verify-local.sh --quick` 退出 0；其中 Linux 原生宿主再次执行 ctest，83/83 通过。滚轮同步单测覆盖 classicui 缺席后的相同偏好重试、成功后的去重，以及写入前改回默认值时取消待写。
