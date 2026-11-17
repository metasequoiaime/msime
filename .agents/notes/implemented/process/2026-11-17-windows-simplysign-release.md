# Agent Note: Windows 手动发布自动完成 SimplySign 签名

Status: implemented

## Problem

Windows 的发布工作流只有 SignPath 与未签名路径，维护者仍需另行签名并替换发布附件。维护者明确要求保留手动触发，同时自动完成 SimplySign 签名和发布；runner 已安装固定版本的 `ssign`，并将 Certum TOTP URI 用当前用户 DPAPI 加密保存。

## Decision

复用现有 `Sign-StagedPackage-SimplySign.ps1`，新增 `WINDOWS_SIGNING_PROVIDER=simplysign` 路径。构建继续在临时构建机完成，专用签名 runner 串行处理六个版本的暂存包，完成包内签名、编译安装器、外层签名及摘要生成；安装验证继续在临时 x64 和 ARM64 runner 执行，通过后才发布。签名客户端摘要和签名证书指纹必须匹配维护者配置。

密钥留在签名机当前用户 DPAPI 存储，工作流不持有长期 seed。签名 runner 分组只允许本仓库 `main` 与 `develop` 的发布工作流使用，避免普通 PR 任务接触签名身份。非发布构建继续不签名；显式选择 SimplySign 却缺配置或从其他分支发布时直接失败。

签名入口将 `ssign` 的实际 SHA-256 与仓库变量比对，将新签文件和外层安装器的证书指纹与发布配置比对；已有有效上游签名仍保留。每个版本在新 TOTP 时间窗登录，避免前一个版本刚用过的验证码被再次提交。每个签名作业清理暂存目录，进程退出时清除 seed 和本轮登录缓存。构建和签名使用同一个派发提交，符号包不重编。

## 既有笔记归属

- [SimplySign TOTP 入口](../../implemented/process/2026-11-16-windows-simplysign-totp.md)：部分重叠。本次用户授权取代其中不接入 CI 的范围决定，保留无桌面客户端、会话清理和上游签名保留的选择。
- [SignPath CI 签名](../../implemented/process/2026-10-08-windows-signpath-ci-signing.md)：部分重叠。保留 SignPath 分支的托管构建来源和审批约束，新路径使用维护者的 Certum 证书。
- [Windows 发布页与符号包](../../implemented/process/2026-10-20-windows-release-page-symbols.md)：无决定变更，符号仍来自同一次构建。
- [未签名 Server 的普通权限能力](../../implemented/bug-fix/2026-10-21-windows-unsigned-server-and-settings-input.md)：沿用可信生产签名打开 UIAccess 的规则，不改变普通未签名构建。

## Alternatives considered

- 通过 SimplySign Desktop 自动填码再用 `signtool` 签名：沿用官方客户端和刚部署的填码能力，但 CI 服务不保证存在交互桌面。复用仓库已有的无桌面入口，桌面自动填码保留给人工操作。
- 将 seed 放入 GitHub secret 并在临时构建机签名：不必维护专用 runner，但会增加凭据分发范围，也绕开现有机器上的加密存储。本次将签名限制在已配置的机器上。

## Verification

`scripts/test-windows-release-signing.py` 执行真实策略脚本，覆盖两条受信任分支、tag 和其他分支拒绝、fork 拒绝、缺失与非法指纹、未签名构建、SignPath 配置失败与测试策略优先级。`scripts/test-windows-editions.py` 确认四个版本矩阵都包含全部六个版本。

Windows 上的 `installer/tests/sign-exe-ssign.ps1` 覆盖签名和时间戳校验，并补充错误证书拒绝；`installer/tests/ci-simplysign.ps1` 用真实 DPAPI 和合成 URI 验证摘要不匹配、凭据缺失、错误算法、异常消息脱敏以及失败后的环境清理。两者接入 `ci-platforms.yml` 的 Windows scripts 作业。

真实签名机通过本次入口读取加密凭据，签署合成 EXE、DLL 和 Inno Setup 安装器；三份产物均通过 `signtool verify /pa /all`，Authenticode 为 Valid，证书匹配配置并带可信时间戳。预装 `ssign` 与固定上游 v0.1.8 发布归档逐字节核对一致。工作流通过 `actionlint`。

## Consequences

专用 runner 或 Certum 服务不可用会阻止发布。TOTP seed 与当前用户身份可代表签名身份，runner 分组必须限制到明确的发布工作流。完整六版本构建与设备安装仍需首次人工派发发布工作流验收，本次配置任务不自动发布产品版本。

维护者无需手工重打并替换安装器，代价是维护专用 Windows 签名机、第三方客户端的固定版本与证书指纹。runner 目前用保存凭据的用户登录任务启动，用户退出或机器重启后尚未登录时不接任务；无桌面签名指不依赖 SimplySign Desktop 的登录窗，不表示用户凭据可以由任意系统服务解密。
