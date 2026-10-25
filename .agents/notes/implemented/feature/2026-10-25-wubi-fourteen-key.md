# Agent Note: 五笔 14 键（iOS、Android、HarmonyOS）

Status: implemented

## Problem

#9691：手机上 26 键五笔误触多，用户要 14 键布局。移动三端已有全拼、双拼的 14 键（`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`），五笔只有 26 键。一键两个字母时组码有歧义：五笔码最长四位，每位两种可能，一组键最多对应 16 个编码。

歧义本身不大，难点在排序。码表 `wubi86` 的 `weight` 只排同一编码里的先后（12.6 万行里 7.7 万行是 10，9 千行是 20），跨编码比不出高低。离线核对（锁定词库 `msime-wubi.db` 与 `msime-pinyin.db`，每个单字和词按自己的每个编码打满，按拼音词库词频加权）：只按码表权重排，单字首选 0.341、前三 0.724，词首选 0.284；26 键的单字首选是 0.956。按 14 键打 QW QW，「你」（`wq`）排在「多」（`qq`）、`QQ`、「欠」（`qw`）之后。

三端必须表现一致，解码只在共享 Rust 里做。

## Decision

三端提供「五笔键盘」26 键 / 14 键，选 14 键时键面就是全拼 14 键，组码从 `grid_key` 进来。引擎不经九宫格会话：五笔方案把每一键记成这一组的代表字母，provider 展开成全部编码查码表，跨编码按拼音主词库的词频排序，五笔的选词、调频、删词、四码唯一自动上屏和第五码顶字照旧。

### 偏好与网格

- client-core 有 `touch_wubi_layout`（`twenty_six_key` 默认、不写进文档 / `fourteen_key`），与 `touch_keyboard_layout`、`touch_shuangpin_layout` 独立，换 86 / 98 码表和别的方案后保留。默认值不变。
- 引擎网格复用 `KeyGrid::FourteenKey`，不另加枚举值：`Session::grid_key` 在当前方案是五笔时把组码交给五笔方案，是全拼时照旧交给九宫格会话。`Runtime::set_key_grid` 让 14 键接受全拼和五笔，C ABI 的网格值 `2` 对五笔会话就是五笔 14 键。
- host-api 的 `layout_key_grid_for` 在 Android、iOS 上按 `touch_wubi_layout` 给五笔会话开 14 键（手写布局不开），会话创建、重建和偏好重新应用时生效；鸿蒙与它的双拼 14 / 18 键一样由宿主自己调用 `msime_client_set_key_grid(2)`。`touch_wubi_layout` 变化算换布局，宿主设下的网格覆盖随之失效。

### 组码

- `wubi::grid`：代表字母是全拼 14 键的组首字母 `q e t u o a d g j l c b m`，只有 ZX 记成 `x`。五笔码没有 `z`，这一组只代表 `x`；组码因此永远不含 `z`，`z` 通配、`z` 引导的拼音反查和「带 `z` 的码不自动上屏」都碰不到它。宿主照全拼 14 键送组首字母（ZX 送 `z`），由引擎换成 `x`。
- `WubiScheme` 记下这次组字是不是 14 键打的（`grid`），由第一个键决定，组字清空、`reset` 和宿主编辑（`set_raw_input`）时复位。14 键组字中不收确定的字母，26 键组字中不收组码，两种码不混在一串里：硬件键盘的字母在 14 键组字中返回 unhandled，与全拼 14 键相同。组码最多四位，不延长。
- 请求带 `wubi_grid`。ImeSession 在 `wubi_grid` 下不查混输的拼音回退、不开临时拼音反查、不延长码长，`wubi_pinyin_continues` 为假：组码每一位是一组字母，不是拼音。

### 查询与排序

provider 的 `query_grid_rows` 把组码展开成全部编码（`wubi::grid::expand_into`，最多 16 个），每个编码各查一次完整编码的行（按 `key` 等值，走 `idx_wubi86_key_weight`），再各查一次更长编码的补全行（范围扫描，按权重各取前 50）。

- 完整编码的行在前，按词频排：每行的分数是它和同一编码里排在它后面的行的最大词频。一个编码的首行带着这个编码最常用的词去和别的编码比，同一编码里码表的先后（包括用户调频后的先后）不变。分数相同按码表权重，再按编码。
- 补全行在后，与 26 键一样按码表权重。总数最多 50 行，与 26 键相同。
- 词频来自 `wubi::frequency::WubiFrequency`，读拼音主词库（`paths.dictionary(msime-pinyin.db)`，通常就是码表所在的代次工作副本）：单字一次扫完 `tbl_1_*`，按词库路径在进程里缓存（约五万行，Studio 上 20 ms）；词按单字读音的首字母拼出简拼，到 `tbl_<字数>_<首字母>` 按 `jp` 索引查 `value`，多音字最多试 16 种拼法，结果进 4096 项的 LRU。拼音主词库不在、表缺失或查询失败一律答 0，排序退回码表权重。

### 显示与上屏

- 快照的 `preedit` 是首选编码的前几位（QW QW 的首选是「你」就显示 `wq`），首选不是这几组上的码表行时显示组码；`nine_key_reading` 与它相同，宿主 14 键的读音行照常读这一项。`editing_text` 是组码（`qq`），宿主不显示它，也不写进编辑框。`nine_key_spellings` 恒为空，单字和笔画筛选不处理：五笔 14 键没有拼音选择和九键三栏面板，展开候选用五笔原有的方式。14 键组码没有可落脚的光标，`set_caret` 不动。
- `Session::grid_composing` 在五笔 14 键组字时也为真：runtime 的 `CommitRaw` / `CommitRawWithoutLearning` 因此与全拼 14 键一样从高亮行结束组字，回车学词策略不把组码学成英文词。
- 四码之后的一键（`Action::GridKey`）先顶首选上屏，再开始下一组编码；「第五码顶字」关着也这样，因为没有混输拼音可以容纳这一键，与 26 键混输关闭时的规则相同。`character_action` 把 `GridKey` 算进去，四码唯一时按 `wubi_auto_commit_unique` 自动上屏。
- 打字统计的来源仍是 `wubi`（与双拼 14 键记各自的方案一样），键位记 `FourteenQW`…`FourteenM`。

### 三端宿主共同规则

- 设置在当前方案是五笔时给出「五笔键盘」26 键 / 14 键，只写 `touch_wubi_layout`，不动 `scheme`、`wubi_profile` 和启用列表；三端的行名一致。预览、皮肤缩略图跟着画 14 键。
- 键面复用各宿主的全拼 14 键（`FourteenKeyLayout`），点按送组首字母到 `grid_key`，统计键位 `Fourteen*`，长按弹出两个字母，英文、网址和密码框、本地模式回落 26 键，与全拼 14 键相同。
- 第三排左端键一直是「符」：五笔不切音节，引擎在五笔里也不收 `'`，组字中点它与全拼 14 键空闲时一样打开符号面板（组字先按高亮行结束）。
- 读音行画 `nine_key_reading`，组码不写进编辑框；没有拼音选择条，展开候选用五笔自己的候选面板，不开九键三栏。
- 编码提示按每个候选自己的编码算已打部分：候选编码的前几位逐位落在 `editing_text` 的各组里，这几位就是已打的，其余是「还需输入」；对不上组时退回预编辑。只拿预编辑（首选的编码）去比，其他编码的候选就没有提示。

### 各宿主

- **iOS**：`TouchWubiLayoutPreference`（共享文档 `touch_wubi_layout`，App Group 镜像 `wubiKeyboardLayout`），`LetterKeyGrid.of` 增加 `wubiLayout`；桥接层 `switchToFourteenKeyWubi()` 切到五笔后设网格 2，记进 `keyGrid` 供重建会话时重放。键盘控制器保存已应用的五笔布局，热加载与双拼布局同一条路径，组字中收到的布局留到上屏后再应用。`typesWubiGrid` 下关掉拼写光标、拼写菜单和空格拖动在组码里移光标；编码提示用 `FourteenKeyLayout.typedPrefix(of:keys:)`。
- **Android**：键盘页「布局」在五笔下给出「五笔键盘」，搜索索引收录。`FourteenKeyLayout.separatesSyllables(composing, scheme)` 决定左端键，`NineKeyPanelPolicy.threeColumn` 多看方案，五笔不开三栏；编码提示在 `WubiCodeHintPolicy.typed`。宿主不调 `setKeyGrid`，由 host-api 按偏好开网格。
- **HarmonyOS**：`KeyboardScheme.keyGrid` 多一个 `wubiLayout` 参数，五笔 14 键在手机和平板上开网格 2，2in1、手写不开；`letterLayout`、`fourteenKeySeparates`、`acceptedKeyGrid`（五笔只放行网格 2）。会话在创建、获得焦点和重建时重发网格。`SchemeCompositionPolicy` 让五笔 14 键的读音行画 `nine_key_reading`、编码留在读音行。设置是共享 React 页的 `HarmonyChineseKeyboardRow`，只在鸿蒙手机形态出现，macOS、Linux 和 2in1 不显示；编码提示在 `WubiCodeHintPolicy.typed`。

### 历史笔记检索

- [全拼 14 键](2026-10-10-fourteen-key.md)：部分重叠，复用键面、`grid_key` 入口、`View.key_grid`、读音行、长按、回落 26 键和统计键位；本篇取代其中「五笔没有 14 键」一项，解码不经九宫格会话。
- [iOS 14 键双拼](2026-10-24-ios-fourteen-key-shuangpin.md)、[Android 14 键双拼](2026-10-23-android-fourteen-key-shuangpin.md)、[HarmonyOS 双拼 14/18 键](2026-10-25-harmony-fourteen-key-shuangpin.md)：部分重叠，独立布局偏好、设置入口、热加载和网格门控照它们的做法。
- [五笔 z 通配与第五码顶字开关](2026-10-21-wubi-input-switches.md)：部分重叠，顶字和四码唯一的纯判定沿用；14 键组码不含 `z`，通配和反查不作用于它，顶字不看混输开关。
- [五笔顶字保留后续按键](../bug-fix/2026-10-10-wubi-top-commit-tail.md)：部分重叠，14 键的第五键沿用「先选首选再接这一键」。

## Alternatives considered

- **14 键五笔走九宫格会话，给 `KeyGrid` 加一种五笔网格。** 理由是全拼、双拼的字母网格都在那里，锁定、三栏面板、学习现成。但九宫格会话的路径搜索、音节切分、整句和简拼都是拼音的；五笔需要的是码表、调频、删词、四码唯一和顶字，这些都在五笔方案和 provider 里。把五笔塞进九宫格会话等于两套五笔逻辑；按方案路由、留在五笔方案里，五笔已有的行为一条不用重写。
- **只按码表权重排（不读拼音词库）。** 最强的理由是零依赖：码表自带权重，查询只碰一张表，五笔版这类没有拼音词库的版本也一样。但权重只是同码内的档位，单字首选 0.341、词首选 0.284，QW QW 打「你」要翻到第三、四位。按词频排后同一批数据单字首选 0.789、前三 0.969，词首选 0.566、前三 0.840。没有拼音词库时仍退回码表权重。
- **纯按词频排，不保留同一编码里的先后。** 离线核对单字首选 0.816、词 0.595，比保留同码先后（0.789 / 0.566）略高。但那样用户调频后的先后在 14 键下不起作用：调频改的是码表权重，词频不变，选过的词永远升不上来；同码里码表的先后也是五笔用户熟悉的顺序。保留同码先后，代价是首选少两三个百分点。
- **在准备代次时把词频写进工作副本（建一张 `value → weight` 表）。** 理由是查询时一次索引就有词频，不必拼简拼。但它要在每次准备代次时扫完拼音各表写一张新表（Studio 上 0.4 s，手机上更久，工作副本多约 2 MB），而大多数用户不用五笔 14 键；改发布词库要先改 dict-builder。查询时按需取词频，只查完整编码的那几十行，单字表进程里只建一次。
- **用一条 GLOB 字符类语句（`[qw][qw]*`）一次查完。** 写法最短，SQLite 也能排序。但首位是字符类时 GLOB 用不上索引，每键整表扫描 12.6 万行；按展开的编码逐个等值和范围查询都走 `("key","weight")` 索引。
- **ZX 键同时代表 `z`（通配）。** 理由是通配开着的用户在 26 键上能用 `z`。但通配匹配任意字母，ZX 键会把一位变成 25 种可能，展开从 16 个变成数百个，而且 ZX 上的 `x` 是常用码；14 键不提供 `z` 通配和 `z` 反查。

## Consequences

- **收益**：
  - 五笔用户在三端都能用 14 键，键面、交互与全拼、双拼 14 键一致；五笔的调频、删词、四码唯一自动上屏、第五码顶字和编码提示照旧可用。
  - 解码在共享引擎里，三端只做键面和设置。
- **代价与已知上限**：
  - 歧义仍在：单字首选 0.789（26 键 0.956），词首选 0.566、前三 0.840，没有词频的词（约 1.8 万行，多是长词和罕用字）排在有词频的后面，靠码表权重。
  - 每键多查若干次：Studio 上 release 构建逐键 p95，1 键 1.19 ms（26 键 0.85 ms）、2 键 0.45 ms（0.15）、3 键 0.30 ms（0.04）、4 键 1.22 ms（0.02），最大 11 ms；进程里第一次查询要多建一次单字表（约 20 ms）。手机上按几倍估计，仍在一帧之内。
  - 14 键不提供 `z` 通配、`z` 拼音反查和五笔拼音混输；这三个开关只作用于 26 键和硬件键盘。
  - 固定候选位置按组码（`qq`）记，与 26 键的码（`wq`）是不同的上下文。快捷短语按用户打出的字母对码，14 键组字时不出现（组码不是这样的字母，拿 `qq` 去对会带出码为 `qq` 的短语，码为 `wq` 的却永远打不出来）。
  - 降级：选了 14 键后装回不认识 `touch_wubi_layout` 的旧版，偏好按既有规则处理未知字段（文档保留、旧版不读），旧版画 26 键。
  - 鸿蒙由宿主自己开网格，以后再加布局要记得三处投影（偏好、宿主门控、设置预览）。

## Verification

- 引擎：`wubi::grid`、`wubi::frequency`、`wubi::scheme` 和 provider 的 14 键单测（展开、代表字母、词频与多音字简拼、同码先后与调频、没有拼音词库时退回码表权重、50 行上限），`session::tests::wubi_fourteen_key_*`（展开与显示、硬件字母分开、四码上限与混输无关、ZX 与四码唯一、方案门控、组码不对快捷短语）。
- runtime：`wubi_fourteen_key_*`（真实引擎上的读音行、第五键顶字两种开关、四码唯一自动上屏、`CommitRaw` 两种命令从高亮行结束）和 `the_fourteen_key_grid_is_offered_to_quanpin_and_wubi_only`；host-api：`fourteen_key_wubi_preference_is_applied_by_touch_hosts`（六个平台的门控）、`fourteen_key_wubi_grid_decodes_through_the_c_abi`（网格值 2、组码、硬件字母、网格 4 拒绝）；client-core：`touch_wubi_layout_defaults_persists_and_rejects_unsupported_layouts`。
- `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-client-core --locked` 全部通过（引擎 2640 项、56 项按既有配置忽略；host-api 983 项、5 项忽略；runtime 389 项；client-core 548 项），四个 crate 的 all-targets Clippy（`-D warnings`）通过。
- iOS：Studio 上 Xcode 27.0、专用 iPhone 17 Pro / iOS 27.0 模拟器，`FourteenKeyKeyboardTests` 20 项通过（桥接层的你、读音、硬件字母、`CommitRaw`、第五键顶字、重建会话、98 码表自动开网格；键面、「符」、读音行、编码提示、五笔候选面板、长按；英文、网址框、数字页回落；空闲与组字中的热加载）；`LanguageSchemesTests` 28 项中 23 项通过、5 项因模拟器缺少语言词库跳过；`TypingStatisticsTests`、`SchemeDocumentSettingsTests`、`CompositionBoundaryTests` 通过；`MSIMEApp` 模拟器构建通过。同批 `EighteenKeyKeyboardTests` 的 `testShuangpinLayoutReloadSwitchesBetweenFourteenAndEighteenKeys`（键盘高度 291 对 295）在未改动的基点上同样失败，与本改动无关。没有在系统键盘入口上验收；iOS 不在密码框加载第三方键盘，回落以网址框验证。
- Android：`check-host.sh` 276 个 JVM smoke 通过、244 个设备套件源文件编译通过；专用 API 35 模拟器上 `FourteenKeyDeviceSmoke -e wubi_only true` 连续两次通过，完整的全拼、双拼、五笔 14 键套件通过一次。设备上覆盖设置 26→14 的持久化与 86 保留、QW QW 出字上屏、左端键「符」、不开三栏、实体键盘 `wq` 走 26 键、中英与 123 层、密码和网址回落、重启保留、切 98 后照常出字、切回 26 键保留 98，编码提示显示「偷笑 还需输入 tt」「金昌 还需输入 jj」。真实词库下 QW QW 的首选是「多」（拼音词库里「多」的词频高于「你」），选过「你」后由调频升到首位、读音行变成 `wq`。API 28、整套 `smoke.sh` 与真机没有跑。
- HarmonyOS：`platforms/harmony/tests/run.sh` 0 失败（含五笔 14 键的网格、键面、左端键、统计来源、读音行与编码提示），设置页 vitest（`tests/settings`、`tests/keyboard`、`tests/community`）通过，`apps/desktop` 与 `apps/harmony` 的 `tsc --noEmit` 通过，`hvigorw assembleHap` 编译通过（用的是旧的原生库，只证明 ArkTS 能编，HAP 不可安装）。没有在模拟器或真机上运行。
- 真实码表：`wubi::provider::tests::wubi_grid_real_table_latency_and_rank`（`#[ignore]`，需要 `MSIME_WUBI_GRID_DB` 指向拼音各表加 `wubi86` 的主词库）在 Studio 上 release 运行，12.5 万行逐个编码打满，得到上文的排名和延迟；与离线 Python 核对的结果一致。
