# Agent Note: 高负载下输入线程的可选工作降级

Status: implemented

## Problem

输入动作、候选刷新和可选神经模型共用宿主的输入线程。系统整体负载高或前一键已经被调度延迟时，继续在这个线程上执行逐键重排或停顿模型会把后续空格、数字和退格排在长计算之后，表现为输入法卡住。商业样本的静态字符串和结构显示了相同问题的工程边界：Android 样本把模型推理放进异步任务，并携带 `sysLoadStatus`、`cpuUsageLevel`、`gen_model_inference_timeout_threshold` 等负载/超时状态；macOS 样本将性能记录和后台队列分开。样本只作机制参考，不运行其安装程序，也不复制其私有实现。

## Decision

`crates/input-runtime` 增加 `LoadMonitor`，在 `Runtime::dispatch` 的输入动作边界记录耗时。Linux 和 Android 每 250ms 取样一次：优先读取 cgroup v2 的 `/sys/fs/cgroup/cpu.pressure`，再回退到 `/proc/loadavg`；其他宿主使用同一个 16ms 输入帧预算的耗时反馈。观察到 CPU 等待或超出一帧后，进入 300ms 的短暂降级窗口。

降级只阻止可选模型工作：逐键 `rerank` 不运行，停顿模型的 `begin_settled_rerank` 与同步回退也不运行；基础词典候选、候选元数据、在线候选座位、分页和选择代次照常执行。窗口结束且系统压力消失后自动恢复，不改变任何持久化偏好，也不要求宿主重新创建会话。

压力探针在单元测试的普通路径中关闭，避免共享 CI 负载改变候选排序；真实探针仅编入生产目标与 Linux 压力专项，其他平台单测不编入未使用的探针，保持严格 Clippy 无死代码告警。Docker 专项测试通过被忽略的 Linux 用例直接验证真实 cgroup 压力。运行时回归用合成候选确认降级时候选仍可见、模型仍在会话中可恢复。

## Existing decisions

本决定延续[异步停顿重排按组字与配置共同失效](2026-10-21-async-settled-rerank.md)的线程边界：模型评分可以脱离会话线程，但本次还在启动前根据负载拒绝可选评分。候选排序资格、座位重排和在线结果的既有笔记不变；本次只增加输入线程的负载门控。

## Alternatives considered

- **只把停顿模型放到后台**：能解决停顿后的空格与模型竞争，但逐键模型和系统 CPU 限流仍可能阻塞输入线程，不能覆盖高负载场景。
- **遇到压力时丢弃整个候选刷新**：能快速返回，但会让预编辑、候选和代次不同步，数字选词可能失效；因此保留基础候选路径，只停掉可选模型。
- **每个平台各自实现 CPU 监测和降级**：可以读取更丰富的平台指标，但会复制阈值、恢复窗口和行为契约，且移动宿主容易漏接；因此把门控放在共享 runtime，Linux/Android 只提供无权限的系统压力信号。

## Consequences

高负载期间输入线程优先保证基础候选和按键响应，神经重排可能延迟到压力窗口结束，候选顺序会暂时保持词典/引擎顺序。Linux/Android 多了一次节流后的 cgroup 或 proc 文件读取；其他平台不做系统文件访问，只测输入动作耗时。压力探针依赖 cgroup v2 时使用其当前进程组信号，无法读取时仍由超时反馈提供保护。

## Verification

- Docker 单核压力脚本 `scripts/test-input-load-docker.sh`：基础降级用例 2 项通过；启动 8 个竞争进程后，`linux_external_pressure_probe_observes_a_loaded_container` 1 项通过。
- Docker 完整 `cargo test --locked -p msime-input-runtime --no-fail-fast`：381 项通过、0 失败、1 项按设计忽略；doc-tests 0 项通过。
- Docker `cargo clippy --locked -p msime-input-runtime --all-targets -- -D warnings`：通过。
- `bash -n scripts/test-input-load-docker.sh` 与 `git diff --check`：通过。
- 本机直接 Cargo 验证受 macOS Homebrew `libgit2` 缺少 `libllhttp.9.4.dylib` 阻塞；最终结论以 Docker 固定 Linux 工具链结果为准。

共享运行时计时沿用 `msime_engine::time::Instant` 的宿主注入时钟：原生平台仍为标准时钟，wasm 不调用缺失的系统时钟。该修正不改变原生降级阈值。
