# Agent Note: 网格辅筛的显式引擎状态

Status: implemented

## Problem

九键、十四键和十八键的组码有歧义，大写已参与精确拼音输入；不能把全键盘的辅助码字符解析直接套进网格。只过滤显示页还会漏掉本轮已生成、但被截断的匹配词。

## Decision

在网格会话持有独立的两槽状态，公开带代次的有类型动作和独立显示快照。普通 character/grid_key 继续输入拼音并清除辅筛；辅码从专用入口追加。一码按首／末方向稳定分组且保留未命中，两码严格匹配，复用共享匹配器取码；覆盖长度先于辅码等级。全键盘默认值和触发规则不变。

本切片在原搜索预算内、候选截断前匹配；模型及固定位置处理后再守住排序边界。不把辅筛列表写回普通历史种子。API 暂在 Rust Session 层提供，不新增 Android UI、C ABI 或持久化偏好；运行时分页与宿主动作接线随阶段 3 实施。

## Historical audit

- [普通网格基线](../testing/2026-10-27-grid-helpcode-baseline.md)部分重叠，保留原行为测试决定。
- [十四键](2026-10-10-fourteen-key.md)负责分组拼音，保留；本篇使用相同拉丁键面，不复用双拼声韵母表。
- [双拼显式 Tab](2026-10-23-shuangpin-manual-tab-helpcode.md)部分重叠，适用全键盘字符流，网格不写入其内部标记。
- 其它命中涉及平台 UI、手写和句子模型，与本决定无关。没有需替代或归档的同一提案。

## Alternatives considered

- 复用大写／竖线字符入口：接线少，但与精确拼音冲突且污染 composing text，因此使用独立动作。
- 只过滤现有 128 项：容易接入，但丢掉本轮截断前已存在的目标；因此匹配先于最终截断。额外路径／SQL 召回留待阶段 4。

## Verification

新增十项匹配／会话测试，其中三项在六种布局上执行。覆盖整串消歧、大小写、满两码、旧代次、空结果、部分提交、继续输入清除、笔画互斥、26 键隔离，以及原 128 项以外但本轮已召回的第 150 项目标。清除后逐字段比较普通候选快照。

共享匹配器差分测试还固定一个容易误读的旧规则：count_han_chars 实际数全部 Unicode 字符；“你A”走多字符分支，对首尾汉字“你”的首码各匹配一次。网格直接复用匹配器，不根据函数名字另写分支。

## Consequences

独立 grid_helpcode_snapshot 提供可用性、编辑状态、下一码大小写、槽来源／精确值、整串选项和 H 显示文本。generation 防止旧操作误用；candidate_generation 只在刷新时前进，后续运行时可区分纯编辑变化与需要回首页的候选条件变化。空编辑、切下一码大小写和结束编辑不查词。

现有搜索预算不变，截断前辅筛改善的是已经召回的候选；仍不保证未查询路径的完整召回。有效辅筛不混入附加候选、不写回普通历史种子。源码清除入口保留单字条件，部分提交先解析原候选再清辅码。两码无结果的 finish 拒绝自动原文回退；显式 CommitRaw 继续沿用旧原文语义，且不含辅码。

本切片只有 Rust Session 接口。输入运行时的分页／高亮、拼写编辑进入、面板覆盖恢复、C ABI、共享偏好与 Android UI 尚未接线；它们是阶段 3 的交付，不宣称当前 APK 可用。繁体取码、应用 composing text 和设备触摸仍待 review。新开关缺省关闭，不更改全键盘出厂值。

## Gate status

Windows 本机验证：新增十项通过；现有 nine_key 268 通过、15 原有忽略；helpcode 合计 97 项、grid_baseline 三项、eighteen_key 七项、runtime grid 八项通过。engine 与 input-runtime 的 clippy --tests -D warnings、格式和三项笔记检查通过。quick 结果单独记录；不修改 known-failures，不把定向测试通过写成完整门禁通过。

完整 quick 已运行，Rust workspace 检查通过，七个环境失败阶段为 Harmony AppGallery、Harmony dictionary sync、Windows enter observation、Windows preedit caret、Windows preference monitor wake、Android host Java 和 pipe-only build。均在此前基线对照中归因；原生 PowerShell 三个 Windows 探针已通过，pipe 配置已绕过但 MinGW 缺少 UIA 声明。实验 draft 保留门禁未全绿状态，不作为合并就绪依据。
