# Agent Note: 键盘里调的高度被设置页的值盖回去

Status: implemented

## Problem

用户反馈：设置页里能调键盘高度，但在键盘里调（拖动把手或 ± 按钮）怎么调都会变回设置里的数值。三端都有这个入口，原因各不相同，而且每端都不止一处。

## Decision

键盘里调出来的高度一旦被用户看见，就存进它的权威存储；只有用户明确取消才丢掉。每端各自修掉把旧值写回或读回的路径：

- **HarmonyOS**（确定的 bug）：`KeyboardSession.changePreferences` 存好文档、更新了版本号，却不刷新缓存的偏好和外观；`preferences()` 返回的还是准备会话时那一份。`applyGeometry` 存完立刻把旧高度读回来，所以拖动弹回原位、± 不累加，调间距时还会把旧高度写回文档；下次聚焦时版本号已经一致，不会重建，旧值就一直留着。现在存好之后用 `refreshPreparedPreferences` 按存下的整份文档换掉缓存并重算外观；`selectGlobalTheme` 原来自己补这一步，并进了同一处。
- **Android**：
  - 调整中键盘被收起时，原来按取消处理（`onFinishInputView` 调 `finishInlineHeight(false)`），现在按完成保存，只有点「取消」才丢掉。
  - 高度原来只在共享偏好（间距、语音入口）写入成功后才写本地设置，共享写入失败或冲突时高度也跟着丢；现在由 `settings/KeyboardHeightSave` 先写本地设置，与共享写入无关，只改了高度时根本不写共享偏好。这推翻了原来代码注释里的「本地高度只在偏好写入成功后再写」。
  - 调整中途发生偏好重载时，原来会用存储里的值盖掉预览，点完成就什么也没存；现在保留预览值。
- **iOS**：
  - 设置页（`KeyboardLayoutSettingsView`）只在 `.onAppear` 读一次，而 `save()` 每次写全部四个几何字段：键盘里改过高度之后，在设置页动任何别的控件都会把旧高度写回去。现在回到前台时重新读取，并且只写改动的那个字段（`geometryMapping`、`saveGeometry` 只写给出的字段）。
  - 组字中会话推迟偏好更新，原来的 `updateAndPersist` 在这种情况下连文档也不写；现在 `persistTouchKeyboardGeometry` 总是写文档并返回是否成功，失败时记 `touch_geometry_not_persisted`，并把 App Group 和键盘按文档对齐。
  - `synchronizeSharedTouchPreferences` 原来只把间距和语音入口镜像进 App Group，没有高度，键盘里的调节器打开时用的是 App Group 里的旧值；现在用 `mirrorGeometry` 把高度也镜像过去。

## Alternatives considered

- **键盘里调的高度只存在键盘本机、不写共享文档，设置页另存一份**：两边各写各的就不会互相覆盖。但设置页显示的就不是键盘实际用的高度了，用户在设置页看到的数和键盘对不上，账号同步也只能同步其中一份。
- **Android 保持「收起即取消」，只修共享写入的耦合**：改动最小，也符合「完成才算数」的严格语义。但用户报的正是「怎么调都会变回去」：调到满意直接收起键盘是最自然的结束方式，要求他记得先点完成，等于继续让高度回弹。

## Consequences

- **收益**：三端键盘里调的高度都会留下，设置页再改别的项也不会把它写回旧值。
- **代价**：Android 在键盘收起时就保存，误拖之后收起也会保存，要撤销只能再调回去或在设置页重置。iOS 设置页在前台切换时多读一次文档。
- **未验证**：都没有在真机上手动走过一遍。Android 的 `KeyboardHeightDeviceSmoke` 新增了「不点完成就收起」的用例，但设备测试套件在 develop 上本身就编不过，没有跑。

## Verification

- HarmonyOS：`bash platforms/harmony/tests/run.sh`；这部分没有能单独抽出来测的纯逻辑。
- Android：`bash platforms/android/check-host.sh`（`KeyboardHeightSaveSmoke`）。
- iOS：`KeyboardGeometrySettingsTests` 覆盖键盘存的高度在设置页另存一项、重新加载、组字中保存之后仍在，以及镜像带上高度。
