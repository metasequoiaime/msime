# Agent Note: 九键左侧符号栏的两张表提为三端共享偏好

Status: implemented

## Problem

#10223：九键左侧符号栏只有 Android 能改，而且是不随账号同步的本机设置（`platform.android.nine_key_symbols` / `nine_key_digit_symbols`，见[九键左侧符号栏与数字键面](2026-10-08-android-nine-key-number-pad.md)和[可视化编辑](2026-10-18-android-nine-key-sidebar-editor.md)）。iOS 与鸿蒙的左列固定是出厂的 ，。？ 和四则运算，换手机或在另一台设备上就得重设。

上游评审稿曾认为「只证实一家主流输入法有这一项」。2026-10-29 维护者在真机上逐家核对，四家都有，且都把字母层和数字层分成两张表：

- 搜狗：「自定义九键符号」，分 9 键 / 大 9 键 / 数字三个标签。
- 百度：「自定义 9 键键盘左侧符号」，分中文 / 数字。
- 微信输入法：「九宫格键盘 - 侧边栏」，分拼音 / 数字。
- 豆包输入法：「侧边栏符号自定义」，分 9 键 / 手写侧边栏、数字侧边栏。

所以这一项按主流功能做：两张表，三端都能改，Android 与鸿蒙随账号同步。手写侧边栏（豆包）和大 9 键（搜狗）不做：水杉的手写键盘没有左侧符号栏，大 9 键不是水杉的键盘类型。

## Decision

### 共享偏好

`crates/client-core/src/preferences.rs` 的 `Preferences` 紧跟 `touch_nine_key_style` 新增两项，都是 `Vec<String>`，空表表示「本端出厂表」，空表不写进文档：

- `touch_nine_key_sidebar_symbols`：字母层，拼音九键、九键双拼、笔画字母层共用，Android 14 键左侧底轨也读它。
- `touch_nine_key_digit_sidebar_symbols`：数字层，所有九宫格数字键盘（九键一族自己的数字层，以及 26 键、14/18 键、手写等按 123 借来的数字九宫格）左列 #+= 上面的部分。

规则集中在 client-core，三端编辑器用同样的数：一张表最多 30 项（`NINE_KEY_SIDEBAR_MAX_SYMBOLS`），一项最多 4 个 Unicode 码位（`NINE_KEY_SIDEBAR_MAX_SYMBOL_CODE_POINTS`，按码位不按字素：👍🏽 可以，👨‍👩‍👧 不行），用一个空格连起来最多 200 个 UTF-16 单元（`NINE_KEY_SIDEBAR_MAX_TEXT_UTF16`，与 Android 原来的 `MAX_TEXT_LENGTH` 相同，也是同步值的长度上限）。`normalize_nine_key_sidebar_symbols` 逐项去首尾空白，丢掉空串、含空白（含全角空格、NBSP）或控制字符的、超长的和重复的项，超出项数或总长时截掉后面的。读文档是宽松的：数组里非字符串的元素丢掉再规范化；不是数组时整份文档照旧报错。`validate` 要求两张表等于自己的规范化结果，宿主保存前自己保证。

宿主写入规则三端一致：编辑器确认时表与本端出厂表逐项、同顺序相同就写空数组 `[]`，否则写用户的表；「恢复默认」就是写空数组；只写这一个字段。读的一侧同样对称：缺键、空数组，以及与本端出厂表逐项相同的非空表（别的程序或旧路径写进来的）都按出厂表画、不描边、右侧大回车照常接 ！（Android `NineKeySidebarPolicy.custom`、iOS `TouchKeyLayoutStyle(in:)` 经 `NineKeySidebar.stored`、鸿蒙 `TouchLayoutStylePolicy.sidebarSymbols(value, defaults)`、共享设置页 `storedNineKeySidebar`）。

### 各端出厂表（有意不拉齐）

| | 字母层 | 数字层 | 表外固定的键 |
| --- | --- | --- | --- |
| Android | ， 。 ？ | + - * / = % ( ) : @ ， 。 ？ ！ | #+= 在可滚动栏下面 |
| iOS | ， 。 ？ | + - * / | #+= 是 `operatorScroll` 栈的最后一项 |
| 鸿蒙 | ， 。 ？（点按仍把 ASCII , . ? 交给 Engine） | + - * / | #+= 是左列最后一格 |

数字层出厂表三端本来就不同，这次不改，免得老用户的键盘变样。右侧大回车只在字母层用出厂表时末尾接 ！；自定义表原样使用，不接（Android 接上的那个 ！ 带上滑出 0、长按上下标的手势，自定义表里的 ！ 是普通符号）。

### 键盘

只换左列内容和可见项数，左列宽高、九宫格、右列、底行都不动。

- 一屏最多几项：Android 字母层 5（笔画 3），数字层 5，`#+=` 在滚动区外；iOS 字母层 3，数字层照原来的 `operatorScroll`；鸿蒙字母层 3，数字层是 4 格高的滚动区加最下面一格 `#+=`。每项等高，不少于 28 dp/pt/vp，键盘调到最矮时少放几项，多的上下滚动。
- 出厂表时 iOS 与鸿蒙走原来的代码路径，画面不变；只有自定义表才走新的滚动栈（iOS `NineKeySidebarScrollView`，鸿蒙 `customLetterSidebar` / `customDigitSidebar` / 笔画 `strokeRailMarks`，`ForEach` 键带符号、下标、高度和 `faceKey`）。
- 点按自定义项：先结束组字，处理与点同一层的出厂项相同（字母层按首选上屏；数字层与出厂运算符一样原样上屏），再按字面上屏。Android 照旧走 `commitNineKeyLiteral`（全角模式下转全角）；iOS 与鸿蒙不做全半角转换。表项正好是一整对符号（`（）`、`()` 等）时光标停在两者中间，三端用同一张 16 对的表，以 Android `PairedPunctuationPolicy.SYMBOL_PAIRS` 为准（iOS `NineKeySidebar.pairedSymbols`，鸿蒙 `PairedPunctuationPolicy.LITERAL_PAIRS`）。左列统计 id 不变，热力图仍画 ，。？。组字时左列被拼音读音盖住，自定义表同样被盖住。
- 长按：Android 打开「按键布局」并直接进入那张表的编辑页（`edit_sidebar_key` 传共享偏好名）；鸿蒙同样直达编辑页，键盘打开设置的链接在页面之外带一个子目标，值也是共享偏好名（见[鸿蒙设置链接的子目标](2026-10-30-harmony-settings-link-target.md)）；iOS 长按不做事（键盘扩展打不开宿主 App 的某一页）。
- 密码框和只收数字的框里底行样式退回标准时，两张表照用户的画。

### 设置页（三端同一结构）

入口从 Android 键盘页「布局」组挪进[按键布局设置页](2026-10-29-key-layout-page-redesign.md)，键盘页不再列这两行，入口副标题不改：

- 9 键分段卡片，在样式胶囊和当前样式说明之后，一行导航行「九键左侧符号」，右侧是摘要（不超过 4 项时空格连起来，多于 4 项时前 4 项加「 等 N 个」，空表显示本端出厂表），只在 9 键分段显示。
- 「数字键盘」一组，在空格位置胶囊和说明之后，一行导航行「数字键盘左侧符号」，跟着这一组显示。Android 原来「没有拼音九键就不列数字行」的条件作废。
- 9 键预览的左列按用户的字母表画一屏（本端上限内的前几项，等高），自定义时左列每格描强调色；出厂表照旧，不描。右列的 ！ 也记 `SoftPunctuation`，所以三端都不按键 id 加进高亮表：Android 用预览专用的 id `NineKeySidebar`，iOS 与共享页按位置描。
- 「按键布局」页以外的键盘预览（#10245 让它们跟随按键布局：Android 皮肤页与社区皮肤的 `KeyboardPreview`、iOS `KeyboardSkinPreview`、共享页 `harmonyTouchRowsForPreferences` 画的屏幕键盘页「尺寸」、皮肤卡片和社区皮肤）9 键左列同样按字母表画这一屏、不描边，三端与「按键布局」页取同一张表（Android `KeyLayoutStylesPolicy.Preview` 带上表，iOS `KeyboardPreviewLetterLayer` 取 `TouchKeyLayoutKind.previewSidebar`，共享页的左列栈在 `harmonyStyledRows` 里），各自的画法能画一格里的竖栈。
- 编辑页两张表共用一个页面：标题同上；顶部说明「拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。」/「所有九宫格数字键盘共用，#+= 固定在最后。」；可删除、可拖动排序（每项前的序号和跟手拖动见[序号与跟手拖动](2026-10-31-nine-key-sidebar-order-drag.md)）；可加入的常用符号是 Android `NineKeySidebarPolicy.ADDABLE_SYMBOLS` 那 51 项里还不在表里的；可手动输入一项（Android 这次补上「输入符号」行，原来只有选择面板）；「恢复默认」；整表不合规时保存键不可点，重复项在加入时挡掉；保存失败照各端现有写法提示并保留原值；编辑页不画预览。
- 设置搜索收两个标题；Android 另收旧名「九键数字键盘左侧符号」作别名。iOS 搜索只到页面一级，命中打开「键盘」页。

上限、可加入的符号、两个标题、两条说明、成对符号表和字母层出厂表由 `scripts/test-nine-key-sidebar-parity.py` 核对三端逐项相同，鸿蒙键盘与共享设置页的两张出厂表也在其中。

### 账号同步

走 `TouchHostKeys`，Android 键 `platform.android.nine_key_symbols` / `nine_key_digit_symbols`，鸿蒙键 `platform.harmony.nine_key_symbols` / `nine_key_digit_symbols`，值是空格连接的字符串，出厂表导出空串（云端才能改回默认）。应用时空串回到出厂表，其余逐项规范化，一项都留不下时只跳过这个键。iOS 不同步，与其他按键布局偏好一样。Android 键与本机旧键同名，旧键不在 `ANDROID_LOCAL_SETTINGS` 里，不冲突。

### Android 迁移

`KeyLayoutStyleMigration` 把两个旧键也列进待迁移：旧值能解析且不等于对应出厂表时放进 `migrate_android_key_layout` 的请求，Rust 在偏好锁内只在盘上缺键或是空数组时写入；请求成功后两个旧键与 `long_space_layout`、`number_row` 一起删掉，不论旧值是什么。删的理由：共享偏好里空表既是缺键也是「恢复默认」，旧键留着的话下一次走迁移入口会把用户刚恢复掉的旧表写回去再同步出去。备份恢复时，备份里有旧键、没有新同步键的，转成新键（出厂表或解析不出写空串）。`AndroidLocalSettings` 保留两个旧键的声明，check-host 钉住只有迁移类读它们。

## Alternatives considered

- **复用 `touch_quick_punctuation_symbols`**：已有一张三端共享的标点表，少一个字段。不采用：默认表、上限和用途都不同，iOS 逗号长按菜单在用它，九键左列改了会连带改掉那个菜单。
- **做成 `TouchNineKeyStyle` 的新值**：入口现成。不采用：符号表与底行样式是自由组合的两件事（右侧大回车加自定义表很常见），而且九键胶囊已满四个。
- **入口留在 Android 键盘页「布局」组，iOS 与鸿蒙也照放**：Android 老用户不用重新找。不采用：#10208 改版后「按键布局」子页正好有「分段卡片里的附加行」和「不随分段的独立组」两种位置，对应字母表和数字表；9 键预览就在同一张卡片上，改完立刻看到左列。老用户的路径用搜索别名和长按直达编辑页兜住。
- **一张表同时管字母层和数字层**：设置页只有一行。不采用：四家主流输入法都是两张表；两层的用途完全不同（数字层要运算符），Android 原来也是两张。
- **三端统一出厂表**：同步过去的表三端一个样。不采用：数字层出厂表三端本来不同，统一会改掉现有用户的键盘；空表表示「本端出厂表」已经让同步不传具体出厂内容。
- **iOS 与鸿蒙的自定义项也转全角**：与 Android 一致。不采用：iOS、鸿蒙原来就没有键盘侧的全半角转换路径，用户在表里写什么就打什么更好懂；这是有意保留的差异。

## Consequences

- 收益：三端都能改两张表，Android 与鸿蒙随账号同步；默认文档、出厂画面逐键不变，Android 老用户的自定义表在升级时迁进共享偏好。
- 有意保留的差异：数字层出厂表三端不同；iOS、鸿蒙自定义项不做全半角转换；iOS 长按不做事；iOS 搜索命中只打开「键盘」页。鸿蒙长按原来只能打开「屏幕键盘」页，2026-10-30 补上了直达编辑页。
- 代价：Android 降级到旧版本后符号栏回到出厂表（旧键已删，与 `number_row` 接受的代价相同）。入口挪位置，Android 老用户一时可能找不到，靠搜索别名和长按直达兜住。
- 发布顺序：服务端账号字段表等带迁移的 Android 版本发布后再收录四个同步键（字符串，最长 200 个 UTF-16 单元）；收录之前上传会被字段表过滤掉，同步往返要等收录后才能验证。
- [按键布局样式](2026-10-27-touch-key-layout-styles.md)里「iOS 没有自定义符号栏列表」那句已不成立，那篇已改指向本篇。[长按快速编辑](2026-10-26-android-nine-key-sidebar-quick-edit.md)的深链目标从 `KEYBOARD_OPTIONS` 改成 `KEY_LAYOUT_STYLES`，那篇已同步。[可视化编辑](2026-10-18-android-nine-key-sidebar-editor.md)的交互沿用，存储换成共享偏好、多了手动输入。[默认符号回到三项](2026-10-13-android-nine-key-default-symbols.md)和[Android 成对标点](2026-10-08-android-paired-punctuation.md)沿用，后者的配对表是三端的权威。

## Verification

在 `/Users/e-hu/worktrees/issue-10223-nine-key-sidebar-symbols` 跑过：

- 共享 Rust 层（分支第一个提交，实现时跑过）：`rbuild cargo test -p msime-client-core -p msime-client-services`、`rbuild cargo clippy -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings` 通过。
- `python3 scripts/test-preferences-field-parity.py`、`python3 platforms/macos/tests/settings/preference_coverage.py .`、`python3 scripts/test-nine-key-sidebar-parity.py` 通过；后者把 iOS 的一项可加入符号换序、删一对成对符号、去掉说明的句号后如期报出三处不同，改回后通过。
- Android：`ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh` 通过（310 项 JVM 冒烟，256 份设备测试源码编译）；Android 宿主实现时另跑过 `platforms/android/tests/device/build-editor.sh` 和 Studio 上的 Gradle `:app:compileFullDebugJavaWithJavac`，都通过。
- iOS：`scripts/test-ios-*.py` 42 个全部通过，`platforms/ios/tests/settings/ProjectConfigurationTests.py` 通过；Studio 上经 `rbuild` 用 xcodegen 生成工程后 `MSIMEClientTests` `build-for-testing` 通过（含读文档时出厂表回落的最后一处改动），`MSIMEApp` `build` 在 iOS 宿主实现时通过。
- 鸿蒙与共享设置页：`bash platforms/harmony/tests/run.sh` 0 失败（含 `nine-key-sidebar.test.ts` 与接线守卫）；`python3 scripts/test-harmony-arkts-subset.py` 通过；`apps/desktop` 下 `npx vitest run tests/settings` 477 个文件 3089 项通过，`npx tsc --noEmit -p .` 通过，改动的 TS 文件 `vp fmt --check`、`vp lint` 通过；Studio 上经 `rbuild` 跑 `ohpm install && hvigorw assembleHap --no-daemon`，CompileArkTS 完成、BUILD SUCCESSFUL，21 条 ArkTS 警告都不在本次改动的文件里（这次编译在删掉 `literalPairs` 之前，之后只重跑了 `run.sh` 和 ArkTS 子集检查）。
- `bash scripts/run-checks.sh` 全部文本契约通过（第一次跑时 `test-harmony-unwired-symbols.py` 报出只给测试用的 `PairedPunctuationPolicy.literalPairs`，删掉它、三端成对符号表改由一致性脚本核对后通过）；`pnpm run -s verify-notes` 通过。

没有验证：

- 三端的 XCTest、设备冒烟都没有运行：Studio 的 CoreSimulator 版本低于 Xcode 要求、跑不了模拟器，本机磁盘放不下测试产物；`NineKeySidebarPairDeviceSmoke` 只编译过。
- 没有在任何真机或模拟器上看过：滚动手感、滚动中长按、出厂表画面不变（iOS 有键帧断言但没跑，鸿蒙只有不变的代码路径和格高算术测试）、长按直达编辑页、编辑页拖动排序。
- Android 迁移在真实升级路径上的时序、备份恢复、账号同步往返（字段表收录前上传会被滤掉）。
