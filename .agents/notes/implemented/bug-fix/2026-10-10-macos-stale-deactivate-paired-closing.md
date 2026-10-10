# Agent Note: macOS 迟到的 deactivate 不再替当前客户端补闭合符

Status: implemented

## Problem

排查 #6076 时发现：`MSIMEInputController` 的 `deactivateServer:` 先调用 `flushPendingPairedClosing` 并清空 `_pairedPunctuation`，之后才判断 `sender` 是不是当前客户端（`!sender || sender != _activeClient` 时按「上一个客户端迟到的回调」直接返回）。

IMK 不保证上一个客户端的 `deactivateServer:` 先于下一个客户端的 `activateServer:` 到达（背景音乐的属主判断就是为此写的）。同一个控制器先 `activateServer:B`、再收到迟到的 `deactivateServer:A` 时：

- `activateServer:` 一开始就把 `_pendingPairedClosing` 置空，A 欠着的闭合符已经被丢掉，不会被写进 B。
- 但 B 在两次回调之间打开的成对标点（例如 `（`，`）` 作为 marked text 的尾巴等着补上）会被迟到的回调用 `insertText:` 直接写进 B：这一对被提前合上，光标落在 `）` 之后；如果 B 正在组字，marked text 被整段替换成 `）`，引擎里的组字和屏幕上的不再一致。
- B 的 `_pairedPunctuation` 跳过记录也被清空，之后在 `）` 前敲的 `）` 不再被跨过，而是重复输入。
- `sender` 为 nil 的 deactivate 同样被当成迟到回调，却同样会替当前客户端补闭合符。

## Decision

`deactivateServer:` 先做迟到判断，再 `flushPendingPairedClosing` 和清空 `_pairedPunctuation`。迟到的回调（`sender` 为 nil 或不是 `_activeClient`）不再碰成对标点的任何状态；正常的 deactivate 行为不变，仍在自己的客户端里补上闭合符。

待补闭合符的归属不另设字段，依靠下面这条不变式：`_pendingPairedClosing` 永远属于 `_activeClient`。改变 `_activeClient` 的三条路径都维持它：

- `activateServer:` 换客户端前把它置空（旧客户端欠的闭合符写不进新文档，丢弃）。
- `handleKeyEvent:client:` 发现客户端变了，先把它补进旧客户端（它的属主），再换成新客户端。
- 正常的 `deactivateServer:` 先补进当前客户端，再把 `_activeClient` 置空。

同文件里其他 deactivate / 换客户端路径上直接写客户端的地方都已经在判断之后：`commitComposition:` 开头就要求 `sender == _activeClient`；`activateServer:` 和 `handleKeyEvent:client:` 里的 `setFocused:NO` 清 marked text 写的是旧客户端；`deactivateServer:` 里判断之前剩下的 `flushKeyPresses`、输入模式 HUD 和打字特效面板不写客户端。

## Alternatives considered

- 给待补闭合符记一个所属客户端，`flushPendingPairedClosing` 只在属主等于 `_activeClient` 时写入、否则丢弃：最强的理由是归属变成显式的，以后新增换客户端的路径也不会写错地方。没有采用，因为上面的不变式已经由全部三条路径维持，迟到回调这条路径的问题只是判断顺序；多一个字段就多一份要和 `_activeClient` 保持同步的状态，而且对这次的问题，按属主过滤也挡不住——迟到回调到来时，待补的闭合符本来就属于当前客户端，真正要避免的是「由不相干的回调替它补上」。

## Consequences

收益：上一个客户端迟到的 deactivate 不再提前合上当前客户端的成对标点、不再替换它组字中的 marked text、不再清掉它的跳过记录。

代价：`sender` 为 nil 的 deactivate 以前会替当前客户端补上闭合符，现在不会；这与它在其余状态上被当成迟到回调的处理一致，这一对会在当前客户端下一次上屏、按 Esc、提交组字或真正的 deactivate 时补上。

## Verification

- `platforms/macos/tests/input/ShortcutTest.mm` 新增 `TestStaleDeactivationLeavesTheCurrentPairOpen`：当前客户端打开 `{}` 后，迟到的 `deactivateServer:`（旧客户端和 nil 各一次）不写任何客户端、待补的 `}` 还在；之后上屏把 `}` 带走，再经过迟到回调，`}` 前敲 `}` 仍被跨过；最后正常的 `deactivateServer:` 照旧在当前客户端补上 `}`。用 `origin/develop` 的 `InputController.mm` 构建时，该用例在第一次迟到回调后的断言处失败；只把补闭合符挪到判断之后、仍在判断之前清空跳过记录时，它在 `}` 被跨过的断言处失败；完整修复后整个 `shortcut-test`（含 `--translations`）通过。
