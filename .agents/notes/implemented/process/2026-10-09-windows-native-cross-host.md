# Agent Note: Windows 交叉编译使用原生 Linux 宿主工具

Status: implemented

## Problem

Windows 交叉编译容器强制 `linux/amd64`，在实测 `aarch64` Docker daemon 上连 Rust 编译器、MinGW 和 vcpkg 宿主工具都需要仿真。Windows 输出架构与运行编译器的 Linux 架构是两个维度；固定为 amd64 使 ARM64 主机承担额外工作。直接切换容器架构又会让已有 Linux 缓存中的 amd64 vcpkg 可执行文件与宿主依赖进入 ARM64 容器。

## Decision

`build-cross-container.sh` 从 Docker daemon 的 `OSType` 和 `Architecture` 选择 amd64 或 ARM64 Linux 镜像，并给 build/run 显式传入相同平台，避开客户端架构和 `DOCKER_DEFAULT_PLATFORM` 的影响。amd64 保留原镜像标签和缓存路径；ARM64 使用 `msime-cross:local-arm64`，在既有 `target/tooling-linux` 与 `target/windows-native-deps-linux` 下使用 `arm64` 子目录。工具和宿主依赖隔离，Cargo 下载缓存继续共用。其他 Linux 架构沿用已有 amd64 仿真路径；非 Linux daemon 在准备缓存之前报错；Docker 不可用的既有跳过行为保持。

`build-cross.sh` 对 Linux `aarch64`/`arm64` 使用 `arm64-linux` 宿主 triplet，Windows 目标仍由 `x86`/`x64` 参数选择。镜像安装本机 Rust 和 Debian MinGW，不更改 Windows ABI、异常模型或产品运行边界。工具链探针按被检查镜像架构运行；Wine 执行镜像仍为 amd64，运行时来源由 [构建方暂存运行时](2026-10-09-windows-producer-runtime.md) 接管。

固定 vcpkg 提交 `ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0` 的 [bootstrap](https://github.com/microsoft/vcpkg/blob/ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0/scripts/bootstrap.sh) 提供 ARM64 glibc 可执行文件，其 [arm64-linux triplet](https://github.com/microsoft/vcpkg/blob/ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0/triplets/community/arm64-linux.cmake) 定义原生 Linux 宿主；[官方宿主依赖说明](https://learn.microsoft.com/en-us/vcpkg/users/host-dependencies) 区分宿主工具和目标库。[Docker info](https://docs.docker.com/reference/cli/docker/system/info/) 给出 daemon 架构。

## Alternatives considered

- 保留强制 amd64：能继续使用现有缓存和唯一镜像，但 ARM64 daemon 的编译器仍需仿真，Windows 目标并不要求 Linux 编译器也为 x86。
- 仅删除 `--platform`：改动更少，但受 `DOCKER_DEFAULT_PLATFORM` 影响，且已有 vcpkg 宿主可执行文件与缓存不隔离。
- 根据客户端 `uname` 选择平台：本地 Docker Desktop 上通常可用，但 Docker context 可指向不同架构的远端 daemon，不能代表实际编译环境。

## Consequences

ARM64 daemon 可以原生执行工具链，首次构建仍需准备镜像与依赖；两个宿主架构各有缓存，占用增加但随 worktree 删除。Windows 产物仍落在已有 target 目录，未另建 Cargo target 根。旧 amd64 缓存保持可用，不迁移、不删除；未验证原生编译的 Linux 架构继续使用原有 amd64 路径。Wine 的执行架构与编译架构分开维护。实际耗时受磁盘、共享 daemon 和缓存影响，没有同条件基线就不报告加速比例。

检索现有 implemented/proposed/rejected 后，[固定工具链](2026-10-09-windows-cross-toolchain.md) 与本篇部分重叠，原地同步探针架构并互链；[头文件别名](2026-10-09-windows-header-aliases.md) 保留独立算法与历史 amd64 测量；[Cargo 下载缓存](2026-10-09-linux-cargo-cache.md) 保留独立持久化决定。未发现相同原生宿主选择决定。

## Verification

ARM64 daemon 两个名称的平台选择回归在原实现上失败；失败下载回归也在旧管线命令上失败。最终 8 项脚本回归通过，覆盖 daemon 与客户端架构不同、默认平台干扰、amd64 原路径保留、ARM64 独立缓存、其他 Linux 架构保留 amd64 路径、非 Linux daemon 拒绝、Docker 不可用时跳过和下载失败传播。

真实 ARM64 镜像构建退出 0。断网新容器的工具链探针退出 0，固定 Cargo、Rustfmt、Clippy 与两个目标标准库可用，生成 `pei-i386` 与 `pei-x86-64`。Rust host 为 `aarch64-unknown-linux-gnu`，两个 MinGW 编译器的 ELF Machine 均为 AArch64；固定 vcpkg 也是 AArch64 ELF。

真实 `build-cross-container.sh x86` 和 `x64` 均退出 0，使用 `arm64-linux` 宿主 triplet，完成两个架构的锁定清单依赖准备，以及 Rust 宿主 DLL、TSF DLL、Server 与全部原生测试链接。检查的主要 DLL 和 Server 分别为 PE32 与 PE32+。Rust 编译阶段分别为 1m 05s、1m 00s；使用脚本默认并发，不能与上轮 4 jobs 的 amd64 结果计算加速比。

随后在全新 `--network none` ARM64 容器中使用相同挂载、Cargo home 和依赖根，顺序执行真实 `build-cross.sh x86`、`x64`，进程退出 0。两个架构依赖均报告已安装，Cargo 没有下载或更新 Git，Rust 阶段分别为 1.31s、0.45s；CMake 仍有少量重新链接，不宣称零编译。

`bash scripts/verify-local.sh --quick` 退出 0：102 项 Windows 主机测试、8 项新增回归、常驻契约、Rust workspace 和共享 Apple bridge 编译通过。Windows 原生与 pipe-only 构建缺少本机工具链而跳过，x86 语法检查缺少 x64 构建旗标；其他平台按 scope 跳过，桌面包因缺少 macOS 资源跳过。笔记、Shell 语法和 diff 校验通过。构建仍报告未改源码的 unused、signedness 等警告。未运行 Windows 程序、Wine 或系统输入法验收，SDK C++/WinRT 手写示例不在 GNU 构建范围内，未打包运行时或安装包。
