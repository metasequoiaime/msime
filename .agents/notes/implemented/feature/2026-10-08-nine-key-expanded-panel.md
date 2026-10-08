# Agent Note: 九宫格展开面板的拼音列、撤销锁定与候选筛选

Status: implemented

## Problem

用户给了一段竞品九宫格的录屏，要 Android、iOS、HarmonyOS 照着做。录屏里的交互有四处是我们没有的：

- 展开候选后，键盘区变成三栏：左边仍是拼音列，中间是候选，右边是「返回 / ⌫ / 重输 / 拼音·笔画 / 全部·单字」。
- 在展开面板里点左栏的拼音（`ning`），只锁定这一个音节，左栏换成下一段的选项（`bai cai ba ca a`），面板不收起。
- 拼音全部锁定后（`ning'bai`），左栏仍是最后一段的选项，可以换选；这时按 ⌫ 先撤销刚选的 `bai`，回到 `ning'224`，再按才删数字。
- 拼音列末尾有这个数字键上的字母和数字本身（`M N O 6`）。

原来三端都只有单栏的展开面板，选拼音或退格都会把它关掉（HarmonyOS 的 ∨ 甚至打不开它，是 #2503 留下的）；引擎在拼音全部锁定后左栏为空，退格永远删数字，没有任何候选筛选。

## Decision

- 「哪些拼音可选、选了之后怎样、退格先撤销什么、怎么筛选」都是输入算法，放在 `crates/engine/src/nine_key.rs`，三端只负责画和转发点击。
- `View.nine_key_spellings` 在全拼九键下依次是：完整音节（小写）；下一个数字键上能起头一个音节的字母，大写（`M`、`N`、`O`；`i`、`u`、`v` 不起头任何音节，不列）；最后是这个数字本身，只在没有锁定的音节时出现。大写是为了和同形的音节 `o`、`a`、`e` 区分开，宿主照原样显示。选择仍走 `msime_client_choose_nine_key_spelling`：
  - 音节：照旧锁进数字。
  - 字母：记成 `initial`，只限定下一个音节的首字母，不是音节边界。`64` 选 `M` 之后候选只剩 `m` 开头的读法，预编辑显示 `m4`，左栏只剩 `m` 开头的音节。
  - 数字：直接上屏这一位，剩下的数字继续组字。前面有锁定时不提供，因为锁定的音节还没上屏，先上屏数字会颠倒文字顺序。
- 每次锁定都记一条 `LockUndo`：这段数字锁定前的样子（拼写比键入长时，锁定会补齐数字）、锁定时丢掉的切分、并进拼写的首字母，以及锁定时左栏的选项。
  - 数字全部锁定时，左栏是最后一条记录里的选项，选其中一项先撤销那次锁定再锁新的，即换选。
  - 退格依次撤销：选完之后没再动过的首字母；数字全部锁定时的最后一次锁定（数字和切分都还原）；末尾的切分；都没有才删数字。
  - 锁定之后还有没锁定的数字时，退格删的是数字而不是锁定，这和录屏一致：`ning'bai` 退两次是 `ning'22`，不是 `6464224`。
  - 部分上屏（选了只覆盖前几个数字的候选）之后，剩下的锁定记录作废，因为数字位置变了。
- 新增 `msime_client_set_nine_key_filter(session, single_character, strokes, length)`，经 input-runtime 的 `Action::SetNineKeyFilter` 走与按键相同的 dispatch。
  - `single_character` 只留单字。
  - `strokes` 是 `hspnz` 组成的笔顺前缀，比较候选的第一个字：从 `msime-stroke.db` 按键前缀做一次区间查询，取出全部单字放进集合，换前缀时才重查。
  - 筛选在截断到 128 条之前做，否则排在后面的单字会被截掉；英文词在有筛选或首字母时不列。
  - 当前状态由 `View.nine_key_single_character` 和 `View.nine_key_strokes` 给出，组字结束时清空。
  - 宿主没有笔画字典时报 `LANGUAGE_DICTIONARY_UNAVAILABLE`，筛选不变。
- 三端的展开面板在全拼九键组字时画成三栏，其他布局保持原样。
  - 左栏：「拼音」模式下是同一份 `nine_key_spellings`；「笔画」模式下是横竖撇点折五个键，上方用一丨丿丶乛显示已选的笔顺。
  - 面板里选拼音、⌫、改筛选都重新取一次 `allCandidates`，面板保持打开；只有选中候选、点「返回」、或组字结束时才关。
  - 笔画模式下 ⌫ 先删一笔，没有笔画才是普通退格。
  - 切回拼音模式会清掉笔画；关闭面板会清掉全部筛选，收起后的候选栏不带筛选。
  - 判断逻辑放在各端可测的地方：Android `keyboard/NineKeyPanelPolicy.java`，HarmonyOS `keyboard/input/NineKeyPanelPolicy.ts`；iOS 的左右两栏在 `KeyboardNineKeyPanelColumns.swift`，由 `NineKeyKeyboardTests` 驱动整个视图控制器来测。
  - iOS 的三栏面板只盖住键盘区，候选栏留在上面，展开按钮变成收起；没有打包 `msime-stroke.db` 时不显示「笔画」切换。
- Android 的拼音选择从横条改成侧栏里的竖列，和另外两端以及录屏一致；注音九键仍是候选行上的横条。Android 的「展开」按钮在九键组字只有一页候选时也显示，因为面板现在还是选拼音和筛选的入口。
- 读音行显示 `nine_key_reading`（`ning'bai`）而不是数字。HarmonyOS 和 iOS 原来显示的是数字，这次补上了。

## Alternatives considered

- **宿主自己维护锁定历史，退格时用 `Cancel` 加重放数字和选择来实现撤销**：不用改引擎，三端各写一份。但重放要求宿主记住每次选择时的列表下标和 generation，三份实现迟早漂移，而且违反 ARCHITECTURE.md 的「输入算法归引擎」。
- **退格始终先撤销最后一次锁定，不管后面还有没有没锁定的数字**：语义更整齐（「撤销上一步」）。但录屏里第二次 ⌫ 删的是数字、`ning` 留着；用户选了 `ning` 之后接着往下打，退格应该删刚打的数字，不应该把前面选好的音节翻掉。
- **按键字母锁成一个只有一个字母的音节（`m'464224`）**：可以直接复用锁定机制。但数字是按音节切的，`m` 之后的 `4` 会被当成下一个音节的开头，`ming` 这种读法就拼不出来了。所以字母只记成首字母限定，下一个音节的边界照旧由数字决定。
- **筛选在宿主做，只过滤已经取到的候选**：单字筛选看起来不需要引擎。但引擎只返回前 128 条，长输入时单字大多在 128 条之外；笔画筛选还要查笔画字典，宿主拿不到。
- **笔画筛选比较候选的每一个字**：对词语更严格。录屏和常见实现都按首字筛，用户心里想的是「这个字怎么写」，按首字也和笔画方案选单字的直觉一致。

## Consequences

- **收益**：三端共用同一套锁定、撤销和筛选语义，调整只改引擎；golden 和单测覆盖了录屏里的每一步（`backspace_takes_back_the_last_lock_only_while_every_digit_is_locked` 等）。
- **代价**：`nine_key_spellings` 不再只是音节，宿主不能再假设每一项都是拼音（无障碍文本按大写字母、数字分别读）。golden 里有 10 个场景的 `nine_key_spellings` 随之变长。
- **已知限制**：
  - 部分上屏之后不能再撤销剩下的锁定。
  - Android 在笔画字典缺失时，点第一笔会显示引擎的诊断并收起面板（iOS 在这种情况下直接不显示「笔画」切换）。三端都随包带着 `msime-stroke.db`，这条路径正常不会走到。
  - 三端都没有在真机上用手指走过一遍；Android 的设备冒烟 `NineKeyPanelDeviceSmoke` 能编进测试包（见 [2026-10-08-compile-gates-for-device-suite-and-arkts.md](../testing/2026-10-08-compile-gates-for-device-suite-and-arkts.md)），但还没有在模拟器上跑过。

## Verification

- 引擎：`cargo test -p msime-engine --lib nine_key`，以及 `cargo test -p msime-engine --test golden`。
- C ABI：`cargo test -p msime-host-api nine_key`（`nine_key_filters_cross_the_host_boundary`）。
- Android：`bash platforms/android/check-host.sh`（`NineKeyPanelPolicySmoke`、`NineKeyLayoutSmoke`），以及 Gradle `compileFullReleaseJavaWithJavac lintFullRelease`。
- iOS：`MSIMEClientTests` 全量（Keyboard 550、Service 69、Shared 55，全部通过；1 个原有的跳过），包括录屏流程、两种筛选和 `InputBridgeResponseTests` 的新字段解码。
- HarmonyOS：`bash platforms/harmony/tests/run.sh`，以及 `hvigorw assembleHap`。
