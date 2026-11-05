# Agent Note: 三个桌面宿主加载本地模型翻译

Status: proposed

## Problem

用户要求 macOS、Linux、Windows 在输入法内加载模型离线翻译，并能选择目标语言。现有 Ollama 路径依赖用户另外运行 HTTP 服务；macOS 的 Apple Translation 只覆盖该系统，均不能满足三桌面共用的模型能力。

## Proposal

翻译规则、语言目录、模型安装和推理由共享 Rust 层提供，宿主只负责原生设置、后台调度和带代次的候选结果应用。推理不进入输入引擎或按键线程，不启动 HTTP 服务，不在模型不可用时回退到在线翻译。模型下载和选择是显式动作，不改变已有用户的翻译默认值。

先验证 Hy-MT2 1.8B Q4_K_M 与 llama.cpp 的嵌入式绑定。模型来自腾讯官方仓库固定提交 `a0c709d9fac510f2c807aa3af52872340dc37a4a`，文件大小 1,133,080,448 字节，SHA-256 为 `dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699`。该版本随模型提供 Apache-2.0 许可证；不把旧版 HY-MT1.5 的许可证沿用到它。模型通过既有资源包安装器下载、校验和原子发布，不随输入法安装包捆绑，也不从运行时目录信任新的来源或摘要。

语言列表来自模型的固定目录，以中文名称显示并存储稳定语言代码。用户可在模型支持的语言中选择，不能宣称支持世界上所有语言。现有云服务的八种语言限制保留，不把扩展的本地语言代码发给不支持它们的云服务。

原生设置入口分别接入三个桌面宿主，不向 `packages/ui` 增加新页面或设置。推理会话限制输入、上下文、生成长度和运行时间，支持取消；缓存与候选代次、模型和语言绑定，切换模型或关闭功能使在途回复失效。模型只在需要时加载，空闲后释放。

## Existing notes

- [候选翻译复用 AI 配置](../../implemented/feature/2026-10-17-ai-candidate-translation.md)、[显式选择 AI 翻译](../../implemented/feature/2026-10-22-ai-translation-provider-selection.md)：部分重叠，复用候选代次和显式选择语义；本地推理不复用 HTTP 请求。
- [AI 接口允许本机 HTTP](../../implemented/feature/2026-10-08-ai-endpoint-local-http.md)：不同能力，继续服务外部 Ollama，不替代嵌入式翻译。
- [全部界面改为原生](../architecture/2026-11-03-all-native-ui.md)：约束新入口必须原生，不扩展共享 React 设置。
- 检索命中的 macOS 账号释义队列、Windows 候选窗对齐和鸿蒙候选即时释义笔记：保留各自调度、显示与系统翻译决定，本次不替换它们。

## Alternatives considered

- **Ollama / LM Studio**：已有完整传输和鉴权基础，改动最少。但用户明确要求输入法内加载模型，外部 HTTP 服务不满足需求。
- **Apple Translation**：macOS 已具备系统管理模型与离线执行能力，集成成本最低。但 Linux 和 Windows 没有这条系统接口，不能作为三桌面的共同实现。
- **Marian / OPUS-MT 小型专用模型**：单个模型小，纯 Rust Candle 已有推理实现。不同语言通常需要分别安装模型，中文到多种目标可能需要英语中转；本次先测可用一个模型覆盖目标语言的 Hy-MT2，再按实测内存和延迟决定是否需要小模型选项。
- **直接复用候选重排的 ONNX Runtime**：仓库已有打包链，但它按重排模型算子精简，并没有翻译分词器与自回归生成实现；不能只换模型路径就使用。

## Acceptance criteria

- 三个桌面宿主都可显式选用本地翻译，安装、取消下载、选择语言、关闭和缺模型错误均有可达入口。
- 下载后断网仍能为合成候选生成译文；不需要 Ollama、Python 或额外服务，失败不访问云端。
- 真实模型验证覆盖中文到英文及另一种目标语言，记录首条延迟和资源占用；假后端测试只证明调度，不代替推理验收。
- 输入线程不等待推理；新候选代次、焦点切换、停用和模型切换不接收过期回复，任务支持取消和资源释放。
- 固定上游代码版本、模型摘要、许可证和第三方通知，运行本地 quick、相关 Rust 与宿主测试，并完成提交、推送和 PR。

## Risks

1.8B 模型的约 1.1 GB 文件与推理内存高于词典和专用小模型；需要显示下载体积并实测空闲释放。CPU-only Windows/Linux 的延迟不能用 macOS Metal 的结果代替。不同桌面的原生设置和发行打包链均须接线，不能把命令行探针当成产品接入完成。

## Evidence

macOS 上使用已安装的 llama.cpp `0.8.0`（提交 `8642541b2`），加载上述已核对摘要的文件，以 `--offline` 执行合成文本翻译得到正确英文句子。该探针证明权重可用，不证明应用集成、跨平台性能或取消行为。

上游：[模型与固定提交](https://huggingface.co/tencent/HY-MT2-1.8B-GGUF/tree/a0c709d9fac510f2c807aa3af52872340dc37a4a)、[llama.cpp](https://github.com/ggml-org/llama.cpp)、[Candle Marian 示例](https://github.com/huggingface/candle/tree/main/candle-examples/examples/marian-mt)。

嵌入式 Rust 探针（`llama-cpp-2` 0.1.161，内部 llama.cpp `d81235049384534c167caea52b85a694f6103d14`）已在本机跑出英、日、法译文，首次加载约 5.1 秒，三次翻译分别约 446 / 238 / 206 毫秒。这是单次 Metal 热模型测量，不是跨设备性能承诺。真实模型回归同时验证任务开始前取消、过期截止时间、预填充后取消以及取消后新请求成功；三个桌面宿主、原生设置和资源包接线仍未完成。


共享接线切片：`client-core::local_translation` 持有 38 项语言目录和 `local_translation.enabled/target_language` 偏好，缺字段保持关闭，不扩大云翻译语言白名单。资源包新增 `local-translation`，下载、导入和删除复用现有固定摘要安装器。旧 `every_url_is_immutable` 测试原本只认可 GitHub 来源，这次增加腾讯官方 Hugging Face 仓库的完整提交地址形式，原有来源约束保留。

推理队列最多保留一个执行任务和一个等待任务，队列满返回 `local_translation_busy`；工作线程缓存同一模型，空闲 60 秒卸载。等待句柄销毁、取消、超时或卸载代次变化都会丢弃结果；加载模型使用 llama.cpp 的进度取消回调。Host API 提供目录、阻塞翻译和按请求 ID 取消三个入口，翻译只能在宿主后台线程调用。冷加载前通过资源包完整 SHA-256 校验，缺模型或未编译 `local-translation` 特性直接返回不可用，不产生网络描述符。权重校验逐个读取块检查取消和截止时间。

Windows x64 GNU 的运行库交叉检查已在远程 macOS 编译机通过；首次失败是 bindgen 缺少 MinGW sysroot 与 Rust 目标，补齐 `x86_64-pc-windows-gnu` 并设置 `BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_gnu=--sysroot=<MinGW sysroot>` 后通过。这仅验证运行库交叉编译，不证明 Windows 产品链接或运行。


真实 Host API 回归暴露并修正两个生命周期问题：成功收取结果后，等待句柄不能再置取消标记，否则宿主会把成功译文丢弃；缓存模型必须在原生后端静态析构前释放，否则 Metal 的进程退出断言会中止程序。等待句柄现在只在没有收到结果时取消，工作队列支持关闭并等待线程结束；Unix 在后端初始化后登记退出清理，Windows 由 Server 在 DLL 卸载前显式调用 `msime_client_local_translation_shutdown`，避免加载器锁内等待线程。真实导入→摘要核验→后台推理→Host API 响应的回归已由失败转为通过，正常进程退出也通过。

`preferences-field-parity` 的既有规则检查 React 控件。新字段按 AGENTS.md 的原生界面迁移约束加入 `RUST_ONLY` 并注明原因，不能为了这个检查向 React 加设置；三个原生入口仍是本草稿 PR 的完成条件。


Linux 容器的共享设置应用 `cargo check -p msime-desktop --locked --all-targets` 已通过；这次补齐其资源包枚举分支，仅明确不在 React 中展示新包，没有增加 React 设置。`platforms/linux/build-container.sh` 的 122 项原生测试全部通过。Linux aarch64 CPU 上真实模型英、日、法翻译与取消测试也通过；构建绑定需要 `libclang-dev`，后续发行构建依赖必须补齐它。

候选接线切片新增独占的 `local` Provider、原生窗口专用的偏好读写 API，以及最多十项候选共享取消标记与截止时间的批次 API。显式开启时只关闭云服务的选择标记、保留凭据；关闭不会恢复任何云服务。保存使用现有偏好修订号比较并交换，返回只含本地字段的快照。Unix socket Provider 明确拒绝传送 `local` 查询，未编译运行库也不会把请求交给外部服务。目标语言变化清除旧释义、推进代次，测试验证旧回复被拒绝。

三个桌面后台任务接入同一个 C ABI：Windows 保留 worker 的焦点租约和取消谓词，IBus/Fcitx5 为各会话保存独立取消标记，macOS 用独立批次对象复用现有自定义翻译结果展示。原生适配器持续发送取消直至后台退出，覆盖取消早于 Rust 任务登记的竞态；该竞态有不依赖模型的行为测试。macOS 的查询身份去掉候选代次，避免应用译文推进代次后重复推理；本地模式不调用 Apple Translation 后备路径。Windows 在所有 worker 析构后显式关闭共享推理线程。

该切片的真实模型批次验证使用两条合成候选，确认同批次第二项不会被第一项的句柄析构误取消；这次运行没有禁网沙箱，断网证据仍来自上一切片的 macOS 单条 Host API 和 Linux 模型测试。原生设置、发行库链接与界面验收仍待完成，不能把宿主编译通过记成产品完成。

原生设置采用独立模块：macOS 新 AppKit 模型窗口由输入法菜单直接打开，不扩展冻结的回退设置；Linux 新 GTK4 进程由 IBus/Fcitx5 菜单启动，Windows 由原生翻译页打开 Win32 对话框。共享 Rust 负责目录、校验和 CAS 保存；窗口只保留异步交互。新窗口是能力的入口，不替代全部原生迁移工程；React 仅修正选择云服务后关闭本地标记以及避免误报 Apple Translation 的兼容问题。

独立窗口让三个平台都能在不增加 WebView 的条件下管理模型；代价是暂时有单独的窗口，而不是完整原生设置容器中的内联页面。已考虑等完整原生迁移后再开放入口，那能统一导航，但会让当前离线能力在 macOS/Linux 不可达。现代 Linux 窗口需要 GTK 4.8，官方包显式要求构建；旧 Debian 基线允许关闭窗口，菜单也随之隐藏。现代 Windows x64 Server 启用运行库；Win7 不声明支持，窗口按共享目录的 available 状态禁用。三个平台的出厂翻译选择保持原值。

设置适配器测试验证了并发修订号冲突不覆盖新设置、删除先关闭翻译、早于安装登记的取消以及取消后的再次保存。Windows 首台验收机 SSH 超时，另一台编译机可达，正在核对工具链；不能把 GNU 语法检查当成完整产品验收。
