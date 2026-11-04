# Agent Note: Windows TSF 反向端点重连后没重开 Main，宿主被重连循环拖到停止响应

Status: implemented

## Problem

在 Notepad4（Scintilla 系、走 CUAS 的经典 Win32 编辑器）里输入会让宿主「停止响应」。Windows 事件日志给出 `AppHangB1`，挂起类型是「顶层窗口空闲」；`procdump` 抓到的宿主主线程栈停在

```
ntdll!NtDelayExecution
KERNELBASE!SleepEx
MetasequoiaImeTsf!...            ← 本输入法的 TSF DLL
user32!UserCallWinProcCheckWow
user32!DispatchMessageWorker
Notepad4+0x11a73e                 ← 宿主消息循环
```

也就是 TSF 在宿主的调用线程上同步等待。同时 `%LOCALAPPDATA%\metasequoiaime-full\logs\server.log` 里 `host_api: settled model attached`（只在会话真的重建时才写）以每秒 10~20 行的速度增长。关掉 Notepad4，增长立刻归零。

循环是这样转起来的：

1. 焦点变化时 `RequireNamedpipeFocusActivation()` 换一代激活令牌、清掉 `workerCommitReady`；
2. `EnsureNamedpipeFocusSessionActivated()` 写出 `ClientActivated` 之后等 worker 的 `FocusSessionReady` 回执。这里 `if (boundWorkerCommitReady)` 判的是**指针是否绑定**，不是它指向的值——绑定之后恒为真，所以一定会等满 `Sleep(1)` 自旋预算（写死的 150ms），然后 `MarkNamedpipeSessionDirty()`：换新令牌、关掉反向下行（reply）管道；
3. reply 管道重新注册时，Server 的 `PipeRegistry` 先 `clear(Main)`，把这次当成整代传输的重来；
4. TSF 侧 `ConnectToAllNamedpipe()` 看到本地 Main 句柄「有效」就直接复用，往一个已经没有对端在收的 Main 写 `ClientActivated` —— 回执不会回来；
5. 回到 2 再超时。每轮 Server 建一次会话，宿主调用线程每轮被按一个完整的等待预算。

第二条放大器在有序偏好切换上：`CMetasequoiaIME::_PollPreferenceTransition()` 等满 2 秒后只 `KillTimer` 并 `MarkNamedpipeSessionDirtyForOwner()`，既不清屏障也不归零计时。于是 `WM_ApplyPreferenceTransition` 和 500ms 的偏好刷新每来一次都仍然判定「已经超时」，每来一次就换一次激活令牌；重连中的会话又让这次应用配置继续失败——超时和重连互为因果。

## Decision

- **反向端点重开，Main 必须跟着重开。** `TryOpenClientPipe()` 在真正新建了一个 reply 或 worker 端点后关掉本地 Main 句柄，由统一的 `ReopenMainAfterPipeOpen(role)`（`IPC/PipeRecoveryPolicy.h`）决定；这一处同时覆盖 `ConnectToAllNamedpipe()` 和按键等待回复时 `ReadFromServer` 里的 reply 重开。**这是真正掐断循环的改动。**
- **回执等不到就是传输失败。** 等待预算改用按键回包共用的现场值 `kServerReplyWaitTimeoutMs`（100ms，现场最高一次回包 61ms），等满仍无回执就 `MarkNamedpipeSessionDirty()` 重连——保留自愈：活着的 Server 由普通重连定时器恢复，而不是让会话永久停在「未确认」。结论由 `FocusEchoOutcomeFor(acknowledged, sessionResetPending)` 表达，不再散落在函数体里。
- **有序偏好切换的放弃要真的放弃。** 等满超时、或者根本没有可应用的对象（配置还没读进来、引擎会话还没建起来）时，`_PollPreferenceTransition()` 清零屏障与计时、安排按键补发，超时那条再重连一次；否则每次轮询都会重新判定失败并重连。

## Alternatives considered

- **把 `if (boundWorkerCommitReady)` 改成「本次发出过激活才等」并去掉超时重连。** 注释更准，但去掉重连会让会话失去自愈：回执真丢了而管道还活着时，后续 `Ensure` 既不重发激活也不再等，补发的键全部落进 `_serverUnavailableFallbackActive` 的离线通道，只能靠换焦点恢复。不用。
- **物理重连时不换激活令牌。** 能让在途回执匹配上，但 `Ipc.cpp` 里那条注释写明了理由：每个物理 worker 端点必须有新的激活令牌，注册 ACK 只证明端点存在，不授权来自上一代端点的提交。换掉它会打开跨代提交的口子，不用。
- **把 `EnsureNamedpipeFocusSessionActivated()` 改成异步（排队等待回执后再发）。** 结构上最干净，但要给按键、状态快照、候选窗三类包各加一条排队通道，并重做 `_DrainOneDeferredKeyDown` 的判定；风险远大于收益，本次不做。

## Consequences

宿主侧不再有每秒十几次的重连与等待：正常路径下一次焦点激活只等一次 worker 往返（现场 1~3ms），出问题时退化成一次重连，由重连定时器按退避节奏恢复。

实测（Windows 10 19045，本机）：修复前 Notepad4 有焦点时 10 秒内新建 167~289 个会话；修复后同样时长 **0 个**，`procdump` 抓到的宿主主线程回到 `GetMessage` 空闲。对照组同一台机器上的 explorer 用旧 DLL 时每秒 16 次以上重建会话，重启 explorer 让它加载新 DLL 后立即归零。

代价与边界：

- `TryOpenClientPipe()` 新建反向端点时会关掉 Main，下一次写 Main 前必须重新握手。重连那一轮多一次 `CreateFile` + hello，正常聚焦路径（句柄都有效）不受影响。
- 焦点回执预算从 150ms 收到 100ms。现场最高一次回包 61ms 仍在预算内；`msime-tsf-reply-wait-policy-test` 断言它不低于 100ms。
- 放弃有序偏好切换时，这一轮设置变更要等下一次修订通知才应用；期间按键按旧偏好处理。这是为了不让「超时 → 重连 → 又超时」继续转。
- 行为测试放在 `msime-tsf-pipe-recovery-policy-test`（`ReopenMainAfterPipeOpen` 与 `FocusEchoOutcomeFor`）；`lifecycle_boundary.cpp` 只断言 IPC 真的调用了这两个函数，不再用源码顺序代替行为。

仍缺的验收：真实键盘输入、候选窗、上屏，以及「切焦点后马上打字」「Server 繁忙时打字」「设置窗口导入词库时打字」三个场景，需要在有输入法手感的机器上人工确认。自动化键盘注入在这台机器上打不进 Notepad4（旧 DLL 同样如此，`Ctrl+V` 粘贴正常）。
