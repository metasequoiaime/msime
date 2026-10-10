# Agent Note: 全拼双拼整句改字，左右键在汉字之间移动

Status: implemented

## Problem

全拼、双拼打一长串，首选整句里常常只有一两个字不对（「我去背景」想要「我去北京」）。候选里没有对的整句，而现有的两条路都不好走：从左往右一段段选词，前面本来对的部分也得重新选一遍；把字母光标移到中间（`MoveLeft`，[组字光标](2026-10-08-composition-caret-tap.md)）只会按光标前的拼音出候选，选中就上屏那一段，整句的其余部分保不住。PC 用户要的是把光标移到错字上、空格换掉那一个字或词，句子其余部分不动。

## Decision

- 引擎（`crates/engine/src/session/conversion.rs`）加整句改字。首选是一个字对一个完整音节、覆盖整个组字、至少两个字的句子（整句候选或词库整词）时，新命令 `ConversionLeft` 进入改字：整句按首选的词切段，光标停在最后一个字前；之后 `ConversionLeft` / `ConversionRight` 每次移一个字，最远到句末。改字期间组字原文不变，字母光标停在末尾。
- 光标处的候选是从光标所在音节开始的字和词：先是光标所在那一段（光标在词中间时从光标到词尾），当前转换的文字排第一，再是更短的段、更长的段，同一长度按权重。词条来自规范全拼的词网格跨度查询（`QuanpinDictionary::conversion_rows`），双拼共用，因为整句的读音本来就是规范全拼。
- 选中候选把这一段钉住，句子其余部分用同一套词网格、n 元表和个人模型重新转换，不能跨过钉住的段（`lattice::decode::decode_pinned`），光标移到这段后面。语义与注音编辑器的 pin 相同：没钉住的部分可能因为新的上下文改变。重新转换解不出路径时，碰到钉住段的段拆成单字，其余原样保留。
- 光标在句末时空格（没有候选时的 `CommitCandidate`）上屏整句；回车（`CommitRaw`）、标点、`finish` 和失去焦点上屏的是改好的整句，不是拼音字母。什么都没钉住时上屏与选中首选完全相同；钉住过时整句当作一条整句候选上屏：存成个人词条（不超过 `MAX_LEARNED_SENTENCE_SYLLABLES`），按句中的词记入个人上下文。
- Esc 和退格只退出改字回到拼音，组字和原来的候选都在，钉住的段丢掉；键入字母、撇号、滑行、字母光标命令（`MoveLeft` 等）、宿主直接放字母光标，都先退出改字再照常处理。数字和空格不退出，留给候选选择。改字的候选不能置顶、删除、固定位置，也不能以词定字。
- 进不了改字时（读音不完整、单字、字母光标在中间而首选只覆盖光标前、其他方案、本地模式、专用英文、九宫格），`ConversionLeft` / `ConversionRight` 就是 `MoveLeft` / `MoveRight`。移动端仍发 4、5，行为不变。
- 接口：host 命令 `MSIME_CONVERSION_LEFT = 17`、`MSIME_CONVERSION_RIGHT = 18`；View 加 `conversion`（改好的整句，不在改字时为空）、`conversion_focus_start`、`conversion_focus_end`（按 Unicode 标量计的光标和焦点段）。runtime 在改字时不对候选做整句重排和读音降位。
- 桌面宿主：全拼、双拼里不带修饰键的左右键发改字命令，Ctrl+左右改为一个字母一个字母地编辑拼音（原来按音节跳，其他方案不变）；改字期间行内画 `phrase_prefix` + `conversion`（Windows 是它自己的造词前缀），不管预编辑样式，光标在焦点字前，焦点段单独标出。三个桌面平台的方向键是同一套规则，不看方案和候选窗横排竖排：组字时 ↑/↓ 移候选高亮，←/→ 移光标或改字；韩文汉字列表、注音列表打开时组字里没有光标，四个方向键都移高亮（←↑ 上一个、→↓ 下一个），不看导航开关，这是 Windows `korean_hanja_key` 原有的行为。macOS 原来按候选窗朝向决定（横排 ←/→ 移高亮、竖排列表 ←/→ 翻页），Linux 列表打开时 ←/→ 会把韩文音节写出去，都改成与 Windows 相同。

## Alternatives considered

- **macOS 只在全拼双拼下改方向键，其他方案保留按候选窗朝向决定**：五笔、日文等方案的 macOS 用户手感不变。第一版就是这样合进去的，但结果是同一个方案在 Windows、Linux 和 macOS 上方向键各做各的，用户明确要求所有平台操作一致，于是统一成 Windows 的规则。
- **保留左右键编辑拼音字母，用 Shift+左右或新偏好进入改字**：不改任何现有按键，老用户的习惯不受影响。但 Shift+左右不好被发现，偏好开关要用户先找到才知道有这个功能；用户选择让左右键直接按汉字移动，字母编辑挪到 Ctrl+左右。
- **不加新命令，直接改 `MoveLeft` / `MoveRight` 的语义**：宿主一行不改。但 Android 点读音行、iOS 空格拖动都靠连发 4、5 移字母光标，改了语义就把移动端的拼写编辑弄坏了。
- **字母光标停在音节边界时，候选改成光标后那一段、选中替换而不是上屏**：不需要汉字光标，读音行照旧。但这推翻了现有光标前缀解码的行为和它的基准（`qp_caret_editing`），而且用户看着拼音很难对准是哪个字错了。
- **钉住一段后其余部分不重新转换，原样保留**：最可预期，改一个字不会动别处。但改对一个字往往正是让相邻的字也该变（「背景」改成「北京」后的搭配），注音编辑器已经是钉住加重新转换的语义，两套编辑行为不一致更难解释。
- **退格删掉光标前的那个字连同它的拼音**：与旧版微软拼音一致。但要从字反推原文里的字母范围，模糊音、纠错和双拼都让这个映射不可靠；退回拼音是安全、可恢复的。

## Consequences

- **收益**：长句里错一两个字时，左键移到错字上、选对的字或词，其余部分保留，回车或空格到句末上屏。整句学习和个人上下文照常吸收改过的句子，下次同样的输入更可能直接对。
- **代价**：桌面上全拼双拼的左右键和 Ctrl+左右行为变了，习惯用左右键改拼音字母的用户要改用 Ctrl+左右；macOS 横排候选的左右键不再移高亮，所有方案都改由上下键移高亮；macOS 竖排的韩文汉字或注音列表里 ←/→ 不再翻页。改字只覆盖一个字对一个完整音节的首选，简拼、未打完的音节和含非汉字的候选进不了改字。钉住后的重新转换可能改变用户没碰过的字。候选窗的读音行仍显示拼音。
- **验证**：见下一节。

## Verification

- `cargo test -p msime-engine`：`session::tests` 的改字用例（进入与按字移动、候选顺序、钉住整词与词中单字、重新转换、回车与标点上屏改好的整句并存成个人词条、Esc 和退格退回拼音、字母退出、进不了改字时退回字母光标、改字候选不能置顶删除固定、双拼），`lattice::decode::tests` 的 `decode_pinned`，`session::conversion::tests` 的切段和退路。
- `cargo test -p msime-input-runtime -p msime-host-api`：真引擎走 runtime 的改字流程（View 字段与 JSON、换焦点高亮复位、数字选择不上屏、句末上屏），host-api 的 17、18 编号。
- macOS：在 Mac Studio 上编过输入法本体、`text-client-test`、`shortcut-test`，`ctest -R '^text-client$'`（改字的行内整句、焦点子句、扩展区汉字的标量换算、句末光标）通过；`shortcut-test` 拉回本机在图形会话里运行通过（全拼的左右键发 17、18，Ctrl+左右发 4、5，五笔照旧按分段；横排候选在全拼下由上下键移高亮，五笔照旧）。没有装进系统、在真实应用里按过。
- Linux：`platforms/linux/build-container.sh` 在容器里编过 IBus engine 和 Fcitx5 插件，77 项 ctest 全部通过（`tests/input/input_schemes.cpp` 覆盖哪些方案和模式走改字、位置换算）；`tests/core/fcitx5_contract.py` 钉住两个前端的方向键路由和改字时的行内显示。没有跑 `check-container.sh` 的真实 daemon 验收，IBus 双下划线和 Fcitx5 高亮的实际画法没有看过。
- Windows：只做了 `x86_64-w64-mingw32-g++ -fsyntax-only` 检查改过的 TSF 与 Server 源文件，以及 `scripts/test-windows-native-run.py` 里不需要 Windows 构建的那部分（`input_key_policy`、`input_scheme_traits`、`host_composition` 通过，`reply_composer` 未运行）。没有交叉链接，没有在 Wine 或 Windows 上运行。Windows 上同一个键同时送进 TIP 和 Server 两个引擎会话，改字时空格、数字、回车、Esc 在两边是否保持一致只经过读代码确认，是这次改动最大的未验证点。
