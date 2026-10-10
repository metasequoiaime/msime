# Agent Note: 日文 provider 复用罗马字与片假名转换缓冲

Status: implemented

## Problem

日文 provider 已复用最终候选行，却每次构造拥有型罗马字转换结果和片假名中间结果。真实红测试显示无词库热查询的单假名、完整、待定与 64 字节长读音分别分配 3、5、6、12 次；公开片假名转换为返回字符串之外的字符向量再分配一次。

## Decision

provider 持有 `RomajiConversion` 与片假名字符串；转换写入函数复用已有容量，通过现有扫描核心写入平假名和待定尾部。公开罗马字拥有型接口保留原冷构造路径。片假名写入逐字把 `U+3041..=U+3096` 加 `0x60`，其余字符原样保留；按输入字节数保留结果容量，不构造中间字符向量。公开片假名返回函数也共用此写入规则。`wana_kana` 固定为 `5.0.0` 测试依赖，作为旧行为对照。

模型仍按合法查询首次加载，裸 `-` 在加载前返回；模型句柄取得后才借用转换字段。候选去重、来源、权重、完整性和动态候选顺序均保持原规则。

## 既有笔记审计

[日文流式扫描](../../implemented/testing/2026-10-09-japanese-reading-stream.md) 部分重叠，继续持有共用扫描、请求与完整性检查的决定，本篇负责 provider 转换存储，两篇互链。[借用小写输入](../../implemented/testing/2026-10-08-japanese-romaji-borrow-lowercase.md) 部分重叠，继续约束 `Cow` 和公开罗马字冷预算，两篇互链。[临时日文候选行](../../implemented/testing/2026-10-08-temporary-japanese-row-buffer.md) 与[临时日文列表](../../implemented/testing/2026-10-08-temporary-japanese-buffer.md) 负责会话最终行，无关且保留。[拼法与假名位置](../../implemented/bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md) 继续约束行为，不改变。搜索 proposed、implemented、rejected 未发现另一套 provider 转换存储提案。

## Alternatives considered

- 继续拥有型转换再复制：接口最短，但最终行复用后临时字符串成为逐键查询的主要剩余分配。
- 为片假名保留字符向量缓存：可以沿用库结构，但需要额外缓存且还要把字符写入字符串；固定 Unicode 位移可以直接输出，并用原库完整对照。
- 为 provider 重写罗马字规则：便于单独优化，但会让 `nn`、促音和待定尾部与请求路径漂移，因此复用现有扫描核心。

## Verification

真实红测试中的单假名、完整、待定、64 字节长读音热查询分别分配 3、5、6、12 次；缓冲复用后均为 0。公开片假名非空冷转换从 2 次降到 1 次，空转换为 0 次。日文 59 项测试通过。完整引擎 1446 项、golden 31 项、Clippy、Rustfmt、差异与笔记检查通过；独立设计和代码审查没有遗留问题。完整 quick 门禁退出为 0 并报告 `quick check passed`：共享 Rust、Android 目标与宿主 Java、Linux 桌面共享层与原生宿主 73 项测试、macOS 原生宿主及共享 Apple bridge 构建通过；Windows、WASM 与 HarmonyOS 的缺失工具链或前置阶段按脚本明确跳过，未做设备验收。

全部有效 Unicode 标量汇集一次与固定旧库对照，结果与输入字节数一致；全部罗马字表及拼法前缀、大写、`nn`、促音、Unicode 待定尾部与拥有型转换一致。缩短、清空、待定、重新增长时转换指针不变且无分配。模型加载短路和失败不重试、完整候选字段和动态候选顺序都有回归覆盖。

## Consequences

provider 无词库热查询只复用原有转换和候选存储，不再创建临时字符串；片假名也不再使用临时字符向量。provider 在会话生命周期内保留峰值转换容量，多出平假名、待定尾部、片假名三个字符串与完整性字段。公开罗马字结果仍沿用原冷构造路径，大写直接调用仍为规范化分配一次；零分配预算只覆盖已有容量的转换和候选写入，不涵盖模型解码或句子搜索。

有词库查询使用内部词条视图减少读音和词面的临时复制，见[词条文本借用](2026-10-09-japanese-lemma-borrow.md)。本篇的转换和无词库热预算继续有效。

[句子结果容器复用](2026-10-09-japanese-sentence-output-buffer.md) 补充有词库查询的固定上限向量；本篇的转换存储与无词库零分配预算继续保持。
