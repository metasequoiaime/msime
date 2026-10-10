# Agent Note: Linux 镜像预装工具链组件

Status: implemented

## Problem

仓库的 `rust-toolchain.toml` 要求 `rustfmt` 和 `clippy`。修复前的原生构建、桌面检查、隔离验收基镜像和 RPM 打包镜像实际只装有 `cargo`、`rustc` 和 `rust-std`，缺少这两个组件。这些路径每次创建临时容器，Cargo 首次启动时会按声明补装；删除容器也删除这次安装，下一次门禁再次下载。

## Decision

在原生构建、桌面检查、隔离验收和 RPM 打包的 Dockerfile 中显式安装两个组件，沿用已安装的固定 Rust 工具链，把组件保存在镜像构建缓存里。组件层位于现有最后一个构建层之后，保留系统开发包、验收镜像的 Wayland 驱动和 RPM 的编译器安装缓存。Fcitx5 和现代 IBus 验收镜像继承验收基镜像，Deb 打包镜像继承原生门禁镜像，无需再重复安装组件。

`tests/tools/check-image-toolchain.sh` 在断网的新容器里只读挂载真实仓库，执行 `cargo`、`cargo fmt` 和 `cargo clippy` 的版本命令。缺失组件或工具链版本与仓库声明不匹配时无法临时下载，应当失败；修复后的四个直接准备工具链的镜像均须通过。

## Alternatives considered

- 从仓库工具链声明中去掉组件：会消除当前补装动作，但开发者不再自动取得格式和 Clippy 工具，削弱原有工具链约束。
- 把 `rustup` 目录另设持久化挂载：能跨运行复用安装，但新增可变缓存、挂载规则与清理责任，而固定版本的组件适合直接存入既有镜像层。
- 把安装合并进既有工具链安装命令或放在系统依赖层之前：步骤更集中，Debian 镜像还可共享组件层，但会使已有编译器或大体积系统包缓存失效。本决定复用既有构建层。

## Consequences

升级仓库 Rust 版本或新增工具链组件时，需要同步镜像准备步骤；断网回归以真实工具链文件为依据，能暴露不匹配。镜像增加组件占用，不引入新的构建目录或共享可变缓存。

运行后产生的 registry/git 缓存由 [Linux Cargo 下载缓存](2026-10-09-linux-cargo-cache.md) 约束，保存在既有 target 挂载内；它不替代固定工具链和组件的镜像层。

断网自检用于镜像构建或工具链升级后的定向回归，不在每次 quick 中另启预检容器。日常门禁仍按原来的两条编译路径执行，避免为减少组件下载而增加容器启动开销。

Linux 测试构建开关笔记约束 CMake 目标生成，与本篇的镜像准备互不替代。[Windows 交叉镜像](2026-10-09-windows-cross-toolchain.md) 同样预装组件，但还需匹配固定工具链名称和目标标准库归属，由另一篇笔记约束。

## Verification

日常原生和桌面门禁修复前在断网容器中首次 Cargo 调用均尝试下载工具链清单，并因 DNS 不可用失败；镜像组件列表均只有 `cargo`、`rustc` 和 `rust-std`。预装后的两个镜像均在断网的新容器中成功执行 Cargo、Rustfmt、Clippy 的六条版本命令，两个回归进程均以 0 退出。两个镜像重建时的原有 apt 层均命中缓存；原生镜像定向重复构建和桌面门禁重复构建的组件安装层均命中缓存，没有再次下载安装组件。

日常门禁切片的 `bash scripts/verify-local.sh --quick` 通过：workspace 与 Android Rust 编译、Linux 桌面编译、原生 73 项 CTest、五笔生产构建及两版本安装/链接隔离、日文登记和共享 Apple bridge 编译。Linux 两条实际 Cargo 构建日志没有工具链同步或组件下载。Android 桌面库仍有原有的 11 条警告；Windows 本机测试、Wasm、Android Java 和 macOS 原生阶段按改动范围跳过，Windows 交叉构建缺少 MinGW，HarmonyOS 设置依赖和真编译准备输入缺失。没有执行设备或真实输入法框架验收。Shell 语法、笔记校验和 `git diff --check` 通过。

验收基镜像和 RPM 镜像扩展回归：修复前组件列表均只有 `cargo`、`rustc` 和 `rust-std`；断网版本检查均在首次 Cargo 调用尝试下载工具链清单时因 DNS 不可用失败，两个进程退出 1。修复后的两个镜像均在断网的新容器中成功执行六条版本命令（Cargo `1.97.1`、Rustfmt `1.9.0-stable`、Clippy `0.1.97`），两个进程退出 0。重复构建的组件层全部命中缓存，验收镜像的 apt 与 Wayland 驱动层、RPM 的 dnf 与编译器安装层均保留缓存。验证在 arm64 容器中执行，未构建派生 Fcitx5、现代 IBus 或 Deb 镜像，也未执行真实输入法框架或发行包验收。

验收/RPM 扩展切片的 `bash scripts/verify-local.sh --quick` 通过：Rust workspace、Linux 桌面编译、原生 73 项 CTest、五笔 53 步生产构建、两版本安装/Host API 链接隔离、日文登记和共享 Apple bridge 编译。Android Rust/Java、Wasm、HarmonyOS、Windows 原生及 macOS 原生按改动范围跳过，workspace 中的桌面包因 macOS 资源未准备而跳过。笔记校验和 `git diff --check` 通过。
