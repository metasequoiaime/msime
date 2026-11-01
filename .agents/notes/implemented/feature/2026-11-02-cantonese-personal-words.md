# Agent Note: 粤语个人词条

Status: implemented

## Problem

#10544 报告在 iOS 上用粤语（含粤语双拼）打字，个人词库没有粤语：手动加词的「类型」只有拼音、两版五笔、快捷短语和英文，粤语也不调频、不记词。原因是粤语方案从一开始就只读 `msime-cantonese.db`：`cantonese/mod.rs` 写明 nothing is learned，`PersonalDictionaryKind` 没有粤语，候选只来自那一份只读库。补粤语词只有 msime-dictionary 的 `custom/cantonese.txt` 一条路（[粤语词库收录人工审校词表](2026-10-31-cantonese-reviewed-word-list.md)），要审校、发版，不适合只给自己用的词。

维护者对第一期的范围：用户能在个人词库里手动加粤语词（不带声调的粤拼，分隔按引擎惯例），存在本机，粤语方案出候选时带上它们；不做粤语的自动学习和调频（会改变现有用户的候选顺序），不做云同步（服务端约束）；凡有粤语方案的平台都接上个人词库页。

## Decision

- `PersonalDictionaryKind` 追加 `Cantonese = 5`，日志名 `cantonese`，已有序号不变。编码是不带声调的粤拼音节、单个空格连接，与 `msime-cantonese.db` 的 `entries.key` 同一写法，查词时不用再转换。`validate_personal_dictionary_entry` 接受空格或 `'` 分隔（连续几个算一个）、每个音节末尾可带声调数字 1–6，存之前去掉声调、收成单个空格；音节只能是小写字母、最长 8 个；音节数必须等于字数（最多 64）。只查写法不查音节表：音节表在 `msime-cantonese.db` 里，没装粤语词库时也要能加词。
- 粤语词只记在用户日志 `user_dictionary_operations`（`dictionary='cantonese'`、`user_inserted=1`），没有工作表。个人词库编辑照常写日志和回执；回放新代次时把 `cantonese` 行计为跳过；代次里没有 `msime-pinyin.db` 时粤语词和英文词一样能列出、编辑（`PersonalDictionaryKind::stored_without_main_dictionary`）。
- 粤语方案读这些词：`cantonese::UserWords` 把日志里的粤语词整份读进内存，之后每次粤语查询前只问一次 `PRAGMA data_version`，别的连接提交过才重读，所以设置页或个人词库队列改了词，活着的会话下一次按键就看得到，逐键查询不碰日志、不分配。连接只读、`busy_timeout` 为 0，读失败时保留上一次的词；日志还不存在时不打开，下次查询再看。`CantoneseLexicon` 包着发布词库和这份词，`lookup_into`、`lookup_completions_into` 把同一个键下的同一个词合成一行取较大的权重，再按发布词库原有的规则（权重从重到轻、同权重按文字）排序截断，粤拼、粤语双拼展开和末音节补全都经它取词。
- 粤语上屏仍然不写任何东西：不调频、不造词，候选窗也不对粤语行开放置顶、删除（`is_editable_source` 不变）。
- 宿主协议：host-api 的 `Kind`、client-services 的 `PersonalWordKind`（个人词库队列）和 `ImportKind` 都加 `cantonese`。`list`、`count`、`export`、`edit`、`import`、个人词库队列都认它；按编码搜索时音节间的分隔可打可不打（`dictionary_row_matches`）。`browse` 的 `cantonese` 仍指发布的粤语词库：host-api 的 `LookupKind` 把资源类型排在前面，untagged 先试它；个人粤语词由 `list` 列出，`dictionary_table_entries` 对粤语总是空页。云词库（`client-core::cloud::dictionary::DictionaryKind`、服务端协议 `backend/validation.rs`）、社区词库、词库集合和词库快照（云同步与本地备份共用的 NDJSON）都不收粤语，快照导出跳过它们。
- 新粤语词的默认权重是 100000（共享设置页 `CANTONESE_NEW_WORD_WEIGHT`、Android `CANTONESE_DEFAULT_WEIGHT`、Windows 新增词条，iOS 本来就是这个值）。`msime-cantonese.db` 的权重按语料词频算，多字词大多在几万以下，`nei hou` 的「你好」是 21493；手动加的词默认排在同一编码的多数发布词前面，单字的高频读音（「你」七百多万）仍在它前面。用户可以改权重。

## 各平台的入口

- 共享设置页（macOS、Linux、HarmonyOS 平板）：「本地词库类型」和「个人词库」下拉都有「中文 · 粤语」，可查、可加、可编辑删除、可导入导出（标准、Windows TSV、Rime；纯汉字注音和细胞词库仍只给拼音）。发布的粤语词库改名「中文 · 粤语（基础词库）」，下拉里资源词库的选项值带 `resource:` 前缀，`DictionaryEntryList` 的只读与否由调用方说明，不再按种类推断。个人粤语词库在 `loadPhrases` 里总是走 `list`。
- HarmonyOS 手机、Android：词库页「已安装」在拼音词库下面多一行「粤语词库」，详情页与拼音词库同一套（搜索、加词、导出），加词对话框的编码叫「粤拼」，按粤拼校验，经个人词库队列写入。
- iOS：`PersonalWordKind` 加 `cantonese`（「粤语」），添加词条、导出、按类型筛选、JSON 和词库文件导入都跟着有；编码输入只留字母、空格、`'` 和 1–6。社区词库的类型里没有它。
- Windows（WinUI 与 Win7）：「本地词库类型」「管理词库类型」和「导出全部」都有粤语；资源词库选项在 WinUI 里带 `resource:` 前缀、在 Win7 里按下拉序号判断，个人粤语词库查询发 `list`。

## Alternatives considered

- **给用户粤语词建一张工作表，像拼音那样回放进代次**：查询时与其他种类一致，日志只是来源。但粤语的发布库是只读的语言词库，不在代次里；要么把它复制进代次再写，要么另建一个用户库文件、再给它一套准备与回放，都比直接读日志多出一整条维护路径，而第一期只要求手动加词。
- **把日志 ATTACH 到粤语词库的连接上，逐次用 SQL 合并**：不用常驻内存，改了马上可见。但每个跨度、每个双拼读音（最多 64 个）都要多一次日志查询，iOS 上日志是回滚日志、写入期间读会碰到锁，按键路径的耗时和失败都要跟着变；读进内存加 `data_version` 只在日志真变了时才读。
- **个人粤语词另起一个名字（例如 `jyutping`），避开与资源词库同名**：host-api 的 untagged 解析、共享设置页和 Windows 的下拉都不用区分同名。但引擎日志名、协议名和界面名会三处不一致，而 `cantonese` 在资源侧已经是用户看到的名字；改为在资源一侧消歧（解析顺序、选项值前缀）只动查询页。
- **新词沿用各端原有的默认权重（共享页 10、Android 10000）**：与其他种类一致。但粤语词库的权重比拼音高一两个数量级，权重 10 的词在有发布同音词的编码下几乎总排在最后，用户会以为没加上。
- **同时做粤语调频和分步造词**：issue 里也提了。但它会改变现有粤语用户的候选顺序，属于默认行为变化，维护者这次没有授权，留到第二期单独决定。

## Consequences

- 收益：六个平台都能手动加粤语词，粤拼和粤语双拼共用；加完不用重启键盘，下一次按键就出现在候选里；没有用户粤语词时，逐键路径只多一次 `PRAGMA data_version`，不分配（`cantonese_candidate_refresh_reuses_row_storage` 仍断言零分配）。
- 代价：日志多了一个 `dictionary` 取值。认得它之前的引擎（降级安装）回放到这一行会算失败、拒绝准备新代次，与当年加 `wubi98` 时一样；`docs/data-formats.md` 的日志表里写明了。`stream_dictionary_state` 在旧引擎上同样不认识它。
- 粤语词不进词库快照：云同步本来就不收；本地备份（`export_snapshot` 带 `include_learning`）也不含它们，恢复备份不会把粤语词带回来，但也不会删掉本机已有的。要让备份带上粤语词，需要快照格式和服务端一起认 `cantonese`，排在云同步那一期。
- 第二期仍待决定：粤语选词调频与分步造词（是否默认开、是否单独开关）、云词库与服务端 `user_dictionary_entries.kind` 的约束、社区词库是否收粤语、快照与本地备份。
- 验证：`cargo test -p msime-engine --lib`（`validation_normalises_and_rejects_with_the_reference_messages`、`cantonese_words_live_in_the_journal_and_survive_replay`、`kind_export_filters_before_offset_and_keeps_export_rules`、`user_words_rank_with_the_dictionary_rows_and_follow_the_journal`、`a_personal_cantonese_word_joins_the_candidates_of_a_live_session`）；`cargo test -p msime-host-api --lib dictionary::`（`cantonese_personal_words_list_count_and_export_while_browse_keeps_the_resource`）；`cargo test -p msime-client-services --lib dictionary::`（`cantonese_rows_keep_spaced_jyutping_for_the_engine`）；共享设置页的 vitest（`dictionary-lookup.test.tsx`）；Android `DictionaryCollectionsStoreSmoke`；Windows `tests/ui/lexicon_import_model.cpp`。各平台的设置页和键盘没有在真机上验收。
