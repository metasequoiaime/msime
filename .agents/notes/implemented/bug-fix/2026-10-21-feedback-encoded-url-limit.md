# Agent Note: 反馈提交按编码后的完整网址裁剪正文

Status: implemented

## Problem

macOS 和 Linux 的原生设置入口承载共享 `SettingsPage`。反馈表单允许 4000 个 UTF-16 码元，`useFeedbackReport` 先将报告截到 4000 再交给 `URLSearchParams`；桌面宿主 `external_url_is_safe` 则拒绝超过 4096 字节的完整网址。合成的 639 字符中文报告编码后是 5652 字节，因此「在 GitHub 提交」把网址交给宿主后被拒绝，浏览器没有打开。

## Decision

沿用 `URLSearchParams` 的表单编码，按 Unicode 码点选报告前缀，限制编码后的完整网址不超过既有外链上限。用二分查找保留能够放下的最长前缀，不截断百分号序列或拆开代理对。原始报告、草稿和「复制报告」仍保留完整内容；界面已经说明过长描述会被截断。

## Historical audit

- [Windows 原生反馈表单](../../implemented/feature/2026-10-11-windows-settings-feedback.md)部分重叠：它记录了原有共享表单的 4000 码元裁剪和原生复制品。保留原生实现及其尚未验证的浏览器边界，互链澄清共享 Tauri 路径的实际限制。
- 检索 `proposed`、`implemented` 和 `rejected` 中的 `URLSearchParams`、`use-feedback-report`、反馈截断及报告截断；其余命中是词库导入、语音回复或剪贴板预算，属于无关决定，不更改。

## Alternatives considered

- 提高宿主外链上限：可以保留更长的预填内容，但会改变所有外链的输入边界，并依赖系统和浏览器接受更长的网址；反馈页已经提供完整复制入口，修复调用方即可。
- 将描述框缩短到固定几百字：实现最少，但编码膨胀随字符不同，固定上限会浪费 ASCII 空间，也会限制复制完整报告。
- 将编码后的网址直接截到 4096：最直接满足长度检查，但会拆坏百分号编码和 UTF-8 字符，甚至截掉参数结构。

## Verification

- macOS、Linux 完整反馈页面的四项长中文、需转义 ASCII 和 emoji 回归在旧实现上全部失败，实际收到的 URL 分别为 5882、5882、12143、13482 字节；短报告对照通过。
- 回归同时检查截断后的正文是原始报告的 Unicode 完整前缀、再加一个码点就超预算、短报告原样预填，以及复制仍包含完整报告。
- 修复后相关 6 文件 29 项测试通过；独立审查者运行其中 5 文件 24 项测试通过。桌面类型检查和生产构建、Harmony 类型检查、相关 lint/格式与笔记校验通过。

## Consequences

网址只预填报告的前缀，长描述后的环境信息可能不在链接里，与既有前缀裁剪语义一致；用户需要复制完整报告补充。Windows 原生反馈表单不走 Tauri 校验，其浏览器边界需要单独验证。
