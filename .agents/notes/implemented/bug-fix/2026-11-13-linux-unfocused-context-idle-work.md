# Agent Note: Fcitx5 下失焦的输入框不再每拍跑整套刷新

Status: implemented

## Problem

Linux 上装 msime 的用户反馈：在微信里输入时，从别的窗口切回微信，输入法就“打不了字”，直到输入法服务被重启。同一台机器上换用 fcitx5 自带的拼音没有这个问题。现场测到 fcitx5 进程持续占 140%–156% CPU（用户的 `fcitx5-cpu-guard` 一天重启它 21 次），此时引擎侧的线程（`msime-keyboard-`、`msime-personal-`、`msime-dictionar`、ONNX 重排）全部在 futex 睡眠，候选重排日志也在预算内（`page 2.6 ms, limit 15.0 ms`），所以不是输入算法慢。

抓到的现场（官方 fcitx5 5.1.23、niri + XWayland、msime 0.16.1）：

- 主线程以每秒约 28 万次读同一个 timerfd，`timerfd_settime` 的 `value` 每次都是“当前 CLOCK_MONOTONIC”、`interval = 0`，调用栈是 `sd_event_prepare → sd_event_run → fcitx::EventLoop::exec`；另一条线程（`libxcb.so` 的 `XCBEventReader`）同时在 `sd_event_run` 里空转。
- 空转时 pending 的事件源只有三个：`fcitx::Instance` 构造时注册的 defer，以及 `libwayland.so`、`libxcb.so` 两个前端注册的 post——没有时间源。
- gdb 采样命中分发中的一帧：`pthread_mutex_lock ← xcb_flush ← libxcb.so（fcitx5 的 xcb 模块）← EventLoop 源分发`。

也就是说：主循环被 fcitx5 xcb 模块的一个 post 源以最快速度反复调度（每次调 `xcb_flush()`，并和 `XCBEventReader` 线程抢同一把连接锁），键盘事件排不进主循环。循环本身在 fcitx5 里；换第三方 fcitx5 分支与官方版都一样，把候选窗退回 classicui 也一样。这部分已经单独整理给上游。

msime 这一侧能确认并收窄的是它给这个循环提供的燃料：**Fcitx5 会给每个进过的输入框留一份 `FcitxState`，每份都有自己的 250 ms 定时器**，而这个定时器每拍无条件跑十项刷新（偏好、provider 套接字、在线候选、翻译、剪贴板、云剪贴板、表情、语音、按键统计）。失焦时 `close()` 已经把会话放掉、按键统计写完、浮层收起，这些刷新对没有会话的上下文什么也做不了，却照样每拍经宿主 ABI 读一次偏好与 provider 套接字（各是一次 JSON 往返），并在有变化时把 UI 更新推给前端。用一天下来点过的窗口越多，这条后台成本越高，正好对应“切几个窗口之后才出现”的触发条件。

## Decision

这一拍在没有焦点、没有会话、也没有未完成的偏好任务时直接跳过，只排下一次唤醒：

```cpp
const bool idle = !ic_.hasFocus() && !session_ && !preferences_job_.valid() &&
                  !preferences_save_job_.valid();
if (idle) { timer->setNextInterval(250000); timer->setOneShot(); return true; }
```

两个 job 必须一起看：进行中的保存（`preferences_save_job_`）与读取（`preferences_job_`）都需要由 `refreshPreferences()` 收取结果。保存失败后的 `preferences_save_retry_` 另由已有菜单重试入口处理，不在这一拍自动重试。

## Alternatives considered

- **只按焦点跳过**（`!ic_.hasFocus()` 就跳过）：最简单，但无法保证收取仍在进行的偏好任务结果；因此保留两个 job 的判据。
- **把每份 `FcitxState` 的定时器合并成进程级一个**：能再省掉一批定时器与空转唤醒。没有采用：每个上下文的刷新有各自的保存/读取任务与 provider 套接字状态，合并要把这些状态全部搬到 engine 上，属于另一件事，收益远小于本次的判据。
- **在 msime 侧检测到空转就自救**（例如统计每拍的唤醒间隔，超过阈值就重启 addon）：能绕开上游 bug，但输入法进程里做“自我重启”会把用户的组字状态一起丢掉，而且判据容易误伤正常的打字高峰，不采用。
- **什么都不改，只报上游**：上游修复的周期不可控，而这批刷新对失焦上下文本来就是纯浪费，先收掉。

## Consequences

- 失焦的输入框不再产生后台的宿主 ABI 往返与 UI 更新；后台成本不再随“今天点过多少个窗口”线性增长。
- 焦点回到某个上下文时，`activate` 会重建会话并按当前偏好重新同步面板，跳过的这几拍不影响它看到最新偏好——`refreshPreferences` 的第一拍本来就要等 `preferences_job_` 就绪才会应用。
- 按键统计不受影响：失焦时 `releaseFocusResources()` 已经把 `key_presses_` 写完，跳过的拍里没有待写的按键。
- 这不是上游那个循环的修复。它只减少 msime 喂进去的事件量；fcitx5 xcb 模块在收不到输入时仍可能自己转起来，见上游问题里的触发条件与栈。

## Testing

- `cmake -S platforms/linux -B target/linux-native -G Ninja -DBUILD_TESTING=ON -DMSIME_ENABLE_FCITX5=ON` 配置通过，`cmake --build` 编出 fcitx5 插件与全部原生目标，`ctest` 为 120/121 通过；唯一失败为 `linux-candidate-native-canvas-chrome` 的像素颜色断言。去掉本次改动后该目标无需重编且仍失败，因此作为本机既有渲染差异记录，不加入已知失败豁免表；平台 CI 的 Linux 原生作业均通过。
- 手动：装包后连续切换 5 个以上窗口再回到聊天窗口，对比改动前后 fcitx5 的 CPU 与 `syscr`/`syscw` 速率；空转是否复发另记（受上游影响，不作为本次验收条件）。
