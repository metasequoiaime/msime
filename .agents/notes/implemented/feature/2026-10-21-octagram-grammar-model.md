# Agent Note: Rime octagram `.gram` 语法模型读取端与逐边打分

Status: implemented

## Problem

旧版 Windows 正式版（`metasequoiaime/msime-windows` v0.11.1）支持下载 octagram 语法模型（`.gram`），整句解码时对每个词边叠加搭配分，能把词频串错句拉回正确搭配。新架构只有 bigram/trigram + 神经重排，没有这条路径。

## Decision

新增 `crates/engine/src/lattice/octagram.rs`：纯 Rust 只读 `.gram` 查找模块。文件头 32 字节格式名 + 校验和 + 双数组大小，`Rime::Grammar` 魔数；正文是 darts-clone 布局的 u32 单元数组，查询键按旧版自定义编码展开码点（ASCII 1 字节、0x4000–0xA000 两字节、其余 7bit 组变长，上限 8 个码点），值为 `ln(共现频次)×10000` 的 i32，负值按 -1 处理。解码侧把查询结果乘 `LOG10_E` 换到主分域。

`GramDb::shared` 经 `memmap2` 惰性映射并按路径共享 `Arc`；`load` 对缺失、截断、魔数不符、单元越界一律回 `None`，与 `msime-bigram.bin` 缺失的既有降级一致。`SentenceAssociationOptions` 新增 `collocation_model`/`collocation_weight`/`collocation_rerank`/`collocation_grammar` 四字段，全拼/双拼词典在 `set_sentence_association` 装载模型，`Option<GramDb>` 为 `None` 时打分器不装配。

解码端 `LatticeOptions` 新增 `collocation_scorer`/`collocation_weight`/`include_collocation_best`。束搜索对每个出边在 `collocation_scorer` 激活时叠加 `weight × scorer(尾窗, 词, is_rear)`；[尾窗按假设回溯一次](../testing/2026-10-23-lattice-collocation-tail.md)，取 `prev` 链最后 8 个码点并供该假设所有出边借用，不增 `Hyp` 字段，未激活或没有出边时不构造尾窗。句尾词（`edge.end == n`）触发 `.gram` 的 `$` rear 项。`CandidateSource::Collocation` 追加在枚举末尾（wire 值 17，已有值不变）；`is_generated_or_fallback`/`is_sentence_learning`/`static_evidence` 把它按 Generated 对待，保证选中搭配句仍走句子学习。

合并端：`include_collocation_best` 开时首条词格行标 `Collocation`（它是解码最优路径，不是独立来源）；`include_lattice_best` 只在搭配开关同开时才让首行之后的备选继续上榜——无重排通道本来忽略它，基线语义不动。九键词典设置透传新字段，`neural_keyboard` 仍强制关。`series_slot_key` 的参数改为引用，避免 `SentenceAssociationOptions` 失去 `Copy` 后的无谓克隆；grammar 字段不入键——模型路径变化会触发 `set_sentence_association` 里的 `reset_cache`，同一路径下模型内容不可变。

许可：万象 LTS = CC-BY-4.0（可用）；八股文 = LGPL（按资源包附带来源声明）；白霜 = GPL-3.0 且构建链许可未声明，本轮排除，进资源包前须重新评估。读取端按 BSD-3 的 librime-octagram 与 darts-clone 布局实现；本 PR 同时接入偏好、锁定 SHA-256 的按需下载与桌面设置，模型不随安装包分发。

## Alternatives considered

- `Hyp` 内联 33 字节尾窗（每个假设固定 40 字节增量）：会让冻结参考实现与现行实现的内存断言全部失真，改为激活时沿 `prev` 链回溯几跳取尾窗，未激活零成本。
- 搭配分后置重排：只换顶层候选顺序，不进入束内路径选择，弱于逐边打分且多一次遍历，不采用。
- 每次查询直接 `GramDb::load`：文件映射重复开销，采用 `shared` 共享；卸载入口通过 host-api 调用 `evict_shared`；活跃会话的引用仍保留到下次空闲重建，Windows 拒绝删除时如实报告失败。

## Testing

- `cargo test -p msime-engine` 2388 项全绿：octagram 单测覆盖编码边界、缺/损文件回 `None`、真实 darts 布局查询、上下文窗口与词首/句尾命中、弱搭配罚分；`decode_collocation_tests` 三项覆盖① 有 gram 时「一名一生」被纠正为「一名医生」、② scorer 在场但 weight=0 与无 scorer 输出逐字节一致、③ 缺/损文件经 `Option` 装配退回基线；`merge_collocation_tests` 三项覆盖 Collocation 标签、只留搭配行、双开关关时无重排基线照旧。
- `cargo fmt --all --check`、`cargo clippy -p msime-engine --all-targets --locked -- -D warnings` 干净；`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 通过。
- **真实模型已实测**（2026-10-21，用下载的模型包而非 fixture）：
  - `zh-hans-t-essay-bgw-compact.gram`（八股文·词级紧凑，39 MB）：`GramDb::load` 成功，`北京+大学` 打 −0.393、`北京+大雪` 打 −2.676，端到端把词频偏向的「北京大雪」纠正成「北京大学」。
  - `zh-moqi.gram`（白霜/墨奇，7 MB）：`load` 成功，但搭配覆盖稀疏，多数二元组落在未命中惩罚 −6.0 上（`我们+一起` −4.867 是少数命中）。端到端纠正那条按模型能力跳过。
  - 两条真实模型测试用 `option_env!("MSIME_REAL_GRAM")` 注入路径，CI 上没有模型包时打印原因后跳过，不会假绿。
- 未验证：万象 LTS（390 MB，下载太慢未取）与八股文·词级（197 MB）；九键/全拼候选窗的 Collocation 行外观。Windows 原生设置页的后续实测见下文。

读取端保留 RIME Developers 的版权声明，完整 BSD-3-Clause 文本经六个平台已有许可渠道分发；模型数据许可与代码移植许可分开记录。

## Consequences

模型缺失或损坏时行为与改动前逐字节一致；有模型时词格解码首选可被搭配分改写并标为 `Collocation`。`SentenceAssociationOptions` 不再 `Copy`，host-api 结构体镜像处已同步默认值。偏好枚举、资源包 SHA-256 校验与桌面设置单选在本 PR 一同接入。

## 模型目录（与旧版 Windows 正式版一致）

| 模型 | 文件 | 大小 | 许可 | 上游 |
|---|---|---|---|---|
| 万象 LTS | `wanxiang-lts-zh-hans.gram` | 约 380 MB | CC-BY-4.0 | `amzxyz/rime-wanxiang-data@LTS` |
| 八股文·词级 | `zh-hans-t-essay-bgw.gram` | 约 197 MB | LGPL-3.0 | `lotem/rime-octagram-data@20260712` |
| 八股文·词级紧凑 | `zh-hans-t-essay-bgw-compact.gram` | 约 39 MB | LGPL-3.0 | 同上 |
| 白霜/墨奇（实验） | `zh-moqi.gram` | 约 7 MB | GPL-3.0，构建链未声明 | `gaboolic/rime-frost@d57f35b3` |

四个模型来自三个仓库，所以锁文件按模型一条记录 `source_commit`，不共用。
白霜的地址钉在 40 位提交上（上游 `master` 会动），实测该地址的字节与锁里的
SHA-256 一致。许可声明在 `resources/licenses/rime-baguwen-LGPL-3.0.txt` 与
`rime-baishuang-GPL-3.0.txt`。

## 2026-10-22 审查补充

资源包卸载通过 `msime-host-api` 的边界摘除共享映射，桌面宿主不再直接依赖 Engine；卸载命令与其他桌面资源包命令使用相同的平台条件，不进入 Android/iOS 的 Tauri 命令表；白霜模型因 GPL 及构建链许可未确认继续不在桌面设置提供。模型包安装或删除后，宿主在下一次聚焦时比较当前 `.gram` 路径并触发 Engine 重建。读取端拒绝数组偏移落在文件末尾之外的非法头部。

资源包卸载复用语音安装器的跨进程锁、目录描述符与先整体改名的流程，只处理已发布的真实目录，安装期间拒绝卸载；符号链接和未发布目录保持原样。活跃会话持有的映射不会被强制释放，Windows 的占用错误仍通过原有错误入口报告。

`msime_client_resource_pack_remove` 在公共 `msime_client.h` 声明与 Rust 导出相同的字节缓冲区签名，原生宿主可直接包含头文件调用，响应使用标准 JSON 封装的 `{removed: bool}`。只补充导出而遗漏头文件会使 C/C++ 调用方编译失败；宿主接口一致性回归与 C、C++ 编译检查覆盖这一边界。

## 2026-10-24 补充：Windows 原生设置页模型入口

Windows 原生设置页现在与共享设置页提供同一组能力：桌面神经联想、去重后补充次选、语法模型选择，以及万象 LTS、八股文词级和八股文紧凑模型的按需下载、校验、取消和删除。页面只把 `state_directory()` 作为 `state_root` 交给 `msime_client_resource_packs*`，不复制锁文件、SHA-256 或模型路径规则；下载使用已保存的资源镜像前缀，删除前由 host-api 释放共享映射。白霜仍因许可未确认不列出，默认值不变。

这次没有把旧版任意 `.gram` 文件导入做成入口：共享层只接受锁定资源包，任意导入会绕过许可和内容校验。模型选择只写 `sentence_association.grammar_model`，安装完成后由现有会话在下次聚焦时重新读取，避免设置窗口直接触碰输入线程。

选择模型后沿既有 `change(..., true)` 延迟刷新页面，让当前模型对应的下载或删除入口立即出现；切回「不使用」时移除模型文件行。只保存值而不刷新能保留下拉框实例，但模型文件行由建页时的模型选择决定，会把首次选择模型后的下载入口留空，或者继续显示旧模型的操作，因此不采用。

契约测试同时钉住三个偏好键、资源包查询/安装/删除入口和白霜未列出，避免 Windows 设置页以后只保留静态下拉而失去实际管理能力。原生 `settings_ui.ps1` 增加浅深色、800／1000 宽下的四种模型切换，检查持久化、立即出现的对应下载入口、旧入口移除及导航后重读，并滚到模型设置处截图；不点击下载按钮、不访问模型服务。该专项已在 Windows CI run `40061182130` 的真实 WinUI 中通过，分别输出浅深色通过记录；已查看浅色 1000 宽的万象、深色 800 宽的紧凑模型与不使用截图。证据来自产品修复提交 `2d2155cfe3`；随后 `9834be79f2` 只修复三个原生测试的 MSVC 严格告警，完整 pipe-only 重建和三项 CTest 通过。模型下载与引擎加载不在此 UI 专项的覆盖范围。

独立 Windows 工作区的同源生产 host-api DLL 另已通过真实资源链路验收（DLL SHA-256 `76520af29246332d7c05240f5e18cb9694032a7fc2308a412aaa9c213bcaa666`）：三种包在登记后的首次回调中均可取消；紧凑模型在实际收到下载字节后取消，状态仍为缺失，重试后完成续传、安装和锁定的 40,925,228 字节及 SHA-256 校验。使用同一 DLL、独立合成词库与状态目录建立全拼会话，发送合成拼音并得到候选，同时用 Windows `VirtualQuery`／`GetMappedFileNameW` 确认进程已映射锁定的 `.gram`。销毁会话后经资源包 C ABI 删除，查询回到缺失，文件与映射均消失。此项不覆盖万象和非紧凑八股文的完整下载，也不证明真实编辑器中的模型排序效果；不把 C ABI 调用写成 WinUI 下载／取消／确认删除按钮已被点击。

## 2026-10-25 补充：Windows 桌面神经联想补上模型下载行

上一条补充说 Windows 原生页「与共享设置页提供同一组能力」，实际只做到了一半：「桌面神经联想」只是一个写 `sentence_association.neural_desktop` 的开关，页面从不列出、也不下载 `settled-model`。Windows 安装包从 `Prepare-PackageFiles.ps1` 改为不内置落定重排模型之后，安装包里没有这个模型，设置页也拿不到它，于是在 Windows 上打开这个开关不会产生任何效果。共享页的做法是开关打开后在下面显示 `settled-model` 资源包行。

`build_neural_desktop_settings` 照共享页的做法：开关打开时显示「神经联想模型」行，下载、更新、取消、删除与语法模型共用 `sentence_pack_row`，状态查询的过滤名单也加上 `settled-model`。删除确认按包区分措辞。下载由用户点按钮发起，不在打开开关时自动开始，与本页语法模型行的交互一致；共享页的自动 `ensure` 依赖它常驻的资源包状态，原生页首次建页时状态可能还没查回来，在那时自动安装会对已装好的包重复下载。

Windows 0.3.0（2026-10-23 发布）早于本节和上一节，更新说明列出了 octagram 支持，但设置页里两个入口都没有；`docs/sentence-models.md` 按版本写明了这一点。契约脚本 `scripts/test-windows-input-habits-settings.py` 增加 `settled-model` 下载行与 `build_neural_desktop_settings` 调用的断言。原生 UI 专项没有覆盖神经联想开关和这一行，WinUI 中的实际下载还没有验收。

## 2026-10-22 补充：octagram 调参系数进偏好（对应 Windows `sentence_collocation_*`）

Windows v0.11.1 出厂模板的 `[association]` 有七个不进设置页的高级调参键：`sentence_collocation_weight`（1.0）、`sentence_collocation_max_length`（6）、`sentence_collocation_min_length`（2）、`sentence_collocation_penalty`（-14.0）、`sentence_collocation_non_penalty`（-6.0）、`sentence_collocation_weak_penalty`（-100.0）、`sentence_collocation_rear_penalty`（-18.0）。新架构的 `GramConfig` 早已是这些参数的结构体，但值是定值，偏好层不可调。本轮把同名七键加进 `SentenceAssociationPreferences`（键名逐字与 Windows 一致），host-api 的 `engine_sentence_association` 逐字段透传进 `collocation_weight` 与 `collocation_grammar`。

**不进设置页**与 Windows 一致：两边都只让它们在配置文件里可写，共享设置页不加控件；TS 侧对象经 `...value` 展开保存，未知键原样保留不会被抹掉。

**校验口径选了「逐键回退」而不是「整份拒绝」**，照 Windows 的实际做法：参考实现只走 `value_or(出厂值)`，类型不可转才回退，没有显式范围或有限性校验（`cn_en_mixed_input_*` 那种 clamp 是别的键的写法，不在这组上）。本侧用 `deserialize_with` 把「类型不符 / i32 放不下 / 非有限」逐键回出厂值，文档其余部分照常生效——偏好文档是全量写入的，整份拒绝会让一个手误把用户全部设置打回默认，比 Windows 的后果重得多。越界的有限值（如 `max_length = -5`）照 Windows 原样透传：引擎端先用饱和减法求窗口，再以 `n < 1` 与 `min(n, 8)` 限定安全范围，不为它发明语义。**与 v0.11.1 的差异（只记录）**：Windows 的 TOML 写 `inf`/`nan` 会原样进打分；本侧拦了非有限值（JSON `1e400` 可解析出 `inf`），属刻意的更严一侧。

`SentenceAssociationPreferences` 因引入 `f64` 字段移除了 `Eq`（保留 `PartialEq`），`Copy` 不变。缺键时由容器级 `#[serde(default)]` 落到 `Default` 出厂值，等于出厂值时 `skip_serializing_if` 不写进文档（与 `quote_style` 一致）。`scripts/test-windows-config-keys.py` 的钉住来源（345cb87a）早于这组键，不受影响；`preference_coverage.py` 因 host-api 直接读这些字段名而判定可达，无需 `NOT_APPLICABLE` 登记。

## 2026-10-23 补充：极小窗口长度先饱和减一

`sentence_collocation_max_length` 接受全部可表示的 `i32`，包括 `i32::MIN`。原查询端先执行 `max_length - 1`，之后才取零下限；这使该值在开启溢出检查时 panic，关闭检查时回绕并意外启用 8 字窗口。默认 macOS 开发构建同样开启溢出检查；启用有效语法模型后，非空上下文的逐边查询会经过这一步运算。

查询端用 `saturating_sub(1)` 保住现有负值退回 `non_collocation_penalty` 的约定；其余值的评分与 8 字上限沿用原逻辑。配置反序列化层拒绝或重置负值能提前排除异常输入，但会改变已允许的手调值，且无法保护直接构造 `GramConfig` 的调用方，因此不采用。关闭溢出检查能避免 panic，却把负值变成有效窗口，也不采用。

`query_bounds_extreme_configured_window_lengths` 使用合成 `.gram`，覆盖最小整数、其相邻值、普通负值、零、1、正常正值及最大整数，分别检查句尾和非句尾查询。修复前在最小整数处复现减法溢出；正常窗口仍需命中已知分值，避免以一律关闭搭配查询来通过边界测试。

## 2026-10-23 补充：人工资源探针绑定模型内容

`real_gram_loads_and_queries` 是显式 `--ignored` 运行的人工资源探针，`GRAM_PROBE_PATH` 必须指向 `resources/grammar-models.lock.json` 里的 `baguwen-compact`。缺少路径时明确失败；对 `GramDb` 已映射的同一份字节先校验大小和 SHA-256，再查询，避免另读一份模型而让校验内容与查询内容分离。普通测试仍忽略这条，不下载或随仓库提交模型。

旧断言假定「一名+医生」总比「一名+一生」强，但锁定八股文紧凑模型实际给出 −4.0443 与 −3.8286，因而在正确加载的模型上失败。独立参考以 [darts-clone 固定提交](https://github.com/s-yata/darts-clone/blob/e40ce4627526985a7767444b6ed6893ab6ff8983/include/darts.h) 的 `exactMatchSearch` 穷举上下文后缀与词前缀，按 `max(频次值 / 10000 - 14)` 和句尾 `词+$` 的 `频次值 / 10000 - 18` 取分；「北京+大学」「北京+大雪」分别为 −0.3926、−2.6763。四个参考值对应资源锁中 SHA-256 为 `d3cb2438c1fdcd6a855dd6ca8f5c1060a29273c6b64c2c2c69af67cd71b6aa7e` 的模型。探针使用默认 `GramConfig` 对照四个分值，`1e-9` 容差只容纳浮点舍入，小于模型 `1/10000` 的分值步长。

只检查搭配排序所需的预期少，也较能容忍模型重训，但模型间没有普适排序，而且两分同时偏移也可能通过；因此采用锁定模型和独立分值。只检查能加载与有限分更适合任意模型的冒烟，却无法发现读到错误模型或错误分值，因此不作为这条探针的验收口径。修复对象是人工探针的模型假设，不改变生产评分。
