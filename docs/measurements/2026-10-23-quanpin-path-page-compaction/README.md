# 全拼路径页压缩测量

基线为 `53df4a89c1db06d264eadf4e57cb0b54758d61fa`。验证期间快进到 `546e89b5a60fe70c16e8eab3dc34965f977654be`，新增的 UI 与本地门禁提交不改 engine/runtime 或依赖；quick 使用更新后的脚本。只使用仓库公开合成集合；不记录私人输入。固定资源锁为 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`，神经依赖仍为 `15caab4fdeb59c11f452b91884371cf30e2e5608`。

## 表示与局部实验

容量保持 128。普通页仅保存规范键、词面、权重及可选的每页读音，默认元数据在输出时还原。与缓存键匹配的页至少 2 行，不匹配至少 3 行，空读音和非默认元数据回退完整页。展开页用 `From<Vec<WordItem>>` 原样转移。空查询使用独立 `Empty` 变体。

实际 `WordItem` 为 136 字节、`DictRow` 为 56 字节、`PathPage` 与旧 `Vec` 均为 24 字节。压缩页的 48 字节 Box 主体、行容量及全部持有字符串都计入局部请求字节。键、FIFO 节点和哈希表的数量及生命周期保持原规则，缓存值尺寸相同；局部请求字节只比较页负载，不代表 RSS。

`cpu.rs` 和 `run_cpu.py` 链接 release 的真实 `WordItem`，包含生产 `PathPage` 和原 FIFO。使用普通分配器及线程 CPU，7 种行数 × 2 种键长 × 2 种读音关系 × 3 种操作共 84 格，每格 21 对 AB/BA 交替。旧臂批量先校准至至少 3 ms CPU。冷臂包含 128 槽初填、256 次淘汰和最终析构；热臂在计时外存页，计时内输出及析构。计时前核对全部字段和请求容量。

预定门槛：32/128 行冷插入中位新/旧比不超过 0.98，其余不超过 1.05。首轮只有 4 个空页热复制格失败（1.0780–1.1189），完整结果在 `v1-cpu*`，原表示在 `v1-path_page.rs`。符号表未见独立的复制方法后，增加 `Empty` 变体，重跑完整 84 格，全部通过；其余控制最大 1.0456。所有失败记录保留，没有筛选格或改变门槛。

| 第二轮操作，32/128 行 | 每格中位新/旧 CPU 比范围 |
| --- | --- |
| 冷插入 | 0.6755–0.7033 |
| 热复制 | 0.8940–0.9076 |
| 热追加 | 0.8917–0.9088 |

128 行、匹配 8 字节键的缓存页请求字节从 20626 降至 9410；匹配 64 字节键从 27794 降至 9410。两轮均处于共享宿主负载下，线程 CPU 的局部对照不能证明端到端墙钟或默认九键 16 ms 达标。

九键 ABBA 的 180 秒启动预检未满足空闲条件（load1 165.69–230.00，监测到 22–65 个外部构建/评测进程），程序退出 1，没有启动任何计时臂。保持原 16 ms 预算与 5 例预热；默认九键 16 ms 仍未验证。

## 复现

在仓库根目录先构建 release 的 `msime-input-runtime` 两个 example，原程序分别复制到 `target/page-compaction-evidence/baseline-{rerank_latency,convert_eval}`，候选程序同理保存为 `candidate-*`。二进制 SHA-256 在构建与质量记录中固定。

```sh
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/run_cpu.py
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/verify.py --resources <verified-resource-dir>
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/quality.py --resources <verified-resource-dir> --variant baseline
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/quality.py --resources <verified-resource-dir> --variant candidate
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/run_abba.py --resources <verified-resource-dir>
python3 docs/measurements/2026-10-23-quanpin-path-page-compaction/package.py --resources <verified-resource-dir> --main-checkout <main-checkout>
```

`run_abba.py` 要求连续 30 秒没有被监测的外部构建/评测进程且 load1 不超过 10，最多等待 180 秒；未满足时不启动计时。实际九键 ABBA 保持 81 例、预热 5 例、预算 16 ms。它需要在其它验证结束后单独运行。

## 验证证据

`red.log.gz` 固定旧入口先失败，`green.log.gz` 记录新增 3 项专项通过。SQLite 申请矩阵覆盖 0/1/2/3/4/32/128 行以及普通、不同、空和 Unicode 缓存键；核对字段、容量、独立输出、重置及析构。完整展开页按原 FIFO 淘汰后恢复 24 行有限页。首次全量回归仅会话光标测试的预算因净省 4 次申请失败（121 对旧 125）；5 条 `ni'h` 匹配行在计量外单独核对，预算精确收紧为 `125 - (5 - 1)`。原失败日志以 `first-*` 保留，没有新增已知失败。修正后的全量 engine 2521 项、golden 31 项、runtime 365 项通过；56 项原有忽略项保持不变。Clippy 全 targets（`-D warnings`）及格式检查通过。完整 quick 在 5250.894 秒后退出 0，覆盖 Rust workspace、Android target 与宿主 Java、两种 WASM 配置、Linux 桌面壳和原生宿主（111 项测试）、macOS 原生宿主构建及 Apple 共享桥接层；本机 Windows 原生逻辑 164 项测试通过。Windows 完整构建、HarmonyOS 真工程因工具链/暂存环境缺失跳过；没有设备装机或界面验收。其余结果见同目录完整日志与状态文件。

`package.py` 去除本机用户目录、worktree 和资源目录；gzip 时间戳固定为 0，`manifest.json` 为每个文件记录字节数与 SHA-256。二进制和构建产物不提交。
