# Agent Note: Android 英文默认小写

Status: implemented

## Problem

Android 切到英文时会读取编辑器的大写标记，试用输入框带句首大写标记，因此空输入框立即显示大写。用户明确要求英文不要默认大写。

## Decision

增加本机「英文自动大写」开关，缺省关闭。关闭时不读取光标前文，也不按编辑器标记启用自动 Shift；手动 Shift 和双击大写锁定继续有效。开启时沿用已有的单词、句子和全字母大写规则。

## 历史决策审计

- [句末标点后隔着空白才自动大写](2026-10-08-sentence-capitalization-needs-space.md) 部分重叠：本决定取代 Android 默认开启的前提，开启后的规则和其他平台继续适用，两篇互链。
- [双击 Shift 锁定大写](2026-10-08-shift-double-tap-caps-lock.md) 部分重叠：继续保护手动单次大写和锁定，不改状态机。
- [Shift 就地更新键面](2026-10-11-android-shift-relabel-in-place.md) 部分重叠：沿用就地刷新，不重建按键。

## Alternatives considered

- 完全删除自动大写最简单，也满足当前需求；保留默认关闭的开关，让需要句首大写的人能恢复原有行为，且不改变其他平台。
- 只移除试用输入框的句首大写标记，改动最小；其他应用仍会自动大写，不能满足默认小写的要求。

## Consequences

缺省设置下，空输入框、换行、句末空格后保持小写；开启自动大写后恢复编辑器规则。手动单次 Shift 的保护和双击锁定继续沿用原状态机。开关只在本机保存，不扩展同步协议。使用默认设置的现有用户也会改为小写，需要自动大写时自行开启。

## Verification

新增 JVM 冒烟覆盖所有编辑器大写模式在关闭时保持小写、开启时沿用旧规则，以及设置缺省关闭、显式开启和不同步。`scripts/verify-local.sh --quick`（含 Android 宿主 JVM 冒烟）、`pnpm run verify-notes` 和 `git diff --check` 通过。签名 APK 覆盖安装到 Android 12 真机；空试用输入框从中文切英文显示小写，点字母得到 `a`，点 Shift 再点字母得到 `A`，随后键面回到小写。合成测试文字已删除，没有发送聊天请求。
