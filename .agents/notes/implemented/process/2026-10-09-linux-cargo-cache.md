# Agent Note: 临时构建容器持久化 Cargo 下载缓存

Status: implemented

## Problem

修复前的 Linux 原生门禁、桌面检查和 Windows 交叉构建容器只持久化构建产物或平台工具，Cargo 下载与源码缓存仍位于临时容器内。镜像已准备固定工具链，但 Cargo registry 与 git 依赖在镜像之后下载，容器删除时一起丢失；即使目标目录已有编译结果，新容器仍需网络准备依赖。

## Decision

沿用隔离验收脚本的做法，原生容器的 `CARGO_HOME` 指向已有挂载内的 `/build/cargo-home`，桌面检查指向 `/ctarget/cargo-home`；Windows 交叉构建指向仓库挂载内的 `/repo/target/windows-cross/cargo-home`。源码、编译产物和镜像工具链路径保持原有布局，不新增构建根目录。三个入口各自在自己的既有 target 目录缓存，Windows 的 x86/x64 共用同一下载目录，不借用主机的 Cargo home，也不引入跨 checkout 的共享可变缓存。

## Alternatives considered

- 将依赖写入镜像：临时容器可直接复用，但依赖缓存随 Cargo.lock 变化会触发镜像重建，还需把 workspace 依赖输入纳入镜像构建上下文。
- 全机共享一个 Cargo home：可以进一步减少跨 worktree 的下载，但新增全局占用、并发锁与清理责任；本切片复用已有挂载，随 worktree 清理。
- 只挂载 registry/cache 和 git/db：可减少持久化占用，但每次运行仍需解包源码和检出 git 依赖。本地目录缓存无需 CI 归档上传，保留 Cargo 自己管理的完整下载目录更直接。

## Consequences

缓存跨临时容器保留，后续同一依赖集可以直接复用。每个 worktree 的 target 会增加 Cargo 下载缓存占用，删除 worktree 时一并清理。首次启用仍需下载，改变源码缓存绝对路径还可能使旧编译结果重新编译；不承诺首次运行完全断网或复用全部旧产物。依赖版本继续由 Cargo.lock 和 --locked 约束，不将 --offline 设为日常构建默认值。

与 [镜像工具链准备](2026-10-09-linux-gate-toolchain-components.md) 部分重叠：该笔记约束固定编译器和组件的镜像层，本篇约束运行后产生的 registry/git 缓存，二者分别保留。[Windows 交叉工具链](2026-10-09-windows-cross-toolchain.md) 约束镜像内的编译器和目标标准库，不接管运行后下载的依赖；Windows 扩展直接更新本篇下载缓存决定。头文件别名笔记涉及构建镜像时的目录准备，其他 registry 命中涉及引擎查询，与本决定无关。检索已有 proposed/implemented/rejected 没有其他 Cargo 下载缓存归属。

官方 [Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html) 说明 CARGO_HOME 管理下载和源码缓存，[Rustup 环境变量](https://rust-lang.github.io/rustup/environment-variables.html) 说明工具链由独立的 RUSTUP_HOME 管理；Windows 镜像的 bin PATH 与 Rustup 位置保持原状。隔离验收脚本已在现有挂载内采用同一方式。

## Verification

### Linux 门禁

真实原生门禁修复前首次运行通过 73 项 CTest，下载 258 个 crates 并更新两个 git 依赖。复用已编译目标目录的断网新容器仍因无法检出 git 依赖而退出 101。在线第二次运行再次下载同样的 258 个 crates 并更新两个 git 依赖，进程退出 0。修复后的实际桌面和原生门禁分别填充持久化 Cargo home。随后两个断网的新容器分别执行 `cargo check -p msime-desktop --locked --offline --all-targets --message-format short`、`cargo build -p msime-host-api --locked --offline`，使用原有挂载与对应的 CARGO_HOME，均成功并退出 0，没有下载依赖。桌面检查的源路径是只读挂载，两个编译目标目录仍可写。

本次 arm64 验证时，两个 Cargo home 占用约 370 MiB 和 274 MiB。它们保存在既有 target 目录内，清理任务 worktree 时一并移除，不留独立缓存根目录。

`bash scripts/verify-local.sh --quick` 全阶段范围通过：当前主机的 102 项 Windows 测试、Rust workspace、Android Rust 与 Java 冒烟、Linux 桌面检查、原生 73 项 CTest、五笔生产构建、安装/链接隔离、日文登记、macOS 原生构建与共享 Apple bridge。Android 桌面库保留原有 11 条警告；HarmonyOS 设置依赖与真实编译准备输入缺失，Wasm 所需工具链缺失，Windows 原生和 pipe-only 缺少 MinGW/工程配置，x86 语法检查缺 x64 构建输入。没有执行设备或真实输入法框架验收。Shell、笔记与 diff 校验通过。

### Windows 交叉构建

在原始容器布局中实际执行 `cargo build --locked -p msime-host-api --target i686-pc-windows-gnu`，下载 273 个 crates、更新两个 Git 依赖，首次冷编译 `14m 59s`，退出 0。随后一个全新断网容器先确认 DLL 已存在，再执行相同 Cargo 构建加 `--offline`，仍因无法检出固定的 `chinese-ime-lm` Git 依赖而退出 101。

按修改后容器脚本的仓库/工具挂载与 `CARGO_HOME` 填充缓存，真实 x86 宿主库构建退出 0，objdump 确认为 `pei-i386`。首次启用新缓存仍下载 273 个 crates、更新两个 Git 依赖，源码绝对路径改变后发生重新编译，本次冷编译耗时 `15m 57s`；不把这次冷编译与后续暖构建的差异当作单纯下载优化的百分比。随后另一个 `--network none` 的全新容器在同一持久化 Cargo home 中实际执行该 Cargo 构建加 `--offline`，`3.83s` 成功，未下载 crates 或更新 Git 依赖，DLL 仍是 `pei-i386`。

设置新 Cargo home 后，Cargo `1.97.1`、Rustfmt `1.9.0-stable`、Clippy `0.1.97` 均正常；断网调用两个 Windows GNU 标准库安装命令，都报告已安装。缓存占用约 415 MiB，位于既有 target 中，随任务 worktree 清理。上述构建直接执行 `build-cross.sh` 的真实 Cargo 阶段，没有运行整个 vcpkg/CMake 产品流水线，未构建 C++ TSF DLL、Server 或全部原生测试，也未运行 DLL、Wine 或系统输入法验收。

本轮 `bash scripts/verify-local.sh --quick` 退出 0：Windows 范围的 102 项主机测试、Rust workspace 和共享 Apple bridge 编译通过；2 项按主机差异排除、59 项需要 Windows 构建、1 项不是测试。Windows 原生/pipe-only 因本机缺少 MinGW 或工程配置跳过，x86 语法检查缺少 x64 旗标，桌面 workspace 包缺少 macOS 资源；Android、Wasm、HarmonyOS、Linux 与 macOS 原生阶段按 scope 跳过。本轮只修改 Windows 构建容器配置与文档，Linux 数据是上个切片的验证。Shell 语法、笔记与 diff 校验通过。
