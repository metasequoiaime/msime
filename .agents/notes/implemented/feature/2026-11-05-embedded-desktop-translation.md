# Agent Note: 三个桌面宿主加载本地模型翻译

Status: implemented

## Problem

用户要求 macOS、Linux、Windows 在输入法内加载模型离线翻译，并能选择目标语言。现有 Ollama 路径需要另外运行 HTTP 服务；Apple Translation 只覆盖 macOS，不能作为三个桌面的共同实现。

## Decision

翻译规则、语言目录、模型安装和推理由共享 Rust 层提供，宿主负责原生设置、后台调度和带代次的候选结果应用。推理不进入引擎或按键线程，不启动 HTTP 服务，失败不回退在线翻译。新偏好缺字段保持关闭；不改变已有翻译默认值。

`client-core::local_translation` 持有 38 项模型语言/变体目录和 `local_translation.enabled/target_language`，并通过修订号 CAS 保存。启用本地翻译时关闭云翻译的选择标记、保留凭据；关闭本地翻译不自动恢复云服务。现有云服务的语言白名单保持原样。只承诺模型目录内的选项，不宣称覆盖所有语言。

`local-translation` 资源包固定腾讯官方 `HY-MT2-1.8B-GGUF` 提交 `a0c709d9fac510f2c807aa3af52872340dc37a4a` 的 `Hy-MT2-1.8B-Q4_K_M.gguf`，大小 1,133,080,448 字节，SHA-256 为 `dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699`。下载、导入、取消与删除复用资源包安装器；冷加载重新完整校验，校验逐块检查取消和截止时间。模型不随安装包或 Git 分发，不接受运行时目录扩展来源或摘要。模型的 Apache-2.0 许可随官方固定提交核验，不沿用 HY-MT1.5 的旧许可。

`msime-local-translation` 通过 `llama-cpp-2` 0.1.161（llama.cpp `d81235049384534c167caea52b85a694f6103d14`）在进程内推理。工作队列最多保留一个执行任务和一个等待任务，超出返回 `local_translation_busy`；输入最多 256 字、输出最多 512 tokens，模型空闲 60 秒卸载。加载、预填充和生成检查取消及截止时间；成功收取结果后不再由等待句柄析构误取消。退出前关闭线程并释放热模型，避免 Metal 静态析构顺序冲突；Windows Server 在 worker 析构后、DLL 卸载前显式 shutdown，不在加载器锁内等待线程。

Host API 提供目录、设置、单条、批次、候选查询、取消和退出接口。最多十项候选共享取消标记和截止时间。独占 `local` Provider 不生成 HTTP 请求，Unix socket 不转发本地查询；未编译运行库直接返回不可用。语言变化清空旧释义并推进候选代次，过期回复被拒绝。

## Native hosts

Windows 使用后台 worker 的焦点租约和取消谓词；IBus/Fcitx5 为会话保存独立取消标记；macOS 用独立批次对象接入现有译文展示，明确跳过 Apple Translation 后备路径。取消持续发送直到后台退出，覆盖取消早于 Rust 任务登记的竞态。macOS 查询身份去掉候选代次，避免应用译文推进代次后再次发起相同推理。

三端入口都提供下载、文件导入、取消、删除、开关和目标语言选择。Windows 原生翻译页打开 Win32 对话框；macOS 输入菜单打开独立 AppKit 窗口；Linux IBus/Fcitx5 菜单启动独立 GTK4 设置进程并传入状态目录。UI 异步调用 Rust，不复制校验和默认规则；删除前先关闭本地翻译。没有给 React 加新入口，兼容修复仅让旧服务选择关闭本地标记、避免本地服务误报 Apple Translation。

现代 macOS、Windows x64 和当前 Linux 发行链启用运行库、打包许可证。Linux 启动器的 `environ` 声明显式保留 C 链接，避免 Debian 10 的系统头文件稍后声明它时发生 C/C++ 链接冲突；现代容器构建与 124 项测试复验无失败（1 项环境跳过）。Win7 与 Linux legacy 不声明支持嵌入式运行库，GTK4 不可用的旧 Linux 构建关闭入口。独立窗口遵守原生迁移方向，不扩展冻结的 AppKit 回退设置；代价是暂时不内联于完整原生设置容器。

## Existing notes

- [候选翻译复用 AI 配置](../../implemented/feature/2026-10-17-ai-candidate-translation.md)、[显式选择 AI 翻译](../../implemented/feature/2026-10-22-ai-translation-provider-selection.md)：部分重叠，复用候选代次和显式选择语义；本地推理不复用 HTTP 请求。
- [AI 接口允许本机 HTTP](../../implemented/feature/2026-10-08-ai-endpoint-local-http.md)：不同能力，继续服务外部 Ollama，不替代嵌入式翻译。
- [全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)：约束新入口必须原生，不扩展共享 React 设置。
- 检索命中的 macOS 账号释义队列、Windows 候选窗对齐和鸿蒙候选即时释义笔记：保留各自调度、显示与系统翻译决定，本次不替换它们。

## Alternatives considered

- **Ollama / LM Studio**：已有完整传输和鉴权基础，改动最少。但用户明确要求输入法内加载模型，外部 HTTP 服务不满足需求。
- **Apple Translation**：macOS 已具备系统管理模型与离线执行能力，集成成本最低。但 Linux 和 Windows 没有这条系统接口，不能作为三桌面的共同实现。
- **Marian / OPUS-MT 小型专用模型**：单个模型小，纯 Rust Candle 已有推理实现。不同语言通常需要分别安装模型，中文到多种目标可能需要英语中转；本次先测可用一个模型覆盖目标语言的 Hy-MT2，再按实测内存和延迟决定是否需要小模型选项。
- **直接复用候选重排的 ONNX Runtime**：仓库已有打包链，但它按重排模型算子精简，并没有翻译分词器与自回归生成实现；不能只换模型路径就使用。


## Testing

- macOS Metal 真实模型英、日、法、取消、超时、取消后重新请求和 Host API 导入链路通过；单条 ABI 在禁止网络的 `sandbox-exec` 下通过。单次探针冷加载约 5.1 秒，热模型短句英/日/法约 446/238/206 毫秒，不作为其他硬件的性能承诺。
- Linux aarch64 CPU 真实模型英、日、法及取消通过；禁网容器中的候选 query ABI 从导入权重到日语译文通过。GTK4 窗口未安装/已安装状态已截图；IBus/Fcitx5 构建与 124 项原生测试通过。
- Windows x64 MSVC 带运行库 Host API DLL、完整 WinUI 设置应用构建和链接通过，新对话框 `/W4 /WX` 编译通过，共享设置行为与许可证收集测试通过。实际 DLL 导入固定摘要权重后生成英、日、法译文，显式 shutdown 正常退出；单次 CPU 首条含校验/加载约 18 秒，后续短句约 1–3 秒，峰值工作集约 2.16 GB。该 Windows 探针未禁网，不代替 macOS/Linux 的禁网证据。
- 共享设置行为覆盖 CAS 冲突不覆盖、删除先关闭、早取消和取消后的再次保存；宿主测试覆盖候选身份、过期结果和焦点取消。React 兼容测试 58 项及类型检查通过。
- 本地 quick 在远端编译机通过；macOS CI arm64 通过。本地完整 macOS ctest 曾为 229/230，`shortcut` 以 SIGTRAP 退出，其余通过；同套件还出现语音启动缓存断言间歇失败。缓冲写入栈将语音状态覆盖定位到 `completePreferenceLoad`；将前一个“新进程恢复方案”夹具的读取阻塞到语音关闭用例完成后再释放，复现同一断言。该夹具现在在两次客户端交接前正常失焦，取消偏好代次与轮询，再移除合成目录；原有默认设置、输入和断言全部保留。相同延迟夹具修复前失败、修复后通过；移除诊断代码后，最终 quick 与完整 230 项 ctest 全部通过。此前的 SIGTRAP 未单独归因，不把一次不再出现的异常冒称为已证明的同一根因。
- macOS 可见窗口已验证启用、日语选择与保存、删除先关闭、下载取消；文件选择器打开/取消可用，安装夹具由共享资源包 ABI 导入，不冒称文件选择器完整导入验收。Windows 原生对话框与真实 DLL 运行验证 38 项语言、缺模型禁用和安装后语言切换，随后在 Windows 活动交互会话运行同一原生对话框，取得未安装与已安装状态的可见窗口截图；截图不包含桌面或私人资料。实际 WinUI 设置应用也在独立合成配置下验证了“标点与翻译 → 翻译服务 → 管理模型与语言”的点击入口及对话框状态加载。

## Consequences

用户安装一次模型后可离线选择其支持的目标语言，不需要额外服务。约 1.1 GB 权重和推理内存高于专用小模型；CPU 性能不能用 Metal 延迟代替。取消与卸载支持节省闲置资源，但不承诺所有平台具备相同首条延迟。

`preferences-field-parity` 的 React 控件规则将新字段列入有说明的 `RUST_ONLY`，原生入口负责可达性；固定来源测试保留旧约束并增加腾讯官方固定提交形式。Windows notices 的合成仓库夹具同步新增许可证，保持完整性断言。Win7 在线服务选择只在本地翻译已启用时追加关闭标记：默认关闭时 Rust 省略整个对象，严格路径编辑器会拒绝写不存在的父对象。CI 原有在线服务切换回归捕获了无条件写入导致的保存失败，修复保留原测试前提；原有 `windows7-settings` 和另外 9 项 Win7 功能测试在 Windows x64 上通过；两个 Python 测试用 `PYTHONUTF8=1` 通过，窗口单实例测试在活动交互会话通过，13 项测试均已覆盖。

产品代码和打包已落地，三端模型窗口状态图与 Windows 设置页对比图已归档；Windows 改前图是依据固定基点代码绘制的布局复原图，改后为真实 WinUI 截图。PR 在最终提交的 CI 完成前保持 draft；安装、签名、真实编辑器及 macOS 文件选择器完整导入等未完成验收明确列在 PR，不冒称通过。本机锁屏时控件离屏绘制只证明布局与交互状态，不能替代可见窗口或真实编辑器验收。

上游：[模型固定提交](https://huggingface.co/tencent/HY-MT2-1.8B-GGUF/tree/a0c709d9fac510f2c807aa3af52872340dc37a4a)、[llama.cpp 固定提交](https://github.com/ggml-org/llama.cpp/tree/d81235049384534c167caea52b85a694f6103d14)。
