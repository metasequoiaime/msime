# Agent Note: Win7 Server 的通信权限与窗口创建边界

Status: implemented

## Problem

单独设置和面板测试成功并不代表已安装 Server 能服务真实 TSF。Win7 SP1 上受信任签名的 Server 启动后退出，真实记事本只收到原始拼音；分阶段诊断确认核心资源准备成功，先在管道初始化、修复后又在候选窗口构造时失败。

实机最小探针确认：SDDL 的 `AC` 别名返回 `ERROR_INVALID_SID`（1337），同一 AppContainer 数值 SID `S-1-15-2-1` 可解析；候选窗使用的 `WS_EX_NOREDIRECTIONBITMAP` 在 Win7 返回 `ERROR_INVALID_PARAMETER`（87），去除此位后相同窗口可以创建。此外，辅助管道的维护请求在 Win7 被不支持的 `TokenIsAppContainer` 查询拒绝。

## Decision

管道继续使用完全相同的三个 ACE：SYSTEM、当前账号、AppContainer 仅连接权限；只将别名改成规范数值 SID。不能为了启动成功去掉完整性标签、接纳 Everyone 或放开管道实例创建。内核 ACL 测试继续核对真实对象的 ACE 数量与权限。

通过 `compat/WindowsVersion.h` 动态读取 `RtlGetVersion`，Win8 及以上保留原窗口样式；Win7 不传它不支持的重定向位，使用既有分层窗口绘制。候选、候选上下文菜单、托盘菜单和双拼键位提示使用同一边界。系统版本未知时不宣称具有新版样式能力。

辅助管道仅在 `TokenIsAppContainer` 返回 `ERROR_INVALID_PARAMETER` 且内核明确早于 Win8 时按非 AppContainer 继续；仍检查同账号、同会话、中完整性下限。现代系统失败或系统版本未知时继续拒绝，不能把任意令牌查询错误当作安全属性通过。

Server 异常仅输出固定阶段名，不记录可能包含路径、凭据或用户输入的异常正文。诊断副本临时关闭 UIAccess 只用于获取重定向结果，真实验收必须使用重新签名、安装到安全目录的产品；不能用副本替代产品验收。

### 既有笔记审计

- [完整构建与安装布局](../../implemented/feature/2026-10-24-windows7-product-build.md)：部分重叠，本篇记录真实 Server 启动暴露的系统 API 边界，产品布局与安装生命周期仍由原笔记持有。
- [DPI 与绘制回退](../../implemented/feature/2026-10-23-windows-optional-dpi-and-composition.md)：部分重叠，原切片处理导入与设备路径，本篇补充 Win7 拒绝窗口样式的事实，不改变现代合成器。
- 检索 Win7、管道权限、AppContainer 与重定向样式的活跃笔记，没有需要翻转的既有决定。

## Alternatives considered

- **删除 AppContainer ACE。** Win7 没有 AppContainer，删除能绕过解析错误；但同一源码在现代系统会失去原有沙箱应用连接能力，因此保留原 SID 和权限。
- **无条件删除重定向样式。** 所有窗口可以用一个样式创建，但会改变现代 DirectComposition 的原生路径，因此只在不支持该位的系统上省略。
- **忽略所有不支持的令牌查询。** 实现更少，但现代系统的真实错误也会放行；仅接受确定旧内核上的特定错误，保留其余身份与完整性校验。

辅助管道回归还暴露测试使用已经达到的七次点击作为新目标，并在上一连接收尾时立即打开空闲连接。改为等待本次计数递增，仅在 `ERROR_PIPE_BUSY` 上等待下一实例且保留两秒上限；其他错误立即失败，继续验证空闲客户端、后续请求和端点恢复。该竞争同时在 Win7 与 Windows 11 x64 复现，不修改产品通信超时。

## Verification

Win7 SP1 x64 上两种进程架构的管道身份、辅助维护、语音控制连接和 DPI 五项测试均通过，共十项；Windows 11 上同套测试也通过。现代布局、DPI 与 transcript 三项回归及完整 Server 入口编译通过。已安装 Server 的 PID 在连续观测和真实管道打开期间保持不变，三条真实管道均返回当前会话与该 Server PID；关闭 UIAccess 的诊断副本也保持运行。

`verify-local.sh --quick` 的两次远程门禁通过（12 分 36 秒、20 分 49 秒）。启动与通信边界通过后，真实记事本暴露的首次文档焦点缺口由[线程焦点修复](2026-10-24-windows-tsf-activation-document-focus.md)补齐。最终受信任签名产品的 x64／x86 系统记事本均已显示候选并提交中文，完整证据见[产品构建](../../implemented/feature/2026-10-24-windows7-product-build.md)。

## Consequences

相同权限策略可以在 Win7 解析，候选相关窗口可以创建，辅助维护请求在保留账号、会话和完整性下限的前提下接受 Win7 桌面进程。现代系统继续原有窗口样式与 AppContainer 查询。

版本探测失败时辅助管道继续拒绝维护请求，不能为可用性放宽安全判断。增加了一个内核能力边界和维护测试；运行库装载、真实编辑器、安装和权限属于不同证据，不能互相替代。
