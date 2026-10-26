# Agent Note: Windows 7 独立正式发布

Status: implemented

## Problem

Win7 构建工作流只上传 Actions artifact，用户无法从 GitHub Release 下载，也收不到应用内的 `windows7-vX.Y.Z` 正式更新。此前把受信任代码签名误判为所有 Win7 功能和正式发布的前提，阻断了已通过安装与真实输入验收的版本。

## Decision

在 `release-windows7.yml` 增加显式 `publish` 开关，仅允许 `main` 上的 `full` 构建创建正式版。发布作业复核 x64、x86 安装包及逐包 SHA-256，生成只列安装包的 `SHA256SUMS`，然后创建 `windows7-vX.Y.Z` Release。正式构建不加本机测试证书；发布说明明确未使用受信任代码签名。原有内部构建仍加测试签名，安装冒烟仍检查该签名；正式构建运行同一套安装、升级和卸载冒烟，只跳过签名断言。无签名构建保持 `UIAccess` 关闭，跨完整性级别能力不在本版承诺范围内。

本决定改变了[Win7 发布构建 CI](2026-10-25-windows7-release-ci.md)中“正式发布仍需受信任签名”的结论；该笔记保留原设计的历史理由。

## Alternatives considered

- **等待受信任代码签名**：可减少 Windows 未知发布者提示，并支持经安全目录安装后的 `UIAccess`。现有 Win7 安装与普通输入能力不以此为前提，继续等待会阻断已完成的功能交付；发布页用摘要校验和明确提示控制下载辨识。
- **直接把旧 Actions artifact 手工挂到 Release**：操作快，但无法保证附件与 `main` 的提交一致，也不能在重跑时自动核对两种架构及其摘要，因此改用从 `main` 重新构建的发布作业。

## Consequences

Win7 正式版进入独立更新通道，用户可下载 x64 或 x86 安装包。安装时可能出现未知发布者提示；`UIAccess` 关闭使高完整性级别窗口中的跨级别输入能力受限。正式发布前仍需等待工作流构建和安装冒烟通过，并复核 Release 附件。此前在 Win7 SP1 真机上的验收来自带受信任测试签名的构建；未签名正式包的真机验收应在发布后补充。
