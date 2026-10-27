# Agent Note: 网格辅筛的显式引擎状态

Status: implemented

## Problem

九键、十四键和十八键的组码有歧义，大写已参与精确拼音输入；不能把全键盘的辅助码字符解析直接套进网格。只过滤显示页还会漏掉本轮已生成、但被截断的匹配词。

## Decision

在网格会话持有独立的两槽状态，公开带代次的有类型动作和独立显示快照。普通 character/grid_key 继续输入拼音并清除辅筛；辅码从专用入口追加。一码按首／末方向稳定分组且保留未命中，两码严格匹配，复用共享匹配器取码；覆盖长度先于辅码等级。全键盘默认值和触发规则不变。

本切片在原搜索预算内、候选截断前匹配；模型及固定位置处理后再守住排序边界。不把辅筛列表写回普通历史种子。阶段 3 继续接通 runtime、C ABI、共享偏好和 Android UI；只在网格辅筛启用且有条件时接管呈现排序。

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

独立 grid_helpcode_snapshot 提供可用性、编辑状态、下一码大小写、槽来源／精确值、整串选项和 H 显示文本。generation 防止旧操作误用；candidate_generation 只在刷新时前进，运行时据此区分纯编辑变化与需要回首页的候选条件变化。空编辑、切下一码大小写和结束编辑不查词。

现有搜索预算不变，截断前辅筛改善的是已经召回的候选；仍不保证未查询路径的完整召回。有效辅筛不混入附加候选、不写回普通历史种子。源码清除入口保留单字条件，部分提交先解析原候选再清辅码。两码无结果的 finish 拒绝自动原文回退；显式 CommitRaw 继续沿用旧原文语义，且不含辅码。

阶段 3 使用 msime_client_grid_helpcode 的有界 JSON 请求，带 runtime session/generation，复用共享动作枚举。View 的独立 grid_helpcode 对象不污染普通预编辑。条件变化回首页首项，纯编辑变化保持位置；有效辅筛关闭呈现重排、在线插入与插件变换。普通退格删辅码也按候选代次复位，即使文字顺序未变。

共享 HelpcodePreferences.grid_enabled 默认 false，由 Android 原生 TypingPage 提供“网格辅码优先”。控制键点按进入／切下一码大小写，长按分词，上滑符号，下滑退出编辑；左栏直接显示引擎的整串解释。符号覆盖层延后到实际选符号才提交，拼写编辑入口清除辅筛并暂时禁用进入。9 键双拼编辑辅码时显示实际拉丁字母组。全键盘入口不变。

API 35 和 API 28 已用原生 APK 验证全拼／双拼各三种网格的辅码、两码混合大小写、左栏锁定、第三码拒绝、符号返回、退出保留和分词，应用 composing text 不含辅码。键帽保持字母组，输入大小写由控制键、码槽和左栏解释表示。繁体取码、复杂光标编辑与 HyperOS 真机体验继续保留 review。

## Gate status

Windows 本机验证：新增十项通过；现有 nine_key 268 通过、15 原有忽略；helpcode 合计 97 项、grid_baseline 三项、eighteen_key 七项、runtime grid 八项通过。engine 与 input-runtime 的 clippy --tests -D warnings、格式和三项笔记检查通过。quick 结果单独记录；不修改 known-failures，不把定向测试通过写成完整门禁通过。

完整 quick 已运行，Rust workspace 检查通过，七个环境失败阶段为 Harmony AppGallery、Harmony dictionary sync、Windows enter observation、Windows preedit caret、Windows preference monitor wake、Android host Java 和 pipe-only build。均在此前基线对照中归因；原生 PowerShell 三个 Windows 探针已通过，pipe 配置已绕过但 MinGW 缺少 UIA 声明。实验 draft 保留门禁未全绿状态，不作为合并就绪依据。

阶段 3：新增共享偏好一项、C ABI 两项、runtime 身份耗尽一项回归通过；runtime 全量 376 项通过，engine/helper 97 项及 engine/runtime clippy 通过。严格 Java 编译、JNI／偏好字段契约通过。API 28 的 client-base/core/providers/services 分别 36/656/274/1359 项通过，按既有 runner 跳过 hard_link；providers 的词表 CRLF 哈希失败在基线复现，恢复 Git 中原始 LF 字节后通过，没有修改资源的 Git 内容。HostOptionsPolicySmoke 和 KeyboardFeedbackStoreSmoke 已在 WSL 的 Unix 文件系统语义下补验通过。

本轮 quick 仍是七项既有环境失败。host-api 的 downloaded_generation_falls_back_to_the_bundled_helpcode_tables 在固定基线同样因 Windows 路径前缀差异失败；没有更改断言或隐藏失败。完整发布门禁与真机体验仍未验收；本阶段 APK 为 x86_64 验证包，未包含可选语音运行库和按需语言词典，不作为发布产物。

原有 Android 专项回归：API35 十八键全拼的设置、组码、消歧、实体字母、布局回落与几何通过；九键候选面板的锁定、退格、笔画／单字筛选和最近读音通过。

API28 原有十四键双拼设置、组码、消歧、分号、自定义表与布局回落专项也通过。首次安装说明弹窗造成的首轮测试阻塞在弹窗结束后复跑解决；未改产品逻辑。
