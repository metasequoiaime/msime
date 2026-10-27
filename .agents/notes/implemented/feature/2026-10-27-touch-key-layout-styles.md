# Agent Note: 触屏按键布局样式与顶部数字行的共享偏好

Status: implemented

## Problem

手机触屏键盘的底行只有一种排法。Android 先后加了两个本机设置来满足不同习惯：[长空格布局](2026-10-27-android-long-space-layout.md)（`platform.android.long_space_layout`）和[数字行](2026-10-09-android-number-row-and-split-duplicates.md)（`platform.android.number_row`），都只存在 Android 本机、不随账号同步，iOS 与鸿蒙没有对应的选择。产品决定把「底行样式」按键面类型做成三端共享的选择，并把顶部数字行一起提成共享开关。

## Decision

共享偏好（`crates/client-core/src/preferences.rs` 的 `Preferences`）在触屏那组字段里新增四项，默认值就是现在的键盘：

- `touch_twenty_six_key_style: TouchTwentySixKeyStyle`，作用于全部中文 26 键字母层（全拼、26 键双拼、五笔 26 键、粤拼等）；英文 26 键、iPad 全尺寸键盘和鸿蒙 2in1 的整排键盘不受它影响。取值 `standard`（默认，123 | 符 | 空格 | ，长按。 | 中/英 | 回车）、`comma_period`（123 | 符 | ， | 空格 | 。 | 中/英 | 回车）、`long_space`（123 | ， | 长空格 | 中/英 | 回车，符号入口移到工具栏，接替 Android 的 `long_space_layout`）、`emoji`（123 | 😀 | 空格 | ， | 中/英 | 回车，符号经长按 123 进入）。
- `touch_fourteen_key_style: TouchFourteenKeyStyle`，14 键与 18 键字母层（全拼、双拼、五笔 14 键）共用。取值 `standard`（默认，123 | ， | 空格 | 。 | 中/英 | 回车）、`long_space`、`emoji`。
- `touch_nine_key_style: TouchNineKeyStyle`，九键字母层（拼音九键、九键双拼、笔画共用这个框架），除 `tall_return` 外左侧符号栏和右侧功能列不变。取值 `standard`（默认，符 | 123 | 空格 | 中/英 | 回车）、`comma_period`、`emoji`、`tall_return`（设置页名「右侧大回车」，排在表情键之后：左列是跨三行的符号栏，栏下方底行放与它同宽的 符；九宫格下方底行 123 | 🌐 | 空格 | 中/英 恰好与九宫格同宽，空格占剩余宽度；右列与现有右列同宽，自上而下 ⌫、重输，回车跨第三行和底行、两倍高且中间无缝。现有右列的 ！ 去掉，只在这个样式里接到符号栏默认字母符号表末尾，自定义列表原样使用。九键数字层照同一框架：符 留在原处，123 的位置放切换键，空格的位置放 0，空格放哪按各端现有数字层规则，回车仍两倍高。中/英 与回车不在同一行，不适用回车让位和点击区规则。热力图照这个框架画，统计 id 不变。较旧的版本读到它按 `standard` 处理）。
- `touch_number_row: bool`，默认关，在 26 键、14 键和 18 键字母层上方画 1 到 0（九键、手写不画）；窗口太矮（如手机横屏）时由宿主按自己的高度门槛不画，存着的值不变。接替 Android 的 `platform.android.number_row`。

三个样式按键面类型分开存，而不是一个枚举套所有键面：每类键面能放的排法不同（14 键本来就有逗号和句号，没有 `comma_period`；九键没有地方放长空格），分开存时每个值都是这类键面真能画出来的样子。

### 文档与读取

- 四项等于默认值时都不写进文档（`skip_serializing_if`），默认文档与加字段之前逐字节相同，旧版本照样能读；缺这几个键的旧文档读出默认值，读完不改写文件。
- 三个枚举各有手写的宽松 `Deserialize`，做法与 `CandidateTranslationScope` 相同：不认识的字符串（较新版本加的样式）按 `standard` 读，整份文档照常读出；不是字符串照样报错。另有严格的 `parse` / `as_str`，同步与宿主用它们。
- 不往 `TouchKeyboardLayout`、`TouchShuangpinLayout`、`TouchKeyboardScheme` 里加成员：样式是同一键面的底行变化，不是新的键面或方案。

### 账号同步

四项随账号同步，走触屏宿主共用的 `TouchHostKeys`（`crates/client-services/src/account/settings_sync.rs`），与 `touch_quanpin_shift` 的[先例](2026-10-26-touch-quanpin-shift-preference.md)相同：

| 共享偏好 | Android 键 | 鸿蒙键 | 值 |
| --- | --- | --- | --- |
| `touch_twenty_six_key_style` | `platform.android.twenty_six_key_style` | `platform.harmony.twenty_six_key_style` | 字符串 |
| `touch_fourteen_key_style` | `platform.android.fourteen_key_style` | `platform.harmony.fourteen_key_style` | 字符串 |
| `touch_nine_key_style` | `platform.android.nine_key_style` | `platform.harmony.nine_key_style` | 字符串 |
| `touch_number_row` | `platform.android.number_row` | `platform.harmony.number_row` | 布尔 |

导出总是写这四个键（默认值也写，云端才能被改回默认）。应用时样式按严格写法解析，本机不认识的样式只跳过这一个键并列进 `skipped`，本机的选择不变。服务端字段表收录这些键之前，上传前的字段表过滤会去掉它们。`platform.android.number_row` 与 Android 本机设置文件里那个不同步的旧键同名：旧键不在 `ANDROID_LOCAL_SETTINGS` 里，宿主不会把它当本地同步值传进来，两者不冲突。iOS 的同步走 `ios_settings.rs` 的自有映射，`touch_quanpin_shift` 在那里也没有键，这次同样不加。

### 宿主

桌面宿主（macOS、Windows、Linux 与 `apps/desktop`）保留但不读这四项。macOS 的偏好覆盖门禁（`platforms/macos/tests/settings/preference_coverage.py`）为它们登记了不适用理由；React 设置页的字段对照（`scripts/test-preferences-field-parity.py`）把它们列进 `RUST_ONLY`，`packages/ui/src/core/preferences-model.ts` 带上了类型。Android、iOS 与鸿蒙的键面、设置入口和本机旧设置的迁移另行接入，接入时就地更新本篇。

Android 把 `platform.android.long_space_layout = true` 迁成 `touch_twenty_six_key_style = long_space` 时，只迁中文 26 键的底行：英文 26 键回到标准底行，独立句点键回来，不再由逗号下滑接替（[长空格布局](2026-10-27-android-long-space-layout.md)里对英文键面的那项改动不随 `long_space` 保留）。要保留它需要另做决定，不能当成 `long_space` 的附带效果。

## Alternatives considered

- 一个枚举覆盖全部键面（如 `touch_bottom_row_style`）：只有一个设置项，但多数取值只对部分键面有意义，宿主得各自定义「这个值在九键上画成什么」，三端很容易画出不一样的结果；按键面类型分开存，每个取值都有唯一的画法。
- 保留 Android 本机设置、iOS 与鸿蒙各加本机设置：改动最小，但同一账号的几台设备要各设一遍，三端的取值也会漂移；底行习惯是跨设备的，照 `touch_quanpin_shift` 的先例做成共享偏好。
- 不认识的样式让整份文档读失败（默认派生的 `Deserialize`）：能更早暴露写错的值，但较新版本加了样式之后，旧版本会读不出整份偏好；按 `CandidateTranslationScope` 的做法宽松读取。

## Consequences

- 收益：三端可以用同一组取值画底行和数字行，选择随账号同步；默认文档不变，老用户和旧版本不受影响。
- 代价：多了四个只对手机触屏有意义的共享字段，桌面宿主要在门禁里逐项说明不读的理由；Android 两个本机旧设置需要宿主做一次性迁移，迁移前两套值可能并存。
- 服务端账号字段表需要收录八个新键（Android 与鸿蒙各四个）后，这几项才会真正上传。
- 字段表收录这些键的那一刻，每台设备的同步基线里都没有它们。`merge_settings`（`crates/client-services/src/account/cloud_sync/policy.rs`）把「基线里没有的键」算作本机改过，所以之后第一次带着本机改动（`dirty`，任何一项设置变了都算）走上传或合并的设备，会用本机的值盖掉云端的值，默认值也一样。例子：手机 A 已把本机数字行 `true` 迁进 `touch_number_row` 并同步，云端是 `true`；同账号的手机 B 从没开过数字行，但改了别的设置后同步，上传 `false`；A 下一轮本机值与基线相同，跟着云端变成 `false`，A 上原有的数字行被关掉。没有本机改动的设备走下载，不会出现这种覆盖。这是合并规则对所有新进字段表的键的共同行为（`touch_quanpin_shift` 也一样），这次不改共享的合并规则：改成「基线里没有、云端已有就用云端」能挡住这个例子，但会吞掉设备在下载之后才做的真实修改，需要单独决定。顺序要求：(1) Android 宿主必须在第一次导出或应用同步之前完成 `number_row`、`long_space_layout` 两个本机旧键的迁移，迁移和导出同版发布，不能先发只导出不迁移的版本，否则会把默认值当成用户的选择传上去；(2) 服务端字段表要等带迁移的 Android 版本发布后再收录这八个键；(3) 收录之前先决定上面的合并规则要不要改。

## Verification

- `rbuild cargo test -p msime-client-core -p msime-client-services`：新增的默认值与不改写旧文档、存储往返、线上写法与不认识的值、Android 同步写法与跳过，以及导出键清单的两处快照和 Android、鸿蒙的整份往返用例。
- `python3 scripts/test-preferences-field-parity.py` 与 `python3 platforms/macos/tests/settings/preference_coverage.py .` 通过。
