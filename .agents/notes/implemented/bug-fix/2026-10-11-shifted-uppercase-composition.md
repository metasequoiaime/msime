# Agent Note: 拼音组合中保留 Shift 大写字母

Status: implemented

## Problem

#7106：辅助码关闭时，中文全拼组合中按 `Shift+字母` 会被 Engine 拒收。macOS 宿主随后完成首选候选并把大写字母交给应用，用户输入 `mac` 后按 `Shift+O` 可能得到「马车O」，无法继续编辑原文或按 Enter 提交英文。双拼和光标移入组合中间的编辑路径也有同一接收限制。

## Decision

全拼、双拼已有组合时，大小写字母都留在当前原文；空组合的大写字母仍按原有快捷模式或宿主直输规则处理。辅助码开启时，完整拼音后的大写尾字母继续作为辅助码；关闭时按普通拼音字母参与小写查询，预编辑和 Enter 原文提交保留实际大小写。选词续输、在线查询身份和纠错拒绝记录按同一个辅助码开关决定是否剥离尾字母。双拼完整编码以大写结尾时也允许发起在线查询。

本决定只改变已有组合内的行为；空组合的 Shift 快捷模式由[总开关决定](../feature/2026-10-11-shift-letter-entry-switch.md)。

原文接收、光标编辑和辅助码优先级继续成立。普通大写参与小写中文查询、双拼大写结尾允许在线查询的部分由[大写字面片段](2026-10-19-uppercase-literal-pinyin-boundaries.md)取代；该篇将字面片段与小写拼音分开查询和消费，并保留实际位于辅助码位置的大写。

## Alternatives considered

只在 macOS 宿主拦截并延迟完成组合，改动面较小，但宿主没有可写入 Engine 预编辑的独立原文；其余平台也会继续在同一 Engine 拒收大写字母。让宿主自行维护一份拼音原文会打破输入算法归 Engine 的边界。

## Consequences

用户可继续编辑混合大小写的拼音组合，并由 Enter 明确提交原文；辅助码开启时的既有筛选行为保留。代价是辅助码关闭后，原先把当前候选立即上屏再写大写字母的快捷行为改为先留在组合中，用户需用空格或选词键确认候选。

## Verification

回归测试先在旧实现下复现拒收，再覆盖全拼行末和光标中间、双拼及在线查询。`cargo test -p msime-engine` 在最新 `origin/develop` 上通过（1822 个单元测试、31 个集成测试；13 个既有忽略项）；9 个受影响的 golden 场景逐字段核对并两次重放得到相同输出。`cargo fmt --all -- --check`、`cargo clippy -p msime-engine --all-targets -- -D warnings`、`pnpm run verify-notes` 和 `bash scripts/verify-local.sh --quick` 通过。
