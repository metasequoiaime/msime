# Agent Note: 日期混输候选里的「📅日期」第三位入口

Status: implemented

## Problem

Windows 正式版有 `utility.date_time_menu`（出厂开）：日期时间混进普通候选时，第三位放一个「📅日期」入口行，选中后候选窗换成这一组的全部日期格式，和 Shift+T 模式的格式全集相同。新架构已经实现了行内日期候选（`local_modes.date_time`），但没有这个入口，行内最多给 3 行格式，其余格式只能进 T 模式或 `/` 模式才能拿到。

## Decision

- 入口行和行内日期行一起由引擎在 `insert_inline_date_time` 里生成：`pinyin` 用 `menu:` 前缀记关键词（参考 `date_time_query.cpp` 的 `kDateTimeMenuPrefix`），入口的 `source` 是 `CandidateSource::Fallback`，行内日期值仍是 `Generated`。入口是换页动作，不是词网格读法：`Generated` 会在增强模式的大整句池下被误认成另一种切分并移到整串词库词之后（#11513），所以入口也明确不回答整段按键（`candidate_answers_key = false`），避免参与模型比较。文案按组取「📅日期」「🕒时间」「📅星期」（农历组落到「📅日期」，与参考的默认分支一致）。入口插在第一条行内行后面；锚定词在首位时正好是参考的第三位。一行行内行都没有（农历换算不出来）时不放入口。
- 「选中入口换候选集」做在引擎里，不交给宿主：`InputSession::date_time_menu_page` 保存整组格式，`candidates()` 在普通混排列表之前返回这一页。`mixed_candidates` 和组字原样保留，任何非选词动作（字符、标点、除 `CommitCandidate` 外的命令、边缘选择、光标移动、滑行、改字入口）把页面置空即恢复原来那一屏；`reset_composition` 同样清掉。读取全部候选或释放暂扣尾部不退出格式页：格式已全部给出，没有词库尾部可放；若先清页再返回「未扩展」，runtime 会继续显示旧格式快照，下次选中却落到普通候选，甚至因索引越界不响应。选中页里一条格式照常上屏并结束组字。
- 入口自己不上屏：`commit`、`finish_composition`、`select_candidate_edge` 在 `index` 处看到 `menu:` 前缀就进页返回 `handled`，与参考的 `EnterDateTimeCandidatePage` 对应——参考靠宿主侧 `selected_text` 置 `OutofRange` 让 `HandleCandidate` 不发提交，这边没有那条路径，直接在引擎里不产提交。
- 开关是新的 `local_modes.date_time_menu` 偏好（默认开，缺键读作开），经 `LocalModePreferences` → host-api `EngineOptions::local_date_time_menu` → `LocalModeOptions::date_time_menu` 进引擎；`date_time` 关掉时行内行本就不出，入口随之消失，设置页把「日期格式入口」行禁用表示依附。Windows 出厂模板补 `utility.date_time_menu = true`，MAPPING 指到 `local_modes.date_time_menu`。
- 九宫格不给入口（`insert_inline_date_time` 传 `None`）：换页交互只在 26 键会话实现，九宫格选中入口会直接把标签上屏。

## Alternatives considered

- **在整句降位算法里按标签排除日期入口。** 改动集中在排序端，但标签是可见文字，排序输入没有 `menu:` 编码，按文字猜入口会误伤同名词；为它扩充整套宿主候选元数据也不值得。沿用网址补全已有的 `Fallback` 做法，在入口生成处表达它不是词网格读法，再在引擎快照里排除模型比较，两个现有判据分别约束降位和评分。
- **把入口做成普通候选、选中时在引擎里把整个列表替换成格式页并保存原列表（参考的实现方式）。** 行为最贴参考，但要新增一份「保存的列表 + 页内状态」并在每条重算 `mixed_candidates` 的路径上处理覆盖关系。视图方案（`candidates()` 覆盖、`mixed_candidates` 不动）天然实现了保存/恢复：退出就是置空，原列表一直在，不用写恢复逻辑。
- **入口放在全部行内行后面（第 4 或第 5 位）。** 行内行完整露出再接入口更直观。但参考的第三位是「中文首选、第一条日期、入口」，任务书也写第三位，锚定词在首位时两种放法只差入口在不在行内行中间；放在第一条后面与参考一致，打 `riqi` 时 `[日期, 2026年8月9日, 📅日期, 2026-08-09, 2026年8月9日 星期日]`。
- **宿主配合方案：引擎只发入口行、各宿主自己识别并换页。** 每个平台各写一份选中拦截和恢复逻辑，安卓、iOS、macOS、Linux、Windows 都要实现一遍才可能对齐；引擎里做一次所有宿主免费得到。参考把换页放在 server/宿主侧是因为它的候选窗和翻页状态在宿主，引擎协议里换页不是原语；这边候选集本来就由引擎给，没有这条边界。

## Consequences

- 收益：全部平台的 26 键全拼自动得到入口，不需要宿主改动；`date_time_menu` 关闭时回到原有行内行行为。
- 代价与已知上限：入口行是不可管理的 `Fallback` 候选，固定位置、pin、删除仍不可用；页内格式行没有 `pinyin`/`canonical_pinyin`，上屏不进词库不记上下文。九键没有入口，触屏九键用户看不到这条路径。页开着时异步候选（云、AI）仍会写进 `mixed_candidates`，退出页面后看到已合并的列表，与参考「页开着时屏蔽异步合并」不同——这里的合并不可见、退出时生效，结果等价或更新。
- 回滚：`insert_inline_date_time` 的 `menu_keyword` 传 `None`，入口不再出现；`date_time_menu_page` 相关守卫留着也不影响行为。

## Verification

- `cargo test -p msime-engine date_time_menu`：`session::tests::date_time_menu_opens_the_full_format_list` 覆盖入口位置、文案、选中换页、页内上屏、退格恢复、关闭开关；`local::date_time::tests::menu_row_follows_the_first_inline_row` 覆盖各组文案、`menu:` 前缀识别、无行不放入口、九键 `None`。
- `crates/input-runtime/src/tests/date_time_menu_order.rs` 使用合成词库与真正挂载的逐键打分器，通过完整输入会话分别输入 `riqi`、`rq`，覆盖无模型及整句池 9、32 的入口相邻顺序、入口不参与模型比较、重排后选中入口换页和格式上屏；运行结果以对应 PR 验证段为准。
