# Agent Note: Windows 交叉镜像与仓库工具链一致

Status: implemented

## Problem

修复前的 Windows 交叉镜像通过最小配置安装浮动的 `stable`，并给该工具链安装两个 Windows GNU 标准库。源码挂载后，仓库的 `rust-toolchain.toml` 选择固定 `1.97.1` 和 `rustfmt`、`clippy`。实测 stable 为 `1.99.0`，且安装列表只有 stable；固定 `1.97.1` 未准备，组件列表也没有 Rustfmt 和 Clippy。容器进入仓库后会安装另一份工具链，然后为它重新下载目标标准库。临时容器删除后准备工作丢失。

## Decision

镜像默认工具链安装为仓库固定版本，显式补齐两个组件，保留原有两个 Windows GNU 目标。系统依赖和 MinGW 线程模型保持原有缓存；头文件大小写兼容由 [单次扫描别名](2026-10-09-windows-header-aliases.md) 约束；替换浮动编译器安装层，不在旧 stable 上再叠一份固定编译器。Rustup 脚本先下载成功再执行，下载失败直接终止该层，避免 `curl | sh` 把空安装缓存为成功。

`tests/tools/check-cross-image-toolchain.sh` 只读挂载真实仓库，按指定镜像架构在断网的新容器中调用 Cargo、Rustfmt、Clippy，执行与构建脚本相同的 `rustup target add`，并使用两个 MinGW 链接器链接包含标准库和线程调用的合成 Rust 程序。用 objdump 验证 x86/x64 的 PE 格式，产物写入已有 `target/windows-cross/<arch>`。该探针验证工具链准备和真实链接，不运行 Windows 程序，不替代产品构建或系统输入法验收。

## Alternatives considered

- 在现有 stable 层之后安装固定工具链：能保留旧编译器安装缓存，但镜像永久携带两份编译器，浮动安装仍影响重建，两个目标的归属也容易混淆。
- 通过 `RUSTUP_TOOLCHAIN=stable` 绕开仓库声明：可复用现有安装，但使交叉构建与其他平台使用不同编译器，并绕开仓库的组件要求。
- 只给 stable 补上组件：改动少，但挂载仓库后仍切换到固定工具链，未解决编译器和目标重复下载。

## Consequences

临时容器复用镜像里的固定工具链和目标安装，避免重复网络准备；镜像增加两个组件的占用。固定版本、组件和目标与仓库声明仍需同步维护；断网探针在升级镜像后手动运行，不为每次 quick 增加容器开销。容器架构由 [原生交叉编译宿主](2026-10-09-windows-native-cross-host.md) 接管；系统包和共享构建环境不保证耗时或字节级可复现，未测量相对耗时就不宣称改善百分比。

与 [Linux 镜像组件准备](2026-10-09-linux-gate-toolchain-components.md) 部分重叠，但本决定还约束固定工具链名称和 Windows GNU 标准库归属，不替代 Linux 决定。检索 implemented/proposed/rejected 未发现 Windows 交叉工具链已有归属；其他 Windows 笔记涉及运行时和产品行为，与镜像准备无关。

Rustup 的 [工具链名称](https://rust-lang.github.io/rustup/concepts/toolchains.html) 与 [最小配置](https://rust-lang.github.io/rustup/concepts/profiles.html) 官方说明支持上述区别。

## Verification

修复前实际构建的 amd64 镜像只有 stable（Rust `1.99.0`），两个 Windows GNU 标准库安装在该工具链上，缺少 Rustfmt、Clippy 和仓库固定 `1.97.1`。断网探针首次 Cargo 调用尝试下载固定工具链清单，因 DNS 不可用失败，进程退出 1。修复后的新容器在断网状态下成功执行 Cargo `1.97.1`、Rustfmt `1.9.0-stable`、Clippy `0.1.97` 的版本命令，两个 `rustup target add` 均报告标准库已安装。合成 Rust 程序由真实 MinGW 链接器分别生成 `pei-i386` 和 `pei-x86-64`，objdump 格式检查通过，进程退出 0。

修复后和定向重复构建均复用系统依赖、MinGW 线程选择与头文件兼容层；重复构建的固定工具链及目标标准库安装层均命中缓存。工具链切片首次原镜像构建的头文件兼容层耗时 `628.7s`，该切片仅保留原层缓存，别名算法的后续优化由上述独立决定记录；不把首次构建与缓存重建的差异当作工具链修复的耗时百分比。

`bash scripts/verify-local.sh --quick` 通过：当前主机的 102 项 Windows 测试、Rust workspace 和共享 Apple bridge 编译；2 项按主机差异排除、59 项需要 Windows 构建、1 项不是测试。Windows 原生/pipe-only 构建缺少本机 MinGW 或工程配置，x86 语法检查缺少 x64 构建旗标；Android、Wasm、HarmonyOS、Linux 与 macOS 原生阶段按 scope 跳过，workspace 桌面包因 macOS 资源未准备跳过。未构建完整 Windows 产品，未运行 PE、Wine 或系统输入法验收。Shell 语法、笔记和 diff 校验通过。

原生宿主验证期间首次镜像安装遭遇 Rustup DNS 失败；旧 `curl | sh` 安装层返回成功，后续目标安装才报 `rustup: not found`。新增合成失败下载回归在原命令上失败，改为先下载再执行后通过，失败不再被管线吞掉。
