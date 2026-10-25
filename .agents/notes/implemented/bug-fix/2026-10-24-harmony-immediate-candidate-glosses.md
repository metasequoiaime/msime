# Agent Note: 鸿蒙离线候选释义即时显示

Status: implemented

## Problem

开启英文释义后，鸿蒙键盘能查到离线翻译，但候选条的 `ForEach` 只按序号、文字和高亮状态复用组件，后到的释义没有触发重建。修正刷新键后，离线释义还要等按键停下 500 毫秒：本地查词和在线翻译共用了网络防抖，而且本地结果一直等在线请求结束才应用。

## Decision

候选刷新键包含候选的全部展示字段。每个新候选查询先读离线词典并按当前会话和候选代次应用，再对可用的在线服务保留 500 毫秒防抖。在线阶段复用本次已查到的离线结果，继续让服务结果补齐英文缺项、优先回答其他目标语言。离线读取不写学习覆盖层；在线阶段沿用已有持久化规则。

## Related decisions

- [鸿蒙展开候选网格填满每行](../../implemented/bug-fix/2026-10-12-harmony-candidate-grid-row-fill.md)：部分重叠，同样说明 `ForEach` 复用键必须覆盖绘制依赖；那篇负责展开网格占列，本篇负责异步释义与时序，互相保留。
- [Fcitx 在线刷新导致翻译闪烁](../../implemented/bug-fix/2026-10-11-fcitx-online-refresh-translation-flicker.md)：无关，约束不同宿主的在线候选代次；本次仍使用共享 ABI 的代次校验。
- [自定义翻译服务识别 DeepL 官方 API](../../implemented/feature/2026-10-11-custom-translation-deepl-api.md)：无关，负责服务描述器和凭据校验，本次不改服务选择或凭据。

## Alternatives considered

- 直接取消整个翻译通路的防抖，改动最小且离线结果立即出现；这也会让连续打字时的在线请求增加，因此只提前本地查询和显示。

## Verification

- `translation-scheduling.test.ts` 直接执行宿主的方法体，用合成原生边界与假时钟验证本地结果同步显示、在线等待 500 毫秒、相同候选页不重复查词、网络阶段复用离线结果，以及过时网络回复不覆盖新候选页。
- `hvigorw assembleHap --no-daemon` 通过。
- 真机安装即时显示改动后的签名 HAP，合成输入“你好”显示 `hello`；此前候选刷新键修复已验证选词上屏。
- `bash platforms/harmony/tests/run.sh` 通过；`bash scripts/verify-local.sh --quick` 的鸿蒙 HAP、设置页构建和 Rust 编译通过，整体因未改动的 `windows application key replay`（`g_connected` 重复定义）与 `windows deferred recovery`（缺少 `_localSessionResetPending` 声明）两项检查失败。

## Consequences

本地词典查询从停键之后移到每个新候选页，增加连续按键时的本地读取次数；复用结果避免在线阶段再次查询。同一个候选页到达离线和在线结果时可能分别重绘，刷新键与代次检查必须都保留。
