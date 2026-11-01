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

**宿主**：iOS、Android、HarmonyOS 在粤拼（含粤语双拼，Engine 里同为方案 5）下把简繁按钮和功能菜单的「繁」变为可用，点按改写 `cantonese_simplified_output`，按钮按它显示简或繁。判定写在可单测的地方：Android `ChineseOutputPolicy.switchFollowsCantonese` / `showsTraditional` / `preferenceKey`，HarmonyOS `input/ChineseOutputPolicy.ts` 的同名函数；iOS 在 `KeyboardViewController` 的 `characterSetFollowsCantonese`，会话桥的 `setCantoneseSimplifiedOutput` 写会话和共享文档。HarmonyOS 的 2in1 工具栏和 Ctrl+Shift+F 走同一个 `toggleCharacterSet`。

macOS、Windows、Linux 这次不接：三个桌面宿主的简繁开关各自存在宿主自己的外观状态里再合进文档，接法互不相同，需要各自的构建和验证，另开 PR，正文写 `Refs #10435`。在此之前桌面上粤拼照旧输出繁体，按钮行为不变。

## Alternatives considered

- **粤拼直接跟随全局简繁开关** — 报告人说可以接受，也不用新增偏好和按钮逻辑。但全局默认是简体，所有现有粤拼用户升级后会悄悄变成简体；升级时把老用户迁移成繁体又会连带全拼、五笔变成繁体。维护者选了单独的开关。
- **在各宿主里做繁转简** — 和简转繁的现有做法一致，Engine 不用改。但六个宿主要各写一份调用；转换后出现的重复候选宿主去不掉，因为去重会打乱宿主与 Engine 之间的候选下标，移动端的隐藏行机制是空实现。放在 Engine 里一处做完，去重和上屏自然一致。
- **把 OpenCC 数据或 client-base 依赖放进 Engine** — 省掉函数注入。但 Engine 会带上一份它自己用不到的转换表，发布到 crates.io 的 Engine 也不能依赖不发布的 client-base。注入只多一个字段。
- **复用 `traditional_to_simplified_characters`** — 不用补数据。它逐字反查、没有词组表，「乾淨」会变成「乾净」，一对多的字取第一行，用于上屏会出错。

## Consequences

- 粤拼和粤语双拼可以输出简体，默认行为不变；全局简繁开关不受粤拼选择影响。
- 打开时每次查询都对粤拼候选逐条转换，每行多一次字符串分配；关着时不经过任何新代码。
- 新键只在打开时写进文档；读不懂未知键的旧版本只在用户打开它之后读不了这份文档，与 `chinese_english_spacing` 等同类开关相同。
- 这个偏好没有接账号同步和 MCP 偏好工具，各设备分别设置。
- 本仓的 OpenCC 锁在评审期间指向字典 PR 的提交；字典 PR 合入后要把锁改成合并后的提交再合本 PR，不让默认分支引用未合并的提交（msime-dictionary `AGENTS.md`「OpenCC 转换数据」）。
- 保留名单目前只有嗰、喎，社区有更完整的粤语简体约定时改 `CANTONESE_KEPT`。
- 注音和笔画同样是原生繁体方案，注入点可以复用，等有需求再接。
- 验证缺口：三个移动端的按钮和上屏需要真机确认；iOS 的 XCTest 只交给 CI。
