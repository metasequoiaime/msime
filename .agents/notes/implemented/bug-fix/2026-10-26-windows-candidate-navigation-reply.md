# Agent Note: Windows 候选导航回复同步到宿主会话

Status: implemented

## Problem

Windows 的上下方向键由 TSF 先交给 Server 判断导航绑定。Server 返回 `MoveSelectionPrevious`、`MoveSelectionNext` 或翻页回复后，TSF 只在 UI-less 游戏路径把回复转成宿主会话命令；普通 Win11 宿主只确认异步请求。宿主会话仍停在旧的候选选择上，连续按键会让 TSF 的组字、候选窗和 Server 会话代次分离，最终走到 `local-reset`，表现为方向键无效且候选栏消失。

## Decision

所有有效的导航回复都在 TSF 消费回复后同步回当前宿主会话。宿主会话存在时使用 Server 返回的导航命令更新 Engine，再按同一份宿主视图重画组字和候选窗；这条路径不再限定为 UI-less。没有宿主会话时，按回复类型把移动或翻页交给旧候选窗的本地高亮逻辑，并使用无 request ID 的编辑会话，避免重复读取已经消费的 Server 回复。`NavigationIgnored` 仍只完成当前请求，不移动选择。

## Alternatives considered

- **继续把普通宿主的导航回复当确认**：改动最少，但普通宿主的 Engine 不会自动收到 Server 已执行的导航命令，正是 #9747 的状态分裂来源，因此不采用。
- **所有路径都直接用 Server 的候选页覆盖 TSF presenter**：可以让页面看起来同步，但有效宿主会话的候选身份、选词和代次属于 Engine，直接覆盖会把显示列表和实际选词列表分开；保留宿主命令和旧 presenter 回退两条所有权清晰的路径。

## Consequences

普通 Win11 宿主的上下键现在会推进 Engine 的候选选择并重画候选栏，连续方向键不会因为两端状态分离而触发组字重置。UI-less 游戏的既有命令路径保持不变；没有宿主会话的旧 TSF 路径仍能移动本地候选高亮。回复已被异步窗口处理消费，因此重画阶段不能再按原 request ID 读取同一帧。

`msime-tsf-candidate-navigation-reply` 覆盖四种移动／翻页回复以及 `NavigationIgnored` 不发宿主命令；`scripts/test-windows-native-run.py` 运行了该测试和现有 Windows 纯策略测试（172 项通过）。仓库 quick 门禁中的 Windows 交叉构建未在本机启动：当前磁盘可用空间低于仓库要求的 50 GB，避免为验证新分支创建完整构建树。当前环境没有 Windows 11 实机，不能据此宣称真实桌面验收完成。
