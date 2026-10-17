# Agent Note: Android JNI 导出清单补齐

Status: implemented

## Problem

`NativeClient` 和 `client_jni.cpp` 已经加入本地语音的 `localSpeechSpeakingRaw` 与数字规整的 `voiceNormalizeNumbersRaw`，但 `verify-native.sh` 的导出清单没有同步更新。构建验证因此没有检查这两个实际导出，仓库合同检查也会在 JNI 名称一致性测试中失败。

## Decision

把两个现有导出加入 `verify-native.sh` 的 JNI 方法清单，并继续用三方名字一致性脚本检查 `NativeClient.java`、`client_jni.cpp` 和清单的集合一致。原生库验证因此覆盖这两个方法，新增 JNI 方法不能绕过发布前检查。

## Alternatives considered

- **只调整合同测试，让它忽略缺项**：改动最小，但会继续让原生导出校验漏检真实方法，无法阻止发布时的 JNI 漏配。
- **重写清单生成流程**：可以减少手工同步，但会扩大本次缺陷修复范围，并不能替当前已有缺项补齐；先保留现有显式清单和一致性测试。

## Consequences

合同检查现在能发现这两个 JNI 导出是否出现在实际 `libmsime_android.so` 中，构建时多检查两个符号。清单仍需在以后新增 JNI 方法时同步维护，但一致性脚本会在提交和 CI 中及时暴露遗漏。

验证：`python3 scripts/test-android-jni-names.py` 通过，报告 131 个 native 方法与 C++ 导出及清单一致；`git diff --check` 和 `pnpm run verify-notes` 通过。
