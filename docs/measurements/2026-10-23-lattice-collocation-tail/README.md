# 搭配评分上下文尾窗复用

基线为远端 `develop` 的 `6d9cb3da7fa6cd85baf7abce937b4fa7a252cb5a`。改动把同一假设的 `collocation_tail` 及最终 `as_str()` UTF-8 校验从出边循环内移到循环前，所有出边仍按原顺序调用评分器。`Hyp` 大小、束宽、候选预算、模型资源和浮点分组保持。

## 确定性验证

旧入口红测试在扇出 2 时回溯 3 次，未达到每假设一次的 2 次上限；新入口通过。两列图的第一列只有一条边，第二列扇出为 1/2/8/32/128，冻结旧入口分别回溯 2/3/9/33/129 次，新入口均为 2。第一版仍逐边调用 `as_str()`，第二轮红测试在扇出 2 时观察到 4 次 UTF-8 校验而非目标 3 次；最终代码外提字符串借用，旧入口校验 3/5/17/65/257 次，新入口均 3 次（包含短词拼接的内部校验）。搭配关闭、权重为零/负数/NaN、没有出边时不构造尾窗。计数只进入 debug 单测，不进入产品或 release 计时程序。

640 组完整旧新对照覆盖 0/1/4/10 列、1/4 行、同分/异分、五种束宽和终点预算组合、主图/附加纠错图、搭配关闭/零权重/开启，以及 bigram/trigram/个人评分。合成字符串包含多字节字符和超过八个码点的历史。所有路径的文本、读音、词、纠错数、分数位模式相同；评分器接收的尾窗、词、句尾标志及回调次序完全相同。普通分配器的 release 程序也执行该对照并通过。

`decode_collocation_tail_frozen.rs` 保留完整基线解码函数，只移除注释和修改私有函数名；`frozen-comparison.json.gz` 记录机械核对。它复用未改变语义的 helper，不以早于当前基线的旧解码器代替实际生产入口。

## 本地门禁

最终源码通过 `cargo fmt --all -- --check`、引擎及 runtime 全目标 Clippy（`-D warnings`）和完整测试：引擎 2480 项、golden 31 项、runtime 362 项；54 项已有或显式性能测试忽略。真实模型相关环境变量未提供。最终 quick 的结果另存 `final-quick.log.gz`；macOS/Linux 原生宿主编译不等于产品运行或设备验收。MinGW 交叉工具链缺失，Windows 完整原生编译与 pipe-only 编译跳过；HarmonyOS 真打包因本机缺少 staged 输入跳过。

## Release 控制流

普通系统分配器、ARM64、Rust 1.97.1 / LLVM 22.1.6 的 release 程序中，旧入口的两处尾窗调用和最终 UTF-8 校验均位于评分器调用所在的最内层自然循环内；新入口的两处调用和校验均在该循环外。两处对应普通和较宽的终点预算。控制流核对表明本工具链未自动消除旧版逐边回溯；它不是完整产品二进制，也不能用循环指令数换算耗时。

`check_assembly.py` 通过 `nm` 与 `llvm-objdump` 取完整函数，在指令级控制流上计算支配关系和自然循环；原反汇编与 `assembly-analysis.json.gz` 保留供审查。脚本只支持这份 ARM64 输出形式，不把未知间接跳转当作可证明的控制流。

## 配对方法

使用普通系统分配器的独立 release lib test 程序。构建脚本仅临时停用已有计数分配器，在 `finally` 逐字恢复；程序 SHA-256 随证据保存。完整解码在同一进程调用冻结旧入口与新入口，包含图上评分、束搜索、结果物化和析构，图及模型夹具在计时外准备。

固定 4/12 列 × 1/4/16 行 × 搭配关闭/简单合成评分器/合成 `.gram` 查询，共 18 形状；每形状两臂先校准到至少 10 ms，再执行 21 对并交替先后，全部原始样本输出。`.gram` 夹具使用真实读取和查询实现，但不是下载的真实语言模型；不能将其耗时换算为产品收益。

运行前要求连续 30 秒无编译或相关基准进程且一分钟 load 不超过 10，最多等待 180 秒。等待记录始终保存；启动后每 0.25 秒观察工具名和 pid，不记录其它进程参数。未满足启动条件则不执行计时，也不记为性能预算失败。

## 本轮计时结果

第一版连续 180 秒未满足空闲条件，未启动计时，`timed=false`，等待记录保存在 `v1/`。最终版未重试计时，没有最终版本的计时样本。没有旧新耗时样本或性能预算失败；采用依据是确定性回溯工作减少、release 调用位置核对和完整输出等价，没有产品收益百分比。

## 复现

在包含本提交的独立 worktree 内运行，只使用规定的 `target/`。不要与其它构建同时操作临时分配器接线。

```sh
cargo test -p msime-engine --lib collocation_tail_tests --locked
cargo test -p msime-engine -p msime-input-runtime --locked
python3 docs/measurements/2026-10-23-lattice-collocation-tail/build_probe.py
python3 docs/measurements/2026-10-23-lattice-collocation-tail/check_assembly.py
python3 docs/measurements/2026-10-23-lattice-collocation-tail/run_probe.py
python3 docs/measurements/2026-10-23-lattice-collocation-tail/analyze.py
```

这里的确定性回溯次数证明重复工作被消除；耗时结论需满足上述环境要求。默认未启用搭配模型的九键 16 ms 超预算仍未解决。本轮不调整前缀解码缓存，不修改产品 I/O、目录、权限或 Android 资源，不把主机测试称为设备验收。

`v1/` 保存只外提回溯的第一版证据，根目录保存最终版；`preliminary-quick.log.gz` 是跨越第二次源码修改的初轮门禁记录，提交依据为最终 quick；第二轮红测使用第一版生产逻辑与新增 UTF-8 计数，`red.log` 只是早期导入错误，不作为有效红测。

证据 gzip 以固定 `mtime=0` 压缩。`manifest.json` 区分原始、脱敏后和压缩字节的哈希；日志中的原始源码/程序哈希不与脱敏文本混淆。没有分发模型或程序，没有包含真实输入、凭据和账号资料。
