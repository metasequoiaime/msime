# Agent Note: 全拼首字母行缓存借用局部键

Status: implemented

## Problem

`query_database` 的单字节单音节分支每次先用 `format!` 创建带 `\u{1}` 前缀的拥有键，再查行缓存；命中时键立即丢弃。行缓存比上层路径缓存活得久或不同显示码查询同一首字母时，仍支付这一次申请，空页命中也一样。

## Decision

长度守卫保证音节是合法 UTF-8 的一个字节，因此必为 ASCII。用 `[1, initial]` 局部数组，经安全 `std::str::from_utf8` 借用为原两字节键，调用现有 `get_ref_by`。只在真正未命中时保留原 `format!` 构造缓存的拥有键，保持冷键字节、申请次数和容量策略。缓存、最终候选和所有显示字段仍各自拥有，不增加状态、依赖、公开接口或 unsafe。

## Existing decisions

按 `query_database`、`segment_row_cache`、首字母缓存键和局部键检索活跃笔记，没有同一两字节查找决定。[行缓存投影](2026-10-14-quanpin-row-cache-projection.md)部分重叠，继续持有行迭代与最终字段策略，本篇只处理查找键；[解析投影](2026-10-14-quanpin-resolution-cache-projection.md)与[系列前缀](2026-10-14-quanpin-series-prefix-borrow.md)属于外层其它键，保持不变。单字节前置条件采用 [Rust `str::len`](https://doc.rust-lang.org/std/primitive.str.html#method.len) 的字节长度语义；[安全字节切片转换](https://doc.rust-lang.org/std/str/fn.from_utf8.html)给出同样的局部数组借用用法。

## Alternatives considered

- 把首字母行改存另一种缓存：能用裸字母索引，查找更简单；不选，因为拆分当前共享 FIFO 会改变普通/首字母页之间的淘汰关系。
- 安全借用后在未命中时直接 `to_owned`：可以减少冷构造的格式化；不选，因为原 `format!` 的预留容量也属于本片保持的冷策略，先只消除热查询键。
- `from_utf8_unchecked`：长度不变量确实保证有效字节；不选，因为安全转换足以表达同一边界，没有引入 unsafe 的必要。

## Verification

- 固定 `194ec019b6a65ac90317ec24b3b1173636bee471` 的完整数据库查询及 `query_exact→query_series→query_single_path` 主链，只改名、转接冻结入口和移除注释。两个性能测试在旧生产上真实失败：空首字母页热命中仍有一次申请，完整 `n/n` 的路径答案清空但行页存留查询仍多一次。修复后四项专项测试通过，首字母行热命中恰好少一次；冷查询和非首字母控制成本相同。
- 128 个单字节输入 × 三种显示码 × 冷/热共 768 组，9 组非首字母输入 × 冷/热共 18 组，7 组完整输入 × 冷查询/行页存留且路径和系列清空/全热共 21 组。覆盖空显示、原显示与 Unicode 合成显示码，以及多字节、空音节和多音节控制；完整结果、顺序、容量、24 行限制、空页、FIFO 年龄、词典失效和独立结果均对照。静态表及预算分类放在测量外；完整链关闭词网格以隔离进程共享 n 元表并发淘汰。


- 首次全量 2,022 项通过，只有既有行投影累计预算失败：首字母热页又恰好少一次键申请。累计预算在原每行字段与热页投影收益之外仅增加这一次差额；冻结旧方法和非首字母控制不改。

- 最终引擎全量 2,023 项通过、16 项忽略，golden 31 项通过；Clippy `--all-targets -- -D warnings`、Rustfmt、差异检查及 867 篇笔记门禁通过。独立审查无阻塞项，四项专项测试独立复跑通过。本地 quick 门禁通过，覆盖工作区、Android/wasm 目标、macOS 原生宿主及共享 Apple bridge 编译。Linux 容器、Windows 交叉构建及 HarmonyOS 打包因缺本机工具链跳过；未进行设备验收或发布。

## Consequences

安全 UTF-8 校验仅靠紧邻的单字节守卫成立，不得移到无长度保证的入口。局部数组借用不能写进缓存，冷键仍拥有。确定收益只为分配次数，不据此推导 CPU、设备延迟、SQLite C 堆或 RSS。
