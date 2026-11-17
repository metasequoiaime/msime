# Agent Note: Android WebDAV 地址逐字符输入

Status: implemented

## Problem

#11466 报告 Android 的 WebDAV 地址输入框每键入一个字符就提示错误，导致地址无法逐字符输入。两个 WebDAV 设置页在 `afterTextChanged` 中直接调用完整地址校验，并把尚未完成的 `h`、`https://` 等中间态当成错误交给 `EditText.setError`。

## Decision

设置页继续使用严格的 `webdavUrlProblem` 作为测试连接和保存的门禁；输入过程通过共享的 `webdavUrlProblemWhileTyping` 只在已经出现 `://` 且主机至少有一个字符后显示地址错误。尚未形成可校验的地址时清除错误提示，避免 Android 在每个字符上弹出错误提示并干扰输入。云剪贴板和词库同步两个 WebDAV 入口共用这一条输入态规则。

## Alternatives considered

- **把严格校验改成允许部分地址**：会让保存路径和共享层规则失去单一事实来源，非法地址可能绕过门禁，因此不采用。
- **完全移除输入框错误提示**：能避免弹窗，但用户在禁用“测试连接/保存”时看不到具体原因，因此不采用。

## Verification

- 逐字符输入 `https://dav.example.test/dav` 时，在形成 `://` 和主机前不显示地址错误，输入可以持续进行。
- 已形成主机后的非法地址仍显示具体错误，严格校验结果不变。
- WebDAV 云剪贴板和词库同步两页都使用同一规则；共享 JVM 冒烟覆盖中间态与完整非法地址。

## Consequences

错误提示会比原来稍晚出现，但只延迟到地址具备可解析的协议和主机，不改变测试连接、保存或共享 Rust 的校验。
