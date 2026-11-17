# Agent Note: Windows 手动发布自动完成 SimplySign 签名

Status: implemented

## Problem

Windows 的发布工作流只有 SignPath 与未签名路径，维护者仍需另行签名并替换发布附件。维护者明确要求保留手动触发，同时自动完成 SimplySign 签名和发布；runner 已安装固定版本的 `ssign`，并将 Certum TOTP URI 用当前用户 DPAPI 加密保存。

## Decision

复用现有 `Sign-StagedPackage-SimplySign.ps1`，新增 `WINDOWS_SIGNING_PROVIDER=simplysign` 路径。构建继续在临时构建机完成，专用签名 runner 串行处理所选版本的暂存包，完成包内签名、编译安装器、外层签名及摘要生成；安装验证继续在临时 x64 和 ARM64 runner 执行，通过后才发布。签名客户端摘要和签名证书指纹必须匹配维护者配置。

密钥留在签名机当前用户 DPAPI 存储，工作流不持有长期 seed。签名 runner 分组只允许本仓库 `main` 与 `develop` 的发布工作流使用，避免普通 PR 任务接触签名身份。非发布构建继续不签名；显式选择 SimplySign 却缺配置或从其他分支发布时直接失败。

签名入口将 `ssign` 的实际 SHA-256 与仓库变量比对，将新签文件和外层安装器的证书指纹与发布配置比对；已有有效上游签名仍保留。每个版本在新 TOTP 时间窗登录，避免前一个版本刚用过的验证码被再次提交。每个签名作业清理暂存目录，进程退出时清除 seed 和本轮登录缓存。构建和签名使用同一个派发提交，符号包不重编。

维护者要求尽量复用有限的签名额度，并允许在能够复用时保留多个版本。`Sign-Files-SimplySign.ps1` 先按未签名字节的 SHA-256 分组，每组只签一个文件，再复制结果。同轮六个串行作业通过签名机当前用户目录的 `CertumAutomation/signature-cache/<GITHUB_RUN_ID>/<证书指纹>` 共享缓存，同一个运行的失败重试也沿用；下一轮自动清理旧运行缓存。缓存只保存签后文件与当前用户 DPAPI 保护的签后摘要，保护附加数据包含原始摘要、证书指纹、时间戳地址、描述和 URL。命中时同时检查绑定、文件摘要、Authenticode、发布证书与可信时间戳；不完整或损坏的缓存直接失败，不尝试以其他合法签名文件替代。缓存目录用独占文件锁避免并发签同一文件。

日志和作业摘要分别统计本次新增签名的唯一文件数与复用文件数。各版本共用的 Rust 产物只消耗一次签名；字节不同的原生组件和外层安装包仍分别签名。统计表示提交给签名客户端的文件数，不等同于供应商账单计量；维护者提到的每月约 3000 次额度尚未核实。

签名复用机制保留；默认发六版的范围决定由维护者对实际额度消耗的反馈改为[默认只发布 full](2026-11-17-windows-release-full-default.md)。显式选择 all 时仍复用同轮签名缓存。

## 既有笔记归属

- [SimplySign TOTP 入口](../../implemented/process/2026-11-16-windows-simplysign-totp.md)：部分重叠。本次用户授权取代其中不接入 CI 的范围决定，保留无桌面客户端、会话清理和上游签名保留的选择。
- [SignPath CI 签名](../../implemented/process/2026-10-08-windows-signpath-ci-signing.md)：部分重叠。保留 SignPath 分支的托管构建来源和审批约束，新路径使用维护者的 Certum 证书。
- [Windows 发布页与符号包](../../implemented/process/2026-10-20-windows-release-page-symbols.md)：无决定变更，符号仍来自同一次构建。
- [未签名 Server 的普通权限能力](../../implemented/bug-fix/2026-10-21-windows-unsigned-server-and-settings-input.md)：沿用可信生产签名打开 UIAccess 的规则，不改变普通未签名构建。

## Alternatives considered

- 通过 SimplySign Desktop 自动填码再用 `signtool` 签名：沿用官方客户端和刚部署的填码能力，但 CI 服务不保证存在交互桌面。复用仓库已有的无桌面入口，桌面自动填码保留给人工操作。
- 将 seed 放入 GitHub secret 并在临时构建机签名：不必维护专用 runner，但会增加凭据分发范围，也绕开现有机器上的加密存储。本次将签名限制在已配置的机器上。
- 只发布 full：最直接减少不同版本的额外签名和构建，但会停止提供已有独立版本。按维护者后续授权，能够共享相同文件的签名时保留六个版本；不按文件名或版本名猜测可复用性。

## Verification

`scripts/test-windows-release-signing.py` 执行真实策略脚本，覆盖两条受信任分支、tag 和其他分支拒绝、fork 拒绝、缺失与非法指纹、未签名构建、SignPath 配置失败与测试策略优先级。`scripts/test-windows-editions.py` 确认四个版本矩阵共用同一个范围，all 包含全部六个版本，full 只包含完整版。

Windows 上的 `installer/tests/sign-exe-ssign.ps1` 覆盖签名和时间戳校验，并补充错误证书拒绝；`installer/tests/ci-simplysign.ps1` 用真实 DPAPI 和合成 URI 验证摘要不匹配、凭据缺失、错误算法、异常消息脱敏以及失败后的环境清理。两者接入 `ci-platforms.yml` 的 Windows scripts 作业。

签名行为测试还按实际提交给合成客户端的文件数验证同批去重、跨调用命中不调用客户端、不同内容独立签名，以及无时间戳、校验失败、合法签名文件替换、签名参数或证书变化、其他输入的 DPAPI 封条和部分缓存均被拒绝。输出目录与原地签名两种方式都覆盖；配置边界测试确认非法凭据不会创建或回收缓存。

真实签名机通过本次入口读取加密凭据，签署合成 EXE、DLL 和 Inno Setup 安装器；三份产物均通过 `signtool verify /pa /all`，Authenticode 为 Valid，证书匹配配置并带可信时间戳。预装 `ssign` 与固定上游 v0.1.8 发布归档逐字节核对一致。工作流通过 `actionlint`。

## Consequences

专用 runner 或 Certum 服务不可用会阻止发布。TOTP seed 与当前用户身份可代表签名身份，runner 分组必须限制到明确的发布工作流。Windows 0.5.0 的发布运行 `44014206427` 已完成六版真实构建、签名、x64 / ARM64 安装卸载和多版本共存验收，并发布正式版；本轮新增 86 个文件签名、复用 70 个文件签名。

维护者无需手工重打并替换安装器，代价是维护专用 Windows 签名机、第三方客户端的固定版本与证书指纹。runner 目前用保存凭据的用户登录任务启动，用户退出或机器重启后尚未登录时不接任务；无桌面签名指不依赖 SimplySign Desktop 的登录窗，不表示用户凭据可以由任意系统服务解密。
