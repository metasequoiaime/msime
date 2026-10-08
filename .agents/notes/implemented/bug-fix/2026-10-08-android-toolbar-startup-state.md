# Agent Note: Android 工具栏在新输入框里先按出厂默认画

Status: implemented

## Problem

#5680：冷启动一个应用、点进输入框，键盘工具栏先只有六格（没有剪贴板），皮肤和输入方式两个按钮是灰的，约一秒后剪贴板按钮补上、其余按钮右移、两个灰按钮恢复。录屏逐帧看得到这一跳。

原因有两处，都在 `MSIMEInputService` 每换一个输入框就重建会话的那条路径上：

- `onStartInput` 先 `stop()`（`preferencesSnapshot = null`），再用 `runtime-options.json` 里的偏好调 `applyEditorPreferences(…, false)`。那份偏好是首次安装时写下的出厂默认，`touch_toolbar.clipboard` 是 Rust 默认的 `false`，于是剪贴板按钮先被藏起来。皮肤早就为同样的原因不认这份副本（只认启动缓存和真正读到的偏好），工具栏开关漏了。
- 皮肤、输入方式按钮的可用条件是 `session != 0 && preferencesSnapshot != null`。会话建好后要等 `preferencesReloader` 在单线程的 `preferencesWorker` 上再读一遍偏好、回主线程应用，才有 `preferencesSnapshot`；可建会话前 `withLivePreferences` 已经在同一个线程上读过一遍实时偏好了，只是读完就丢了原始响应。

## Decision

- `withLivePreferences` 返回 `EngineStartOptions(options, livePreferences)`，把 `loadPreferences` 的原始响应一起交回主线程。`startEngineSession` 建好会话、设好 `preferencesDirectory` 后立刻 `applyPreferencesSnapshot(value(livePreferences))`，再 `render()`，然后才启动 `preferencesReloader`。应用失败只吞掉 `JSONException | LinkageError`，会话照常，reloader 随后再读并负责报告。
- 皮肤片段文件 `keyboard-skin-hint.json` 的键加上 `touch_toolbar`。`rememberSkinHint` 和 `readSkinHint` 把它记进 `rememberedToolbar`；`applyEditorPreferences` 在 `appearance == false`（运行时副本）时用 `rememberedToolbar` 决定工具栏按钮，没有记录时才退回副本。`applyToolbarPreferences` 多了一个显式传 `toolbar` 的重载。
- `check-host.sh` 守住这三处：`startEngineSession` 里应用 `livePreferences`、片段键含 `touch_toolbar`、`appearance || rememberedToolbar == null` 这条分支。

## Alternatives considered

- **皮肤、输入方式按钮不再因 `preferencesSnapshot == null` 变灰，点按在就绪前静默忽略** — 改动最小，图标永远不灰；但按钮看着能点、点了没反应，等于把「设置加载中」藏起来，而且剪贴板按钮缺一格的问题照旧。
- **让 `preferencesReloader` 的第一次读取提前或缩短间隔** — reloader 已经是会话建好就立刻读；慢的是单线程 worker 上排着的其他任务和第二次 JNI 读盘，提前不了多少，而建会话前那次读到的同一份数据本来就在手里。
- **启动时把 `runtime-options.json` 的偏好刷成最新** — `Bootstrap.prepare` 只在首次安装写这份文件，改成每次保存偏好都同步会牵扯设置页和宿主的写入时序，范围远大于这个问题。

## Consequences

- **收益**：会话建好的那一帧就有 `preferencesSnapshot`，皮肤、输入方式按钮与表情、常用语同时可用；工具栏按钮的显隐从第一帧起就是用户上次的设置，不再挪位。
- **代价**：会话启动时多一次主线程上的 `updatePreferences` JNI 调用（随后 reloader 读到同一 revision，再调一次，Rust 侧对相同 revision 的相同快照直接接受）。全新安装、还没读到过任何实时偏好时，第一次仍按出厂默认画工具栏。
- **未覆盖**：会话建好之前（`session == 0`）所有依赖会话的按钮仍是灰的；录屏里键盘露出时会话已经建好，这一段没有被观察到。设备上的效果没有在真机上复现验证，只有 `check-host.sh` 的契约守卫和编译。
