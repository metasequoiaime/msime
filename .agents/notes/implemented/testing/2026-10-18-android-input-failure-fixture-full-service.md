# Agent Note: Android 输入失败分类冒烟在主线程上建真实服务

Status: implemented

## Problem

`android-device.yml` 在 API 28 和 35 上都在同一阶段失败，develop 的 `241923435` 同样复现，与本套件无关的改动也挡不住：

```
MSIME_DEVICE_SMOKE_FAILED: disconnected edit panel (InvocationTargetException)
```

`InputFailureCategoryDeviceSmoke` 遍历 `TextEditPanelModel.Action` 的所有取值（跳过 `PASTE`）调用面板的 `perform`，验证「编辑器断开时任何动作都不该把异常抛给用户」。`abb0d27e9`（#8496）新增的 `NUMBER_PAD` 会离开面板走到服务：

```
ImeTextEditPanel.perform(NUMBER_PAD) → MSIMEInputService.showNumberKeyboardFromTextEdit()
                                     → imeTextEditPanel.close()   // 夹具里是 null
                                     → render()                   // 一上来就用 imeBottomBar、imeFrame
```

两处叠加：

- 夹具用无参构造造服务，不调用 `onCreate` / `onCreateInputView`，所以 `imeTextEditPanel`、`imeCalculator` 和 `render()` 用到的一整套协作对象与键区都是 null（它们只在 `MSIMEInputService.java:1528` 起和 `onCreateInputView` 里创建）。夹具原先只反射造了 `imeLetterRows`，因为 `apply()` 的 `imeLetterRows.drawnTouchLayout()` 需要它；`apply()` 之所以没更早暴露问题，是夹具的编辑器代理对 `setComposingText` 返回 false，`apply()` 在 `render()` 之前就抛出 `EDITOR_REJECTED`（`MSIMEInputService.java:2845` 早于 `:2855`），`render()` 从没在这个夹具里跑过。
- 只有「造服务」那一步包在 `runOnMainSync` 里，其余断言都在 instrumentation 线程上。这次 `perform` 之后加根因链打出来的真实异常是：

```
android.util.AndroidRuntimeException: Animators may only be run on Looper threads
  at android.animation.StateListAnimator.setState
  at android.view.View.drawableStateChanged
```

`render()` 会改真实按钮的 selected / activated，那会驱动主题里的 `StateListAnimator`。

## Decision

- 夹具改用其它设备冒烟已经在用的建法：`attachBaseContext(getTargetContext())` → `onCreate()` → `onCreateInputView()`，删掉手工反射构造 `imeLetterRows` 的那段。
- 整个测试体放进一次 `runOnMainSync`，与 `ToolbarEdgesDeviceSmoke` 相同；为了让 diff 只有增量，阶段本身抽成 `runStages()`，原来的缩进不动。嵌套 `runOnMainSync` 会死锁，所以只剩外层一次。
- 失败信息补上 `causeChain`，顺着 `getCause()` 打出每层的类型、消息和前六帧。反射调用抛的是 `InvocationTargetException`，只报包装类型定位不到真正的抛出点——这次就是靠它才看到 `StateListAnimator`。
- 夹具的 `finally` 在同一条主线程上调用服务的 `onDestroy()`，让完整服务注销剪贴板监听、取消主线程回调并结束 Telemetry 会话；随后保留反射兜底关闭执行器，避免 API 31+ 的 `--no-restart` 连续套件把旧服务留在目标进程里。

夹具自己设置的断开连接和空 `view`、以及各阶段断言都不变。

## Alternatives considered

- **只补 `imeTextEditPanel` 和 `imeCalculator`**：`perform(NUMBER_PAD)` 接着就会走 `render()`，而它从 `imeBottomBar.apply(imeFrame.keyboard)`（`MSIMEInputService.java:9292`）开始逐级使用键区，补两个字段只是把空指针挪到下一行；要补齐就得逐个复刻 `onCreate` 的二十来个对象，比直接调用它更容易随产品漂移。
- **给 `showNumberKeyboardFromTextEdit` 加空值保护**：产品路径上这个字段不可能为空（`onCreate` 必先于任何面板动作），保护只为让骨架夹具过关，属于让产品给测试让步。
- **夹具不再遍历导航类动作**：会丢掉 `NUMBER_PAD`、`SPACE`、`RETURN` 这三个新动作的覆盖，而它们正是这次要覆盖的对象。
- **只改线程、不建真实服务**：`imeTextEditPanel` 仍是 null，`close()` 直接空指针。
- **只关闭执行器、不调用 `onDestroy()`**：可以停掉工作线程，却会留下剪贴板监听、主线程回调和 Telemetry 会话；API 31+ 的连续 instrumentation 会把这些状态带进后续用例。

## Consequences

- **收益**：本套件在 API 28 和 35 上都恢复（`MSIME_DEVICE_SMOKE_PASSED: installed response and editor failure categories, private exception exclusion`），验收得以继续往下跑；`NUMBER_PAD`、`SPACE`、`RETURN` 继续被「断开编辑器时不得抛异常」这条断言覆盖；以后再出现包装异常，日志直接给出根因链。
- **仍然红的部分**：修好本套件后，验收又停在后面的两个独立失败上——API 35 是 `CandidateGlossDeviceSmoke` 的 `expanded candidate offline gloss`，API 28 是词典导入。两者都不经过本套件，也不经过本 PR 改动的代码，需要各自再修。`device` 是硬门禁，所以在这三个都修好之前 Android 仍然发不了版。
- **代价**：夹具从骨架变成近似真实的键盘，跑得比原来慢，也依赖本机设置与键区能在一个 instrumentation 里建起来——这两点其它四个设备冒烟（`ToolbarEdgesDeviceSmoke`、`BottomPaddingHeightDeviceSmoke`、`CandidateExpansionDeviceSmoke`、`StaleCandidateDeviceSmoke`）已经在用，风险与它们相同。因为必须在主线程上跑，整套阶段期间会占用输入法进程的主线程。
- **清理**：完整服务现在走 `onDestroy()`，所以会注销监听、取消回调并结束 Telemetry；反射关闭执行器只是服务生命周期异常时的兜底。
- **影响面**：设备任务恢复之前，`Release Android` 的 `device → publish` 依赖链是断的，Android 发不了版。
- **验证**：本机是 Windows，没有 Android SDK，只能靠 `ci-platforms.yml` 手动 dispatch 在 API 28 和 35 上跑 `android-device` 验收。
