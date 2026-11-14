# Agent Note: HarmonyOS 14/18 键宽窄字母键布局

Status: implemented

## Problem

Issue #11051 已在 Android 和 iOS 提供 14/18 键的「等宽 / 宽窄」选择，HarmonyOS 仍按旧的每排铺满布局绘制，也没有读取 `touch_fourteen_key_letter_widths`。同一账号在鸿蒙上因此无法使用已选择的几何，第二排的宽窄空白也会成为不可点击的死区。

## Decision

HarmonyOS 读取共享偏好 `touch_fourteen_key_letter_widths`，缺字段和未知值按既有的 `equal`。新增 `FourteenKeyLetterWidths.ts` 作为键盘侧几何策略：`equal` 保持现有份额，`varied` 使用单字母键 0.75 份、双字母键 1 份，三排共用首排单位宽度。较短的第一、二排两端加入透明占位格，占位格分别把触摸转发给同排最外侧字母键；第三排让分词和删除键分掉两端剩余宽度。双字母键绘制使用 0.15em 字距，双拼 `L;` 使用 0.08em。

账号同步把该共享偏好加入 `TouchHostKeys`，Android 与鸿蒙分别使用 `platform.android.fourteen_key_letter_widths` 和 `platform.harmony.fourteen_key_letter_widths`，避免两台设备互相覆盖；设置预览与键盘使用同一份宽度规则和两枚胶囊（「等宽」「宽窄」）。

`@Builder` 中只保留组件调用，按键份额作为参数交给普通的 `fourteenKeyOptions` / `eighteenKeyOptions` 方法构造。ArkTS 的真实 HAP 编译拒绝在 Builder 分支内声明 `KeyOptions` 变量和赋值；Node 几何测试与 TypeScript 子集检查不能替代这项编译。方法的默认份额仍为 1，长按键外层的份额规则不变。

设置预览按键面中的字母数计算份额，双拼的 `L;` 仍只有一个字母，取 0.75 份；兼带的分号不把它变成双字母键。预览与原生宽度策略的逐排行为对照同时覆盖全拼、双拼的 14/18 键及两种宽度模式，防止预览与实际几何分叉。

## Alternatives considered

- **只在 `KeyboardView.ets` 中为第二排加固定空白。** 这样可快速改变视觉，但无法复用到设置预览，也无法给第一、三排计算同一单位宽度，容易重新产生死区，因此采用独立几何策略并让占位格拥有按键动作。
- **把偏好放进 Android 命名空间让鸿蒙直接读取。** 这样会让鸿蒙读取另一台设备的宿主设置，违反触屏宿主按平台隔离命名空间的同步约定，因此增加鸿蒙自己的同步键。

## Consequences

键盘和设置预览的三排宽度会随偏好即时重排，组字状态不被打断；双字母键面更容易辨认，第二排空白仍可输入。共享偏好字段和设置控件增加了跨文件维护面。DevEco 工具链已可用于完整 HAP 构建，当前仍没有可接受签名的设备。设备触摸转发、字距、真实设置页验收和前后截图尚缺，PR 保持 draft，构建与设备证据分别列在验证段。
