# Agent Note: 鸿蒙补齐全拼十四键与十八键

Status: implemented

## Problem

鸿蒙键盘已有全拼十四键的部分宿主逻辑，但触屏方案、共享偏好、统计键位和设置预览没有与新增的十八键组码能力形成完整闭环。十八键还缺少鸿蒙键表、键面、长按、组码网格、方案选择和同步映射，用户无法在鸿蒙上到达这两种键面。

## Decision

鸿蒙手机和平板提供全拼十四键和十八键；2in1 设备保留实体键盘行为并回落到二十六键。十四键沿用现有三排组码键面，十八键采用共享引擎约定的 `Q WE RT Y U IO P`、`A SD FG H JK L`、`Z XC V BN M` 三排布局。两种键面都通过 `grid_key` 进入共享全拼组码，会话、候选、读音选择、退格、分词、长按和回落规则复用现有十四键路径。

本次复用共享偏好中已有的 `eighteen_key`、账号同步字段和统计契约，补上鸿蒙方案映射、共享设置页面和键盘预览对这个值的识别。鸿蒙统计接入已有的 `eighteenKey` 来源及十八个 `Eighteen*` 键位，键表由独立检查脚本与共享引擎分组互相校验。

## Historical audit

- [全拼十四键](../../implemented/feature/2026-10-10-fourteen-key.md)：部分重叠，沿用其鸿蒙 `KeyGrid`、组码入口、数字层回落和长按约定；本篇记录鸿蒙方案收口及十八键接入。
- [十八键全拼共用字母归组引擎](../../implemented/feature/2026-10-24-eighteen-key-quanpin-engine.md)：部分重叠，沿用引擎的分组、编号 4 和 `eighteen_key` wire 名称；本篇补宿主 UI、设置和统计链路。

## Alternatives considered

**只在鸿蒙添加十八键键面，不扩展共享偏好**：改动较小，但同步和设置页会把该布局当成未知值，重启或跨设备后无法恢复，因此不用。

**把十八键复用十四键方案标识**：可以少一个方案卡片，但会让键位表、统计来源和引擎网格无法区分，也会破坏 `KeyGrid::EighteenKey` 的独立契约，因此不用。

## Consequences

- 鸿蒙手机和平板可在设置中选择全拼十四键或十八键；键面显示组码，双字母键长按可选确定字母，组字时仍使用共享候选和读音选择。
- 共享偏好列表和统计键位契约各追加十八键内容，旧布局顺序和默认启用集合保持不变；旧客户端遇到 `eighteen_key` 时仍可能按未知值跳过，这是版本兼容边界。
- 本次没有改变十四键的鸿蒙账号上传保护，也没有把十八键加入新装默认方案；默认值仍需单独的产品决定。

## Verification

- `cargo check -p msime-client-core -p msime-client-services -p msime-host-api`
- `cargo test -p msime-client-core --lib preferences::tests`
- `cargo test -p msime-client-services --lib typing_statistics`
- `pnpm --filter @msime/harmony typecheck`
- `python3 scripts/test-harmony-eighteen-key-table.py`
- `bash platforms/harmony/tests/run.sh`（3998 项断言通过）

完整 HAP 打包和设备实打需要本机 Harmony 原生依赖与模拟器环境，未把逻辑测试结果当作设备验收。
