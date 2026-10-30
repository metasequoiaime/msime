# Agent Note: 中/英键可放在空格左边

Status: implemented

## Problem

#10228：用户习惯别家输入法把中/英切换键放在空格左边，水杉三个手机端的中/英键自[手机底行的中/英键挪到回车左边](2026-10-09-mobile-language-key-beside-return.md)之后一直紧挨回车，没有办法挪回左手边。Android 有本机的[「交换语言键与数字键」](2026-10-14-android-language-number-key-swap.md)，但它把中/英换到最左边、123 换到右边，不是「空格左边」；iOS 和鸿蒙什么都没有。

2026-10-29 维护者在真机上核对过两家的做法：

- 豆包输入法设置里有「中英切换键：空格左侧 / 空格右侧」两个选项，选左侧时中/英紧挨在空格左边。
- 百度输入法九键有两种底行：「经典布局」是 `中英 | 123 | 空格 | 换行`，「系统布局」是 `123 | 中英 | 空格`。两种的中/英都在空格左边，其中系统布局与 iOS 系统键盘的次序一样，中/英紧挨空格。

两家能看到的行都是空格左边没有标点的行。水杉的底行样式里有几种空格左边本来就有标点（「逗号句号在空格两侧」、长空格、14/18 键、英文 26 键空格左边的句点、手写），这些行该怎么排，两家没有可照搬的样子。

## Decision

共享偏好（`crates/client-core/src/preferences.rs`）新增 `touch_language_key_side: TouchLanguageKeySide`，取值 `right`（默认，现在的排法）和 `left`。等于默认值时不写进文档，默认文档与加字段之前逐字节相同。读文档宽松：不认识的字符串按 `right` 读，不是字符串照样让整份文档报错，与 `touch_number_pad_space` 相同；另有严格的 `parse` / `as_str` 给同步用。宿主只在值恰好是字符串 `"left"` 时算左，其余一律按右；设置页打开写 `"left"`、关上写 `"right"`，client-core 保存时把 `right` 省掉。

### 排法：一条规则管所有底行

偏好为 `left` 时，在底行样式、[「符」与「123」互换和数字键盘空格位置](2026-10-29-touch-symbol-first-and-number-pad-space.md)都排完之后，对排好的底行再做一步：

> 找到 中/英。它左边紧挨着的、由 空格 / 0 / 逗号 / 句号 组成的一串连续的键整串右移一格，中/英 挪到这一串的最左边。

所有键的宽度份额不变，只换次序。行里没有中/英（iOS 打开了「隐藏中/英键」），或它左边紧挨着的不是这四种键时，原样返回。三端各有一个同名语义的纯函数：Android `KeyboardActionRow.languageBeforeSpace`，iOS `TouchBottomRowLayout.languageBeforeSpace`（另有一个按任意元素排的泛型版本，键盘控制器拿它排按键视图），鸿蒙 `TouchLayoutStylePolicy.languageBeforeSpace`（设计底行）与 `NineKeyLayout.languageBeforeSpace`（九键外框，`actionRow` / `tallReturnRow` 的 `languageLeft` 参数），共享设置页和热力图 `withLanguageKeyLeft`。

结果是：中/英落在左侧那组功能键（123 / 返回、地球键、符、😀）之后。空格左边没有标点时中/英紧挨空格，与豆包「空格左侧」、百度「系统布局」一致；空格左边有标点时标点仍紧挨空格，中/英排在标点左边。几行典型的结果（不带地球键）：

| 行 | 右（默认） | 左 |
| --- | --- | --- |
| 中文 26 键标准 | 123 \| 符 \| 空格 \| ， \| 中 \| ↵ | 123 \| 符 \| 中 \| 空格 \| ， \| ↵ |
| 逗号句号在空格两侧 | 123 \| 符 \| ， \| 空格 \| 。 \| 中 \| ↵ | 123 \| 符 \| 中 \| ， \| 空格 \| 。 \| ↵ |
| 英文 26 键 | 123 \| 符 \| . \| 空格 \| , \| 中 \| ↵ | 123 \| 符 \| 中 \| . \| 空格 \| , \| ↵ |
| 14 / 18 键标准 | 123 \| ， \| 空格 \| 。 \| 中 \| ↵ | 123 \| 中 \| ， \| 空格 \| 。 \| ↵ |
| 九键标准 | 符 \| 123 \| 空格 \| 中 \| ↵ | 符 \| 123 \| 中 \| 空格 \| ↵ |
| 九键数字层（空格在 0 右边） | 符 \| 九键 \| 0 \| 空格 \| 中 \| ↵ | 符 \| 九键 \| 中 \| 0 \| 空格 \| ↵ |
| 右侧大回车网格下方（字母层 / 数字层） | 123 \| 空格 \| 中；切换 \| 0 \| 空格 \| 中 | 123 \| 中 \| 空格；切换 \| 中 \| 0 \| 空格 |
| 手写 | 123 \| ， \| 空格 \| 。 \| 中 \| 重写 \| ⌫ \| ↵ | 123 \| 中 \| ， \| 空格 \| 。 \| 重写 \| ⌫ \| ↵ |

空格左边有标点时让标点贴着空格，是我们自己的决定，理由有三条：

- 「逗号句号在空格两侧」这个样式的定义就是逗号句号贴着空格，中/英插进逗号和空格之间会把样式拆掉。
- 中/英在中文和英文 26 键上停在同一格（都是第三格，表情键样式里 😀 与 符 同宽，同样如此），切中/英时这个键不跳。
- 规则只有一句话，三端键盘、设置页预览和热力图都不需要按样式写特例。

百度的「经典布局」`中英 | 123 | 空格` 在 Android 上由本机「交换语言键与数字键」叠在本偏好上得到，本偏好自己不排这一种。

### 作用范围

作用于所有手机触屏底行：中文 26 键一族（全拼、双拼、五笔、粤拼等）的四种样式；英文 26 键和韩文、越南文、藏文、日文罗马字等共用这条底行的键面（范围与「符」与「123」互换相同，切中/英、切语言时中/英不跳）；14 键、18 键三种样式和共用这一行的大千注音；九键外框（拼音九键、九键双拼、笔画的字母层三种样式，鸿蒙英文九键）；右侧大回车网格下方那一段（字母层和数字层）；九键一族自己的数字层（三种空格位置）以及从 26 / 14 / 18 键和手写借来的数字九宫格；手写底行。

不作用于：123 / #+= 层（自带底行，没有中/英）；iPad 全尺寸 `.tablet` 底行、iPad 大千 `.standard` 底行和鸿蒙 2in1 底行（各自有自己的底行，存着的值不变）；日文假名九键、注音九键和 Android 日文九键（中/英在侧边竖列）；键盘内的皮肤选择器。

[「键盘」页和皮肤页的键盘预览跟随按键布局](../bug-fix/2026-10-29-keyboard-previews-follow-layout.md)（#10245）之后，「键盘」页、皮肤页、AI 皮肤页和社区皮肤的整块键盘预览改按用户的按键布局偏好画，本偏好也跟着进去：Android `KeyLayoutStylesPolicy.Preview` 的 `languageLeft`（`KeyboardPreview.layout` 读偏好，先挪中/英、再做本机「交换语言键与数字键」），iOS `KeyboardPreviewLetterLayer`（九键直接排的那一行同样过一遍 `languageBeforeSpace`），共享设置页 `harmonyStyledRows`（鸿蒙「屏幕键盘」页、皮肤与社区预览）。这些预览不高亮。

密码框和只收数字的框里底行样式退回标准，本偏好不退回：它是位置习惯，与「符」与「123」互换相同。

14/18 键字母层的中/英在第二格（`123 | 中 | ， …`），借来的数字九宫格多一个 符（`返回 | 符 | 中 | 0 …`），中/英在第三格，两层差一格；26 键和九键的字母层与借来的数字九宫格同一格。这一格的差别接受，三端测试照这个结果钉住。

### 连带行为

- 回车让位：中/英不再紧挨回车，回车不再向中/英让出触摸分界。Android 的 `languageBesideReturn`、鸿蒙的 `besideReturn` 都按最终的行计算；iOS 的 `KeyAreaStackView.yieldsToLeft` 本来就要求两键相邻。三端都不用改判断，测试钉住。
- 键的身份不变：中/英的点按、长按（Android 的语言轮换和输入法选择、iOS 和鸿蒙的长按菜单）、读屏标签和按键统计 id 跟着键走；读屏顺序跟着视觉次序。
- 分离键盘的中缝仍在空格里：iOS 的中缝和第二个空格在重排时算作空格，Android `SplitKeyboardPolicySmoke` 的「空隙居中」断言在 `left` 下也跑一遍。
- 底行重建与 `touch_twenty_six_symbol_first` 走同一条路：Android 进 `touchGeometryKey`、`SKIN_HINT_KEYS` 和 `ImeBottomRow` 的底行签名，右侧大回车外框由 `ImeLayoutRows.tallReturnStale()` 记下建框时的值、变了就整块重建；iOS 经 `activeKeyLayoutStyle` 进 `updateKeyboardLayout`；鸿蒙进 `faceKey()`。

### 与本机设置的关系

- iOS「隐藏中/英键」（App Group 本机开关）：隐藏时底行没有中/英，重排是空操作。设置页开关照样显示、照样写文档（同步到别的设备照样生效），预览里没有中/英，所以看不出变化，也不加额外说明（文字要三端逐字相同）。
- Android「交换语言键与数字键」（`platform.android.swap_language_number_keys`，本机）：交换在最后一步做，先把中/英挪到左边，再对调 123（或返回）与中/英，宽度跟着键走。例如 26 键标准 + 左 + 交换是 `中 | 符 | 123 | 空格 | ， | ↵`。右侧大回车的 `tallReturnEntries` 原来写死对调首尾两格，现在改成按键找 LAYER 和 LANGUAGE 对调。设置页预览和热力图不画本机交换，与现状一致。

### 设置页

三端都放在「键盘 › 按键布局」子页（[按键布局设置页改版](2026-10-29-key-layout-page-redesign.md)的结构不变），每个分段（26 键、14/18 键、9 键）的样式卡片里、当前样式说明下面多一个开关；26 键分段里排在「26 键「符」与「123」互换」之后，另两个分段里是卡片里唯一的开关。三个分段是同一项偏好，切分段时开关状态不变；只有一个分段时照样出现在那张卡片里。

- 标题：`中/英键放在空格左边`
- 说明：`中/英 排到空格左边，逗号、句号仍紧挨空格。`
- 先重画预览再写偏好，与互换开关相同。
- 预览底行按同一个函数重排；偏好为 `left` 时，在样式自己的高亮之外再高亮中/英这一个键（Android `KeyLayoutStylesPolicy.LANGUAGE`、iOS `.language`、共享页 `SoftLanguage`）。被挤动的空格和标点宽度、样子都没变，不高亮。
- 统计热力图的 26 键和 9 键（含右侧大回车）底行跟着画，按键 id 不变。
- 设置搜索：三端都能用标题找到这一页。Android 在 `PageId.KEY_LAYOUT_STYLES` 登记了标题和近义词「中英切换键」；iOS 在「键盘」页的关键词里登记了标题；共享页的开关不挂在任何分段的 `SettingsSearchReveal` 下，命中时停在当前分段（子页刚打开时是用户在用的那一类，判断不了时是第一段）。

文字的位置：Android `TouchKeyStyle.LANGUAGE_LEFT_TITLE` / `LANGUAGE_LEFT_NOTE`，iOS `TouchKeyLayoutOffering.languageLeftTitle` / `languageLeftSubtitle`，共享页 `languageKeyLeftTitle` / `languageKeyLeftDescription`（`packages/ui/src/keyboard/touch-key-layout-styles.ts`）。入口副标题「各类键盘的底行样式、键位和顶部数字行」已经盖住这一项，不改。

### 账号同步

| 共享偏好 | Android 键 | 鸿蒙键 | 值 |
| --- | --- | --- | --- |
| `touch_language_key_side` | `platform.android.language_key_side` | `platform.harmony.language_key_side` | 字符串 `right` / `left` |

走触屏宿主共用的 `TouchHostKeys`（`crates/client-services/src/account/settings_sync.rs`）和鸿蒙 `HARMONY_KEYS`。导出总是写这个键，默认值也写（`right`），云端才能被改回右边；应用时严格解析，本机不认识的值只跳过这一个键。服务端账号字段表收录这两个键之前，上传前的字段过滤会去掉它们。iOS 的同步走 `ios_settings.rs` 的自有映射，与其他按键布局偏好一样暂不同步。

## 各端实现

### Android

- `TouchKeyStyle`：`LANGUAGE_KEY_SIDE_PREFERENCE`、标题和说明常量、`languageLeft(Object)`（只有 `"left"` 算左）和 `languageKeySideWire(boolean)`。宿主侧用布尔表示，不另建枚举。
- `KeyboardActionRow`：`languageBeforeSpace`；`designEntries`、`numberPadEntries`、`tallReturnEntries` 各加一个带 `languageLeft` 的重载，旧重载传 false，默认行为逐键不变。手写底行和九键数字层经 `designEntries` 跟上。
- `MSIMEInputService` 读偏好，`ImeBottomRow`、`ImeLayoutRows` 接线；设置页 `KeyLayoutStylesPage` 把原来单独的互换卡片和中/英开关合成说明下面的一张无标题卡片，互换行只在 26 键分段显示；`KeyLayoutStylesPolicy.keyboard` / `highlights` 加 `languageLeft`；`KeyHeatmapLayout`、`StatisticsFragment` 画热力图。

### iOS

- `TouchKeyLayoutStylePreference.swift`：`TouchLanguageKeySide`、`TouchKeyLayoutStyle.languageKeySide`（文档读法、App Group 镜像 `keyboard.style.languageKeySide`、`save(languageKeySide:)`）、`TouchBottomRowLayout.languageBeforeSpace`、标题和说明；预览底行 `TouchKeyLayoutKind.bottomRow` 与 `highlightedSlots` 接上。
- `KeyboardViewController`：有样式表的行（非标准样式、九键数字层、右侧大回车）在 `styledBottomRow` 里把表过一遍 `languageBeforeSpace`；没有表的行（标准 26 键、英文 26 键、标准 14/18 键和大千、标准九键、借来的数字九宫格、手写）在 `arrangeActionRow` 里对按键视图套同一个泛型函数。宽度约束只约束宽度、不看次序，所以每个键的宽度与右边时相同。借来的数字九宫格「空格在 0 右边」原来的列对齐约束（0、空格、中/英 各占一列）在左边时照旧生效，只换次序后仍与上方的数字列对齐。只在手机底行（`.phone`）和九键外框（`.nineKey`）、而且中/英显示着时生效。
- 设置页开关的标识是 `keyLayoutLanguageLeftSwitch`；热力图 `TypingKeyHeatmap.letterRows(for:)` 与 `nineKeyBottomCells(for:languageLeft:)` 跟着排。

### 鸿蒙与共享设置页

- `TouchLayoutStylePolicy.languageKeyLeft(value)` / `languageBeforeSpace(row, enabled)`（设计底行没有 0，越过的是空格、逗号、句号）；`NineKeyLayout.actionRow` 末尾加 `languageLeft = false`，`tallReturnRow` 同样加一个可选参数，宽度算法不变。
- `KeyboardSession` 读进 `PreparedPreferences`，`KeyboardView` 在 `applySettings` 换值、进 `faceKey()`；`designRowEntries` 在 `symbolFirst` 之后套 `languageBeforeSpace`，`nineKeyActions` 和右侧大回车传参。手写底行在视图里写死，左边时在 123 后面画 `handwritingLeftLanguageKey`，右边时原来 。 后面那一格保持原样，`scripts/test-mobile-handwriting-pad-parity.py` 按右边那一格读三端的默认键序。
- 共享 React 页 `TouchKeyLayoutSettings` 的开关、`keyLayoutPreviewRows` 的预览与高亮、`keyboard-heatmap.ts` 的热力图（右侧大回车另有一份左边的外框）。

### 既有笔记审计

- [手机底行的中/英键挪到回车左边](2026-10-09-mobile-language-key-beside-return.md)：沿用。默认位置仍是它定的「紧挨回车」，本篇只加一个可选的左边位置；它「不在范围内」的面（iPad `.tablet`、假名九键、注音九键、123 / #+= 层）本篇同样不动。
- [Android 语言键与数字键交换](2026-10-14-android-language-number-key-swap.md)：沿用，不迁移、不合并。交换在本偏好之后做；「按语言键是否实际紧邻回车决定让位」这条规则正好让左边时的回车不让位。
- [iOS 可隐藏底行中英键](2026-10-15-ios-hide-language-key.md)：沿用。隐藏时本偏好不起作用，存着的值不变。
- [26 键「符」与「123」互换和数字键盘空格位置](2026-10-29-touch-symbol-first-and-number-pad-space.md)：沿用其做法（共享偏好、`TouchHostKeys`、同一个设置子页、热力图落点），本偏好在互换和空格位置之后排。
- [按键布局设置页改版](2026-10-29-key-layout-page-redesign.md)：沿用其结构，开关放在样式卡片的说明下面。
- [按键布局样式与顶部数字行](2026-10-27-touch-key-layout-styles.md)：沿用，底行样式表不变，本偏好是叠在表上的一步。

## Alternatives considered

- **中/英一律紧挨空格，把空格左边的标点挤到中/英左边**（如 `123 | 符 | ， | 中 | 空格 | 。 | ↵`）：与豆包、百度「中英紧挨空格」的说法字面上最接近。不采用：「逗号句号在空格两侧」样式会被拆开；英文 26 键（句点在空格左边）和中文标准（空格左边没有标点）上的中/英不在同一格，切中/英时这个键会跳一格。
- **只作用于中文 26 键和九键**：#10228 举的例子是这两类，改动面最小。不采用：英文 26 键和其他语言共用同一条底行，切中/英、切语言时中/英会在左右之间跳；14/18 键和手写的用户同样有这个习惯，只做一部分会让同一个开关在不同键面上时灵时不灵。
- **做成布尔偏好**（`touch_language_key_left: bool`）：少一个枚举类型。不采用：线上写法用字符串枚举，以后真要加别的位置（比如百度经典布局那种最左边）时不用换字段；宿主侧仍可以用布尔表示（Android 就是），设置页是开关。
- **做「符 / 123 / 中」三键自定义排序面板**：一项覆盖所有排法，包括百度的经典布局。不采用：要在三端设置页加排序面板、和每种样式组合，还要迁移 Android 本机的交换；#10228 要的就是一个左右的选择，[#10182](2026-10-29-touch-symbol-first-and-number-pad-space.md) 当时也把排序面板放下了。
- **与 Android 本机「交换语言键与数字键」合并，或把它迁进本偏好**：少一个设置。不采用：两者排出来的位置不同（交换是中/英到最左、123 到右边），合并会改掉已打开交换的用户的键盘；它是本机设置，迁进共享偏好会把它同步到 iOS 和鸿蒙，而那两端没有对应的排法。两者叠加还能得到百度经典布局。

## Consequences

- 收益：习惯中/英在左手边的用户可以在三端各改一处，随账号同步到 Android 和鸿蒙；默认文档、默认键盘逐键不变，老用户无感。
- 代价：多一项只对手机触屏有意义的共享字段，macOS 在 `preference_coverage.py` 里登记不读的理由。左边时回车不再向中/英让出触摸分界，右手拇指离中/英远了，这是用户自己选的。14/18 键字母层与借来的数字九宫格的中/英差一格。鸿蒙手写底行里中/英键的写法有两份（右边原样、左边一个新 builder），以后改中/英键的样子要改两处。
- 服务端账号字段表要收录 `platform.android.language_key_side` 和 `platform.harmony.language_key_side` 才会真正上传，PR 里写明。

## Verification

集成时（2026-10-29，在 worktree 根目录）本机跑过：

- `ANDROID_SDK_ROOT=~/Library/Android/sdk platforms/android/check-host.sh`：exit 0，末行「Android service Java/API and manifest/resource checks passed」。其中 `TouchKeyStyleSmoke.languageLeft`、`KeyLayoutStylesPolicySmoke.languageLeft`、`KeyHeatmapLayoutSmoke`、`SplitKeyboardPolicySmoke` 覆盖本项，check-host 的 #10228 一段钉住接线。
- `bash platforms/harmony/tests/run.sh`：exit 0，0 failure(s)；三组新测试（偏好读法、设计底行、九键外框与右侧大回车）都是 ok。
- `apps/desktop` 下 `npx vitest run tests/settings`：476 个文件、3084 项通过。
- `scripts/test-ios-*.py` 全部 42 个和 `scripts/test-mobile-handwriting-pad-parity.py` 通过；`scripts/test-preferences-field-parity.py` 通过。
- `pnpm run -s verify-notes` 通过。

各端在实现时另跑过（集成时没有重跑）：

- 共享层：`rbuild cargo test -p msime-client-core -p msime-client-services` 通过（client-core 690 项、client-services 1447 项），`rbuild cargo clippy -p msime-client-core -p msime-client-services --all-targets -- -D warnings` 无警告，`platforms/macos/tests/settings/preference_coverage.py` 通过。
- Android：`tests/device/build-editor.sh` 通过；`rbuild` 上 Gradle `:app:compileFullDebugJavaWithJavac` 实际执行并成功。
- iOS：`rbuild` 上 MSIMEApp `build` 与 MSIMEClientTests `build-for-testing` 都成功；`KeyLayoutStyleTests` 新增 7 个用例（读法和保存、各行的表、预览和高亮、热力图、真键盘的可见键序与宽度、回车不让位与读屏顺序、密码框和 iPad 底行），只编译、没有运行。
- 鸿蒙：`apps/desktop` 下 `npx tsc --noEmit -p .` 通过，改动的 TS/TSX 文件 `vp fmt --check`、`vp lint` 通过。

没有验证的：

- iOS 的 XCTest 没有运行（Studio 起不了模拟器，本机盘放不下测试产物），真键盘那几项（尤其手写方案在测试宿主里能不能选上、宽度 1pt 的容差）交给 PR 的 iOS Simulator 任务。
- 鸿蒙的 `.ets` 没有编译（没有跑 `hvigorw assembleHap`），ArkTS 只到 `run.sh` 的接线守卫和 node 下的 `tsc`。
- Android 设备测试套件只编译、没有运行。
- 三端都没有在模拟器或真机上装包看过：键盘底行、右侧大回车的重建、设置页开关的样子、热力图、设置搜索命中后停在哪个分段，都只到单元测试与编译这一级。
