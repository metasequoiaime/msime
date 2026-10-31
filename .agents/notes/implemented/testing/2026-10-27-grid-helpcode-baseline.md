# Agent Note: 网格辅筛实验前的普通输入回归基线

Status: implemented

## Problem

网格辅筛将接入 9／14／18 键共享的输入、读音选择、筛选和候选刷新。已有单项测试不能单独说明六种全拼／双拼组合在连续操作中的状态恢复仍保持一致。需要先固定普通输入轨迹，再实施新状态，避免实验改变默认输入而没有可比较的基线。

## Decision

`session/tests/grid_baseline.rs` 使用合成 SQLite 词条和固定按键串，经现有 Session 入口覆盖六种布局／方案组合。测试不调用编码函数生成预期，也不复制生产刷新算法。单字频率特意高于词语，断言完整覆盖仍优先；读音锁定撤销、筛选清除及新一段输入比较完整快照，部分提交检查剩余键码、读音及单字条件。

runtime 的 `grid_baseline_paging_then_spelling_rejects_old_candidates` 使用已有假引擎，覆盖它支持的九键与十四键路由：全局候选索引、换读音回首页、旧候选及旧侧栏代次拒绝、新页选择提交。它不声称验证十八键词典生成；六种方案的真实生成由 Session 用例覆盖。

这一切片只增加测试，不接入辅助码、不改变现有出厂值、候选搜索预算、排序或笔画语义。短前缀补全在布局间存在差别，固定共同的前两项及完整输入后的全列表，不强制不同布局产生同一补全池。

## Historical audit

检索网格回归、候选筛选和部分提交。以下笔记部分重叠但各自保留原决定：

- [九键过滤投影](2026-10-16-nine-key-filter-before-clone.md)对照旧刷新算法，保护优化；本篇保护跨入口操作轨迹，不替代其性能与缓存断言。
- [移动端部分选择](../bug-fix/2026-10-25-mobile-expanded-partial-selection.md)负责宿主面板保留及清笔画；本篇只断言真实引擎单字条件跨部分提交保留，不在引擎测试中模拟 UI 自动清笔画。
- [十四键](../feature/2026-10-10-fourteen-key.md)持有布局编码与路由决定，本篇固定按键复用该行为。

未发现完全吸收或需废弃的同一基线笔记；宽泛搜索命中的其他平台 UI、模型优化与本切片无关，不改动。

## Alternatives considered

- 只跑现有单项测试：无需增加夹具且已有覆盖广，但缺少六种方案统一的锁定—撤销—恢复轨迹；保留旧套件并补少量会话用例。
- 复制一份旧刷新作为实验 oracle：能逐字段比较，但仓库已有冻结刷新测试，再复制会增加维护成本；这里固定可观察结果和状态恢复，不再复制算法。
- 使用发布大词库生成快照：更接近实际输入，但依赖外部资源且频率变更会造成噪声；本切片采用合成词条，真实召回质量和时延留给后续阶段。

## Consequences

新增三个会话测试（各执行六组布局）及一个 runtime 测试（两种假引擎路由），能明确定位普通输入状态和候选身份回归。维护成本是固定样例需随有意的产品行为变化评审，而不是自动更新预期。

本机使用 Windows MSVC Rust 1.97.1。改动前现有 `cargo test -p msime-engine --lib nine_key -- --test-threads=1` 为 268 通过、15 忽略，无失败；忽略项保持原理由。后续本切片结果见同次交付记录，不修改 known-failures。

Android 完整 check-host 在 Git Bash 调 Windows rg 时因 `/proc/.../fd` 进程替换路径不可访问而失败；这不是产品回归。直接调用 JDK 的 JVM 子集与完整脚本分开报告，不能将子集通过写成全部 Android 门禁通过。繁体、真实编辑器、设备手势和辅码新功能不在本切片验证范围。

## Verification

- 新增 engine `grid_baseline` 三项通过（每项六种布局）；runtime `grid` 八项通过（包含新增分页用例）。
- 既有 engine `helpcode` 87 项及 `eighteen_key` 七项通过。
- `cargo clippy -p msime-engine -p msime-input-runtime --tests -- -D warnings` 与两 crate 的格式检查通过。
- `verify-notes` 包装器受 pnpm 自动安装权限限制，直接执行 package.json 中同三条 Node 校验，全部通过。
- Windows JDK 依 check-host 的选择规则编译 JVM 源码成功；290 项运行中 268 通过、22 失败。单独复跑本任务相关九项（网格布局／横滑、笔画面板、组字显示、部分选词面板）全部通过。失败位于未修改源码，涉及文件系统权限、链接与其他断言，未逐项归因，不增加豁免。
- 完整 quick 门禁单独记录其结果；本切片不将局部通过宣称成完整门禁或设备验收通过。
- 完整 quick 已执行但未通过：Rust workspace（不含尚未构建的 msime-desktop）检查通过；失败项为 harmony appgallery、harmony dictionary sync、windows enter observation、windows preedit caret、windows preference monitor wake、android host java、pipe-only configure。存在工具缺失、Windows 路径／MSYS 兼容及原生配置缺口；本次不修改这些未涉及文件，不添加 known-failures 豁免。此实验切片不能作为完整门禁全绿的交付。
