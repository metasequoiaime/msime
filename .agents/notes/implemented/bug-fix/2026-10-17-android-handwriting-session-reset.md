# Agent Note: Android 触屏手写延迟任务随输入会话失效

Status: implemented

## Problem

Android 输入法服务在切换编辑器或收起并重建键盘视图时会销毁服务侧的手写识别器，但 `ImeLayoutRows` 仍保留停笔后的延迟识别、已上屏提示定时器和后台拼音查询的代次。旧查询回到主线程后可能看到新建的手写画布与相同的笔迹修订号，把旧编辑器的提示写到新输入框的手写面板。

## Decision

`ImeLayoutRows` 提供输入会话边界 `resetForInput()`：取消停笔识别和已上屏提示，递增拼音查询代次。`MSIMEInputService.deactivateHandwriting()` 在销毁服务侧手写状态前调用它；服务销毁时的线程关闭也复用同一清理路径。

## Alternatives considered

- **只依赖 `HandwritingCanvas` 的修订号**：修订号能区分同一画布里的新笔迹，但新输入会话会创建新画布，修订号可能重新从相同的值开始，无法表达编辑器身份，因此不采用。
- **只在 `onFinishInput()` 清理**：可以覆盖切换编辑器，但键盘视图单独收起、重建或 `onCreateInputView()` 重新建面板时仍会留下旧定时器；把清理放在已有的手写停用边界能覆盖两条路径。
- **每个后台回调都读取服务的编辑器代次**：能逐个加守卫，但会把服务生命周期契约复制到面板内部，且停笔与提示定时器仍需另外清理；统一的面板输入边界更小且可复用。

## Consequences

旧输入会话的手写识别、提示和拼音结果不会再写回新会话；当前会话内的手写行为和识别延迟不变。切换输入框时会主动取消尚未触发的提示和拼音查询，后台线程仍在关闭时由既有 `shutdownNow()` 回收。

## Verification

先运行 `platforms/android/check-host.sh`，新增的 `HandwritingLifecycleSmoke` 在缺少清理契约时按预期失败；加入清理后同一检查通过，包含 243 项 Android JVM 冒烟测试。Android 官方 `InputMethodService` 文档确认输入目标切换会调用 `onFinishInput()` 与 `onStartInput()`；真实设备上的手写模型识别仍需 Android 14 手写笔环境，未在本次变更中声称完成该硬件验收。
