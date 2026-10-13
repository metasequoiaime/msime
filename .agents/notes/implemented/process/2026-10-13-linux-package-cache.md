# Agent Note: Linux 打包复用镜像与编译缓存

Status: implemented

## Problem

#7601 的本地重复打包仍调用镜像构建，临时容器丢弃 Cargo 下载缓存，并删除全部 CMake 构建树；只保留 Rust target 不能让整条打包路径增量运行。

## Decision

Deb、RPM 与 legacy 打包按 Dockerfile、目标平台、构建参数和仓库工具链声明计算镜像标签。命中已有镜像时直接使用其不可变 ID；Deb 的父镜像 ID 参与子镜像键。Deb 的父镜像另有从 ID 派生的固定标签：BuildKit 把 `FROM sha256:<本地 ID>` 当作名为 sha256 的仓库去拉取，不能直接用本地 ID；ID 标签不被其他配方覆盖。Dockerfile 通过标准输入传入空上下文，不把测试或产品源码写入镜像，源码继续挂载。保留临时容器和既有 target 目录，把 Cargo home 放进同一个挂载；同一镜像保留原生对象文件，每轮删除 CMakeCache.txt 并配置，以免上一轮的版本和可选资源配置残留。镜像 ID 变化时清掉编译产物，保留依赖下载。

## Alternatives considered

- 保留同一个运行容器：运行后安装的依赖也能复用，但失败残留、跨任务并发和版本追踪更难管理；持久化镜像与挂载已经涵盖当前构建需要的状态。
- 只依赖 Docker 层缓存：无需新增镜像键，但每轮仍要解析构建并接触基础镜像元数据，也不能解决运行后的下载缓存和 CMake 全量重编译。
- 保留全部 CMake 配置：最省配置时间，但旧版本号、桌面二进制和可选资源路径可能进入下一次包。只删除配置缓存，保留对象文件与 Ninja 依赖记录。

## Verification

`python3 scripts/test-linux-package-cache.py` 的 9 项检查通过，覆盖重复调用与源码变化命中、配方/平台/工具链/父镜像失效、镜像删除和构建失败、空上下文约束、BuildKit 父标签输出，以及环境变化后仅删除编译结果。其中实际运行 Deb/RPM 入口两轮，第二轮没有调用 `docker build`，验证 Cargo home 与原有 target 挂载接线。

真实 Docker 实验使用已有原生门禁镜像派生合成配方。首次构建后再次命中同一镜像；第二个 `--network none` 的临时容器成功离线构建固定 `itoa 1.0.15` 依赖，Rust 二进制与 C++ 对象时间戳不变，Ninja 报告无需编译。删除 CMakeCache.txt 后把合成包版本从 0.0.1 改为 0.0.2，并移除上一轮设置窗口路径，生成结果只含新配置。

实际 full 版 arm64 Deb 打包以 `MSIME_PACKAGE_DESKTOP=0 MSIME_PACKAGE_EDITIONS=full` 运行，首轮生成 `.deb`、`.tar.gz` 和 SHA256SUMS，84 项 CTest 全部通过。第二轮通过 Docker 包装器拒绝任何 build 调用，并把 run 限制为 `--network none`；两镜像命中、没有依赖下载，84 项 CTest 全部通过，原生阶段只重新链接 Rust 库依赖的目标。第二轮完整打包退出 0，再次生成三个附件；119 个 C/C++ 对象的纳秒时间戳全部保持，打包镜像 ID 未变。

quick 首轮暴露 `input-runtime` 已有七处完整夹具初始化遗漏 `first_page_rows`，补上 `..Fixture::default()`；该机械修补通过 `cargo fmt --all --check`、`cargo check -p msime-input-runtime --all-targets` 和对应 Clippy。包含修补的 `CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 bash scripts/verify-local.sh --quick` 退出 0：workspace、Android Rust/JVM、Wasm 编译与 Clippy、Linux 桌面与原生 83 项 CTest、Windows x64 交叉构建和 GNU 桌面检查、macOS 原生与 Apple bridge、x64/x86 pipe-only 编译通过；HarmonyOS 真打包输入缺失，按既有门禁规则跳过。ShellCheck、笔记和 diff 检查通过。

没有执行 RPM/legacy 完整打包、设置窗口随包构建或真实桌面安装验收。

## Consequences

重复构建可跳过镜像准备和依赖下载，并让 Ninja/Cargo 复用对象文件；每轮仍重新生成配置与发行附件。镜像需显式删除才能释放 Docker 占用，缓存目录随 worktree 清理。新缓存布局首次运行会重新编译，以免旧环境产物混入。完整安装包构建需要足够空间；合成容器实验只证明缓存与配置生命周期，不能替代产品打包和安装验收。

## Related notes

检索 proposed、implemented、rejected 中的 Linux 镜像、Cargo home 与打包笔记：[Cargo 缓存](../../implemented/process/2026-10-09-linux-cargo-cache.md) 部分重叠，扩展其挂载决定；[工具链组件](../../implemented/process/2026-10-09-linux-gate-toolchain-components.md) 部分重叠，仍由 Dockerfile 预装组件；[legacy 基线](../../implemented/feature/2026-10-09-linux-legacy-glibc228-deb.md) 只约束 ABI 与干净安装验收，后半段验收容器不复用。没有完全吸收或过时的提案。

空上下文的适用条件见 [Docker 官方文档](https://docs.docker.com/build/concepts/context/)，删除配置缓存与保留对象文件的边界见 [CMake Cache](https://cmake.org/cmake/help/book/mastering-cmake/chapter/CMake%20Cache.html)。
