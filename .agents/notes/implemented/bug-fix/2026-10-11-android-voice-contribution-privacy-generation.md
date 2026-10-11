# Agent Note: Android 语音贡献上传隐私代次

Status: implemented

## Problem

键盘语音识别完成后，音频和识别文本会排入后台贡献上传。原实现只在识别结果回调里检查隐私状态；用户随后开启隐私模式并触发会话重启时，已经排队的 worker 仍可能继续上传这次语音。

## Root cause

`ImeVoiceEntry.contribute` 把贡献任务提交给长期存在的 worker，却没有绑定输入服务代次，也没有在 worker 真正开始网络请求前重新读取隐私门。`restartSessionForPrivacy` 先调用 `stop(false)`，旧的 `engineStartGeneration` 在解析新会话选项期间仍保持有效，不能阻止排队任务。

## Decision

语音贡献记录当前输入服务代次；worker 开始上传前同时确认代次仍相同、隐私门仍允许贡献。隐私重启在停止旧会话前立即推进 `engineStartGeneration`，并把同一代次传给新会话启动任务。

## Alternatives considered

- 只在识别结果回调处再次检查隐私：任务入队后到真正上传之间仍可切换隐私模式。
- 只取消 worker：线程可能正在执行网络请求，且取消整个共享 worker 会影响语音统计等独立任务。
- 只在上传失败后补救：音频和识别文本已经离开设备，无法撤回。

## Consequences

隐私重启、输入框切换和服务销毁都会使尚未开始的贡献任务失效；已经开始的网络请求仍由上传客户端自身完成，代次检查不伪装成可撤回的网络传输。

## Verification

- `python3 scripts/test-android-voice-contribution-privacy-generation.py`：旧实现先失败，修复后通过。
- `bash platforms/android/check-host.sh`
- `git diff --check`
