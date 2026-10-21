# Agent Note: 方向键按候选窗朝向和首选分工

Status: implemented

## Problem

`navigation.left_right` 默认开启以后，macOS 上用左右键选词的手感不对：→ 选到第二个候选后按 ←，期望回到首选，实际是拼音光标往左移了一格，高亮留在第二个。原因是共享 runtime 的规则只看光标位置：光标没到要移动的那一端就编辑输入串，到了才移高亮。选词时光标停在末尾，← 永远先去编辑，必须把光标一路移到开头才轮到移高亮。

宿主在偏好关闭时的回退分支（`InputController.mm` 里拼音普通候选那一段）也有两处同类问题：它把「当前页的第一个」当成首选，翻到第二页后 ← 去编辑拼音；用 ↓ 离开首选而光标停在中间时，→ 仍去编辑拼音。

横排里高亮离开首选后，←/→ 只移动候选，用户要在第三个候选上改拼音就没有直接的键。原本的 Ctrl+左右在全拼、双拼里发的是逐个字母的光标命令（`MSIME_MOVE_LEFT` / `MSIME_MOVE_RIGHT`），runtime 只看命令号，同样把它换成了移动候选；而 macOS 默认把 Ctrl+左右分给调度中心切换桌面，按键根本到不了输入法。

另一个问题是朝向：这条规则横排竖排都一样，于是竖排候选窗里 ←/→ 和 ↑/↓ 都能移高亮，← 跳到上面一项，按键方向与候选的排列方向对不上。

## Decision

方向键沿着候选的排列方向移动高亮，垂直方向的键编辑输入串。三个桌面平台用同一条规则，判断都在共享 runtime 里。

横排候选窗里左右键按高亮分工：

- 高亮在首选、光标在末尾：← 编辑输入串，→ 选下一个。
- 高亮在首选、光标不在末尾：← 编辑输入串，→ 把光标往末尾送，到了末尾以后才选下一个。
- 高亮不在首选：← 选上一个，→ 选下一个，不再编辑输入串。离开首选的那一下光标就在末尾；用 ↓ 离开首选时光标可能在中间，这时左右键也只移动候选。

首选指全部候选里的第一个（runtime 的 `highlighted == 0`，宿主是 `page == 0` 且当前页高亮在第一个），翻到后面一页的第一个不算。

竖排候选窗里 ←/→ 只编辑输入串，高亮由 ↑/↓ 移动。已打开的候选列表（韩文汉字、注音）不看朝向，四个方向键照旧都移高亮。

高亮不在首选时改拼音用修饰键：组字时 Shift+←/→ 和 Ctrl+←/→（只按这一个修饰键）在所有方案里按音节移动拼音光标，停在哪个候选上都一样。Shift+Backspace 不在其中。没有组字时这两个组合键照旧是应用的选择文字和按词移动。锁光标的方案（韩文、注音、越南文、藏文）照旧不接管。

Home 分两段：候选窗开着时，高亮不在首选就回到首选，已经在首选才把拼音光标移到开头。End 不变，仍跳到末选。整句改字进行中 Home 一直只回到首选：改字的光标不在输入串里，`MoveHome` 会让引擎退出改字、丢掉已经钉住的段，而刚进改字时高亮就在首选，按一下 Home 就会把改好的句子扔掉。

实现：

- `Runtime::caret_edge_candidate_shortcut` 按上面的规则决定这一次左右键是编辑还是选词，`Runtime::set_vertical_candidates` 记录候选窗是否竖排，竖排时不接管。整句改字、已打开的候选列表、当前页没有候选这三条不接管的边界不变。
- 朝向由 host-api 推给 runtime（`host_vertical_candidates`，创建会话和每次偏好更新两处），与 `set_left_right_navigation` 同一条路。桌面宿主（`HostPlatform::is_desktop`）按偏好 `candidate_layout` 判断；触屏宿主的候选栏总是横排，不读这一项，因为它的缺省是竖排，照读会让 Android、HarmonyOS 外接键盘的左右键不再选词。macOS、Windows、Linux 实际画候选窗用的也是同一个 `candidate_layout`（Linux 的临时覆盖同样写进传给会话的偏好），runtime 看到的朝向与屏幕上的一致。
- macOS 在偏好关闭时自己的回退分支用同一条规则，竖排时落到普通的光标命令；双拼按音节移动的那条分支不变。
- Windows TSF 靠「高亮变了、预编辑没变」决定重画候选窗，Linux 两个前端也只把左右键发成光标命令，规则换了不用改它们的代码，只更新注释。
- 分段组合键发宿主已有的 `MSIME_MOVE_LEFT_SEGMENT` / `MSIME_MOVE_RIGHT_SEGMENT`。runtime 把它们交给引擎按分段边界重复移动字母光标，从不经过 `caret_edge_candidate_shortcut`，所以不需要新的光标命令。四个宿主路径（macOS、Windows TSF 与 Server、Fcitx5、IBus）都去掉了「全拼、双拼的 Ctrl+左右换成逐个字母命令」的特例，C ABI 文档的整句改字一段随之改写。Shift 单击切中英在三个平台上都是「Shift 按住期间有别的键就不算单击」：macOS 的 `ModifierTap.h` 在收到按键时解除，Fcitx5 和 IBus 在走到分段分支之前就把这次按键记为组合。
- Windows Server 路径原来没有 Home、End 和组字时 Shift+左右的路由：`ReplyComposer::configured_key` 返回空回复，`SessionPump` 把它当派发失败，结束这个客户端的处理循环。现在组字时 Home、End（不带修饰键或只带 Shift，与 TSF 的分类一致）在 Server 视图有候选时发与 TSF 相同的命令（106、`MSIME_LAST_CANDIDATE`），没有候选时回空操作、不动会话；Shift+左右走分段命令。
- Home 用新命令 `MSIME_FIRST_CANDIDATE_OR_HOME`（106），由 `Runtime::home_key_action` 换成 `FirstCandidate` 或 `MoveHome`。不直接改 `MSIME_FIRST_CANDIDATE`（104）：日文转换用它从末选循环回首选，只有一个候选时高亮本来就在首选，换成 `MoveHome` 会去动读音的光标。宿主只在 Home 键上发 106。
- 共享设置页速查：左右键那一行叫「移动候选项（横排候选，首选上先编辑输入串）」；桌面加一行「按音节移动输入串光标 Shift + ← / →」，Home/End 那一行写明已在首项时 Home 把光标移到输入串开头。触屏宿主的外接键盘没有这两项，不显示。

## Historical audit

- [整句改字](../feature/2026-10-10-sentence-conversion-edit.md)：部分取代。它定下「三个桌面平台的方向键不看候选窗横排竖排」，本篇在 `navigation.left_right` 开启时让左右键按朝向分工；它要的跨平台一致仍然成立，因为判断在共享 runtime 里，不回到 2026-10-10 之前只有 macOS 按朝向判断的做法。
- [左右键在输入串两端移动候选](../feature/2026-10-16-left-right-candidate-navigation.md)：部分取代。它的「编辑优先，光标到端点才移高亮」语义由本篇改成「横排里首选上编辑，其余候选上选词；竖排只编辑」；判断放在 runtime、宿主只发光标命令、三条不接管的边界、界面位置这些决定保留。
- [默认启用输入串两端的左右候选导航](2026-10-20-issue-8847-left-right-default.md)：不冲突，默认开启和显式 `false` 落盘不变，它描述的「输入串两端」语义以本篇为准。注意 Windows、Linux 的 `candidate_layout` 缺省是竖排，那里默认安装的左右键因此只编辑输入串，#8847 说的末尾按 → 选词要在横排候选窗里才有。
- [macOS 拼音光标与候选方向键分工](2026-10-14-macos-arrow-candidate-navigation.md)：部分取代。宿主回退分支原来的「← 在当前页第一个上编辑、→ 光标没到末尾就编辑」收紧成本篇的规则，竖排时不再选词。
- [整句改字](../feature/2026-10-10-sentence-conversion-edit.md) 的另一条「Ctrl+左右一个字母一个字母地编辑拼音」也由本篇取代：Ctrl+左右改为按音节移动，与 Rime、fcitx5 拼音一致。逐个字母移动只剩首选上的普通左右键（横排）和竖排里的左右键。
- [macOS 普通左右键编辑拼音光标](2026-10-14-macos-plain-arrow-pinyin-caret.md)：不冲突。macOS 普通左右键仍发字母光标命令，它说的「保留 Ctrl 分段编辑策略」现在对全拼、双拼也成立。
- [智能 ABC 双拼](../feature/2026-10-16-smart-abc-shuangpin.md)：只在验证里提到 IBus 用 Ctrl+左右测逐字母编辑，那条 smoke 测试已按新规则改为按音节。
- [翻页与以词定字共用偏好转换](../simplification/2026-10-15-candidate-paging-preferences.md)：无关，只引用了旧篇标题。

## Alternatives considered

- **保留编辑优先，只在高亮不在首选时把 ← 改成选上一个。** 改动最小，→ 在光标没到末尾时仍编辑，与原来的「光标还能动就先动光标」一致；但用 ↓ 离开首选时光标停在中间，→ 会去编辑拼音，而高亮停在某个候选上，接下来按空格上屏的是那个候选而不是编辑后的结果，按键的意思取决于看不见的光标位置。
- **只在 macOS 宿主接管，不改 runtime。** 只有 macOS 用户报了这个问题，改宿主不碰其它平台；但 `navigation.left_right` 是共享偏好，宿主自判会让同一个开关在 macOS 和 Windows、Linux 上意思不同，2026-10-10 统一方向键正是为了消除这种差别。
- **把当前页的第一个也当首选。** 翻到后面几页时 ← 可以直接回去编辑拼音；但用户在后面一页按 ← 是想回看上一个候选，进编辑就把高亮丢在后面一页，空格上屏的是看不见的那一个。
- **继续不看朝向。** 三个平台、两种朝向只有一套规则，最好记；但竖排里 ←/→ 选词和 ↑/↓ 选词重复，← 往上跳与键的方向相反，而竖排里左右键编辑输入串不跟任何别的键冲突。
- **竖排时 ←/→ 翻页。** 这是 2026-10-10 以前 macOS 竖排列表的做法，竖排里左右键也有了与候选相关的用途；但翻页已经有 Page Up/Page Down、减号等号、方括号等键，左右键留给编辑输入串，就不必为改一个字母去按 Ctrl+左右。
- **高亮不在首选时改拼音，mac 用 Option+左右。** Option+方向键在 macOS 文本里是按词移动，组字时现在会先把拼音原样上屏再交给应用；但没有主流输入法这样用。Rime 把 Shift+方向键当 Ctrl+方向键，鼠须管在 mac 上实际用的就是 Shift，三个平台都能用同一个组合键。
- **新增一对 runtime 不截走的逐字母光标命令，Ctrl+左右保持逐个字母。** 全拼、双拼里 Ctrl+左右的旧手感不变；但要扩 C ABI 的命令表、四个宿主路径都换命令号，而 Rime 和 fcitx5 拼音的 Ctrl+左右本来就是按音节跳，逐字母编辑在首选上的普通左右键已经有了。
- **两段式 Home 放在宿主里判断。** 不加命令号；但四个宿主路径要各读一遍高亮和页码，正是放在 runtime 里要避免的那种重复。
- **让宿主各自把朝向告诉 runtime（新增 C ABI 调用）。** 朝向以宿主实际画出来的为准，不必担心偏好和屏幕不一致；但三个桌面宿主画候选窗读的本来就是 `candidate_layout`，多一个调用只是把同一个值换条路传一遍，每个宿主还要在切换朝向时记得调用。

## Consequences

收益：选词时方向键的方向和候选的排列方向一致；高亮停在任何候选上都能用 Shift+←/→ 按音节改拼音，Home 两下回到首选再到开头；横排里 ← 和 → 只移动高亮，不会在用户看不到的地方动拼音光标；编辑拼音只发生在首选上，那时空格上屏的就是编辑后的首选。竖排里每个方向只有一种用途。

代价：横排里在别的候选上想逐个字母改拼音，要先回到首选（按 Home 或 ↑）再按 ←；Shift/Ctrl+←/→ 只能按音节跳。全拼、双拼里 Ctrl+左右从逐个字母改成按音节，习惯旧手感的 Windows、Linux 用户会察觉。竖排用户的 ←/→ 不再能选词，Windows、Linux 的 `candidate_layout` 缺省是竖排，这两个平台默认安装时 `navigation.left_right` 实际不起作用，要改成横排才有。速查组件拿不到朝向，那一行在竖排时照样显示，只靠标题写明「横排候选」。

验证：

- `cargo test -p msime-input-runtime` 329 项通过。`left_right_edits_only_on_the_first_candidate` 走完横排的四种情况和跨页的首个候选，修改前在「不在首选时按 ←」一步失败（光标从 7 变成 6）；`vertical_candidates_leave_left_right_to_the_input_string` 覆盖竖排；`home_key_returns_to_the_first_candidate_before_it_moves_the_caret` 覆盖两段式 Home，并确认 `FirstCandidate` 本身不分段。`home_key_keeps_sentence_conversion` 确认整句改字中 Home 不退出改字，去掉判断时失败。`msime-host-api` 的 `left_right_navigation_preference_reaches_real_sessions_and_live_updates` 用真引擎验证横排首选末尾 → 选下一个、偏好关闭和改成竖排后都恢复光标命令。`cargo fmt --check`、两个 crate 的 clippy `-D warnings` 通过。
- macOS `shortcut-test` 在本机图形会话中运行通过：偏好开启时交给 runtime、关闭时宿主回退分支横排按规则选词、竖排只编辑（含第二页首个候选）；全拼和五笔里 Ctrl/Shift+左右发分段命令，Shift+Backspace 仍是普通退格，没有组字时 Shift+左不发分段命令；候选窗显示时 Home 发 106。
- Linux：`platforms/linux/build-container.sh` 编译 IBus 与 Fcitx5 两个前端，94 项 ctest 通过（1 项原本就跳过），`fcitx5_contract.py` 的新断言固定了分段分支、Shift 组合标记的先后顺序和 Home 命令号。`fcitx5-native-test` 与 `ibus-engine-smoke` 里新增的按键断言只编译过，没有执行到：两个测试要词库资源，门禁和 CI 都不带资源运行；带资源运行时，它们分别在更早的 `Disabled expression mode must pass through` 和英文录入的断言处失败，在 PR 基线提交上用同样方式运行失败相同，是已有问题。
- Windows：`input_key_policy`、`edit_policy`、TSF 的 `host_composition` 用 clang++ 编译运行通过；Server 侧改动文件和测试用 MinGW `-fsyntax-only -Wall -Wextra -Werror` 通过，TSF 四个文件语法检查无错误；`test-windows-deferred-classification.py` 217 项等六份脚本通过。`reply_composer` 和 `session_smoke` 的真实会话断言（分段边界 0/2/5、两段式 Home、End 跳到末页末选）要链接 Windows 版 Rust 库，由 PR 上的 CI 运行。
- 共享设置页的速查与翻页相关 vitest 通过。
- 没有在任何平台的真机上按过这些键。
