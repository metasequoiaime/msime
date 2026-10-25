# Agent Note: Windows 桌面对象与候选 UI 归属

Status: implemented

## Problem

#9439 使用 Sysinternals Desktops 的独立 Windows desktop object。原实现由一个 Server 的主线程创建一个 `CandidateWindow`，所有 TSF 客户端连接本版本的固定管道；在其他桌面输入时，候选仍只能属于建窗的桌面。改变已有窗口的位置或置顶状态不能改变其桌面归属。

## Decision

保留一个生产 Server 及现有会话、词库、统计、剪贴板和后台服务，将候选 UI 的线程归属按输入客户端所属桌面拆开。每个候选 UI 线程在任何建窗和钩子之前绑定自己的桌面，独占候选窗、上下文菜单、读屏对象与候选关联浮层的创建、更新和销毁。主线程不得直接调用另一个桌面线程的窗口方法。

候选所属桌面必须来自已经验证归属的客户端与焦点租约，不能靠主线程的当前前台窗口猜测，也不能把任意客户端字符串当成可打开的桌面。路由信息绑定现有连接代次和焦点代次；断开、失焦、桌面切换及旧代次到达时，各桌面窗口按既有隐私收起规则丢弃失效快照。点击、翻页、展开和渲染回执继续使用原会话身份，不创建第二份引擎或复用另一个桌面的候选索引。

复用现有 `PipeTicket.client = pid << 32 | tid`。高 32 位已经由 `PipePeer` 对过管道对端；`ClientDesktop` 用系统线程对象核对低 32 位的归属与存活，再查询桌面。`GetThreadDesktop` 返回借用句柄，解析器按对象名在 Server 当前窗口站打开一个拥有句柄，并用 `CompareObjectHandles` 比较对象身份，拒绝其他窗口站的同名桌面与无法确认的归属。不增加协议事件、能力位或客户端提供的桌面字符串。

## Verified platform constraints

`platforms/windows/experiments/desktop-ownership.ps1` 在 Windows 原生运行时创建两个私有桌面，每个独立线程绑定后创建一个隐藏的标准窗口，通过 `EnumDesktopWindows` 检查窗口互不串入；主线程通过窗口线程 ID 和 `GetThreadDesktop` 查询两者的桌面归属。窗口和桌面在实验结束时销毁，实验不调用 `SwitchDesktop`，不启用或修改真实输入法。

同一实验确认未按桌面区分的命名互斥量在两个桌面都报告已存在，而各自带合成桌面后缀的名字互不冲突。因此「每个桌面直接启动现有 Server」会被当前 Server、启动门和 Watchdog 单实例限制挡住；只改候选坐标或单个互斥量不足以解决需求。

实验在远端 Windows runner 通过，退出 0，跨进程版本也确认系统线程对象可核对进程和桌面身份。另一个桌面的 HWND 在主线程上查询返回 1400，不能把 `GetWindowThreadProcessId` 当作跨桌面认证入口。

原生 `windows-client-desktop` 测试调用产品使用的 `ClientDesktop`，覆盖跨进程查询、两桌面隔离、用拥有句柄绑定新的 UI 线程、错误进程 ID 与退出线程的拒绝。子进程采用重定向标准流、继承所需句柄的隔离启动方式；没有重定向与继承的早期测试组合返回拒绝访问，尚未证明是何种窗口站连接条件造成差异。解析器在查询失败时拒绝路由，不能把 C# 实验的成功推广成所有进程组合都可查询。真实 TSF 编辑器、签名产品和焦点／选词验收仍待完成。

Microsoft 的 [Desktops 说明](https://learn.microsoft.com/en-us/sysinternals/downloads/desktops)明确每个窗口在创建时绑定桌面对象，系统不提供把已有窗口移到另一桌面的操作；[SetThreadDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop)要求目标属于进程窗口站，且线程已有窗口或钩子时会失败；[线程连接规则](https://learn.microsoft.com/en-us/windows/win32/winstation/thread-connection-to-a-desktop)区分显式绑定、继承桌面与启动桌面。

## Implementation

Win7 没有 [`CompareObjectHandles`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-compareobjecthandles)，仅动态查找后返回假会使所有候选归属解析失败。兼容路径使用进程内共用的无窗口探针线程，依次 `SetThreadDesktop` 绑定两个句柄，再比较系统桌面根窗口；绑定同时由系统核对进程窗口站，另一窗口站的同名桌面必须拒绝。探针用独立请求锁串行调用，通过条件变量同步，调用返回前恢复原桌面，不保留调用方的拥有句柄，不改变调用方的 UI 线程归属，也不在每次候选刷新时创建新线程。现代系统仍使用原生对象比较接口。

`DesktopCandidates` 的协调线程读取不可变候选快照，按经过认证的桌面对象路由给专属 UI 线程。字体、布局、风格和解析后的皮肤来自主线程发布的值快照；图像与外观有独立代次。每个桌面持有自己的候选窗、双拼键位图、打字特效浮层、COM 和绘制资源。全屏分类及字体回退的可变缓存改为线程独立。最多保留八个候选 UI 线程，覆盖 Desktops 的四个桌面并限制客户端制造资源的上界。协调线程与各桌面线程平时各按 16ms 轮询；按键或点选的渲染屏障等回执时经 `DesktopCandidates::wake()` 立即叫醒两级，见 [渲染屏障叫醒候选窗](../bug-fix/2026-10-25-windows-candidate-render-wake.md)。

读空仍按原租约保持可见帧；失去焦点或无法确认桌面时，租约检查立即拒绝保帧，且取消旧线程的候选上下文菜单。点击、翻页、展开及渲染回执继续经过原来的租约、会话与候选代次。全局打字特效只由当前路由窗口领取。停止时先通知全部 UI 线程、关闭菜单并等待窗口销毁，再停止 Server 与点击工作线程；候选 UI 故障保留 Server 退出并由既有看门狗恢复的行为。

Windows MSVC 已成功编译链接 Server、TSF DLL 与两个新增测试。`windows-client-desktop` 覆盖真实解析器，`windows-desktop-candidates` 在两个私有桌面中覆盖候选显示请求、指定 UI 线程上的显式绘制与回执、桌面切换、读取争用保帧、失焦收起和窗口销毁。所有数据为合成值，没有触及用户桌面或真实输入法。

Win7 兼容路径的 x64／x86 实机回归覆盖真实跨进程桌面归属、错误进程、退出线程、新 UI 线程绑定，以及四条调用线程对共用探针的并发请求与线程退出。Win7 普通令牌创建额外窗口站返回 `ERROR_ACCESS_DENIED`，测试明确输出这一个边界未覆盖，继续执行不需提权的全部检查；同一二进制用 `--require-window-station` 在提权专项强制创建另一窗口站，两个架构均确认同名桌面被拒绝。没有放宽等待阈值。

私有桌面未切换为输入桌面时，其根窗口不可见；候选自身已有 `WS_VISIBLE`，`IsWindowVisible` 仍因祖先不可见而返回假，系统也不会自动绘制。测试因此枚举候选自身的显示标志，并显式发送 `WM_PAINT` 验证绘制资源与回执线程归属。未激活的私有桌面跳过鼠标翻页；这不等于真实桌面显示或输入验收。候选菜单是非模态 `CandidateFlyoutWindow`，失焦时直接隐藏并清除菜单目标，不能用 `EndMenu` 当成其退出路径。

Windows runner 的完整 MSVC Debug 原生工程及 pipe-only 工程均编译、链接通过，包括 Server、TSF DLL 与实验编辑器。已复现并修正的阻塞包括 `C4100`／`C4189`、鼠标订阅 cookie 的 `C4701`、局部变量遮蔽 `C4456`、常量断言 `C4127`、测试目标缺少 `/utf-8` 或 `NOMINMAX`，以及合成设备名的 `strcpy` 弃用警告。没有关闭 `/WX`、屏蔽警告或增加失败基线。

10 项相关原生专项全部通过（1.30 秒），覆盖桌面隔离、候选尺寸、HUD、工具栏、TSF 配置、录音设备身份、离屏像素和两个标点策略；pipe-only 的候选尺寸、HUD 与工具栏三项全部通过（0.14 秒）。当前完整 quick 已重跑并退出 1：原生与 pipe-only 编译阶段通过，可移植 runner 仍有原失败集合内的 6 项失败，16 个失败阶段均已在上次记录中；没有新增失败阶段。其余 Windows 契约与可移植测试问题尚未解决，不能认为完整门禁通过。

固定基点 `839cf5d2b6` 的可移植 runner 有 30 项失败；补齐 MSVC 链接诊断收集、公共头文件目录及 `NOMINMAX` 后剩余 6 项，全部已在基点失败集合中，新增原生测试没有新增误报。实际编译器的语法错误和已编译程序的非零退出仍被报告为失败。两个正式原生专项最终均通过（总计 0.74 秒）。Windows 笔记校验修正路径转换后实际扫描 1766 篇与 4 个归档封印通过，本机同组校验通过。

真实 TSF 编辑器、签名产品、已激活桌面的鼠标翻页与菜单操作仍未验收，PR 保持草稿并使用 `Refs #9439`。

## Related notes audit

[Windows 本地门禁兼容](../testing/2026-10-14-windows-local-gate-portability.md)持有可移植 runner 和工具调用边界。本次验证顺带补齐 MSVC 未解析符号分类及实验编辑器 `NOMINMAX`，就地更新该记录，不把既有失败加进基线。

检索活跃笔记中的 issue 编号、桌面对象和按桌面候选路由，未发现持有同一决定的记录。[游戏 UI-less 与 IMM](../../implemented/bug-fix/2026-10-16-windows-game-uiless-and-imm.md)及[后续候选协议](../../implemented/feature/2026-10-16-windows-source2-profile-and-local-online.md)持有宿主自绘和会话身份边界；本记录部分关联其点击与回包约束，不替代它们。[候选非阻塞读取](../../implemented/bug-fix/2026-10-23-windows-candidate-read-contention.md)持有锁争用下的可见性规则，增加 UI 线程时仍需保留。

## Alternatives considered

- **每个调用线程持有线程本地探针。** 可减少请求锁争用，但 Win7 实机在线程退出时复现加载锁死锁：线程本地对象析构等待探针退出，而探针退出还要经过系统线程卸载。改为进程内唯一探针，由主程序在候选线程已汇合后回收。
- **比较 `NtQueryObject(ObjectNameInformation)` 的对象名。** 可以只查询句柄而不维护探针线程，但 Win7 实机对桌面只返回 `\\<桌面短名>`，没有窗口站路径，不能区分不同窗口站的同名桌面。因此不能用这条看似完整的内核对象名代替实际绑定核验。
- 每个桌面一套 Server 和 Watchdog：复用当前单线程窗口与消息循环，线程归属最直接；但需同时区分全部主／回包／worker／Aux 管道、启动门、实例事件和监督归属，还会产生多个统计、剪贴板与后台服务写入者。优先保留共享状态的单一所有者，先验证候选线程路由。
- 在 TSF DLL 内画候选：窗口天然归属正在输入的应用桌面；但会把完整候选渲染、皮肤、资源与故障面带进宿主进程，改变当前 DLL 与 Server 的产品边界，因此不作为首选。
- 切换现有 Server UI 线程的桌面：不必维护多套候选窗；但线程已创建窗口与钩子，`SetThreadDesktop` 不允许这种切换，不能作为实现。
- 仅依赖已有 TSF UI-less：应用参与自绘时不需要外部窗口；但普通应用未必实现自绘，无法覆盖每个桌面正在输入的普通编辑器。

## Remaining verification

- 在两个以上独立桌面对象中的真实 TSF 编辑器分别输入、点击选词、键盘选词、翻页和展开候选，结果只写入原文档。
- 反复切回原桌面、失焦、销毁客户端与重连时，旧窗口及时收起，旧点击和旧渲染回执不能影响新会话。
- 默认桌面与旧协议行为保持兼容；错误桌面归属、拒绝访问、创建窗口失败和 UI 线程退出均有有界失败路径。
- 验证候选皮肤、鼠标操作、关联浮层与 UI-less；非候选的全局服务保持单一所有者。
- 原生对象探针、协议与路由回归、Windows 产品构建和完整 quick 通过；API 对象探针不能替代真实桌面编辑器验收。

## Consequences

单一 Server 保留共享状态的唯一写入者；代价是最多八套桌面 UI 资源及线程生命周期管理。Windows runner 的 SSH 会话不等于交互桌面，现有实验没有验证跨进程 TSF 与 UIAccess 产品权限。多 UI 线程会影响当前候选字号、皮肤、键位图、打字特效和诊断回调的主线程所有权；消息传递与销毁顺序由不可变快照、窗口登记锁及线程汇合约束。不得为了少改代码而让多个线程共享现有 `CandidateWindow` 或可变绘制状态。
