# Agent Note: Android 输入失败分类冒烟补建服务协作对象

Status: implemented

## Problem

`android-device.yml` 在两个 API 上都在同一阶段失败：

```
MSIME_DEVICE_SMOKE_FAILED: disconnected edit panel (InvocationTargetException)
```

`InputFailureCategoryDeviceSmoke` 遍历 `TextEditPanelModel.Action` 的所有取值（跳过 `PASTE`）调用面板的 `perform`，用断开的编辑器验证「任何动作都不该把异常抛给用户」。`abb0d27e9`（#8496）给这个枚举加了 `NUMBER_PAD`、`SPACE`、`RETURN`，其中 `NUMBER_PAD` 会离开面板走到服务：

```
ImeTextEditPanel.perform(NUMBER_PAD) → MSIMEInputService.showNumberKeyboardFromTextEdit()
                                     → imeTextEditPanel.close()   // 夹具里是 null
                                     → render()                   // 一上来就用 imeBottomBar、imeFrame
```

夹具原先只反射造了 `imeLetterRows`（因为 `apply()` 的 `imeLetterRows.drawnTouchLayout()` 需要它），而 `imeTextEditPanel`、`imeCalculator` 和 `render()` 用到的一整套协作对象都由 `MSIMEInputService.onCreate` 创建（`MSIMEInputService.java:1528` 起），`onCreateInputView` 再建键区。夹具用无参构造造服务、不调用这两步，所以在这一步空指针。

`apply()` 之所以早先没暴露同样的问题，是因为夹具的编辑器代理对 `setComposingText` 返回 false，`apply()` 在 `render()` 之前就抛出 `EDITOR_REJECTED`（`MSIMEInputService.java:2845` 早于 `:2855`），`render()` 从没在这个夹具里跑过。

## Decision

夹具改用其它设备冒烟已经在用的建法：`attachBaseContext(getTargetContext())` → `onCreate()` → `onCreateInputView()`，然后删掉手工反射构造 `imeLetterRows` 的那段。测试要断开的编辑器和空 `view` 仍由夹具自己设置，各阶段的断言不变。

代价是这个夹具现在会和真实服务一样读本机设置、建键区，不再是一具只有单个字段的骨架。这是刻意的：`NUMBER_PAD` 这类动作的语义本来就是「离开面板去动键盘」，用骨架验证它在动作之间是否安全，只能验证骨架本身没造出来的东西。

## Alternatives considered

- **只补 `imeTextEditPanel` 和 `imeCalculator`**：最小，但 `perform(NUMBER_PAD)` 接着就会走 `render()`，而 `render()` 从 `imeBottomBar.apply(imeFrame.keyboard)`（`MSIMEInputService.java:9292`）开始逐级使用键区，补两个字段只会把空指针挪到下一行。要补齐就得逐个复刻 `onCreate` 的二十来个对象，比直接调用它更容易随产品漂移。
- **给 `showNumberKeyboardFromTextEdit` 加空值保护**：产品路径上这个字段不可能为空（`onCreate` 必先于任何面板动作），保护只为让骨架夹具过关，属于让产品给测试让步。
- **夹具不再遍历导航类动作**：会丢掉 `NUMBER_PAD`、`SPACE`、`RETURN` 这三个新动作的覆盖，而它们正是这次要覆盖的对象。

## Consequences

- **收益**：设备套件在两个 API 上恢复；`NUMBER_PAD`、`SPACE`、`RETURN` 继续被「断开编辑器时不得抛异常」这条断言覆盖。
- **代价**：夹具从骨架变成近似真实的键盘，跑得比原来慢，也依赖本机设置与键区能在一个 instrumentation 里建起来——这两点其它四个设备冒烟（`ToolbarEdgesDeviceSmoke`、`BottomPaddingHeightDeviceSmoke`、`CandidateExpansionDeviceSmoke`、`StaleCandidateDeviceSmoke`）已经在用，风险与它们相同。
- **影响面**：设备任务恢复之前，`Release Android` 的 `device → publish` 依赖链是断的，Android 发不了版。
- **验证**：本机是 Windows，没有 Android SDK，只能靠 `ci-platforms.yml` 手动 dispatch 在 API 28 和 35 上跑 `android-device` 验收。
