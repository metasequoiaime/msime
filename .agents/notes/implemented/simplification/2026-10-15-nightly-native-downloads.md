# Agent Note: nightly 直接分发原生安装文件

Status: implemented

## Problem

nightly 用户需要安装对应平台的产品，原先将全部平台再打成总包，重复上传约 3.26 GB，并因单附件超限引入分卷。用户明确要求移除总包，随后实际下载 iOS 包时解压得到模拟器 `.app`，误以为能用于 iPhone；平台外层归档还混入源码、调试符号和多层 artifact 目录，掩盖了真正的安装文件。

## Decision

`package-nightly.py` 按平台只选原生分发文件，直接平铺到 Release：Android 的 APK、macOS 的 DMG、Windows 的 EXE、Linux 的 DEB/RPM、HarmonyOS 的 HAP。保留各平台现有全量、拼音、五笔等版本，用户直接下载自己的版本。总包、平台外层压缩包、分卷、源码、调试符号和内部校验文件均不再发布。

iOS 继续采用用户已选定的外部 TestFlight 分发，沿用既有签名上传、测试组交付及审核提交流程。模拟器 `.app` 归档只保留在 Actions artifacts，nightly 汇总不再下载或上传该归档，也不将其当作手机安装包。TestFlight 构建不能靠重命名为 IPA 实现任意设备安装。

Release 正文复用 `README.txt` 的安装说明，明确 iOS 应通过外部 TestFlight 邀请安装、Apple 审核批准后才对外可用，以及 HarmonyOS 当前产物未签名的实际限制；macOS 的 DMG 已签名并公证（见[macOS nightly 用 Developer ID 签名并公证](../process/2026-10-15-macos-nightly-notarized.md)），说明里写的是正常安装步骤。不能把构建成功说成所有产物都能在零售设备直接安装。

保留一份 `SHA256SUMS`，直接校验 Release 中的原生文件。选中的平台必须有分发文件；重名、残留输出和单文件超限都在替换旧 nightly 前拒绝。六个平台的构建和各自已有验收仍须全部成功，iOS 分发成功仍是汇总发布前提。

本决定部分取代[六平台 nightly 构建笔记](../process/2026-10-14-six-platform-nightly-builds.md)的总包、平台归档及分卷选择，六平台共用发布链、同一源提交和全部成功后发布的要求继续有效。历史笔记检索只命中该篇，其余决定没有重叠。

## Alternatives considered

- **保留总包并分卷**：提供一次下载所有平台的入口，已在远端验证能绕过附件上限；但用户没有这种需要，产物重复上传且需要合并分卷，因此删除。
- **只删总包，仍把每个平台包在外层归档里**：只需小改并保留原有下载布局；但用户仍要逐层寻找安装文件，iOS 模拟器包仍会被误当手机安装包，因此直接分发原生文件，iOS 说明真实的 TestFlight 入口。
- **把模拟器 `.app` 重新压成 `.ipa`**：看起来符合用户对扩展名的预期；但不会改变目标架构、运行平台和签名要求，不能在 iPhone 上安装，因此不做伪装。

## Consequences

普通用户可以直接找到 APK、DMG、EXE、DEB/RPM，无需先下载同平台所有版本再解压。Release 资产数量增加，但单文件对应实际分发物；下载说明按平台解释安装方式，TestFlight 审核和未签名 HAP 的限制公开说明。

当前已发布 nightly 复用运行 `38661748815` 的原始 artifacts，不重新编译六个平台。原生文件与原始 artifact 的逐文件 SHA-256 必须匹配，源提交保留 `70db2dfb2d13b8269b33271a9bc6097fe00dd5cf`；先上传原生文件和新清单，再移除旧归档，最后核对资产集合。

## Verification

合成测试检查原生文件逐字节不变、单份校验清单、过滤源码/符号/模拟器包，以及缺平台原生文件、文件重名、残留输出和单文件超限时拒绝发布。合并前运行本地 quick、actionlint、verify-notes 和 diff check；现有 Release 调整后通过 GitHub asset digest 对照输入文件，核对五个平台原生分发文件和 iOS TestFlight 说明。
