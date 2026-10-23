# Agent Note: macOS 上屏重入保留较新组字

Status: implemented

## Problem

#9271 报告某个 Electron 应用在输入框中间反复双拼上屏后删字，并残留原始字母；报告未写具体应用名称，用户也无法提供该信息，尚不能在目标应用验收。调查中使用合成文档确认一个独立缺陷：`insertText:` 同步回调中处理下一轮组字后，旧转换仍发送空的 `setMarkedText:`，把较新组字从文档中删除。控制器已有的 `_applySequence` 只在全部客户端写入之后防止旧视图回写，保护不到这个写入。

## Decision

共享文本适配增加可选的当前转换判定，宿主继续持有代次与客户端身份；在上屏调用返回后核对，过期转换不再写预编辑。既有入口继续使用无判定的兼容路径。上屏消耗的标记状态在调用客户端前发布，嵌套转换写入的状态不得被外层改回。

macOS 使用现有 `_applySequence` 与客户端身份，不另建代次缓存。成对标点的待闭合状态在同步客户端调用之前发布或消耗，智能标点影子同样在客户端调用之前发布，让回调中的下一键读到刚上屏的末字；嵌套转换自行覆盖或清除，旧转换返回后不重写影子，也不补写旧的闭合标记。已经落地的上屏仍各自计入统计，不因转换过期提前丢掉统计。

## Historical audit

检索 `MSIMEApplyTransition`、`clientHasMarkedText`、同步重入与预编辑笔记，没有既有归属。现有 `TestNestedApplyKeepsTheNewerView` 只检查最终视图，不检查上屏回调后文档中的下一轮组字；本次补齐客户端写入边界。[行内预编辑光标](../../implemented/bug-fix/2026-10-19-macos-inline-preedit-caret.md) 持有标记样式与位置，本次不改变样式或位置规则。

## Alternatives considered

- **上屏后不再发送空预编辑。** 简单且能避开本次复现中的旧清空，但没有保护上屏后仍有剩余组字的转换，也没有保护控制器后续的闭合标记写入；采用代次判定覆盖整个同步写入边界。
- **把嵌套转换排队，等外层结束才处理。** 能完整保序，但嵌套按键处理中的控制器会读到旧视图，还需要连同来源覆盖、标点键和客户端身份一起排队；当前已有代次所有权，使用它拒绝旧写入，保留同步处理语义。

## Verification

合成原生复现使用 `prefix|suffix` 文档，在上屏回调中开始下一轮 `ne` 组字；原实现得到 `prefix词|suffix` 且标记状态为关闭，丢失新组字。正式 `TextClientReentrancyTest` 中三种上屏重入在修复前均失败；修复后连同标记回调重入共四种场景通过，既有 `TextClientTest` 也通过。

控制器专项覆盖普通上屏、外层剩余组字、开启标点对、消耗待闭合标点、嵌套再次上屏及客户端切换六种场景。回调内读取智能标点影子的新增断言在影子仍置于写入后时失败，提前发布后通过；最终断言同时确保嵌套末字与新客户端清除状态不被外层覆盖。最初直接编译本分支源码、复用其余原生对象的专项通过；随后由真实 CMake 目标重新构建，同步 `develop` 后，`text-client`、`text-client-reentrancy`、`shortcut`、`shortcut-vim-mode`、`shortcut-nested-commit`、`shortcut-translations` 和新基线 `shortcut-english-punctuation` 七项 CTest 全部通过（205.78 秒）。macOS 全目标和共享 Apple 桥接层构建通过。合入固定 `develop` 提交 `f3e9250e64c1db7fe5598af4221c1522fc2dccb9` 后，同样七项原生回归再次通过（87.30 秒），上游 `shortcut-test --session-recovery` 独立运行也通过。

第一次完整 quick 因 iOS 豆包传输测试的首次 Swift 编译超过原有 120 秒上限而失败，没有得到该套件的断言结果；没有修改超时或绕过钩子。同一脚本原命令复跑后，17 项测试全部通过，正常推送的完整 quick 已通过；同步 `develop` 的合并结果也再次通过完整 quick，随后七项原生专项全部通过。固定 `f3e9250e64c1db7fe5598af4221c1522fc2dccb9` 的合并结果再次通过完整 quick，包括 Linux 原生 111 项、macOS 全目标及 Apple 桥构建；MinGW 全构建与 HarmonyOS 真打包仍按脚本明确跳过。

尚未在报告中的应用验收，只能关联 #9271，不能据此关闭报告。

## Consequences

共享适配不持有宿主状态，也不新增全局客户端缓存。修改影响同步重入时的旧写入；正常上屏、剩余组字、成对标点及统计需要分别回归。特定 Electron 应用的完整删字现象仍需实机核对。
