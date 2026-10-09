# Agent Note: Debian 10 基线的 legacy .deb：只含 IBus，在 buster 容器里构建

Status: implemented

## Problem

#6311：UOS 20 专业版（Debian 10.10 基线，glibc 2.28、GCC 8.3、Python 3.7、IBus 1.5.19 可装、默认框架 fcitx 4.2.9）装不上发布页的 `msime-linux_<版本>_<架构>.deb`。发布页的包在 `rust:1.97.1-bookworm` 里构建，`dpkg-shlibdeps` 按 bookworm 的库算出 `libc6 (>= 2.35)` 一类依赖，二进制本身也引用 `GLIBC_2.34` 起的符号；包里还有要 WebKitGTK 4.1、libsoup 3 的 Tauri 设置窗口和要 Fcitx5 5.0.20 的插件，这些 Debian 10 都没有。另外 `CMakeLists.txt` 写死 `ibus-1.0>=1.5.20`，`packaging.cmake` 写死 `ibus (>= 1.5.20), python3 (>= 3.9)`，随包脚本用了 Python 3.8 起的语法。

用户已定的范围：第一步只做「只有 IBus、没有设置窗口」的 `.deb`，amd64 与 arm64，不改发布工作流，不写 fcitx4 前端。

## Decision

- 新增构建镜像 `platforms/linux/tests/tools/Dockerfile.legacy`：`debian:buster` 按清单摘要固定，软件源改指 `archive.debian.org` 并关闭 Release 有效期检查；开发包与 `Dockerfile.build-gate` 相同，只去掉 Fcitx5；buster 的 CMake 3.13 与 nlohmann-json 3.5 太旧，换成按 SHA-256 固定的 Kitware CMake 3.25.1 预编译包与 nlohmann-json 3.11.2 源码（版本与 bookworm 门禁相同）；Rust 用 `Dockerfile.package-rpm` 同一个固定的 rustup-init 装 1.97.1。
- 新增 `platforms/linux/package-legacy-container.sh`，与 `build-container.sh`、`package-container.sh` 并列。第 1 步在上面的镜像里以 Release 编译 Host API 库和 `msime-mcp`、收集 Rust 许可证，用 `-DMSIME_ENABLE_FCITX5=OFF -DMSIME_IBUS_MIN_VERSION=1.5.19 -DMSIME_PYTHON_MIN_VERSION=3.7` 配置 full 版本，不传 `MSIME_DESKTOP_BINARY`，CPack 只出 DEB；随后解开包，逐个 ELF 文件读 `readelf -V`，任何一个要求高于 `GLIBC_2.28` 就失败，Depends 里出现 WebKitGTK、libsoup 3、GTK 3 或 Fcitx5 也失败。第 2 步在只有基础系统的 `debian:buster` 里 `apt-get install --no-install-recommends ./<deb>`，`ldd` 包内每个 ELF 文件不得有 `not found`，再用从构建镜像拷出的 CMake 跑第 1 步构建树的 ctest。`MSIME_LEGACY_ARCH=amd64|arm64` 经 `docker --platform` 选架构，build 和 run 都显式传，因为同一个清单摘要在本机已有另一架构的镜像时，不传会静默用上那一个。sherpa-onnx 运行库在宿主上用 `scripts/fetch_voice_runtime.py --platform` 取，该脚本要 Python 3.8，又不随包发布。
- 包名仍是 `msime-linux`，安装路径与发布页的包相同。
- `CMakeLists.txt` 新增缓存变量 `MSIME_IBUS_MIN_VERSION`（缺省 1.5.20）和 `MSIME_PYTHON_MIN_VERSION`（缺省 3.9），同时用于 `pkg_check_modules` 和 `packaging.cmake` 的 `.deb` Depends、RPM Requires。发布页的包不传，声明不变。

### 让同一份源码在 Debian 10 上构建所改的地方

每一处都按能力或版本条件生效，bookworm 上构建出的东西不变：

- IBus：对照 1.5.19 的头文件逐个检查宿主与测试用到的 `ibus_*`/`IBUS_*` 符号，只缺 `IBUS_INPUT_HINT_PRIVATE`（1.5.26）。`src/core/ClientEngine.h` 在 `!IBUS_CHECK_VERSION(1, 5, 26)` 时按协议的固定值 `1u << 11` 补上这个宏。1.5.27 的 `focus_in_id`、`has-focus-id` 原本就有条件编译；旧版 IBus 不报告客户端身份，`ime_mode_scope = app` 退化成所有窗口共用一个状态，这是 `ClientInputModeMemory.h` 已经写明的行为。
- GCC 8 的 `std::filesystem` 在 `libstdc++fs`：GNU 编译器低于 9 时在 `CMAKE_CXX_STANDARD_LIBRARIES` 末尾加 `-lstdc++fs`。
- glibc 2.34 以前 `shm_open` 在 librt、`pthread_create` 在 libpthread：Wayland 浮层在 `check_symbol_exists(shm_open)` 失败时链接 `rt`；三个直接用 `std::thread` 却没链接 `Threads::Threads` 的目标（`msime-linux-online-provider-contract`、`linux-typing-statistics-test`、`ibus-engine-smoke`）补上它。
- libwayland 1.16 的 `wl_pointer_listener` 没有 `axis_value120`：赋值按生成头文件里的 `WL_POINTER_AXIS_VALUE120_SINCE_VERSION` 条件编译。座位绑定的版本上限是 5，这个第 8 版的事件本来就不会送来。
- Python 3.7：随包的 `msime-linux-setup`、`msime-linux-voice-provider` 去掉海象运算符和 `Path.unlink(missing_ok=)`；`msime_voice_doubao.py` 把 `importlib.metadata` 的导入挪进原有的 `try`，3.7 上与缺少 websockets 一样报告依赖不满足。ctest 里三个 Python 测试的 `unlink(missing_ok=)` 同样改掉。

## Alternatives considered

- **在 bookworm 门禁镜像里用 cargo-zigbuild 指定 `<triple>.2.28` 编译 Rust**：好处是沿用现有镜像和新编译器，分诊里列过这条路。没有采用：要新增 zig 和 cargo-zigbuild 两个工具依赖；而且只解决 Rust，C++ 宿主仍要对着 glibc 2.28、libstdc++ 8 和 IBus 1.5.19 的头文件与库编译，等于还要在 bookworm 里另备一个 buster sysroot。在 buster 容器里原生编译，两边一次解决，也不新增工具。
- **把发布页包的下限也降到 IBus 1.5.19、Python 3.7**：源码现在确实能对着 1.5.19 编译。没有采用：发布页的包在 bookworm 上构建，`dpkg-shlibdeps` 给出的库依赖本来就高于这两个下限，降低手写的那两项对能装在哪里没有影响，只会让声明与实际测过的环境脱节。所以做成缓存变量，缺省值保持原样。
- **另起包名（例如 `msime-linux-legacy`）**：可以和发布页的包一起放在同一个发布页里而不撞名。没有采用：两者装同一套 `/usr` 路径，另起包名就必须写 `Conflicts`/`Replaces`，还要让 `msime-linux-setup`、维护脚本和 IBus 组件按新包名改写；同一个包名下 apt 把两者当作同一个包的不同构建处理，升级和卸载的行为与发布页的包一致。用户已经决定不改发布工作流，撞名问题眼下不存在。
- **legacy 包也打其他版本（五笔、日文等）**：配置这些版本要运行 `scripts/edition_linux.py` 改写脚本，它用到 Python 3.10 的 `Path.write_text(newline=)`。为了第一步只打 full 去改这个构建工具，收益不够，脚本只构建 full 并在用法里写明。
- **把 `scripts/fetch_voice_runtime.py` 改成 3.7 能跑，留在容器里执行**：与 `package-container.sh` 的做法一致。没有采用：它是构建工具，不随包发布，改它只为了在 buster 里运行；在宿主上按 `--platform` 取同样可靠，宿主本来就要有 python3。
- **用 buster-backports 的 CMake**：buster-backports 只有 3.18，`CMakeLists.txt` 和 `cmake/Edition.cmake` 用的 `string(JSON)` 要 3.19，最低版本又写的是 3.25。

## Consequences

- **收益**：Debian 10 基线的 amd64、arm64 系统有了一个能用 apt 装上的包，IBus 宿主、provider 程序、`msime-linux-setup` 和 `msime` 命令都可用，本地语音识别运行库也在 glibc 2.28 上能加载。构建脚本自己核对 glibc 上限、在干净的 buster 里安装并跑 ctest，不依赖人工检查。
- **代价**：没有设置窗口，词库管理、皮肤、账号、手写模型下载、检查更新都没有入口，偏好只能经 IBus 菜单和 `msime config set` 修改；豆包实时识别要 Python 3.9，不可用；buster 的 systemd 241 不支持 `systemctl --user -M`，升级和卸载时维护脚本只打印每个用户要执行的命令。
- **维护负担与重访信号**：这条构建线不在 CI、`verify-local.sh` 和发布工作流里，主线以后用到 Python 3.8+ 语法、IBus 1.5.20+ 接口、glibc 2.29+ 函数或 GCC 9+ 才支持的 C++17 特性时，要等有人跑 `package-legacy-container.sh` 才会发现。若决定把 legacy 包作为正式附件发布，先把这个脚本接进 `release-linux.yml`，再考虑给门禁加一道 Python 3.7 语法检查；若 UOS/麒麟用户要的是 fcitx 4 前端，那是一个新的平台宿主，另开决定。
- 与 [发布页 .deb 只推荐 Fcitx5](../bug-fix/2026-10-09-linux-deb-fcitx5-recommends.md) 互补：那篇让 Ubuntu 22.04 能装上发布页的包，没有改构建基线；这篇不动发布页的包，另给 Debian 10 基线一条构建线。

## Verification

- `RBUILD_JOBS=2 rbuild env CMAKE_BUILD_PARALLEL_LEVEL=2 bash platforms/linux/package-legacy-container.sh`（Studio，arm64 原生）：Depends 为 `ibus (>= 1.5.19), python3 (>= 3.7), procps, libasound2 (>= 1.1.0), libc6 (>= 2.28), …`，包内 ELF 文件没有高于 `GLIBC_2.28` 的符号版本；干净的 buster 里 apt 安装成功，`ldd` 无缺失，ctest 70 项通过。
- `RBUILD_JOBS=2 rbuild bash platforms/linux/build-container.sh`：bookworm 门禁照常通过，确认上面的条件编译和链接改动不影响现有构建。
- 没有在 UOS 20 真机或任何图形会话里选中输入法打字。
