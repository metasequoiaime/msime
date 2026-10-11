# Agent Note: Android 语音贡献撤回同意

Status: implemented

## Problem

设置页允许用户随时关闭“上传语音以改进识别”。键盘语音结果回调会把贡献上传排入长期 worker，但原任务只使用入队时的设置判断；用户在任务开始前关闭同意时，排队任务仍会上传音频和识别文本。

## Root cause

`VoicePage` 和键盘进程共享 `android-settings.json`，但 `ImeVoiceEntry.contribute` 在 worker 执行前没有重新读取 `VOICE_CONTRIBUTE_AUDIO`。键盘进程内存快照可能仍是开启状态，设置页的跨进程撤回不会自动改变它。

## Decision

在贡献 worker 真正开始上传前，通过 `AndroidLocalSettings.load(s)` 重新读取本地同意开关；缺失、损坏或关闭都按默认关闭处理。现有生命周期代次和隐私门检查继续保留。

## Alternatives considered

- 只在设置页关闭时通知键盘进程：进程可能不在、通知可能丢失，worker 仍需要最终的持久化读取。
- 只刷新键盘内存快照：刷新与上传之间仍有竞态，不能替代上传前的检查。
- 取消整个语音 worker：会同时影响识别和统计任务，且无法停止已经开始的网络请求。

## Consequences

尚未开始的贡献任务会尊重最新的本地同意状态；已经开始的上传不伪装成可撤回，仍由网络客户端完成当前请求。

## Verification

- `python3 scripts/test-android-voice-contribution-consent.py`：旧实现失败，修复后通过。
- `bash platforms/android/check-host.sh`：204 个 Android JVM 冒烟、78 个设备源编译通过。
- `git diff --check`
