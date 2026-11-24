# Agent Note: 客户端桌面解析不到时候选窗退到 Server 桌面

Status: implemented

## Problem

#11793：Windows 10 的开始菜单搜索框（`SearchApp.exe`，低完整性宿主）里用全拼输入，字母正常进入组字缓冲，候选窗始终不出现；同一版本在普通应用里候选窗正常。Server 日志每 5 秒一条 `Candidate desktop routing unavailable`（`platforms/windows/src/candidate/DesktopCandidates.cpp` 的诊断节流），只在 SearchApp 输入期间出现。

这条日志是协调线程解析客户端桌面（`ClientDesktop::resolve`）失败之后的兜底诊断：拿不到桌面就没有目标 `Surface`，那一帧不发布给任何 UI 线程，候选窗自然不出现。`resolve` 的每一步都可能在这一类宿主上失败——查询客户端线程所属桌面、按桌面名打开拥有句柄、核对句柄身份——报告环境里只有「失败」这一个事实，没有分清是哪一步。

按[候选 UI 桌面路由](../feature/2026-10-24-windows-desktop-candidate-routing.md)的原有决定，解析失败就不回退到 Server 自己的桌面：宁可没有候选窗，也不把候选窗画到一个不知道是否属于用户的桌面上。那条规则保护的是「多桌面输入时把候选画到错误桌面」这个隐私边界，代价是任何解析不到桌面的宿主彻底没有候选窗。

## Decision

1. `ClientDesktop::resolve_or_server`（`platforms/windows/src/system/ClientDesktop.h`）把「票据认证」和「桌面解析」分开：`authenticate` 只核对票据声明的进程与线程归属，认证不通过就返回空，**不允许任何回落**——票据不可信时把候选窗画到任何地方都是越权。
2. 认证通过但桌面解析不到时，退到 `ClientDesktop::server_desktop()`：Server 自己线程所在的那块桌面，按名字打开一个拥有句柄，`fallback()` 为真、`current()` 只看句柄是否还开着。落回的这枚句柄和解析出来的同一桌面经 `same_desktop_object` 认作同一个对象，因此与既有桌面共用一套 UI 线程，不额外建窗。
3. 协调线程用 `resolve_or_server`，回落时用 `diagnose` 记下失败的阶段和系统错误（`desktop name query failed, error=<N>`、`desktop open failed, error=<N>`、`client desktop identity mismatch`），再补 `using the server desktop`。日志仍然只有一条，但换成能定位到具体步骤的文案，SearchApp 一类宿主下次再出问题不必再猜。
4. `resolve` 保持原语义（不回落），供需要严格归属的调用方与原生测试使用。

## Alternatives considered

- **把 `resolve` 失败时仍不回落，改为逐步骤放宽权限**：只在报告者环境能分清是哪一步失败之后才有意义；`GetThreadDesktop`、`GetUserObjectInformationW`、`OpenDesktopW` 各自的访问检查和低完整性令牌的限制并不相同，猜着放宽等于把归属校验一项项拆掉，而且无法在本机验证。
- **去掉桌面绑定层，改走绝对坐标 + `WS_EX_TOPMOST`**：和 Weasel 一样按光标矩形直接 `SetWindowPos`，实现最省事，但会推翻 #9439 的多桌面隔离——在 Sysinternals Desktops 一类独立桌面上输入时，候选窗会跑到另一块桌面上。
- **只在 `indexed` 的特定进程名（`SearchApp.exe`、`StartMenuExperienceHost.exe`）上回落**：把宿主名单写死在产品里，下一个低完整性宿主（设置、通知中心、第三方 UWP）照样没有候选窗，而且名单本身就是对系统实现的猜测。
- **解析失败时不画候选、只改日志与文档**：问题原样保留，报告者的输入法在开始菜单里仍然不可用。

## Consequences

- 低完整性宿主上候选窗重新出现：落回的桌面就是开始菜单所在的桌面，坐标仍按 TSF 上报的光标矩形算。
- 代价：真正跨桌面（Sysinternals Desktops 一类）且客户端桌面解析失败的客户端，现在会把候选窗画在 Server 自己所处的桌面上，这个窗口可能出现在用户没有在输入的那块桌面上。票据认证不通过的帧仍然什么都不画，所以单靠伪造票据不能借用这条回落。
- `ClientDesktop` 多一个 `fallback_` 状态和两个工厂：`server_desktop()` 与 `resolve_or_server()`；`resolve()` 与既有调用点行为不变。
- 回落是按票据粘住的：同一客户端线程的后续帧一直用这块桌面，不会在「解析失败 → 解析成功」之间来回在两套 UI 线程间切换；换票据（进程或线程变了）才重新解析一次。
- 未改 `surfaces` 的数量上限（8 套）与桌面销毁顺序。

## Verification

- `platforms/windows/tests/system/client_desktop.cpp` 新增 `server_fallback`：`server_desktop()` 的句柄等于当前线程所在桌面、与两块私有桌面互不相同、能交给新线程 `SetThreadDesktop` 后建窗；认证通过的票据经 `resolve_or_server` 仍解析客户端自己的桌面且 `fallback()` 为假；`ticket(pid, 0)` 与 `ticket(0, tid)` 这类认证不通过的票据一律返回空。
- 本机用 `x86_64-w64-mingw32-g++`、`i686-w64-mingw32-g++` 以 CMake 中的标志（`-Wall -Wextra -Werror -UNDEBUG`）单独编译了 `client_desktop.cpp`、`DesktopCandidates.cpp` 与 `desktop_candidates.cpp`，无警告。
- 未验证：`windows-client-desktop` 与 `windows-desktop-candidates` 是原生 Windows 测试，本机没有 Windows 主机、也没有 Wine/Rosetta，无法运行；这两个测试也不在任何 CI 作业里执行，只由 Windows GNU 交叉构建编译。开始菜单搜索框里的实际候选窗需要报告者在 Windows 10 19045 上复验，日志里应出现 `Candidate …; using the server desktop` 并能看到候选。
