# Agent Note: Android 世界时钟增加数字与长格式

Status: implemented

## Problem

世界时钟已有标准时间、日期、ISO 8601 和 Unix 时间等插入格式，但缺少适合文件名、日志和连续数字输入的年月日及长时间格式。系统时间和城市时间都应提供同一组格式。

## Decision

在共享 `ClockCard` 格式列表中增加三个固定格式：`年日数` 为 `YYYYMMDD`，`长时数` 为 `YYYYMMDDHHMMSS`，`长时符` 为 `YYYY-MM-DD-HH:MM:SS`。格式使用目标时区的同一个 `DateTime` 生成，Android 页面无需单独分支。

## Existing note audit

- [离线世界时钟](../feature/2026-10-23-offline-world-clock.md)：沿用其共享格式列表、系统时间和城市时间共用的快照协议。

## Alternatives considered

- 在 Android 宿主单独拼接字符串：不用，会让系统时间和城市时间产生分歧，并绕开共享时区计算。
- 用本地化日期格式替代固定格式：不用，数字格式用于复制、文件名和日志，必须稳定可预测。

## Consequences

世界时钟格式页新增三个插入按钮，已有格式和持久化城市列表不变。两个时间来源都使用各自卡片的当地日期与时区。

## Verification

- `local_dates_formats_and_system_offset` 增加三个格式的精确值断言。
- Android host、Rust 测试和设备验收待磁盘空间恢复到仓库规定的 50 GiB 启动线后补跑。
