# Agent Note: Android 遥测观察夹具暂时缺少状态文件

Status: implemented

## Problem

API 35 的 `android-device` 验收在真实 `:ime` 进程仍存活时读不到合成夹具的 `status.json`，把一次状态文件替换窗口直接判成夹具不可读。API 35 的文件系统和 `adb shell cat` 时序更容易暴露这段窗口，导致 develop 的 Native Platform CI 变红；这不是输入法产品进程重启证据。

## Decision

`telemetry-watch.py` 的 `read_status` 对“文件不存在”与读空、解析失败、adb 失败采用同一套有限重试，重试耗尽才返回空文档并由现有 `process_mismatch` 报告。启动阶段仍由 `start()` 的轮询等待首份状态，运行期间的短暂替换窗口不再被当成进程重启。

## Alternatives considered

- 在夹具中永久保留上一份状态：未采用，会掩盖观察器已经停止写状态的真实故障。
- 放宽 `process_mismatch`，只看 `pidof`：未采用，会失去夹具线程异常的检测能力。
- 修改产品遥测实现：未采用，失败证据只指向测试夹具读取时序，不支持改变产品行为。

## Verification

- `python3 -m py_compile platforms/android/tests/device/telemetry-watch.py`
- API 35 重跑由 #11079 的现有 workflow 提供；修复 PR 仍需 API 28/API 35 设备 job 终态验证。

## Consequences

启动首份状态最多多等几次 100 ms 读取，运行中短暂的原子替换窗口不会误报；持续缺失仍会失败并保留原始 adb 输出，便于区分夹具故障与产品进程变化。
