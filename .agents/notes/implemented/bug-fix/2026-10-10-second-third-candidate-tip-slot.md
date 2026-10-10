# Agent Note: 修正二三候选在 TIP 中固定选第一项

Status: implemented

## Problem

PR #6521 让 TIP 把 `;` 和 `'` 归类为 `FUNCTION_SELECT_BY_NUMBER`，但候选处理器的 `CCandidateRange::GetIndex` 仍无条件返回 `0`。Server 已按共享规则选择当前页第二或第三项时，TIP 自己的 Engine 会再次选择第一项，两边的组合状态和上屏结果因此分歧；数字键也受同一处死代码影响。

## Decision

候选键到当前页下标的映射集中到 `common/SecondThirdCandidatePolicy.h`：数字行和数字小键盘按虚拟键码映射，`;` 与 `'` 复用二三候选规则并校验字符。TIP 的 `CCandidateRange::GetIndex` 使用这份映射，并接收原始字符，让它与 Server 对同一个物理键给出同一个下标。

## Alternatives considered

- **只在 TIP 中为 `;` 和 `'` 加两个特判**：改动较小，但会继续保留数字键映射固定为第一项的错误，也会让共享规则在两处复制。放弃，统一所有候选选择键的下标映射。
- **让 Server 回复已选文本，TIP 不再执行本地候选选择**：可以绕开本地映射，但会改变现有数字选词的 TIP/Engine 同步路径，并需要额外处理候选窗口状态。放弃，修复现有路径的根因。

## Consequences

- Server 与 TIP 对数字键、数字小键盘、`;` 和 `'` 使用同一个当前页下标；候选不足时仍由现有页面边界逻辑拒绝选择。
- `CCandidateRange::GetIndex` 不再保留不可达的固定返回值，修复数字选词也固定取第一项的问题。
- 新增独立的 TIP policy 测试，覆盖两种二三候选键、数字行、数字小键盘和 Shift 字符不匹配情况。

## Testing

- `clang++ -std=c++17 -Wall -Wextra -Werror -DMSIME_EDITION_FULL platforms/windows/tsf/tests/input/candidate_selection_slot.cpp -o target/candidate-selection-slot-test`
- `./target/candidate-selection-slot-test`
- `git diff --check`

Windows TSF 完整构建仍需 MinGW/MSVC 环境；本机没有 Windows 交叉编译器，因此未声称运行原生 TSF 二进制。
