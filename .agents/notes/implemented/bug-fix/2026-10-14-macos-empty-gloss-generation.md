# Agent Note: macOS 空释义不推进过期服务代次

Status: implemented

## Problem

macOS 候选释义的异步服务快照可能比当前屏幕上的候选页领先一代。没有任何释义可应用时，`applyCandidateTranslationResults` 仍把空载荷和领先代次交给 Engine，随后同步流程再次看到未应用的当前页，可能重复跑同一轮空结果；更严重时会把当前页推进到后台快照的代次。

## Decision

当释义结果为空、服务快照代次与当前显示代次不一致时，只记录当前显示代次已经尝试过，撤销本轮缓存快照并重新进入服务同步。这样空结果不会改写 Engine 的显示页，也不会用过期服务代次触发下一轮重复应用；本地、设备、账号和用户服务仍按原有顺序继续尝试。结果非空或两代次一致时保留原有 `applyTranslations` 路径。

## Alternatives considered

- **照旧把空载荷应用到服务快照代次**：会把后台代次写进当前显示页，并在空结果路径重复同步。
- **所有空结果都跳过应用**：会破坏同一代次下清理旧释义和启动后续服务的既有契约，因此只处理代次不一致的竞态。

## Consequences

过期服务没有释义时，当前候选页保持不变，下一轮服务查询从新的快照开始；真实释义结果、同代次的空结果和候选页变化继续走原有流程。

## Verification

新增 macOS `ShortcutTest` 回归用例，旧实现先因调用 `applyTranslations` 把代次 3 写入而失败；修复后 `shortcut-test --translations` 通过。CMake 构建的 `shortcut` 与 `shortcut-translations` 两项也通过；未构建的 `voice-hold-shortcut` 目标由 CTest 报告为未运行，与本改动无关。
