# Agent Note: 腾讯示例凭据仍提示下载 macOS 系统翻译语言

Status: implemented

## Problem

共享设置页以两个腾讯字段是否为非空字符串判断本机翻译是否启用。`<YOUR_TENCENT_SECRET_ID>` 和 `FAKESECRET_` 等示例占位值非空，却不能产生共享层的腾讯服务描述。macOS 实际使用系统翻译补全，此时页面反而隐藏未下载语言的提示。

## Decision

复用页面选择服务时已有的 `tencentSecretConfigured` 判断两个字段。它与 `client-core::translation::usable_credential` 一致；任一凭据不可用时，腾讯服务不阻止本机翻译提示。已有自定义服务、小牛翻译、水杉账号以及候选翻译关闭的条件继续决定提示是否可见。

## Alternatives considered

- **在提示计算里复制占位值识别**：改动集中，但同一文件已有公共判断，复制会再产生两套规则。
- **只依据下拉框选择腾讯就隐藏提示**：最简单，但腾讯默认启用且无凭据时依然需要系统翻译，忽略了宿主实际执行条件。

## Verification

四项占位值界面回归在旧实现失败，空凭据控制用例通过。测试覆盖两类占位值及只填好一项的组合，均应提示缺少系统语言；有效腾讯凭据和主动选择水杉账号时保持隐藏。现有测试覆盖下载语言筛选、打开系统设置和禁用状态。

修复后运行 `translation-settings.test.tsx` 与 `tencent-translation.test.tsx`，共 64 项通过，全部使用合成凭据和宿主回执。

## Consequences

这里只决定共享设置页提示，实际服务选择仍由共享 Rust 层与原生宿主管理。占位值识别应与共享凭据判断保持一致。

## Existing notes audit

检索 `use-translation-settings`、`onDeviceMissingLanguages`、`downloadableLanguages` 和系统翻译语言的活跃笔记。[语言刷新顺序](../../implemented/bug-fix/2026-10-12-macos-on-device-language-refresh-order.md)部分重叠，但负责最新读取的结果归属；本篇负责是否需要显示该结果，保留并互链。Windows 翻译服务、候选释义分栏和 Android 用户翻译服务笔记不涉及 macOS 系统语言提示，无相同决定。
