# Agent Note: 手机键盘的输入方式面板里双拼只留一种

Status: implemented

## Problem

手机键盘工具栏的「输入方式」面板（Android `ImePanels.renderSchemePicker`、iOS `KeyboardSchemePickerView`、鸿蒙 `KeyboardView.schemePickerEntries`）把小鹤、自然码、微软、首道四种双拼各列一张卡片。新装默认启用的方案（`TouchKeyboardScheme::DEFAULT_ENABLED`）里四种双拼都在；Android 的面板更是列出全部已安装方案，不看启用列表。手机每页 4 × 2，英文固定占第三格，四张双拼卡片把手写挤到了第二页（#6450）。绝大多数人只用一种双拼。

## Decision

- 面板里的双拼只留一种，其余方案原样、按原顺序保留。留下的那一种依次取：当前选中的方案本身是双拼时就是它；否则是共享偏好 `shuangpin_profile` 对应的那一种（缺省或不认识的值按小鹤，与 client-core 和各宿主 `mapping` 的规整相同）；它不在列表里时取列表里第一种双拼，保证用户只启用了别的双拼时面板上仍有双拼入口。
- 三个宿主各有一份同名纯函数：Android `KeyboardScheme.pickerSchemes`、iOS `InputSchemePreference.pickerSchemes`、鸿蒙 `KeyboardScheme.pickerSchemes`，注释互相指向。只用在面板的列表上：Android 先按全部已安装方案解析选中项，再把过滤结果存进只给面板用的 `visibleSchemes`；iOS 在 `KeyboardSchemePickerView` 里过滤 `offeredSchemes`，`shuangpin_profile` 由 `KeyboardViewController` 从共享文档读出传入；鸿蒙在 `schemePickerEntries` 里过滤，`shuangpin_profile` 取 `settings.shuangpinProfile`。
- 启用列表、选中项的解析、存进文档的内容都不变。换双拼方案走设置里已有的「双拼」子菜单（Android `TypingPage`、iOS 引导页与输入页、共享 `LanguageCard`），选中时写 `shuangpin_profile`，面板随之换成新的那一种。
- 行为对所有用户直接生效，不加开关。

## Alternatives considered

- **把过滤写进共享层 client-core，经 host-api 导出**：一处实现、天然一致。但三个面板的方案列表本来就由各宿主在原生代码里算（`offeredBy`、`installedOf`、`withInstalledDictionaries` 都是各宿主镜像 client-core 的同名规则），面板渲染不经过 host-api；为一个十几行的列表过滤加一个 C ABI 入口，再分别接 JNI、Objective-C++ 和 NAPI，改动面远大于收益。沿用现有做法：各宿主一份、注释互指、各自有测试。
- **把 `TouchKeyboardScheme::DEFAULT_ENABLED` 改成只启用小鹤**：只影响新装，已有用户的面板不变；Android 的面板也不看启用列表，改了等于没改。而且用户实际设置的是别的双拼时，默认启用小鹤反而更不对。
- **加一个「面板只显示一种双拼」开关，默认关**：保留现有行为最稳，但 issue 的诉求正是默认情况下的冗余，默认关就解决不了问题；需要多种双拼来回切的人仍可在设置的子菜单里切，代价是多点两下。是否要补一个开关留给维护者决定。
- **把四种双拼合成一张「双拼」卡片，点开再选**：面板目前是一层网格，没有二级菜单；为它加一层交互超出这个 issue 的范围。

## Consequences

- **收益**：面板少三张卡片。手写在 Android 上从第 9 格回到第 6 格（装了日文词典时是第 8 格），在 iOS 和鸿蒙默认启用的方案下也都回到第一页；用户设置了哪种双拼就显示哪种。
- **代价**：同时用两种以上双拼的人不能再直接在面板里切换，要去设置的「双拼」子菜单。
- **不变量**：当前选中的双拼一定在面板里（否则选中标记会丢）；只要列表里有双拼，面板上就至少有一种。

## Verification

- Android：`KeyboardSchemeSmoke.pickerSchemes`（`platforms/android/check-host.sh`）。
- iOS：`NineKeyKeyboardTests.testSchemePickerListsOnlyTheConfiguredShuangpin`，`testSchemeCardsSelectAndKeepKeyboardHeight` 改为按每种双拼只出一张卡片检查。
- 鸿蒙：`platforms/harmony/tests/keyboard-logic.test.ts` 的「输入方式面板里双拼只留用户设置的那一种」组（`platforms/harmony/tests/run.sh`）。
