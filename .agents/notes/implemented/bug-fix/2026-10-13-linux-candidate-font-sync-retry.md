# Agent Note: Linux 候选字体写入失败后重试

Status: implemented

## Problem

IBus 和 Fcitx5 的候选字体同步器在宿主真正写入面板前就把待写描述记进缓存。IBus 缺少 `org.freedesktop.ibus.panel` schema 或写设置失败时，Fcitx5 的 classicui 插件尚未加载时，后续同一份字号偏好都会被缓存吞掉，用户只能再次修改字号才会重试。

## Decision

`CandidateFontSync::next` 只产生待写描述，新增 `mark_applied` 在宿主确认写入后提交缓存；未确认的描述保留并在下一次同步再次返回。IBus 用 `g_settings_set_string` 和 `g_settings_set_boolean` 的结果确认，Fcitx5 在 classicui 存在并完成 `setConfig` 调用后确认。两个宿主都保留定时刷新重试路径。

字号单位按[运行版本](2026-10-21-fcitx-runtime-font-unit.md)选择；本篇的写入失败后的重试与去重规则继续保留。

## Alternatives considered

- **只在设置页再次保存时重试**：不能覆盖 schema 或插件稍后出现的正常启动路径，且用户会误以为字号没有效果。
- **每次同步都无条件写入**：会持续改写桌面级配置并产生不必要的 IO，无法保留现有的去重行为。
- **宿主写入前更新缓存**：就是现有缺陷，失败后没有重试信号。

## Consequences

成功写入后的行为不变，仍只在字体选择发生变化时写一次。写入失败会在宿主已有的 IBus/Fcitx5 定时刷新中再次尝试；在 Fcitx5 classicui 未加载时，字体选择会等插件可用后应用。同步器保留一个待写描述，生命周期与宿主进程一致。

## Verification

- `platforms/linux/tests/candidate/candidate_font_policy.cpp` 先覆盖失败重试，再覆盖确认后不重复写入和升级迁移重试。
- `c++ -std=c++17 -I/opt/homebrew/include -Iplatforms/linux/src ... && target/candidate-font-policy-red` 通过。
- `bash platforms/linux/build-container.sh`：Linux 原生宿主和 Fcitx5/IBus 构建通过，83/83 ctest 通过。
