# Agent Note: 粤拼和粤语双拼输出简体

Status: implemented

## Problem

#10435 要让粤拼和粤语双拼也能输出简体。`msime-cantonese.db` 是繁体词库，粤拼被定为原生繁体方案（`SchemeType::outputs_traditional_natively` 为真、`script_conversion_applies` 为假），宿主的简繁转换只有简转繁一个方向，所以移动端在粤拼下把简繁按钮置灰，桌面端的按钮在粤拼下点了也不起作用。共享层只有 `traditional_to_simplified_characters`，它是逐字反查 `STCharacters` 的释义查询键，不是 OpenCC 的 `t2s`，不能用于上屏。

全局开关 `traditional_chinese_output` 默认是简体，粤拼如果直接跟随它，现有粤拼用户升级后会突然变成简体，这是出厂行为的改变。

## Decision

**偏好**：新增 `Preferences::cantonese_simplified_output`，默认 false（粤拼照旧输出繁体），关着时不写进文档。它与 `traditional_chinese_output` 互不影响：粤拼下切换的是它，切回普通话方案时全局字形保持原样。

**转换数据**：msime-dictionary 的 `sources/opencc/` 补入 OpenCC `TSPhrases.txt`、`TSCharacters.txt`，与已有 ST* 文件取自同一个上游提交 `26753884`，原样收录；本仓 `resources/opencc.lock.json` 固定新的字典提交和两条摘要。不收 `TSCharactersExt`，上游 `t2s.json` 把它标为可能输出豆腐块。

**转换函数**：`chinese_conversion::traditional_to_simplified` 按 `t2s.json` 去掉 `TSCharactersExt`：规范化兼容汉字后，对 `TSPhrases`、`TSCharacters` 组成的短路组做一遍最大正向匹配（乾淨→干净，乾隆不变）。`cantonese_to_simplified` 在字表之前保留 `CANTONESE_KEPT` 里的嗰、喎：字表会把它们转成扩展区的 𠮶、㖞，常见字体缺字，粤语简体写法也通常保留原字。嘅、咗、冇、喺、嚟、啲、佢、嘢等字表本来就没有条目，原样通过。

**在 Engine 里转换**：Engine 不依赖 client-base，也要能单独发布，所以转换以 `TextConversion(fn(&str) -> String)` 注入：`EngineOptions` 与 `SessionOptions` 的 `cantonese_output_conversion`，host-api 在偏好打开时传入 `cantonese_to_simplified`。`ProviderRegistry` 的 `fill_cantonese_candidates` 是粤拼候选文字写进 `WordItem.word` 的唯一地方，它在这里把每条候选转换后再按（字面，覆盖音节数）去重，發、髮都成了「发」时只留排在前面的那条。候选、整段上屏和分段选词上屏都取这里的文字，所以都是简体；预编辑是粤拼字母，不受影响。视图的 `script_conversion` 对粤拼仍为 false，宿主不会再转一次。`TextConversion` 按函数地址比较相等，`EngineOptions` 照旧能判断有没有变。

**生效时机**：它是 Engine 偏好，和「只出单字」一样经 host-api 在空闲时重建 Engine；组字中切换时这段组字照旧，结束后生效。

**学习保留词库原文**：[粤语学习](2026-11-03-cantonese-learning.md)按词库原文回放调频和造词，不能把简体输出直接写成学习键。`ime/cantonese_output.rs` 为最近一次查询保存（规范读音、显示文字）到原文的映射，与转换去重一样保留先到的候选；调频比较行、分段造词和整句造词都取这份原文，候选与上屏仍取转换后的文字。每次查询先清空映射，包括无词库、无效输入和查询失败的路径。未开启转换时不建立映射，取原文只克隆现有 `SharedString`，保持首选上屏的分配上限；不扩展公共 `WordItem` 或宿主 ABI。

**宿主**：iOS、Android、HarmonyOS 在粤拼（含粤语双拼，Engine 里同为方案 5）下把简繁按钮和功能菜单的「繁」变为可用，点按改写 `cantonese_simplified_output`，按钮按它显示简或繁。判定写在可单测的地方：Android `ChineseOutputPolicy.switchFollowsCantonese` / `showsTraditional` / `preferenceKey`，HarmonyOS `input/ChineseOutputPolicy.ts` 的同名函数；iOS 在 `KeyboardViewController` 的 `characterSetFollowsCantonese`，会话桥的 `setCantoneseSimplifiedOutput` 写会话和共享文档。HarmonyOS 的 2in1 工具栏和 Ctrl+Shift+F 走同一个 `toggleCharacterSet`。

macOS、Windows、Linux 这次不接：三个桌面宿主的简繁开关各自存在宿主自己的外观状态里再合进文档，接法互不相同，需要各自的构建和验证，另开 PR，正文写 `Refs #10435`。在此之前桌面上粤拼照旧输出繁体，按钮行为不变。

## Alternatives considered

- **粤拼直接跟随全局简繁开关** — 报告人说可以接受，也不用新增偏好和按钮逻辑。但全局默认是简体，所有现有粤拼用户升级后会悄悄变成简体；升级时把老用户迁移成繁体又会连带全拼、五笔变成繁体。维护者选了单独的开关。
- **在各宿主里做繁转简** — 和简转繁的现有做法一致，Engine 不用改。但六个宿主要各写一份调用；转换后出现的重复候选宿主去不掉，因为去重会打乱宿主与 Engine 之间的候选下标，移动端的隐藏行机制是空实现。放在 Engine 里一处做完，去重和上屏自然一致。
- **把 OpenCC 数据或 client-base 依赖放进 Engine** — 省掉函数注入。但 Engine 会带上一份它自己用不到的转换表，发布到 crates.io 的 Engine 也不能依赖不发布的 client-base。注入只多一个字段。
- **复用 `traditional_to_simplified_characters`** — 不用补数据。它逐字反查、没有词组表，「乾淨」会变成「乾净」，一对多的字取第一行，用于上屏会出错。
- **学习时把简体反向转回繁体** — 不需要保存原文，学习入口也可以保持独立。但简繁转换不是一一对应，例如「妳」与「你」都会显示为「你」，反向转换不能确认去重时留下的是哪一条词库行，因此保存当次查询的真实原文。

## Consequences

- 粤拼和粤语双拼可以输出简体，默认行为不变；全局简繁开关不受粤拼选择影响。
- 打开时每次查询都对粤拼候选逐条转换，并为原文映射保留规范读音、显示文字和原文字串，增加查询期的分配与内存；关着时不建立映射、不复制原文字串。
- 新键只在打开时写进文档；读不懂未知键的旧版本只在用户打开它之后读不了这份文档，与 `chinese_english_spacing` 等同类开关相同。
- 这个偏好没有接账号同步和 MCP 偏好工具，各设备分别设置。
- 字典 PR #113 已合入上游 `main`；本仓 OpenCC 锁已改为合并提交 `4fd95815adf5e93b7ad835f048348dd3fe341777`，逐份下载核对全部锁定文件的大小和 SHA-256 均一致，不再引用未合并的 PR 提交。
- 保留名单目前只有嗰、喎，社区有更完整的粤语简体约定时改 `CANTONESE_KEPT`。
- 注音和笔画同样是原生繁体方案，注入点可以复用，等有需求再接。
- Android 的原有粤语双拼键面、会话空窗记忆、特殊韵母键读屏名称和重排模型后台准备保持现有接线；简繁输出不能以撤回这些已落地能力为代价。宿主键面回归直接抽取真实方法编译执行，覆盖粤语双拼、普通粤拼、会话空窗、普通话双拼与关闭提示开关。
- 与最新 develop 的粤语学习合并时，行为回归先证实简体「号」被写入调频日志；修复后覆盖调频、分段「你號」及整句「我愛」的日志原文与关闭转换后的候选，并保留默认路径的分配门禁。
- 验证缺口：三个移动端的按钮和上屏需要真机确认；iOS 的 XCTest 只交给 CI。

## Verification

- 三条新增行为回归先复现调频日志误写「号」、分段造词误写「你号」、整句造词误写「我号」，修正后均通过；同时验证切回繁体及新会话仍能按词库原文回放。
- `cargo test -p msime-engine --lib cantonese`：92 项通过、1 项忽略，包含 `selecting_a_cantonese_candidate_does_not_clone_the_full_row` 的分配上限回归。
- 原转换测试保留默认学习设置：分段选择后应学到「你號」，不传转换时断言该新词和原有词库行全部显示繁体。旧期望产生于粤语尚不学习时，不能用关闭学习来恢复旧列表。
- OpenCC 锁定的 10 份文件已从上游合并提交逐份下载核对大小和 SHA-256；`python3 scripts/test-fetch-opencc.py` 的 9 项测试通过。
- `cargo clippy -p msime-engine --all-targets -- -D warnings`、修改文件的 rustfmt、笔记和差异检查通过；`client-base` 转换表测试 15 项、Host API 转换注入测试 1 项、引擎 golden 31 项均通过。
- 同步 develop `a0dfe261eb` 后完整本地 `bash scripts/verify-local.sh --quick` 通过：Windows 原生行为检查 214 项、Linux 原生测试 122 项、Rust/wasm、Android 宿主检查、macOS 和 Apple 桥接编译、两架构 pipe-only 编译。缺少依赖的完整 Windows 原生工程及 HarmonyOS ArkTS 构建按脚本提示跳过；设备及截图验收缺口保留在 PR 正文。
