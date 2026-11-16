# Agent Note: Windows runner 用 SimplySign TOTP 自动签名 EXE

Status: implemented

## Problem

现有 Windows 发布脚本通过 SimplySign Desktop 的虚拟卡和 `signtool` 签名，需要在交互桌面输入手机上的一次性验证码。PVE9 上的 Windows runner 主要通过 SSH 使用，不能把登录桌面当成可靠的自动化前提；同时不能把 SimplySign 的长期 TOTP seed 写进仓库或命令行。

## Decision

仓库提供 `platforms/windows/installer/Sign-Exe-SimplySign.ps1` 作为单独的 SSH 签名入口。它只接受 `.exe`，把 `CERTUM_EMAIL` 和 `CERTUM_OTP` 留在当前进程的环境变量中，调用外部、由维护者自行固定和审核版本的 `ssign.exe`，一次调用签名全部输入文件，然后用 Windows SDK 的 `signtool verify /pa /all` 与 `Get-AuthenticodeSignature` 检查 Authenticode 状态和 RFC 3161 时间戳。没有凭据、工具或输入文件时在联网前失败；脚本不会下载 `ssign.exe`，也不会把 seed 作为参数传给子进程。

这个入口只解决 `.exe` 的无桌面签名。现有 `Package-SimplySign.ps1` 继续负责完整安装包构建和 SimplySign Desktop 的手工证书存储流程；SignPath 的生产发布流程也保持不变。`ssign` 的版本、下载摘要和 seed 管理属于 runner 的运维配置，不进入仓库。

## Alternatives considered

- **继续要求 SimplySign Desktop 和 RDP 登录。** 这是 Certum 官方文档描述的路径，且不需要把长期 seed 放到 runner；但 SSH 构建无法可靠保证同一交互会话、托盘程序和证书卡都在线，所以不能满足无桌面自动签名。
- **把 TOTP seed 作为脚本参数或写入仓库。** 调用简单，但会进入 shell 历史、进程列表或 Git 历史；seed 与邮箱可以长期代表签名身份，因此不采用。
- **在 release workflow 中自动启用 `ssign`。** 这会把非官方客户端和长期签名凭据扩大到所有发布 job，并绕开现有 SignPath 审批边界；本次只提供受维护者明确调用的 SSH 入口。

## Consequences

维护者可以在专用 Windows runner 上通过 SSH 签一个或多个 EXE，脚本只需一次登录并自动计算验证码。输出文件默认原地签名，也可以用 `-OutputDirectory` 写入独立目录；每个输出都会经过签名和时间戳校验。

代价是 `ssign` 不是 Certum 官方客户端，Certum 接口变化或服务条款变化都可能使入口失效；runner 必须保持时钟同步，并把 TOTP seed 当作私钥保管。seed 泄漏时需要重新签发 SimplySign 激活二维码。

验证脚本覆盖缺少邮箱、缺少 seed、缺少 `ssign.exe` 和非 `.exe` 输入的前置失败路径；真实云签名需要维护者在 runner 上提供受保护凭据后单独执行。
