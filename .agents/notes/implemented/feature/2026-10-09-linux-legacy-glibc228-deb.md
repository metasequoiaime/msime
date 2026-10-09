# Agent Note: Debian 10 基线的 legacy .deb：只含 IBus，在 buster 容器里构建

Status: implemented

## Problem

#6311：UOS 20 专业版（Debian 10.10 基线，glibc 2.28、GCC 8.3、Python 3.7、IBus 1.5.19 可装、默认框架 fcitx 4.2.9）装不上发布页的 `msime-linux_<版本>_<架构>.deb`。发布页的包在 `rust:1.97.1-bookworm` 里构建，`dpkg-shlibdeps` 按 bookworm 的库算出 `libc6 (>= 2.35)` 一类依赖，二进制本身也引用 `GLIBC_2.34` 起的符号；包里还有要 WebKitGTK 4.1、libsoup 3 的 Tauri 设置窗口和要 Fcitx5 5.0.20 的插件，这些 Debian 10 都没有。另外 `CMakeLists.txt` 写死 `ibus-1.0>=1.5.20`，`packaging.cmake` 写死 `ibus (>= 1.5.20), python3 (>= 3.9)`，随包脚本用了 Python 3.8 起的语法。

用户已定的范围：第一步只做「只有 IBus、没有设置窗口」的 `.deb`，amd64 与 arm64，不改发布工作流，不写 fcitx4 前端。发布工作流随后接上了它，见 [legacy .deb 随 Release Linux 发布](../../implemented/process/2026-10-10-linux-legacy-deb-release.md)。

## Decision

- 新增构建镜像 `platforms/linux/tests/tools/Dockerfile.legacy`：`debian:buster` 按清单摘要固定，软件源改指 `archive.debian.org` 并关闭 Release 有效期检查；开发包与 `Dockerfile.build-gate` 相同，只去掉 Fcitx5；buster 的 CMake 3.13 与 nlohmann-json 3.5 太旧，换成按 SHA-256 固定的 Kitware CMake 3.25.1 预编译包与 nlohmann-json 3.11.2 源码（版本与 bookworm 门禁相同）；Rust 用 `Dockerfile.package-rpm` 同一个固定的 rustup-init 装 1.97.1。
- 新增 `platforms/linux/package-legacy-container.sh`，与 `build-container.sh`、`package-container.sh` 并列。第 1 步在上面的镜像里以 Release 编译 Host API 库和 `msime-mcp`、收集 Rust 许可证，用 `-DMSIME_ENABLE_FCITX5=OFF -DMSIME_IBUS_MIN_VERSION=1.5.19 -DMSIME_PYTHON_MIN_VERSION=3.7` 配置 full 版本，不传 `MSIME_DESKTOP_BINARY`，CPack 只出 DEB；随后由 `tests/tools/check-elf-symbol-versions.sh` 解开包、逐个 ELF 文件读 `readelf -V`，`GLIBC_`、`GLIBCXX_`、`CXXABI_`、`GCC_` 任何一类要求高于 buster 镜像里 libc、libstdc++、libgcc 自己定义的最高版本（2.28、3.4.25、1.3.11、7.0.0）就失败，Depends 里出现 WebKitGTK、libsoup 3、GTK 3 或 Fcitx5 也失败；最后用 `msime-client-core` 的 `install_resources` 示例（Windows 发布工作流取词库的同一个）按 `desktop-dictionary.lock.json` 取回词库到 `target/linux-package-legacy-<架构>/resources-cache`。第 2 步在只有基础系统的 `debian:buster` 里 `apt-get install --no-install-recommends ./<deb>`，`ldd` 包内每个 ELF 文件不得有 `not found`（缺库和缺符号版本两种报错都算），再用从构建镜像拷出的 CMake 跑第 1 步构建树的 ctest，然后由 `tests/tools/legacy-runtime.sh` 做运行时验收：Python 3.7 上跑 `tests/tools/python-contracts.list` 里的合约测试，`ibus-engine-smoke <词库> --page-number`，已安装的 `/usr/bin/msime-linux-ibus` 在 buster 的 ibus-daemon 1.5.19 下跑 `daemon_smoke.py` 和 Xvfb 上的 `gtk_smoke.py`。`MSIME_LEGACY_ARCH=amd64|arm64` 经 `docker --platform` 选架构，build 和 run 都显式传，因为同一个清单摘要在本机已有另一架构的镜像时，不传会静默用上那一个。sherpa-onnx 运行库在宿主上用 `scripts/fetch_voice_runtime.py --platform` 取，该脚本要 Python 3.8，又不随包发布。
- 包名仍是 `msime-linux`，安装路径与发布页的包相同。
- `CMakeLists.txt` 新增缓存变量 `MSIME_IBUS_MIN_VERSION`（缺省 1.5.20）和 `MSIME_PYTHON_MIN_VERSION`（缺省 3.9），同时用于 `pkg_check_modules` 和 `packaging.cmake` 的 `.deb` Depends、RPM Requires。发布页的包不传，声明不变。

### 让同一份源码在 Debian 10 上构建所改的地方

每一处都按能力或版本条件生效，bookworm 上构建出的东西不变：

- IBus：对照 1.5.19 的头文件逐个检查宿主与测试用到的 `ibus_*`/`IBUS_*` 符号，只缺 `IBUS_INPUT_HINT_PRIVATE`（1.5.26）。`src/core/ClientEngine.h` 在 `!IBUS_CHECK_VERSION(1, 5, 26)` 时按协议的固定值 `1u << 11` 补上这个宏。1.5.27 的 `focus_in_id`、`has-focus-id` 原本就有条件编译；旧版 IBus 不报告客户端身份，`ime_mode_scope = app` 退化成所有窗口共用一个状态，这是 `ClientInputModeMemory.h` 已经写明的行为。
- GCC 8 的 `std::filesystem` 在 `libstdc++fs`：GNU 编译器低于 9 时在 `CMAKE_CXX_STANDARD_LIBRARIES` 末尾加 `-lstdc++fs`。
- glibc 2.34 以前 `shm_open` 在 librt、`pthread_create` 在 libpthread：Wayland 浮层在 `check_symbol_exists(shm_open)` 失败时链接 `rt`；三个直接用 `std::thread` 却没链接 `Threads::Threads` 的目标（`msime-linux-online-provider-contract`、`linux-typing-statistics-test`、`ibus-engine-smoke`）补上它。
- libwayland 1.16 的 `wl_pointer_listener` 没有 `axis_value120`：赋值按生成头文件里的 `WL_POINTER_AXIS_VALUE120_SINCE_VERSION` 条件编译。座位绑定的版本上限是 5，这个第 8 版的事件本来就不会送来。
- Python 3.7：随包的 `msime-linux-setup`、`msime-linux-voice-provider` 去掉海象运算符和 `Path.unlink(missing_ok=)`；`msime_voice_doubao.py` 把 `importlib.metadata` 的导入挪进原有的 `try`，3.7 上与缺少 websockets 一样报告依赖不满足（同一文件里 `gzip.compress(mtime=)` 也要 3.8，但依赖检查先失败，走不到）。ctest 里三个 Python 测试的 `unlink(missing_ok=)` 同样改掉。
- 运行时验收要的测试改动：`daemon_smoke.sh` 在 `ibus version` 低于 1.5.20 时用 `unix:tmpdir=` 起守护进程（1.5.19 的 `bus/server.c` 只接受这种 `--address`，1.5.20 才加上 `unix:path=`），地址文件写在测试自己的目录，用 `ibus address` 读回，1.5.20 起的分支与原来逐行相同。合约测试里 `provider_config_discovery.py` 对已 `communicate` 过的进程不再调第二次（3.7 上 stderr 已关闭会抛 `ValueError`，bpo-35182 在 3.8 修掉），`ai_service_contract.py` 的字典 `|` 合并改成 `{**a, **b}`；测试大量用到 3.8 才有的 mock 调用记录 `.args`/`.kwargs`，不逐个改测试，`legacy-runtime.sh` 在测试进程里给 `unittest.mock._Call` 补上这两个只读属性再 `runpy` 运行测试文件。合约清单从 `in-container.sh` 抽到 `python-contracts.list`，两处共用；legacy 跳过 `panel_keymap.py`（核对 Tauri 设置窗口的按键表，要 rustc）和 `doubao_auth.py`（靠 `importlib.metadata` 伪造 websockets 版本），后者换成一条断言：3.7 上 `websocket_dependency()` 报告依赖不满足。

## Alternatives considered

- **在 bookworm 门禁镜像里用 cargo-zigbuild 指定 `<triple>.2.28` 编译 Rust**：好处是沿用现有镜像和新编译器，分诊里列过这条路。没有采用：要新增 zig 和 cargo-zigbuild 两个工具依赖；而且只解决 Rust，C++ 宿主仍要对着 glibc 2.28、libstdc++ 8 和 IBus 1.5.19 的头文件与库编译，等于还要在 bookworm 里另备一个 buster sysroot。在 buster 容器里原生编译，两边一次解决，也不新增工具。
- **把发布页包的下限也降到 IBus 1.5.19、Python 3.7**：源码现在确实能对着 1.5.19 编译。没有采用：发布页的包在 bookworm 上构建，`dpkg-shlibdeps` 给出的库依赖本来就高于这两个下限，降低手写的那两项对能装在哪里没有影响，只会让声明与实际测过的环境脱节。所以做成缓存变量，缺省值保持原样。
- **另起包名（例如 `msime-linux-legacy`）**：可以和发布页的包一起放在同一个发布页里而不撞名。没有采用：两者装同一套 `/usr` 路径，另起包名就必须写 `Conflicts`/`Replaces`，还要让 `msime-linux-setup`、维护脚本和 IBus 组件按新包名改写；同一个包名下 apt 把两者当作同一个包的不同构建处理，升级和卸载的行为与发布页的包一致。用户已经决定不改发布工作流，撞名问题眼下不存在。
- **legacy 包也打其他版本（五笔、日文等）**：配置这些版本要运行 `scripts/edition_linux.py` 改写脚本，它用到 Python 3.10 的 `Path.write_text(newline=)`。为了第一步只打 full 去改这个构建工具，收益不够，脚本只构建 full 并在用法里写明。
- **把 `scripts/fetch_voice_runtime.py` 改成 3.7 能跑，留在容器里执行**：与 `package-container.sh` 的做法一致。没有采用：它是构建工具，不随包发布，改它只为了在 buster 里运行；在宿主上按 `--platform` 取同样可靠，宿主本来就要有 python3。
- **配置时传 `-DMSIME_ENGINE_RESOURCES`，让 ctest 自己登记 `ibus-page-number-visibility`**：评审建议过这条路。没有采用：这个变量同时决定把词库装进包里（约 170 MB），发布页的包不带词库、由用户 `msime-linux-setup --download` 取回，legacy 包应当一样。所以词库只放在构建目录的缓存里，由 `legacy-runtime.sh` 直接调用构建树里的 `ibus-engine-smoke`。
- **跑不带参数的 `ibus-engine-smoke` 完整流程**：它现在在 bookworm 与 IBus 1.5.27 上同样失败（先是中英文提示定时器对空 view 调 `value()` 抛异常，这一处已修；修好后停在「English-mode recording did not show streaming preedit」），与 IBus 版本无关，所以 legacy 验收只跑 ctest 登记的 `--page-number`，由真实守护进程上的 `daemon_smoke.py` 与 `gtk_smoke.py` 覆盖打字主流程。
- **用 buster-backports 的 CMake**：buster-backports 只有 3.18，`CMakeLists.txt` 和 `cmake/Edition.cmake` 用的 `string(JSON)` 要 3.19，最低版本又写的是 3.25。

## Consequences

- **收益**：Debian 10 基线的 amd64、arm64 系统有了一个能用 apt 装上的包，IBus 宿主、provider 程序、`msime-linux-setup` 和 `msime` 命令都可用，本地语音识别运行库也在 glibc 2.28 上能加载。构建脚本自己核对符号版本上限、在干净的 buster 里安装并跑 ctest，再让已安装的宿主在 IBus 1.5.19 的守护进程下打字，不依赖人工检查。
- **代价**：没有设置窗口，词库管理、皮肤、账号、手写模型下载、检查更新都没有入口，偏好只能经 IBus 菜单和 `msime config set` 修改；豆包实时识别要 Python 3.9，不可用；buster 的 systemd 241 不支持 `systemctl --user -M`，升级和卸载时维护脚本只打印每个用户要执行的命令。
- **维护负担与重访信号**：这条构建线不在 `verify-local.sh` 里；`build-linux-legacy.yml` 在发版时、每周一在 develop 上、以及改动它自己文件的 Pull Request 上跑它（见 [legacy .deb 随 Release Linux 发布](../../implemented/process/2026-10-10-linux-legacy-deb-release.md)），所以主线用到 Python 3.8+ 语法、IBus 1.5.20+ 接口、glibc 2.29+ 函数或 GCC 9+ 才支持的 C++17 特性时，最晚在下一个周一或下一次发版时报出，而不是合并当天。若要更早发现，可以给门禁加一道 Python 3.7 语法检查；若 UOS/麒麟用户要的是 fcitx 4 前端，那是一个新的平台宿主，另开决定。
- 与 [发布页 .deb 只推荐 Fcitx5](../bug-fix/2026-10-09-linux-deb-fcitx5-recommends.md) 互补：那篇让 Ubuntu 22.04 能装上发布页的包，没有改构建基线；这篇不动发布页的包，另给 Debian 10 基线一条构建线。

## Verification

- `RBUILD_JOBS=2 rbuild env CMAKE_BUILD_PARALLEL_LEVEL=2 bash platforms/linux/package-legacy-container.sh`（Studio，arm64 原生，基于 rebase 后的 develop）：Depends 为 `ibus (>= 1.5.19), python3 (>= 3.7), procps, libasound2 (>= 1.1.0), libc6 (>= 2.28), libgcc1 (>= 1:4.2), …`；读出的上限是 `GLIBC_2.28 GLIBCXX_3.4.25 CXXABI_1.3.11 GCC_7.0.0`，包内没有 ELF 文件超出；干净的 buster 里 apt 安装成功，`ldd` 无缺库、无缺符号版本，ctest 71 项全部通过；运行时验收：16 个合约测试文件在 Python 3.7.3 上通过，`ibus-engine-smoke --page-number` 通过，ibus-daemon 1.5.19-4+deb10u1 下 `daemon_smoke.py`（打字「你好」、切到 `xkb:us::eng` 再切回、SIGSEGV 与 SIGKILL 后由监护进程重启并继续打字、维护停止、`ibus exit`）与 `gtk_smoke.py`（GTK 3 X11 输入法模块的候选、编辑、布局、焦点、密码框与周边文本）全部通过。
- `RBUILD_JOBS=2 rbuild bash platforms/linux/build-container.sh`：bookworm 门禁 ctest 73 项全部通过，五笔与日文版的配置检查照常；bookworm 的 CMake 检测到 libc 自带 `shm_open`，不链接 librt；bookworm 的 `wayland-client-protocol.h` 定义了 `WL_POINTER_AXIS_VALUE120_SINCE_VERSION`，`axis_value120` 照旧赋值。
- amd64 只部分验证：`MSIME_LEGACY_ARCH=amd64` 在 Studio（arm64）上经 Rosetta 构建出了镜像（Kitware CMake、rustup-init 的 x86_64 校验和与 GCC 8.3、IBus 1.5.19、Python 3.7.3 都对）；按锁取回的 x64 sherpa-onnx 与 ONNX Runtime 库用 `llvm-readelf -V` 核对，最高要求 `GLIBC_2.17`、`GLIBCXX_3.4.19`、`CXXABI_1.3.7`、`GCC_4.0.0`，都不超过 buster；但模拟下 Rust Release 构建约 70 分钟只编完 337 个 crate 中的 130 个（当时负载 200 到 340），整条构建估计要四五个小时，就停掉了，amd64 的 .deb 没有产出，也没有跑安装、ctest 和运行时验收。
- 没有在 UOS 20 真机或真实桌面会话里选中输入法打字；Qt 程序和 Wayland 会话没有在 IBus 1.5.19 上测过。
