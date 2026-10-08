# Agent Note: 鸿蒙键盘设计皮肤的功能键与回车键配色对齐 Android 和 iOS

Status: implemented

## Problem

触屏键盘新装的默认皮肤是薄荷晨光（`TouchKeyboardSkinDesign::mint_morning`），字母键白底深绿字 `#173D30`，动作色是深绿 `#245A43`。鸿蒙键盘把这套皮肤画成了：

- 功能键（⇧、⌫、123、中/英）填动作色深绿，文字仍用字母键的深绿，几乎看不见。
- 回车键用应用的季节强调色（10 月是秋杉橙 `#B5562B`），绿键盘上出现一个橙红色的键。

原因在 `KeyboardSkin.ts`：`fromDesign` 把 `function_key` 设成设计的 `actionBackground`，而功能键文字一律取 `palette.text`；构造函数里回车键只看季节种子和平台强调色，不看设计。两处都来自 #5792 的改版，没有对应的决策记录，而且与那次改版笔记自己定的两条规则冲突：「设计稿与 Android 不一致时跟 Android 走」，以及「季节主题只用于应用强调色、`system` 键盘和品牌标」。

## Decision

照 Android（`KeyboardSkin.functionBackground` / `returnBackground`）和 iOS（`KeyboardTheme.functionKeyBackground`、`SkinKeySurfaceView`）画用户的键盘设计：

- 功能键画在字母键的底色上，用字母键的文字色。
- 回车键用设计自己的 `actionBackground`，文字按亮度取黑或白（`CustomKeyboardSkin.actionForeground`）。
- 季节仍然给开启态图块和品牌标着色，这两处 Android 也跟季节走；内置主题和 `system` 键盘的配色不变。

## Alternatives considered

- **功能键保留动作色，只把文字改成按亮度取黑白。** 最强的理由是改动最小，`fromDesign` 注释里说这是共享 `custom_keyboard` 的展平方式。不用它，是因为 Android 和 iOS 都已经刻意不这么画（Android 源码注释：用动作色铺满一打功能键会让社区皮肤像拼布），三个触屏宿主的同一套皮肤应该长得一样。
- **回车键继续跟季节走。** 理由是应用和键盘的强调色统一。不用它，是因为季节会改掉用户自己设计的键，绿皮肤配橙回车就是这样来的；Android 和 iOS 都用设计自己的动作色。

## Consequences

- **收益**：默认皮肤的功能键可读；鸿蒙上同一套设计皮肤与 Android、iOS 一致；皮肤选择器缩略图和调高度条里的强调色也随之变成设计自己的动作色，与实际键盘一致。
- **代价**：用自定义设计的用户，键盘回车键不再随季节变色。

## Verification

`platforms/harmony/tests/keyboard-logic.test.ts` 的三组用例（设计皮肤的功能键、季节只染图块不染键、薄荷晨光每个键都可读）在旧代码上失败、在新代码上通过；`bash platforms/harmony/tests/run.sh` 全部通过，`hvigorw assembleHap` 通过。HarmonyOS 6.0.2(22) phone 模拟器上看过：⇧、⌫、123、中是白底深绿字，回车键是深绿底白字。模拟器上的原生库是当天较早从 develop 编的，没有用最新 develop 重编。
