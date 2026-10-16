# Agent Note: 用 cargo-fuzz 覆盖扩展包导入与宿主 C ABI

Status: implemented

## Problem

仓库里没有任何模糊测试。威胁模型（`.oss-scanner/threat_model.md`）列为最高风险的两处入口都直接接收第三方或任意应用送来的数据：扩展包导入解析用户选中的 `.zip`，`host-api` 的 C ABI 接收系统输入法框架转来的每一次按键和宿主传入的缓冲区。单测只覆盖写测试的人想到的输入；内存安全问题、panic、无界分配和病态耗时要靠变异输入去撞。

## Decision

在仓库根目录的 `fuzz/` 放一个 cargo-fuzz crate（`msime-fuzz`），它有自己的 `[workspace]` 和 `Cargo.lock`，不进根 workspace。新增的依赖只有 `libfuzzer-sys` 0.4，只出现在 `fuzz/Cargo.toml`；`rusqlite` 固定为与根 workspace 相同的 0.40，因为 engine 链接的 libsqlite3-sys 在一个程序里只能有一份。`fuzz/Cargo.lock` 从根 `Cargo.lock` 复制后再解析，共享依赖的版本与主 workspace 一致。

两个目标：

- `plugin_pack`：把输入写成 `pack.zip` 交给 `msime_client_services::plugins::validate`。它与 `plugins::import` 共用 `stage` 和 `check_staged`，覆盖 zip 目录解析、成员名与路径规则、大小上限和各类清单解析，只是不装进插件目录。
- `host_session`：第一个字节从 12 种方案与触屏布局组合里选一个，用 `msime_client_create` 打开会话；其余字节解码成一串 C ABI 调用（字符、标点、编辑与翻页命令、选词、置顶与删词、九键选拼音与筛选、14 键、各类开关、滑行、英文补全、带上下文的标点、中途经 `msime_client_update_preferences` 换方案、重排、取视图），最后 `msime_client_destroy`。回应按宿主的方式读出并交回 `msime_client_string_free`，并断言它是带 `ok` 的 JSON。

设计上的几条约束：

- 每轮用新的临时目录和一份只有十几个词条的拼音、五笔、快捷短语词库。学习写下的东西不带到下一轮，崩溃输入因此能单独复现。
- 选词类调用带上一步回应里的 `generation`，否则都会被当作过期选择拒掉，碰不到选词之后的代码。
- `response()` 用 `catch_unwind` 把 panic 变成 `internal runtime failure`，但 libfuzzer-sys 装的 panic hook 会先中止进程，所以被接住的 panic 照样报告。
- 产物放在 `target/fuzz`，沿用仓库唯一的产物目录；带插桩和 ASan 的构建不能与 `target/` 下的普通构建共用产物。
- 种子语料提交在 `fuzz/seeds/`（两个最小的合法扩展包 zip，十二条覆盖各方案的按键序列）；运行时新发现的输入写进被忽略的 `fuzz/corpus/`。

`fuzz/` 不接进 `verify-local.sh` 或 CI。它需要 nightly 和 cargo-fuzz，一次带 ASan 的完整构建在 Mac Studio 上要二十多分钟，有价值的是长时间运行，而不是每次推送都跑一下。运行命令写在 `fuzz/Cargo.toml` 开头。

## Alternatives considered

- **在 `host-api` 内用 proptest 写性质测试**：最强的理由是不用 nightly，能直接进 `cargo test` 和现有门禁。不用它，是因为 proptest 没有覆盖率反馈，在这么大的状态空间里只能随机撒点；也没有 ASan，而这次最想抓的正是 C ABI 和 `unsafe` 边界上的内存问题。
- **直接向 OSS-Fuzz 提交项目**：最强的理由是 Google 免费长期运行并自动报告。不先做这个，是因为 OSS-Fuzz 的收录门槛（用户量或对基础设施的重要性）与仓库目前的年龄不相称，审核要几周；而且无论收不收，都要先在仓库里有能跑的 fuzz target，现在这一步是它的前提。
- **只 fuzz 纯函数（例如单独的 zip 解析或清单解析）**：每秒执行次数会高一两个数量级。不这样做，是因为真实风险在组合起来的路径上：解压后的路径规则、暂存目录、清单与文件清单的对账，以及按键经过 `host-api` → `input-runtime` → `engine` 的整条链。只测某一层，接缝处的问题照样漏掉。

## Testing

在 Mac Studio（arm64，nightly 2026-10-10，cargo-fuzz 0.13.2，默认 ASan）上，从种子语料各跑一段：

- `plugin_pack` 5 分钟：83,617 次执行，约 277 次/秒，新增 949 个语料单元，无崩溃，峰值 RSS 296 MB。
- `host_session` 10 分钟：11,552 次执行，约 18 次/秒，新增 2,298 个语料单元，覆盖约 28,800 条边，无崩溃。RSS 在约 5,000 次之后稳定在 505–528 MB，与 ASan 的 quarantine 相符，不是逐轮泄漏。

`host_session` 报过一次 31 秒的 slow unit（日文方案下的退格、移动和重排序列）。单独重放只要 105 ms，两次结果相同；当时 Studio 的 load average 在 116 左右，几个会话在同时编译，所以记为环境噪声，不是性能问题。

之后在 MacBook Pro 上的长时间运行里，`host_session` 跑到约 21 万次时在藏文方案下撞到 ewts 0.1.3 的 `usize` 下溢（最短输入 `bw`），处理见 [对 ewts 关掉溢出检查](../bug-fix/2026-10-11-ewts-overflow-checks.md)；最短复现输入已加入 `fuzz/seeds/host_session/tibetan-subscript-wa`。

## Consequences

收益：两处最高风险的入口有了可以长时间运行、可复现的变异测试，ASan 能抓到 C ABI 和 `unsafe` 路径上的越界与释放后使用。`plugin_pack` 还会顺带检查 zip 炸弹和路径穿越类的拒绝逻辑，至少不会 panic 或失控。威胁模型指向这些 harness，安全扫描能据此找到入口。

代价：

- 要 nightly 和 cargo-fuzz，不在任何门禁里，靠人或定时任务去跑。`fuzz/Cargo.lock` 需要在根 workspace 升级依赖后同步，否则两边的共享依赖会漂移。
- `host_session` 每轮新建会话和词库，在 ASan 下只有十几次每秒。它的种子都很短，libFuzzer 的 `lim` 按最长种子起步、缓慢增长；想测更长的按键序列时，给 `-max_len`。
- 日文、粤拼、注音和笔画在夹具里没有词库，这几种方案只测到缺词库时的处理，测不到它们各自的组词逻辑。
