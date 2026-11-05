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
