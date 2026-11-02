# Agent Note: 按键重排的 ONNX Runtime 后端：精简构建、uint8 权重、按自测速度分档

Status: implemented

## Problem

整句联想「增强」的逐键重排用随包的 chinese-ime-lm `model-v1`（4.25M），推理是 chinese-ime-lm 的纯 Rust 参考实现：单线程、int8 权重逐元素转 f32，文档记的吞吐约 12.5 GMAC/s，已接近单核上限。2026-11-01 在真实运行时（`rerank_latency`，sentences-v2 与 typing-chat-v1 共 643 条、约 8100 次按键）里量，它在 Pixel 3 XL 上让 20% 的按键超过一帧（16 ms），整键 p95 38–40 ms；iPhone XR 上整键 p95 11–12 ms。更准的模型更大：chinese-ime-lm `rerank-v1` 的 8x448 在 ctx-v1 上有上下文 91.8%、无上下文 83.4%（4.25M 同口径 89.6% / 81.3%），用同一个参考实现一次决策 71 ms，上不了按键路径。

## Decision

按键重排在宿主编译时打开 `onnx-rerank` 特性、资源里又有 ORT 格式的重排模型时，跑在 ONNX Runtime 上（`crates/onnx-rerank` 的 `OnnxScorer`，经 [CandidateScorer](2026-11-01-candidate-scorer-boundary.md) 挂上），不开特性或没有模型时照旧用随包模型。

落地到哪一步：后端 crate、host-api 的挂载与分档、`rerank_latency` 的 `--onnx-model`、精简构建的脚本（`tools/onnxruntime-minimal`）、Linux x86_64 上开着特性的 CI（`onnx-rerank.yml`），以及静态库的分发（Release `onnxruntime-minimal-1.24.4-r1`、锁文件和下载脚本，见下面「静态链接」一条）已经在仓库里。还没有的是接入：没有哪个平台的构建打开 `onnx-rerank`、把 `rerank-model/` 放进资源，所以现在每个平台照旧用随包模型。

- **单图、跨按键缓存。** 模型是 chinese-ime-lm 的 `lm_step`：上屏时把上文编码进缓存，之后每次按键只算缓存里还没有的候选前缀。候选共享的前缀算一次，候选的最后一个字不用算，上一次按键算过的前缀直接复用。缓存簿记（`prefix.rs`）不依赖原生库，单测覆盖。
- **ONNX Runtime 精简构建、静态链接。** 只编进两个模型用到的 27 个算子和类型（`tools/onnxruntime-minimal/required_operators_and_types.config`），只读 ORT 格式，链进程序约 1.5 MB；官方安卓库 34 MB。每个平台一个静态库（MSVC 目标是 `onnxruntime.lib`），gzip 压缩后作为 GitHub Release `onnxruntime-minimal-1.24.4-r1` 的附件发布；`resources/onnxruntime-minimal.lock.json` 按 Rust 目标三元组锁住地址、SHA-256 和大小，`scripts/fetch_onnxruntime.py` 下载核对后打印 `ORT_LIB_LOCATION` 要指的目录。r1 有苹果四个目标（macOS 两种架构、iOS 真机与 arm64 模拟器）、安卓 arm64 与 x86_64、Linux aarch64、Windows x64 与 arm64；没有 Linux x86_64（CI 照旧现编）和 32 位（32 位设备不跑 ONNX 重排）。锁里记着算子清单的 SHA-256，改清单不重新发布时检查失败。Windows arm64 的库用静态 CRT，因为 arm64 的 host-api 是 `+crt-static` 编的，用动态 CRT 的库链接不上；Windows 两种架构都关掉调试信息，`/Zi` 的 `.debug$S` 让 x64 的库从 111 MB 涨到 374 MB 而没有 PDB 可用。Rust 绑定是 `ort` 2.0.0-rc.15（`api-24`，不带默认特性），对齐 ONNX Runtime 1.24.4。
- **uint8 权重。** ONNX Runtime 按 CPU 特性选 int8 内核：int8 权重时 M4 Pro（有 I8MM）与 M1 Max（没有）在 ctx-v1 的 6317 例里有 42 例选得不同（90.7% 对 90.5%），uint8 权重两边分数逐位相同、准确率不变。
- **大小两档，按自测速度选。** 资源里有 `rerank-model`（8x448）和 `rerank-model-small`（6x256，训练时自归一化，读候选字只算一次点积）时，先加载大的，用 `OnnxScorer::calibrate` 量它打一页重负载（32 字上文、9 个 6 字候选、37 个新前缀），超过 10 ms 换小的。真机自测值：iPhone 17 大模型 3.8–4.8 ms、Mac 4–7 ms，iPhone XR 21.9–23.4 ms、Pixel 3 XL 30.4 ms，与各机在真实运行时里量出的最优选择一致。
- **线程数。** 安卓 4 线程（Pixel 3 XL 上整键 p95 两线程 10.8 ms、三线程 10.0 ms、四线程 9.3 ms），其余 2 线程（苹果 A 系列只有两个大核，四线程在 iPhone XR 和 iPhone 17 上都更慢）。

结果（真实运行时，整键 p95）：Mac 约 4 ms（8x448），iPhone 17 3.8–4.1 ms（8x448），iPhone XR 4.4–4.9 ms（6x256），Pixel 3 XL 9.3 ms（6x256）。

## Alternatives considered

- **官方预编译的 ONNX Runtime（动态库或 xcframework）** — 不用自己维护构建，CI 也省事；安卓已经因为离线语音和 OCR 带着它的两份。但那两份都不在键盘本体里（离线语音是用户打开时才下载的资源包，OCR 是单独的模块），iOS 那份静态链接在 sherpa-onnx 的 xcframework 里、不保证导出 C 接口；为重排单独带一份完整库每个平台多二三十 MB，安卓 34 MB。精简构建只有 1.5 MB，代价是每个平台自己编、每换一次模型算子清单就要核对一次。
- **把 chinese-ime-lm 的纯 Rust 实现做快（SIMD、多线程）** — 不加原生依赖，网页引擎也能用。但它约 12.5 GMAC/s 已接近单核标量上限，ONNX Runtime 在 ARM 上用 SDOT/I8MM 等整数点积指令，同一个模型在 M4 Pro 上快一个数量级以上；在 `#![forbid(unsafe_code)]`、低工具链门槛的参考实现里手写这些内核，等于另造一个推理引擎。
- **XNNPACK 执行提供者或 fp32 权重** — 在 Pixel 3 XL 上都试过：XNNPACK 跑 fp32 比默认的 int8 慢约 3.7 倍，跑 int8 慢约 1.7 倍；不量化的 fp32 权重比 int8 慢约 2 倍。
- **int8 权重配 `reduce_range` 或逐通道量化** — 都想减小不同 CPU 之间的差异：7 位权重时 M4 与 M1 有 30 例选得不同，逐通道 48 例；只有 uint8 权重两边逐位相同。
- **按机型名单选模型** — 名单要人维护，新机型出来总是落后；自测在每台机器上量的就是要决定的那件事，十毫秒以内的一页负载在加载时只多花几十毫秒。

## Consequences

- **收益**：同一台设备上，新模型比 `model-v1` 既更准又更快：Pixel 3 XL 上重排开销 p95 从 36 ms 降到 6.9 ms，超过一帧的按键从 20% 降到 0.1%；iPhone XR 整键 p95 从约 11.5 ms 降到约 4.6 ms。
- **代价**：多一个 C++ 依赖要按平台自己编（苹果与安卓在 macOS 上编，Linux 在 msime 发 Linux 版的同一个镜像里编，Windows 要 MSVC），换模型若算子清单变了要重编全部平台、发一个新的 `-r<序号>` 并更新锁。`ort-sys` 会把静态库复制进自己的输出目录，换库后要 `cargo clean -p ort-sys`。权重是动态量化的，同一个前缀与不同的节点一起算时分数会差在第三位小数（约 0.005），测试按这个量级放宽。
- **还没做的**：各平台构建打开 `onnx-rerank`、用下载脚本取库并带上 `rerank-model/`、`rerank-model-small/` 资源；锁里补上 Linux x86_64。做完之前上面的收益只在 `rerank_latency` 里量得到，用户拿到的仍是随包模型。
- **已知上限**：iPhone 17 上 8x448 整键 p95 在 4 ms 上下，手机档 6x256 在 ctx-v1 有上下文只高出 90% 的线 0.1 个点；HarmonyOS 没有 ONNX Runtime 产物（不在官方支持的平台里），照旧用随包模型；Windows 有库，但还没在真机上量过 `rerank_latency` 和自测分档。量化或模型换代后，要重跑 `rerank_latency` 与 `calibrate` 并更新 `LARGE_RERANK_MODEL_BUDGET_MS` 旁的数字。

## Verification

- `crates/onnx-rerank/tests/golden.rs`：随机小模型（`tests/fixtures`，算子与正式模型相同）逐键打分与 ONNX Runtime Python 包相差 1e-3 以内，复用缓存与从头算只差在量化误差以内，自测后不留缓存。
- `crates/host-api/src/tests.rs` 的 `keyboard_reranker_prefers_the_onnx_model_when_one_is_installed`：大小两档放的 `model.json` 上文字数不同，按 `context_length` 认出挂的是哪一个；只装大的时用大的，两个都装时自测预算够用大的、不够换小的（预算经 `onnx_reranker_within` 注入），大的坏了用小的，都坏了退回随包的句子模型，连句子模型也没有就不挂。
- `scripts/test-fetch-onnxruntime.py`：下载不符就丢掉、解压不符不替换原来的库、库已是锁里那一份不下载，锁的每个目标指向同一个 Release、库名按 MSVC 与否区分、没有 32 位目标、算子清单的 SHA-256 与锁一致。r1 发布时在 Mac 上用下载脚本取 `aarch64-apple-darwin` 的库跑过 `msime-onnx-rerank` 的 golden 测试；Windows x64 的库在 Windows 上跑过 golden 测试，arm64 的静态 CRT 库在 `Build-RustOutputs.ps1` 的 arm64 配置下链接通过。
- `.github/workflows/onnx-rerank.yml`：在 Linux x86_64 上开着 `runtime` / `onnx-rerank` 特性跑 clippy，用 `build.sh` 编出（并缓存）精简库，再跑上面两组测试。别的 CI 作业都不开这些特性。
- 真实运行时延迟：`ORT_LIB_LOCATION=… cargo build --release -p msime-input-runtime --features onnx-rerank --example rerank_latency`，再 `rerank_latency --onnx-model <目录>`；各平台的数字见上。
