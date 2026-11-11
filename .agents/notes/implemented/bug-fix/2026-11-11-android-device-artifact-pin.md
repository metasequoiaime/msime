# Agent Note: Android 设备 workflow 固定可解析的 artifact action

Status: implemented

## Problem

#11268 的 nightly 运行中，Android API 28 和 API 35 设备 job 在任何设备步骤开始前都因 `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9` 无法解析而失败。两个 job 的失败日志一致；同一运行的 Android release job 已成功，说明故障集中在设备 workflow 使用的 action 引用解析，而不是 APK 或设备测试本身。

## Decision

`android-device.yml` 中的六个 artifact 上传步骤统一固定到已存在且可解析的 `actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a`（`v7.0.1`）。范围只覆盖触发 #11268 的设备 workflow，不顺带修改其它 workflow 中当次运行已经成功的相同引用。

验证使用 actionlint、Android nightly signing workflow contract、`git diff --check`，并通过 GitHub API 和远端 tag ref 确认 `v7.0.1` 的提交可解析。设备测试本身无需重跑，因为失败发生在设备步骤前。

## Alternatives considered

- **全仓替换所有 `v7.0.2` 引用**：覆盖面更大，但 #11268 的失败只有两个 Android device job，其他 job 在同一次 nightly 已成功；扩大到发布和其它平台 workflow 会改变未受该 issue 影响的执行链，因此不采用。
- **继续使用 `v7.0.2` 并重跑 nightly**：可能依赖 runner 缓存或暂时性解析状态，不能消除已观测到的不可解析引用，因此不采用。
- **取消 artifact 上传步骤**：会丢失设备验收日志和对比截图，削弱失败诊断能力，因此不采用。

## Consequences

Android 设备 job 恢复使用已在仓库其它成功 job 中验证过的 artifact action 版本，并继续上传语音面板、空间复制、按键面、拼写编辑、主题结果和失败日志。其余 workflow 的 action 版本保持不变，若后续出现同样的独立失败，应按对应 issue 单独确认范围。
