# Agent Note: macOS 原生输入法与语音辅助进程崩溃采集

Status: implemented

## Problem

macOS 输入法进程和本地语音 helper 需要与共享遥测层使用同一套持续授权、落盘和构建身份。原有 Objective-C++ 捕获只保留普通栈文本，发布脚本在剥离后再生成 dSYM，无法保证 universal 二进制的 UUID、源码行和 Swift/Rust 预生成 dSYM 仍然对应；输入法被正常 `SIGTERM` 结束时也必须结束会话，`SIGKILL` 则只能留下待补报标记。

## Decision

macOS 输入法通过共享 `CrashRecorder` 和 Host API 记录自身异常/信号，捕获头保存模块 UUID、架构、版本和构建身份；异常摘要不读取私有异常原因。会话生命周期在输入法主入口统一管理，`SIGTERM` 走正常结束，强制终止保留未结束会话。语音 helper 通过共享的 `HelperCrashCapture` 接口绑定 macOS 的真实偏好目录，不让通用语音库猜测产品身份。

发布脚本在每个架构的 Rust、Swift 和原生目标仍带行号构建，先合并并校验 dSYM，再剥离用户安装副本。符号工具核对 UUID、架构、构建身份、哈希和源码行，打包后再从标准 ZIP 解压复核；已有 `macos-symbols` 发布流程继续负责长期保存。

## Alternatives considered

- **在剥离后的 universal 文件上重新运行 `dsymutil`**：流程简单，但 Rust/Swift 的原始对象和架构信息可能已被合并或丢失；因此保留每个架构的原始 dSYM 后再合并。
- **只保留 `atos` 栈文本，不保存模块 UUID**：对当前机器诊断方便，但升级后同名二进制无法区分；因此把 UUID 和相对偏移写入捕获记录。
- **让共享语音库自行读取 macOS 偏好**：减少平台适配代码，但会越过宿主配置边界，在不同版本或沙盒中读错目录；因此由平台适配实现 `HelperCrashCapture`。
- **把 `SIGTERM` 当作崩溃**：能减少会话标记，但安装更新、注销和关机会污染崩溃率；因此保留正常结束与强制终止的差别。

## Verification

- macOS CMake 新增输入法/语音 helper、授权生命周期、真实故障和符号工具测试；发布脚本在剥离前验证 universal 自有模块及 Swift/Rust dSYM。
- Python 符号夹具使用合成源码，覆盖架构缺失、UUID/哈希篡改、混合构建身份、Unicode ZIP 和源码行还原；不包含真实输入或私人路径。
- 当前工作树通过 `git diff --check`；本机不是 macOS，未把静态检查当成 Apple 编译通过。macOS arm64/x86_64 CI 负责原生编译、ctest 和符号门禁；真实编辑器/真机证据仍单独记录。

## Consequences

发布包除维护者侧 dSYM 归档外，安装的语音 helper 也变大：它为崩溃采集经 apple-client 链入 Rust 宿主 staticlib。arm64 单架构、Release Rust 库、`strip -x` 后实测从约 0.25 MB 增到 2.5 MB（只依赖 Foundation/CoreFoundation）；不加 `-dead_strip`/`-dead_strip_dylibs` 时是 51 MB 并加载 AppKit，所以 helper 的 Release 链接与输入法一样去除无用归档成员和动态库。universal 包约为两倍。用户安装副本仍剥离局部符号。捕获记录可在对应构建身份下还原源码行，但不携带异常消息或目录。语音 helper 的平台适配多一个小型生命周期对象，换来授权目录和沙盒边界清晰。

共享语义见[共享崩溃记录基础](../../implemented/architecture/2026-11-01-shared-crash-reporting-core.md)。
