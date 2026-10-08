# Agent Note: Android 成对标点自动补全

Status: implemented

## Problem

#5608：设置「表达 › 标点」里「自动补全成对标点」开着，Android 键盘上输入括号、引号仍然只上屏半个。追下去，Android 宿主从来没有读过共享偏好 `paired_punctuation`：Engine 一次只上屏一个标点、补后半个是宿主的活（`crates/engine/src/punctuation.rs` 的模块注释），Windows、Linux、macOS、iOS、HarmonyOS 宿主各自补，Android 一处都没有，开关只是写进了偏好文件。用户截图里点的是符号面板，那条路不经过 Engine，同样没有补。用户还希望长按时只输入半个。

## Decision

- 键盘标点键（中文标点经 Engine 的那条路）沿用 iOS 宿主的规则，放在无 Android 依赖的 `PairedPunctuationPolicy`：Engine 的上屏以（、【、《、〈、“、‘ 结尾时补对应的后半个；开关打开时把 Engine 交替出来的后引号改回前引号，每次按引号键都开一对新的；补的是书名号时调 `msime_client_balance_paired_punctuation_after_auto_close`（新增 JNI `balancePairedPunctuationAfterAutoCloseRaw`），否则下一次 `<` 会被当成嵌套的〈。
- 后半个用 `InputConnection.commitText(closing, 0)` 写：第二个参数不大于 0 时按新文字的开头算，光标就停在后半个前面。选区预期按 `SelectionEchoTracker.commit(0)` 记，迟到的回声不会被当成光标移动。
- 补上的后半个记进 `PairedPunctuationPolicy.Stack`（与 iOS 的 `PairedPunctuationStack` 相同：最多 16 层，按输入框区分）。没有组字时再按 `)`、`]`、`>`、`"`、`'`，光标后正好是记录里最里层那个后半个，就跨过它而不是再写一个；跨过的做法是在一次批量编辑里删掉光标后的那个字再原样上屏。读不出光标后的文字时放弃记录、照常输入，因为删的可能不是那个后半个。删除、用户移动光标（`onUpdateSelection` 里不是自己写入的回声）、换输入框、关掉开关都清空记录。
- 符号面板不经过 Engine，由宿主按同一个开关决定：轻点 `PairedPunctuationPolicy.symbolClosing` 列出的前半个（中文括号、书名号、〈「『〔〖、全角 ＜｛［、中文引号“‘、ASCII 的 ( [ { ）时整对上屏、光标在中间；长按只上屏这半个，读屏描述里写明「长按只输入这半个」。符号面板补上的后半个同样进记录，之后在键盘上按 `)` 可以跨过。面板里前后半个挨着摆，在面板里轻点和最里层记录完全相同的后半个（`Stack.stepOverSymbol`）也跨过，否则点「补出「|」、打字后再点」会得到「内容」」；轻点的是别的后半个时记录作废、照字面上屏，点的不是后半个时记录不动，长按后半个总是照字面上屏。
- 符号面板里 ASCII 的 `<` 不成对（#5762 合入后的后续调整）：它绝大多数时候是小于号，轻点补成 `<>` 后打 `a < b` 会多出一个 `>` 要删。全角 ＜ 和 〈《 照常成对。
- 英文模式和英文标点下键盘上的 ASCII 括号不补，与其他宿主一致（它们只补 Engine 给出的中文前半个）。

## Alternatives considered

- **整对一起 `commitText("（）", n)` 一次写入**：一次 IPC，看上去更原子。但 `newCursorPosition` 大于 0 时从末尾减一算起、不大于 0 时从开头算起，让光标停在倒数第一个字符前正好需要 0，而 0 落在「从开头算」那一支，算出来是整对的前面。分两次写、后一次用 0 才对得上。
- **跨过后半个时用 `setSelection` 或发送右方向键**：`setSelection` 要绝对位置，这个宿主只在 `SelectionEchoTracker` 里推算位置，推算失效时没有可靠的值；发方向键在部分编辑器里会被当成导航、跳出输入框。删后再写只用相对位置，结果确定。
- **像 HarmonyOS 那样只补、不做跨过**：实现少一半。但每次按引号键都开新的一对，没有跨过就根本打不出后引号；习惯自己输入后半个的人会得到（内容））。
- **符号面板里 ASCII 的 `<` 也成对**：#5762 最初就是这样做的，理由是它和 ( [ { 同为 ASCII 的前半个，HTML 标签和泛型尖括号也确实要成对。但面板上的 `<` 更多是比较运算符，补出的 `>` 需要手动删掉；真要尖括号的场合多半在代码编辑器里，那边有自己的补全，产品上决定不补。
- **符号面板里 ASCII 的 `"`、`'` 也成对**：代码编辑器常这样做。但它们前后半个是同一个字符，`'` 更常是撇号，点一下得到两个反而要删，所以只让中文引号成对。

## Consequences

- **收益**：设置里的开关在 Android 上第一次真正生效，键盘和符号面板两条路一致；补全和跨过的规则能在 JVM 上测（`tests/keyboard/PairedPunctuationPolicySmoke.java`），`check-host.sh` 加了守卫，防止宿主再次不读这个偏好。
- **代价**：跨过后半个是删后再写，个别会记录撤销历史的编辑器里会多一步撤销；读不出光标后文字的编辑器里不跨过，用户会得到两个后半个。只在 `check-host.sh`（JVM 冒烟、NDK 编译 `client_jni.cpp`）上验证，没有做原生构建，也没有在真机或模拟器上按过。
