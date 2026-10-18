# Agent Note: Windows 本地门禁保留可运行的检查

Status: implemented

## Problem

在 Windows 上验证原生设置页和鸿蒙工具栏时，本地 quick 门禁暴露了既有的跨平台检查问题：共享运行时测试和引擎示例无条件引用 Unix 接口，合同检查把 Windows 路径当成 POSIX 文本比较，SQLite 测试连接阻止临时目录清理，Unicode 许可证的 CRLF 检出使原始字节校验失败。部分 Linux 专用测试依赖 `fcntl`、POSIX 可执行脚本和路径分隔符，不能在原生 Windows Python 中执行。真实 MSVC 构建还发现测试中的整数窄化、局部变量遮蔽，以及用运行期 `assert` 检查常量协议布局，这些都在 `/WX` 下阻止测试产物生成。

## Decision

保留所有能在 Windows 上执行的检查，补齐测试和示例的目标条件，路径契约按 POSIX 表示比较，测试退出前显式释放 SQLite 连接，并将需要原始字节校验的 Unicode 许可证固定为 LF。只对确实依赖 POSIX 运行时的测试声明 Windows 跳过；Linux 上仍运行它们。引擎诊断示例继续构造符号链接资源视图，Windows 使用对应的文件或目录链接接口，不引入资源复制。C++ 测试使用与接口一致的整数类型，局部变量只改名称，常量协议布局用 `static_assert` 保留检查；不关闭 `/WX` 或屏蔽警告。

本机 Python 启动器、Git Bash 搜索次序和原生工具读取 `/dev/fd` 的问题通过临时调用环境处理，不把本机路径、凭据或工具 shim 加入仓库。缺少 SDK、设备或构建依赖的阶段仍如实报告，不能据此声称全门禁通过。

`candidate_layout_reload` 的离线释义夹具用原子 `create_directory` 取得独立目录，名字含单调时钟值，碰撞时换名重试；只清理自己成功创建的目录。多个 worktree 的本机门禁可以同时运行，不能在测试开头删除固定名字的共享目录。

## Existing note audit

- [引擎测试进 CI](2026-10-10-engine-tests-in-ci.md)：部分重叠，保留 CI 覆盖与拒绝符号链接的约定，本篇补充 Windows 示例和本机门禁的兼容范围。
- [Linux 打包缓存](../process/2026-10-13-linux-package-cache.md)：部分重叠，保留镜像和缓存策略，本篇只限定 POSIX 命令入口测试的运行平台。
- [脚本插件合集](../architecture/2026-10-12-script-plugin-collection.md)：部分重叠，保留数据来源与许可证字节校验，本篇固定许可证检出的换行。
- Windows 头文件别名、Linux 工具链缓存及拆字功能笔记与本次策略无关；检索命中的日文、拼音和九键分配优化笔记仅记录曾运行门禁，也不被本篇替代。没有完全吸收的旧笔记或过时提案。

## Alternatives considered

- 在 Windows 上跳过全部合同检查。这样可以立即消除平台误报；没有采用，因为大多数检查只读仓库文本，路径比较和连接清理修正后仍能提供真实覆盖。
- 用 WSL 执行全部门禁。它提供完整的 POSIX 环境，适合 Linux 检查；没有用它代替本次 Windows 验证，因为它不能确认原生 Windows Rust 目标和 MSVC 设置程序的构建，工具混用还会改变路径与换行处理。
- 诊断示例复制全部资源。它不要求创建符号链接；没有采用，因为既有示例用链接共享大型词库，编译兼容修复不应引入额外资源副本或改变 Unix 行为。
- 诊断示例复用 `msime_path_trust::untrusted_symlink`。这样能把平台接口集中到已有路径工具；没有采用，因为它只在 Unix 测试支持配置下导出，并会在 root 下修改链接属主，用于测试拒绝路径。诊断资源视图需要被读取，不能改变原有链接的信任语义。

## Consequences

- Windows 上可以继续执行仓库文本、包定义和临时 SQLite 数据的检查，无需把平台误报记成失败基线。
- 平台跳过缩小了 Windows 上能执行的覆盖面，但不改变 Linux 上的测试集合。`test-linux-package-cache.py` 仍执行 8 项可运行测试，只跳过 1 项依赖 POSIX 命令入口的测试。
- Windows 示例的运行仍受系统链接创建权限约束，编译通过不等于已运行词库诊断。临时工具 shim 只修正调用环境，不屏蔽失败或替代真实检查；后续运行本机门禁仍需相同的工具环境。

## Verification

- `cargo check -p msime-input-runtime --all-targets --locked --offline`、215 项测试和 `cargo clippy -p msime-input-runtime --all-targets --locked --offline --no-deps -- -D warnings` 通过。
- `cargo check --examples -p msime-engine` 通过，两个诊断示例在 Windows 原生目标上编译通过。修改的 Rust 文件通过 `rustfmt --check`。
- `cargo clippy -p msime-engine --examples --locked --offline --no-deps -- -D warnings` 通过。
- `target/windows-pipe` 的 MSVC Debug 构建通过；修正的四个 C++ 测试通过 `ctest`，没有关闭 `/WX`。
- 路径比较、SQLite 清理和许可证校验的定向合同检查通过。拆字表检查核对 17942 个键、20387 个字和 12 项许可渠道；缺上游压缩包时的重生成、缺 Unihan 缓存时的数据生成按既有契约跳过。
- 本机 Windows 原生设置页通过 MSBuild 完整构建与链接，所需 35 个 Host API 导入均存在于同目录 DLL 的导出中。旧 Host API 缓存只按现有构建入口离线更新，没有改动接口契约。
- 使用真实 Python、Git Bash、Node 和 pnpm 的临时调用环境执行 `bash scripts/verify-local.sh --quick`，退出 0，输出 `quick check passed`。可运行合同、771 篇笔记和 2 项归档封印、Windows workspace Rust 全目标检查及 pipe-only MSVC 构建通过；未准备资源的 `msime-desktop` 按既有门禁条件排除。
- 本机未安装 Android SDK/NDK、DevEco 工具、相关 wasm 目标或 Docker，不具备 Apple 编译环境；对应平台编译与设备阶段不能据此认定通过。

九键搜索页复用的本机 quick 在 Windows 可移植测试中暴露固定临时目录竞争。在独立临时根下，16 路并发执行同一旧测试 160 次，有 39 次因目录清理或内容断言失败；原子取得独立目录后，同样 160 次全部通过。此修正只涉及 Windows 测试夹具，不改变宿主的文件访问规则，也不据此声明 Android 设备验证通过。
