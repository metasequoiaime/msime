# Agent Note: 触屏键盘把拼写写进输入框，候选栏不再画读音

Status: implemented

## Problem

iOS 把读音画在候选行最左边（[读音并入候选行](2026-10-13-ios-inline-reading-top-row.md)），这一段的宽度跟着拼写长短变，每按一个键第一个候选就左右挪一次（打 `nihaoshijie` 挪 9 次），九键的读音长度还会随歧义跳。读音单独一行能让候选不动，但顶栏要多出 14pt，#7405 正是因为嫌顶栏高才把读音并进候选行的。产品方看过五种布局的对比（读音并入候选行、读音段只长不缩、细读音行加候选行、旧的两行、写进输入框），定下：像苹果自带键盘和 Gboard 那样把拼写写进输入框，候选栏只放候选，不要开关，直接作为默认。

## Decision

- iOS、Android、HarmonyOS 手机的触屏键盘固定把正在拼写的内容作为组字文本写进输入框（iOS marked text、Android composing text、HarmonyOS 预上屏文字），候选栏不再画读音，候选从行首开始，拼写变长也不移动。
- 写进输入框的形式：拼音分词或 Engine 的 `preedit`。九键写 Engine 给的拼音读音（`nine_key_reading`，`ni'hao`），绝不写数字串（`64426`）；读音为空时才退回 `preedit`。全拼 26 键的 `preedit` 是原始按键，所以输入框里是 `nihao` 而不是 `ni'hao`，View 目前没有分好词的拼音字段。
- 例外照旧：已选定的半个词（`phrase_prefix`）和本地输入模式的名字仍画在候选栏；全拼 14 键的组码（`buguo`）不写输入框，读音仍单独一行；日语假名读音、韩语音节等就地组字照各自原来的规则。HarmonyOS 的编辑框不支持预上屏文字时，读音仍画在候选栏，否则拼写在屏幕上无处可见。
- 设置：iOS 的「行内预编辑」「候选栏预编辑」两项去掉，`InlinePreeditPreference` 只保留几种写入形式的定义；共享设置页 `PreeditSettingsSection` 在触屏宿主（`mobile`）上只剩「双拼预编辑」。共享偏好 `candidate_preedit_style`、`tsf_preedit_style` 字段不动，桌面端和 HarmonyOS 2in1 照常读，触屏端不再读。
- 顶栏高度不变：iOS 读音段收成零宽、仍是 42pt；Android 和 HarmonyOS 手机原来就是读音单独一行，这一行的高度保留（页码、提示、漢键等也在这一行），只是不再画拼写。

这一篇取代 [读音并入候选行](2026-10-13-ios-inline-reading-top-row.md) 里「26 键等布局把读音画在候选行最左边、最多三成宽」这一条；该篇的顶栏高度（42pt、14 键单独读音行）仍成立。

## Alternatives considered

- **读音段只长不缩**：起步四五个字母宽，按档位加宽、退格不缩、上限三成宽、超出截开头。不加高度，但只是让跳动变少（对比里 9 次降到 2 次），没有消除。
- **细读音行加候选行（国内输入法的做法）**：读音 11pt 压在候选上方，候选永远不动，空闲工具栏用同一高度、键盘不跳。代价是顶栏比现在高 4pt，开翻译时高 8pt（42→50）；产品方选了零高度代价的写进输入框。
- **保留开关、默认关**：顾虑是第三方 App 对键盘扩展 marked text 的画法参差不齐；但仓库里没有任何具体 App 出问题的记录，只是推测。产品方要求不留开关。

## Consequences

- 收益：候选位置固定，开不开翻译都不加高度；三端触屏行为一致，iOS 与 Android 原本就写输入框的做法对齐。
- 代价：拼写只在输入框里可见，依赖宿主正确画组字文本；画不好的 App 里没有退路（HarmonyOS 不支持预上屏时除外）。
- 代价：iOS 候选栏的读音菜单（改拼写位置）随读音一起不见，只有空格拖动等移进拼写后才画出光标所在的拼写。
- 代价：打字本身就改动了文档，iOS 的回声窗口在每次按键后生效，窗口内宿主改动（例如用户点到别处）会当作自己的回声，要等窗口过去才结束组字。
- 遗留：全拼 26 键写原始按键（`nihao`），九键首选是英文词（`65` 出 `ok`）或没有读音时退回含数字的 `preedit`；两者都要 Engine 在 View 里给分好词的拼音或补九键英文行的读音才能彻底解决。

## Verification

- iOS：新增 `InlineCompositionDefaultTests`（全拼逐键打 `nihao` 输入框里是拼写、候选栏不画读音、第一个候选不移动；九键写进输入框的不含数字），改写顶栏布局测试为读音段零宽、候选贴行首且长拼写不移动，其余按读音读组字的测试改读读屏值。测试在 Studio 上 `build-for-testing`、拉回本机 iOS 27 模拟器运行。
- Android：`check-host.sh` 通过（231 项 JVM 冒烟，含重写的 `CandidatePreeditStylePolicySmoke`：九键 `64426` 配 `ni'hao` 写出 `ni'hao`），设备用例只编译、未在设备上运行。
- HarmonyOS：`tests/run.sh`（keyboard-logic 4151 条断言，含九键写读音不含数字）、51 个 `scripts/test-harmony-*.py` 和 `hvigorw assembleHap` 通过，未在设备上运行。
- 共享设置页：`apps/desktop` 的 settings 测试 354 个文件、2101 项通过，`tsc --noEmit` 通过。

## Related notes

[读音并入候选行](2026-10-13-ios-inline-reading-top-row.md) 被本篇部分取代（见上），双方互链；[组字光标点按](2026-10-08-composition-caret-tap.md) 的点读音移动光标现在只在光标已离开末尾、拼写画出来时可用；[全拼 14 键](2026-10-10-fourteen-key.md) 的读音行与拼音选择条不变；[预编辑偏好类型](../simplification/2026-10-15-preedit-preferences.md) 的字段与类型不变，只是触屏端不再显示其中两行。

#9016 的临时放大编辑、输入框组字内点击与右侧偏移手柄由[Android 拼写放大编辑](2026-10-21-android-composition-magnifier.md)接手；引擎光标和触屏拼写入框规则继续保留。
