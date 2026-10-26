# Agent Note: 移动光标后编辑光标后的部分

Status: implemented

## Problem

组字光标停在拼音中间时，现有的[光标前缀解码](../bug-fix/2026-10-16-runtime-caret-candidate-generation.md)只重新解码光标前那段原文，候选是光标前那段的字词，选中就先交出光标前那段，光标后的部分要等之后再转换。另一种常见的用法正好相反：用户打完一长串，发现错在后半段，想让光标前面那段保持现在的转换不动，只换光标后面的部分。例如 `wohaoxianchihuoguo` 整句是「我好现吃火锅」，用户把光标移到 `wohaoxian|chihuoguo`，想要的是前半段固定为「我好现」，后半段从「吃货、痴货、尺蠖、吃……」里重新挑。现有两种做法都做不到：光标前缀解码给的是光标前那段的候选，[整句改字](2026-10-10-sentence-conversion-edit.md)要首选覆盖整个组字、按字移动，而且要用改字命令进入。

## Decision

新增共享偏好 `caret_edit_suffix`（「移动光标后编辑光标后的部分」，默认关），开着时在全拼和双拼里改变光标停在中间时的候选，关着时行为与以前逐字节相同。

### 生效条件

开关打开、方案是全拼或双拼、不在本地模式和专用英文里、组字不是字面原文，并且组字光标严格在组字中间（按光标前缀解码同样的音节边界取整后，光标前后都有完整单元）。光标在开头、在末尾，或光标前那段找不到一行覆盖它全部音节的转换时，退回光标前缀解码。规则在 `crates/engine/src/session/caret_edit.rs`。

### 候选

- 光标前那段原文固定为它当前的最佳转换：临时查询里第一行覆盖了这段全部音节的拼音行（`selection_completes_request` 按这段的临时请求判断）。它只显示，不上屏；行内仍是原文和光标，用户可以继续移动光标改变固定的范围。
- 候选是光标后那段原文当作整段组字时的候选，走同一条管线：临时查询带上模糊音、纠错和辅助码的设置，经过个人上下文重排和固定位置，runtime 的整句读法降位照常进行。
- 首选显示为「固定的那段 + 光标后那段的首行」。光标后那段的首行通常是覆盖它全部音节的整句，所以首选就是整句；首行不覆盖全部音节时（例如模糊音只查到词），首选也只是固定的那段接上这一行。
- 引擎快照里的候选仍是光标后那段的行，`SessionSnapshot::caret_edit_prefix` 另外给出固定的那段文字，runtime 只在视图里把它接到首位那一行前面（`displayed_candidate_text`）。这样重排、读法降位、释义和选择核对都按行本身的文字进行。

### 选择

选中第 N 条（数字、点选、空格、`finish`、标点结束）时，先把固定的那段按一次部分选择交出去：记个人上下文和造词进度，不调词频，也不算一次显式选择；组字随之只剩光标后那段。然后照常选中光标后那段的这一行：它只消耗自己的音节，引擎的上屏文字是「固定的那段 + 这一行」。剩下的拼写接着在本模式里转换：已经交出去的文字记在 `CaretEdit::held`，即使光标回到末尾，首选也仍显示为「已交出去的文字 + 剩余部分的首行」，提示照常显示。组字结束或删空时 `held` 清掉。

三个桌面宿主都请求 `phrase_preedit`，交出去的文字由 runtime 暂存在 `View::phrase_prefix` 里，整段完成时一起上屏：`wohaoxian|chihuoguo` 选「吃货」后组字区是「我好现吃货」加 `guo`，首选「我好现吃货锅」，再选首选上屏「我好现吃货锅」。

### 其他按键

- 退格、删除、插入字母照常编辑原文，固定的那段跟着重新转换；选过一个词之后，退格删到最后一个字母时按[暂存词段的规则](2026-10-16-backspace-selected-phrase.md)撤回上一次选择。
- Esc 丢掉固定的那段、已选的词和剩下的拼写（打开「Esc 保留已选词」时保留已暂存的部分）。
- 回车上屏原文；选过之后是暂存的文字接上剩下的原文，与部分选择的规则相同。
- 以词定字：固定的那段照样交出去，接上光标后那一行的首字或尾字，其余拼写丢掉，不学习。
- 再次移动光标改变固定的范围；runtime 在移动光标前推进候选代次，旧 ID 失效（既有规则）。
- 翻页照常；光标后只剩一个声母时展开的是光标后那段的列表（`expand_current_raw_range_initial_candidates`）。

### 不混入的行和在线候选

本模式里不混入英文、emoji、颜文字、符号、日文、连续多语言、快捷短语、行内日期时间、联系人和网址补全：选中这些行会结束整段组字，扔掉固定的那段或光标后剩下的拼写。云候选和 AI 只能回答整段原文，本模式里不发查询，进入模式之前发出的查询回包也不接。runtime 在本模式里不做每键重排、停顿重排和脚本插件变换，首位始终是引擎的首行。

### 学习

沿用部分选择的规则：光标后那段的选择在固定的那段交出去之后学习，调频的上下文就是光标后那段，与把它当作整段组字时相同；固定的那段只按默认转换记个人上下文，不调词频；整段完成时已选的各段按造词规则存成个人词条（例如「我好现吃货锅」）。学习关着时都不写。

### 视图契约

`View` 加两项，没有生效时都不出现在 JSON 里：`caret_edit_suffix`（模式正在生效）和 `caret_edit_prefix`（首选开头固定的那段，按 Unicode 标量数画淡）。说明写在 `crates/input-runtime/src/types.rs` 和 `msime_client.h`。

### 设置与宿主

- 偏好在 `crates/client-core` 的 `Preferences::caret_edit_suffix`，关着时不写进文档，缺键读作关。host-api 只在三个桌面宿主上交给 Engine（`CARET_EDIT_SUFFIX_HOST`），偏好随文档到了手机也按关；Engine 偏好照例等组字空闲时重建，组字中途开关不改变这一段组字。
- 共享 React 设置页（macOS、Linux）在「候选与联想 → 更多选项」开头、「快捷短语混输」之前列出它，按能力 `HostCapabilities::caret_edit_suffix`（三个桌面平台）显示；HarmonyOS 不列出。Windows 原生设置（WinUI 和 Win7 设置窗口）放在同一组的「更多选项」里「只出单字」之后，Windows 没有快捷短语混输那一行。手机设置不加。
- 候选窗只在模式生效时显示「候选编辑：光标后」，不能点，模式没生效时各宿主的画法和以前相同：
  - macOS（`InputController.mm` 的 `renderCandidates`）：首行右侧、翻页控件左边，与「1 / 3」同字号同次要色，算进卡片的最小宽度；自适应横排里放下它会把翻页键挤到一半以下时不画。首选开头固定的那几个字用释义色画（`MSIMECandidateButton::dimmedPrefixLength`），按 Unicode 标量数换成 UTF-16 长度，不切开代理对。
  - Windows Server 候选窗（`CandidatePresentation.h`、`CandidateWindow.cpp`）：首行右侧、翻页之前，页码字号和次要色，拼音在它左边截断；读屏树多一个文字元素。首选用 `IDWriteTextLayout` 把固定的那段画成编号色；繁体输出时前缀按同样的写法换算，对不上就不画淡。
  - Linux（`platforms/linux/src/core/CaretEditSuffix.h`）：IBus 把提示接在候选上方的辅助文字里，首选开头加前景色属性；Fcitx5 经典界面接在 `auxDown` 里，`fcitx::Text` 没有颜色格式，首选照常画；Fcitx5 原生画布在页脚右侧画提示，提示（和同一行的本地模式、连击）算进卡片的最小宽度，首选开头用编号色。两个宿主都把提示排在打字连击数之前。

## Historical note audit

检索活跃笔记的 `光标前缀`、`caret prefix`、`prefix_candidates`、`phrase_preedit`、`暂存词段`：

- [直接移动光标使旧候选身份失效](../bug-fix/2026-10-16-runtime-caret-candidate-generation.md)：部分重叠，沿用其移动光标前推进代次的规则，本篇在光标停在中间时换了一种候选；互链。
- [本地模式入口标记](../bug-fix/2026-10-17-local-caret-entry-marker.md)：无关，本模式不在本地模式里生效。
- [整句改字](2026-10-10-sentence-conversion-edit.md)：部分重叠，两者都在组字中间改转换；改字按字移动、要求首选覆盖整个组字，本模式按字母光标、固定光标前那段。本模式的初始状态下光标后那段的行不覆盖整个组字，进不了改字；只剩已交出去的文字时可以照常进入。
- [退格回退已选候选词](2026-10-16-backspace-selected-phrase.md)和 [Windows 部分选词交给 Runtime 暂存](../architecture/2026-10-17-windows-shared-phrase-preedit.md)：本篇复用它们的暂存、撤回和 Esc 规则，不改变它们。
- 光标前缀的性能笔记（`testing/2026-10-09-caret-prefix-request-reuse.md` 等）：本篇共用 `prefix_candidates` 及其缓存键，没有改变它们记录的分配行为。

## Alternatives considered

- **在候选窗里放一个可点的开关，随时切换两种光标候选**：发现性最好，用户不必去设置页。但候选窗的点击区目前只有候选、翻页和展开，加一个状态开关要在四个宿主的命中测试、读屏树和皮肤布局里各做一份，还会让同一次组字中途换一种候选、旧 ID 的处理多出一种情形。维护者决定只在设置里开关，候选窗只显示状态。
- **默认打开**：对习惯「改后半段」的用户直接可用。但它改变了所有老用户光标移进拼音后的候选和上屏结果，属于需要单独授权的出厂行为变化（见 AGENTS.md 关于默认值的约定），所以默认关。
- **由 Engine 自己暂存固定的那段和已选的部分，到整段完成才交出上屏文字**：不依赖宿主打开 `phrase_preedit`。但退格撤回、Esc 保留已选词、失焦送出、回车接原文这些规则 runtime 已经有一份，三个桌面宿主也都按 `phrase_prefix` 画行内，Engine 再存一份就要把这些规则再实现一遍。改为交出部分上屏，由 runtime 暂存。
- **在引擎快照里直接把固定的那段接到首行的文字上**：宿主和 runtime 拿到的就是要显示的文字。试过之后，runtime 的整句读法降位按首个整句的字数认读法，接上前缀后字数对不上，「痴火锅」「尺火锅」这类读法全排到了首页；释义和选择核对也要先剥掉前缀。改为快照另给前缀，只在视图里接上。
- **行内把光标前那段画成转换后的汉字**：更直观地表现「固定」。但光标位置是原文里的字节偏移，四个宿主都按原文画光标和分段编辑，混画汉字和拼音要重新约定光标坐标；固定的那段已经在首选里显示并可画淡，行内保留原文也方便继续移动光标。
- **把偏好放进账号同步的 `input.*` 键**：换设备也能带过去。但 `input.*` 由 Android、鸿蒙和桌面一起上传合并，手机端没有这个设置，会把账号里的值盖回关；同组的「只出单字」「Esc 保留已选词」等候选偏好同样只在偏好文档里，不进账号同步。

## Consequences

- 收益：打错在后半段时不用从头逐段选词，前半段保持当前转换，后半段按整段组字的候选挑；首选始终是整句，空格就能上屏。
- 代价：同一个字母光标在开关开着和关着时出不同的候选，靠候选窗提示区分；固定的那段算作一次部分选择，整段完成时会按造词规则存成个人词条。
- 本模式需要宿主打开 `phrase_preedit` 才能把各段一起上屏；直接使用 Engine 的调用方会依次收到「固定的那段 + 选中的词」和后面各段的上屏。
- 云候选和 AI 在本模式里不出现；回到光标末尾且没有交出过文字时恢复。
- 手机宿主不提供这个设置，偏好随文档同步过去也不生效。

## Verification

引擎测试 `crates/engine/src/session/tests/caret_suffix.rs`（13 项）、runtime 测试 `crates/input-runtime/src/tests/caret_edit.rs`（5 项）、host-api 测试 `caret_edit_suffix_reaches_the_engine_after_composition_and_the_view`、client-core 的偏好与能力测试、共享设置页的 `caret-edit-suffix-section.test.tsx`。把 `caret_edit_allowed` 临时改成恒假（等于没有这个功能）时，引擎 12 项、runtime 4 项和 host-api 那一项失败，只剩断言「关着与开头末尾不变」的那一项和 runtime 的重排保护测试通过；恢复后全部通过。`cargo test -p msime-engine --locked` 全量（2660 项和 golden 31 项）通过，说明关着时的行为没有变化；`cargo test -p msime-input-runtime -p msime-host-api -p msime-client-core --locked` 全部通过（host-api 的一项 Unix socket 测试只在临时目录路径过长时失败，换短路径后通过），这四个 crate 的 Clippy `-D warnings`、`cargo check --workspace --exclude msime-desktop --all-targets`、共享设置页 vitest 和 `tsc --noEmit`、`scripts/run-checks.sh`、`pnpm run verify-notes` 通过。测试词库全部是合成值。

宿主层：macOS 新增 ctest `candidate-caret-edit-suffix`，连同 6 项既有候选布局测试在 Mac Studio 上通过（含像素比对）；Linux 新增 `linux-caret-edit-suffix`，原生画布的两项测试扩展后用 clang 在 macOS 上编译运行通过；Windows 新增 `windows-caret-edit-presentation`，与扩展后的 `candidate_card_size`、`candidate_accessibility` 用 clang 在 macOS 上编译运行通过；Win7 设置测试加了开关的读写断言。

Linux 原生画布的前缀颜色测试使用绿色作为序号色，正文仍为白色：白字抗锯齿本来就会产生灰像素，缺字回退字形甚至会使变淡后的纯灰像素比变淡前少，不能用纯灰数量增加证明前缀变色。像素测试使用基础字体可显示的 `MMMM`，与 canvas 包的既有颜色测试一致，不要求 CI 安装中文字体；分别核对前缀变色、后缀仍白、整行变色与几何不变。中文、扩展 B 字符的 Unicode 标量与 UTF-8 字节范围由 `tests/core/caret_edit_suffix.cpp` 验证；提示布局仍使用中文文本。在只有拉丁字体的 Fontconfig 环境里也运行同一套画布测试。

没有验证的：Mac Studio 上没有 MinGW、Wine，OrbStack 没有运行，系统盘也几乎写满，所以 Windows 的 `CandidateWindow.cpp`、WinUI 与 Win7 设置工程、Linux 的 `ClientEngine.cpp` 和 `FcitxEngine.cpp` 只能由 PR 上的 Windows 和 Linux CI 第一次编译；没有在任何系统的真实输入框里装机验收（按要求不安装、不切换 macOS 输入法），提示在真实桌面上的观感、窄卡片上的截断和高亮行上的淡色都未看过。
