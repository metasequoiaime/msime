# Agent Note: Linux 关闭测试时排除测试构建目标

Status: implemented

## Problem

Linux 容器门禁在 full 构建并运行完整原生测试后，还构建五笔版验证版本隔离。五笔配置已经设置 `BUILD_TESTING=OFF`，但主 CMake 无条件声明多组测试目标，仍产生 166 个 Ninja 构建步骤，其中 52 个目标读取测试源码。相同测试在第二个版本重复编译，却不执行。

## Decision

主 CMake 的测试目标及登记放在 `if(BUILD_TESTING)` 内，与 Fcitx5 子工程保持一致。`ibus-engine-smoke` 也是测试目标：现有隔离验收脚本使用默认开启测试的配置，它没有产品安装规则。语音 worker 的测试目标和安装规则同时要求 `BUILD_TESTING` 与 `MSIME_LINUX_VOICE`。

容器门禁显式开启 full 的测试，五笔版继续关闭测试；两个配置请求 CMake File API 的 `codemodel-v2`。回归检查读取生成图中的目标与源码，要求关闭测试的配置没有测试源码目标，两个配置的生产目标集合相同，并保留主要产品入口。检查在五笔编译前执行，因此重新引入遗漏条件块会立即失败。

## Alternatives considered

- 只把测试目标移出默认 `all`：改动较少，但关闭测试仍生成测试目标，显式构建时依然重复编译，且偏离 `BUILD_TESTING` 的惯例。
- 删除第二次版本构建：节省更多时间，但失去版本脚本改写、安装路径和 Host API 动态库隔离验证。门禁仍保留五笔生产构建、两版本安装比较和动态链接检查。
- 以源码正则检查条件块：无需 Linux 工具链，但不能证明 CMake 生成的目标图，容易漏掉嵌套条件与辅助目标。

## Consequences

默认开发和验收配置保留测试，产品配置不再编译测试源码。需要 `ibus-engine-smoke` 或语音 worker 验收程序时必须开启测试。目标图检查由固定容器门禁调用，不增加任何构建目录或依赖。

检索活跃笔记中的 `BUILD_TESTING`、`build-container.sh` 和 Linux 构建门禁，没有此开关的既有归属。原生语音套接字就绪笔记处理测试启动竞态，PR 打包笔记处理 CI 触发策略，都与本次目标生成边界无关，保持原有决定。

## Verification

修复前，目标图回归在五笔 `BUILD_TESTING=OFF` 配置失败，列出 52 个测试目标；修复后通过，full 生成 57 个测试目标，五笔版没有测试目标，两边均保留 16 个生产目标。开启测试的 73 项 CTest 登记名称及顺序与修复前一致。在相同固定容器、尚未编译 C++ 的配置下，`ninja -n` 的五笔首次构建计划从 166 步降至 53 步，减少 113 步（约 68%）；这是构建工作量对照，不是耗时基准。`MSIME_LINUX_VOICE=ON` 时，full 生成 58 个测试目标并保留 worker 安装规则，五笔关闭测试的配置不生成 worker 目标或安装规则；恢复默认选项后再次通过目标图检查。

`bash scripts/verify-local.sh --quick` 通过：Windows 本机测试 102 项、workspace 与 Android Rust 编译、Android Java 宿主检查、Linux 桌面编译、Linux 原生 73 项 CTest、五笔生产构建与安装/链接隔离、日文登记、macOS 原生编译和共享 Apple bridge 编译。Android 桌面库仍有原有的 11 条警告。Wasm、MinGW、HarmonyOS 设置依赖及真编译准备输入缺失的阶段按脚本跳过；没有执行设备或真实输入法框架验收。Shell 语法、笔记校验和 `git diff --check` 通过。
