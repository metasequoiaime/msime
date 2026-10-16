# Agent Note: 九键热候选先按读法筛选再复制

Status: implemented

## Problem

固定 `3cef2fd2439328c1f20b185c065d536f190ee0be` 的九键按键采样包含系列答案的候选深复制及随后析构。`refresh` 取得拥有页之后才排除超过路径音节数、不符数字前缀、锁定读法、首字母或单字限制的行，热槽也复制随即丢弃的行。采样只用于定位，不作为延迟成绩。

## Decision

精确查询提供同步投影，热槽借用，冷页移交；普通查询继续取得原拥有输出。九键入口的筛选谓词只接收词面、显示读法、规范读法和模糊标记，热页只复制通过行，冷页原地保留。模糊音先完整查询再筛选，保留合并语义。九键沿用同一条件，搜索与排序预算不变。

## Historical audit

检索候选投影、行缓存、`query_exact` 和过滤。[原始行投影](../../implemented/testing/2026-10-14-quanpin-row-cache-projection.md)负责 `DictRow` 到候选，本片处理外层系列答案，部分重叠并互链。[短前缀流式追加](../../implemented/testing/2026-10-16-quanpin-prefix-row-stream.md)管理系列内部页；解析投影、槽键所有权和标记笔记继续持有原状态与FIFO语义。没有完全吸收或过时提案。

## Alternatives considered

- 全链改共享字符串：可以省更多复制，但扩大缓存、会话、快照和学习的所有权边界；本片只同步借用。
- 模糊合并前筛选：收益更广，但按词去重中被拒行可能挡住另一行，提前删除会改变结果；保持合并后筛选。
- 缩小查询限额：实现简单，但会丢掉较后符合读法的候选，不能保持质量。

## Verification

旧查询包装先完整复制再筛选，单行全部拒绝的真实分配红测试中，旧/新均为7次；要求节省三个字符串副本而失败。新生产通过精确预算：热页每个拒绝行恰好省去其全部字段复制，页向量仍按原行数分配，全部保留时次数不增加。覆盖1/4/32/128行、全部/单条/半数/零行保留。

固定 `3cef2fd2439328c1f20b185c065d536f190ee0be` 的完整旧 `refresh`，仅改名并移除注释。对照覆盖三种网格、模糊开关、单字限制、六种合成读法、锁定/切分/首字母和冷/热两轮，共576轮完整刷新，比较快照、全部候选字段、字段容量、候选容器容量及种子缓冲。另以三条补全热页严格要求省去9次字段申请。读法筛选入口的真实SQLite矩阵覆盖冷热、别名、自动切分、模糊、缺词、非法与空输入，检查状态、清缓存、输出修改后的独立性，申请不增加。

完整 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 退出0：2188项引擎单测、31项golden和294项runtime测试通过，33项既有忽略。清除 `MSIME_EVAL_RESOURCES`，设置固定模型目录，真实模型用例运行。两crate全targets Clippy `-D warnings`、fmt和笔记门禁通过。独立只读审查核对冻结函数体及生产分配预算，无阻塞或重要问题。

## Fixed-resource quality

共同基线的旧release程序与本片新程序分别跑全拼60例、九键81例，完整质量JSON逐项一致；没有更新金标。模型SHA256为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。旧/新 `rerank_latency` SHA256为 `bcb55b0b292356b64a4efca422f11be810499a3b569b00f3e1ba90afbc53b905` / `9b1722fe34ce1170597947306c6a8f4314fc4f1d77597c57aa7c604efeb492de`。完整热刷新三条补全夹具为692→683次申请，严格省9次；这不是CPU或逐键时延。

## Local gate

`env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 退出0；契约、actionlint、笔记、Rust workspace、Android目标/Java、WASM、macOS原生宿主及共享Apple bridge通过，Windows本机可运行152项通过。Linux缺Docker，完整Windows及pipe-only缺交叉工具链，HarmonyOS缺完整打包环境，按脚本跳过；macOS workspace的桌面公共组件因未准备bundle资源跳过。未作设备或真实宿主输入验收。本片没有无采样延迟测量，不能宣称16ms达标。

## Consequences

筛选先于纠错来源标记，接口只提供允许读取的字段。临时容器容量可不同，输出字段及容量需保持。数字前缀覆盖仍在拥有行之后检查且只算一次，避免重复九键双拼的临时编码申请；本片只提前检查单字限制、音节数、锁定读法和首字母。热页容器仍保留原容量，只有被拒行的字段不再复制；冷页和模糊行仍先拥有。不能由分配节省宣称16ms达标。无文件访问、权限或Android资源改动，不作设备验收结论。实现依据[Rust Cow](https://doc.rust-lang.org/std/borrow/enum.Cow.html)和[Vec retain](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain)。
