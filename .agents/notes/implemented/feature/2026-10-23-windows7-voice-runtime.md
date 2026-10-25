# Agent Note: Windows 7 本地语音运行库的独立构建与实际装载验证

Status: implemented

## Problem

当前锁定的 Windows 语音包包含 sherpa-onnx 1.13.8 与 ONNX Runtime 1.28.2。原始 ONNX DLL 静态导入 Win7 没有的文件、时钟、路径和进程缓解策略接口。仅适配 Server 与 Rust 核心不能使本地识别工作。兼容包还必须避免改变现有模型目录、识别接口与现代 Windows 的运行库。

## Decision

`Build-Windows7VoiceRuntime.ps1` 在独立的 `target/windows7-voice/<arch>` 中构建同版本运行库。ONNX 固定提交为 `33ca9628233dc8f002435e868d4c2e9f82766ca1`，sherpa 固定提交为 `11afbd009a7f8c08f4bcf2fc1b265d0df4670fbf`。源码缓存位于 `target/tooling`，构建拒绝错误提交与已跟踪源码修改；不改普通发布锁，不使用其他 worktree 的未提交源码。构建要求至少 50 GB 空间。

ONNX 保留完整 CPU 运算集合，不启用 CUDA 或 DirectML。sherpa 保留流式、整段和 VAD 识别，TTS 与原始 `no-tts` 包一致关闭。ONNX、Abseil 与 sherpa 统一使用静态 CRT，避免混用 `/MD` 和 `/MT` 导致链接失败或额外引入未验证的动态运行库。sherpa 的 ONNX 依赖必须指向本次兼容构建的导入库；延迟 CMake 检查拒绝上游回退下载预编译运行库。

`Windows7Voice.cmake` 只向固定上游的四个系统适配编译单元强制包含重定向头，不修改上游源码。新接口存在时透传真实接口；缺少 `CreateFile2` 时完整转交 `CreateFileW` 的访问、共享、目录、安全与模板参数；精确时钟使用真实系统 UTC 时钟回退；不支持的缓解策略查询保留失败。路径回退保留根、UNC 与长路径语义，系统 PathCch 模块只按系统目录绝对路径探测。

固定上游的 NCHWc 优化器在 x86 使用 32 位 `size_t` 取反，再零扩展为 64 位，触发 `/WX`。适配器检查恰好三个已知表达式，在构建目录生成先扩宽到 `int64_t` 再取反的编译单元副本；不改源码缓存，不压制警告，不关闭卷积优化。入口数量变化即拒绝构建，要求重新审查。

构建同时检查 ONNX 主 DLL、provider 桥和 sherpa DLL 的已知阻断导入。序号导入不能跳过：目前只接受 ONNX 的序号 1，且必须读取实际相邻 DLL 的导出并核对 `OrtGetApiBase`。全部导入与实际运行测试通过后才写 `voice-runtime.build.json`，记录来源、架构、构建选项和适配器及各 DLL 的摘要；开始重建时撤去旧完成清单，失败不能留下本轮成功的标记。

### 实际依赖解析

运行测试发现，给 `LoadLibraryExW(..., LOAD_WITH_ALTERED_SEARCH_PATH)` 传正斜杠路径时，识别 DLL 可能拾取系统自带的 ONNX 1.17.1，而非同目录的 1.28.2。仅看主 DLL 的路径、导入清单或“加载成功”发现不了它。共享 `LocalAsr.cpp` 对带目录的路径先解析为绝对路径，再用 `make_preferred()` 转成 Windows 分隔符；无目录的库名保持原有搜索方式。新公共入口回归必须检查实际加载的 ONNX 文件与指定识别库相邻，不能改用另一份裸加载逻辑代替产品入口。

Win7 实机暴露固定 ONNX 设备发现的静态 `CreateDXGIFactory2` 依赖。专用构建将它经已有强制包含适配为导出探测：现代系统透传全部参数和返回值，缺失时只在零标志下调用真实 `CreateDXGIFactory1`，不伪造新接口或接受无法提供的调试标志；CPU 推理与上游设备发现的失败处理保留。静态导入门禁同时拒绝 `CreateDXGIFactory2`，并在 `dumpbin` 的 `Summary` 停止解析，避免将 `_RDATA` 段名误认作最后一个库的导入。

延迟加载回归用明确系统目录再次加载后核对模块身份，兼容 Win7 WOW64 已加载模块报告 `System32` 别名；不以路径字符串前缀代替系统模块身份。

## Related notes

- [完整 Win7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分重叠。运行库属于其本地语音依赖切片，完整产品和实机结果由总笔记持有。
- [本地语音模型导入](2026-10-09-local-voice-model-import.md)：部分重叠。固定模型的安装、校验、目录和清单边界保持原状，本篇只补兼容运行库与 Windows DLL 解析。
- 按 Win7、sherpa、ONNX 和运行库检索活跃笔记。移动端与 macOS 的资源下载、签名和 UI 记录不受此构建影响；没有需要替换或归档的同一运行库决定。

## Testing

Windows 11 的 x64 与 x86 构建均通过已知阻断导入检查、具名和序号 C ABI 核对，以及实际合成 ONNX Add 图和启用完整优化的 1×1 Conv 图的 CPU 会话、张量与结果验证。卷积使用三个输入通道与十六个输出通道，检查全部输出。sherpa 测试核对版本、共享识别器依赖的全部在线、离线与 VAD 导出、实际相邻 ONNX 路径，并用合成音频样本验证跨 DLL 的缓冲分配、读写与释放。强制缺少 API 的测试与系统 PathCch 对照根、UNC 和长路径，实际验证文件句柄、共享拒绝、目录句柄、时钟与不支持的策略查询。

共享识别公共入口的正斜杠路径回归已分别使用 x64 和 x86 的本次兼容 DLL 在 Windows 11 上通过；x64 的既有 `shared-voice-local-asr` 也通过。Windows 7 SP1 x64 实机同时运行两个架构的核心装载、延迟导入、系统适配、实际 Add／优化 Conv 推理和 sherpa 依赖／缓冲生命周期检查，十项全部通过。系统不支持的缓解策略和新版 DXGI 接口保留真实失败，上游 CPU 推理仍产生正确结果。小型兼容 API 测试接入现有 Windows CI；完整运行库从源码构建仍为显式命令，已由 Win7 产品脚本收集；普通现代产品发布不改。固定模型另对 SAPI 合成语音完成非空识别；虚拟声卡的双架构核心录音与同源 Server 录音、播放及提示音另见[产品构建](2026-10-24-windows7-product-build.md)，不等同于物理麦克风或扬声器验收。

Win7 上两架构还实际运行固定目录项 `x-asr-zh-en-streaming` 的模型，通过共享 `LocalAsrSession` 识别 SAPI 合成的 16 kHz 单声道测试语音，均成功产生非空转录。模型下载按 `resources/local-asr-models.json` 的大小和 SHA-256 核对，仅解包白名单普通文件。测试不打印音频或转录，结束删除合成 WAV。该结果覆盖真实识别模型，仍不证明麦克风录音或完整产品安装通过。

## Alternatives considered

- **退到旧 ONNX 版本。** 旧系统接口更少，可能减少移植工作。但会改变模型格式、运算与识别行为的支持范围，因此保持当前版本，适配已确认的系统边界。
- **只替换 ONNX，继续使用原始动态 CRT 的 sherpa。** 可以减少源码编译量，但把另一组运行时依赖留在兼容包里。这里以固定来源重编相同 C API，再核对实际依赖与操作。
- **只做静态导入检查。** 快且不执行第三方代码，但发现不了错误目录里另一版同名 DLL，也证明不了推理接口能工作，因此补实际会话和依赖路径验证。

## Consequences

现代发布包的依赖不变，兼容语音运行库拥有独立、可重复的构建和实际运行检查。代价是维护固定上游的窄适配边界，并为每个架构从源码构建完整 CPU 推理库。当前检查只覆盖已发现的导入障碍与合成模型，不能代替 Win7 SP1 上的所有系统导出、真实语音模型、音频设备、安装升级及现代 Windows 产品回归验收。
