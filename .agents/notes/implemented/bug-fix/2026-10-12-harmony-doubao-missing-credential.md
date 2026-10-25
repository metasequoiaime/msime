# Agent Note: 鸿蒙键盘在豆包凭据没填全时不再悄悄改用系统识别

Status: implemented

## Problem

鸿蒙键盘的 `HarmonyVoiceRecognizer.start` 只在 `asr_provider` 为 `doubao` 且 `asr_token` 非空时才走豆包，其他情况一律落到 CoreSpeechKit 系统识别，不给任何提示。于是：

- 填了 App ID（旧版鉴权）还没填 Access Token 的用户，按下语音键用的是系统识别，以为自己在用豆包。
- 密钥存在分槽 `asr_tokens.doubao`、当前框为空时（共享设置页切换服务的分槽写法会留下这种文档），共享解析 `resolve_voice_fields` 认这个密钥，鸿蒙键盘不认，同样悄悄用系统识别。
- 出厂配置就是 `doubao` 不带凭据，语音面板的说明 `VoiceInputConfigurationPolicy.description` 对它落到最后一支，写的是「当前语音服务不可用，请检查设置」，可它明明能用，只是用的是系统识别。设置页「服务配置」行也只写「未填写」，看不出键盘实际在用什么。

不能简单地改成「选了豆包没密钥就报错」：出厂的 `asr_provider` 就是 `doubao`，这样会让所有从没配置过语音的用户一开口就报错，Android 那边的[键区内系统识别修复](2026-10-08-android-platform-voice-in-keyboard.md)出于同样的理由也没有这么提示。2026-10-26 起出厂值改为 `local`（见[六端出厂识别服务改为本地模型](../feature/2026-10-26-voice-local-default.md)），按旧默认装好的用户文档里仍是不带凭据的 `doubao`，这条规则继续适用于他们。

## Decision

- 新增 `DoubaoCredentialPolicy`（`keyboard/input/VoiceInputConfiguration.ts`），把「要不要用豆包、缺什么」放在一处：
  - `token`：当前框优先，空时取 `asr_tokens.doubao`，与共享解析一致。`VoiceAsrTokens` 加上 `doubao` 字段。
  - `attempted`：选了豆包，并且密钥或 App ID 至少填过一项。
  - `missing`：填了一部分时缺的那一项（「豆包缺少 API Key」「豆包缺少 Access Token」「豆包旧版鉴权缺少 App ID」，都带「请在设置中填写」）；没选豆包、一项都没填或凭据齐全时为 null。
- 键盘按它分三种情况：
  - 一项都没填（出厂配置）：照旧用系统识别，不报错；语音面板写「豆包未填写密钥，暂用 HarmonyOS 系统语音识别」。
  - 填了一部分：`start` 直接抛出 `missing` 的那句话，显示在语音面板上，不再落到系统识别。
  - 填齐了：走豆包。`HarmonyDoubaoRecognizer` 的握手头和配置校验改用 `DoubaoCredentialPolicy.token`。格式问题（非法字符、接口地址）仍由握手前的 `validConfig` 报「请先配置有效的豆包接口、资源 ID 和凭据」。
- 共享设置页在鸿蒙手机上的「服务配置」行，豆包用同一套判断显示「未填写，暂用系统识别」「未填写完整」「已填写」（`doubaoServiceStatus`，`packages/ui/src/settings/voice-settings-content.tsx`）。
- 顺带改正两处过时的说明：`module.json5` 的麦克风权限注释和 `platforms/harmony/README.md` 的语音段落都还写着「原始音频始终留在系统服务内」，实际是键盘自己用 `HarmonyPcmCapture` 采音，再交给选中的识别方式。

## Alternatives considered

- **选了豆包没有密钥就报错**：最直白，用户一定知道自己没配好。不采用：出厂默认就是这个状态，会把所有没碰过语音设置的用户挡在门外，而系统识别对他们是能用的。
- **出厂默认改成 `system`**：根上消除歧义，豆包只在用户主动选择后出现。不采用：默认值在共享偏好里，所有宿主共用，改它牵动 Windows、macOS 等宿主的出厂行为和随包模板（`scripts/test-default-config-parity.py` 守着这些一致性），超出这次修复的范围；而且已经写进文档的 `doubao` 改不掉，鸿蒙键盘照样要处理它。
- **只改设置页文案，键盘行为不动**：改动最小。不采用：填了一半的用户按下语音键仍然悄悄用系统识别，问题出在键盘那一刻，设置页上的提示不在眼前。

## Consequences

- 收益：没配置的用户行为不变，说明从错误的「服务不可用」改成实话；配到一半的用户在语音面板上直接看到缺什么；分槽里的豆包密钥也能用了。
- 代价：「一项都没填」与「填了一部分」的界线靠用户有没有碰过凭据来判断。旧版鉴权切回 API Key 时留下的 App ID 会让键盘认为用户在配豆包，从而报「缺少 API Key」，而不是回到系统识别。键盘（ArkTS）和设置页（TS）各有一份这套判断，靠两边的测试保持一致。
- 验证：`platforms/harmony/tests/run.sh`（新组 `an untouched Doubao default stays on the system recognizer, a partial one says what is missing`，0 failure）；`scripts/test-harmony-voice-lifecycle.py`、`scripts/test-harmony-unwired-symbols.py`；`apps/desktop` 的 vitest `tests/settings`、`tests/voice` 全部通过；typecheck、lint 通过。没有在鸿蒙设备上验证。
