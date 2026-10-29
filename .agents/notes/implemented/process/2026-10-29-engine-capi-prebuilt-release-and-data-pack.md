# Agent Note: msime-engine-capi 的预编译归档、最小数据包与快速上手

Status: implemented

## Problem

引擎三个 crate（msime-path-trust、msime-engine、msime-engine-capi 的 0.1.0）已经发到 crates.io，但别的输入法要用它，仍得自己做两件事：C 采用方要装 Rust 和 cargo-c、从源码编出库；所有采用方都要去 msime-dictionary 的发布里认出该下哪几个文件、各要带哪些许可证文本。可嵌入引擎库的提案（`implemented/architecture/2026-10-29-embeddable-engine-library.md`）的 P5 要「按方案组装最小数据集，每个文件带许可证」，P6 写了「capi 的二进制、头文件和 `.pc` 走 GitHub Releases」，[C ABI 笔记](../architecture/2026-10-29-engine-capi.md) 把二进制发布留作「没做的」。两个 README 也没有一段从零到打出第一个字的说明。

## Decision

### 发布流程

新 workflow `.github/workflows/release-engine-capi.yml`，照 `release-engine-crates.yml` 和 `release-pack-tool.yml` 的写法：手动触发，带 `publish` 开关；勾了 publish 才建 release，只能在 develop 或 main 上，同一个 tag 已存在时在第一个作业里就失败（发出去的 release 不替换，要修正就改版本号）。版本取 `crates/engine-capi/Cargo.toml`，tag 是 `msime-engine-capi-vX.Y.Z`：应用内更新检查只认 `<平台>-` 前缀（`crates/client-services/src/update_check.rs` 的 `select_platform_release`），这个 tag 不会被当成任何平台的版本；release 一律 `--latest=false`，仓库的「最新发布」留给应用。release 带三个库归档、数据包的 `.tar.gz` 与 `.zip` 和一个 `SHA256SUMS`。

PR 改了这个 workflow 或 `crates/engine-capi/**` 时整条流程照跑（构建、打包、验收、数据包、smoke），产物作为 artifact 上传，不建 release：发布流程本身由 PR 的 CI 证明。它不是必需检查。

### 库归档

三个平台各一个，复用 CI 的 `engine-capi` 作业的做法（sccache、按版本缓存的 cargo-c 0.10.26、Windows 上一律用 bash）：

- **Linux x86_64**：在 `depot-ubuntu-22.04-8` 上构建，glibc 2.35。不用 manylinux 容器：那要在容器里另装 rustup、CMake、Python 和 pkg-config，而 22.04 的 2.35 已经覆盖仍在支持期的主流发行版。打包后用 `objdump -T` 取库要的最高 `GLIBC_` 版本，超过 workflow 顶层的 `GLIBC_CEILING` 就失败，README 和 release 说明写的就是这个值。cargo-c 的缓存键带 runner 标签：24.04 上编出的 cargo-c 在 22.04 上缺 glibc 符号。
- **macOS universal**：Depot 的 macOS runner 都是 Apple 芯片，在 `depot-macos-15` 上按 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 各 `cargo cinstall` 一次，`lipo` 合成一份共享库和一份静态库，头文件、`.pc` 和符号链接两边相同、取 arm64 的。只发一个 universal 归档而不是两个：采用方的应用本身多半是 universal，自己再 lipo 一遍是白费的步骤，体积多出的一倍对一个库无所谓；cargo-c 不能一次编两个架构，但 lipo 只是一条命令。两个目标都设 `MACOSX_DEPLOYMENT_TARGET=11.0`（rustc 和编 bundled SQLite 的 cc 都读它），打包前用 `vtool -show-build` 核对每个架构的 `minos`。用 macOS 15 而不是 CI 的 26：`release-pack-tool.yml` 在同一种 runner 上经 Rosetta 跑 x86_64 程序，验收要两个架构都跑。
- **Windows x86_64 MSVC**：`depot-windows-2025-8`，动态 CRT（Rust 的默认），README 写明运行要 Visual C++ 运行库、静态链接时调用方也用 `/MD`。DLL 在 `bin/`，导入库和静态库在 `lib/`，即 cargo-c 的布局。

打包在 `crates/engine-capi/packaging/package.sh`：拷贝装好的前缀，把 `.pc` 的 prefix 改成 `${pcfiledir}/../..`（pkg-config 和 pkgconf 都认，解到哪里都能用），macOS 上把 install name 从构建机上的绝对路径改成 `@rpath/libmsime_engine.1.dylib` 并重新 ad-hoc 签名；放进 C 示例和 CMakeLists.txt、crate 的 README、MPL-2.0 的 `LICENSE`，`licenses/` 里放引擎的 `LICENSE`、`crates/engine/licenses/` 的全部文件（README「许可证」表里编进引擎的数据与改编代码要的全文）和 `ewts-MIT.txt`；`THIRD_PARTY_NOTICES.txt` 在本平台上用 `platforms/linux/collect-notices.py` 按 `msime-engine-capi` 的依赖图生成，正是 capi README「二进制归档与第三方声明」写的命令；`SOURCE.txt` 写明对应源码的提交和 crates.io 上的版本（MPL-2.0 §3.2），以及 rink-core 的源码位置。Linux、macOS 打 `.tar.gz`（保留 `.so` 的版本符号链接），Windows 打 `.zip`。

**验收的是归档本身。** 打完之后把归档解到另一个目录，跑 `crates/engine-capi/tests/c-example.sh --prefix <解开的目录>`：这个脚本原来只会自己用 `cargo cinstall --debug` 装一份再测，现在 `--prefix` 跳过构建，对给定的前缀做同样的检查（头文件没有漂移、SONAME 或 install name 带 ABI 主版本、`.pc` 的版本是 ABI 版本、第三方声明在且列着 `libsqlite3-sys`、不带许可证文件的 crate 仍是那三个），再用 CMake 按动态、静态（非 Windows 再加链接系统 SQLite 的静态）编译 C 示例并在夹具词库上运行。macOS 上示例按库里的架构编成 universal，每个架构各跑一遍，arm64 runner 上的 x86_64 经 Rosetta；跑不了的架构直接失败，`MSIME_EXAMPLE_ARCHS` 只给没装 Rosetta 的开发机用。解到别的目录才能证明 `.pc` 和 install name 不再依赖构建目录。

### 数据包

`crates/engine-capi/packaging/data-pack.py` 按 `docs/data-formats.md` §20「全拼、双拼、九键」一行取必需加产品同等的文件（`msime-pinyin.db`、`msime-english.db`、`msime-bigram.bin`、`msime-trigram.bin`）和随 `msime-english.db` 必带的 `msime-scowl_Copyright.txt`，按 `resources/desktop-dictionary.lock.json` 下载，复用 `scripts/download_retry.py` 的重试，逐个核对大小和 SHA-256，并要求五个文件来自同一个 `dict-v*` 发布，包名 `msime-engine-data-quanpin-<词库版本>` 里的版本就是它的。包的顶层目录直接就是 `resources`。随附的许可证按数据的条款选：GPL-3.0 全文（仓库根目录的 `LICENSE`）、行政区划名称的 WTFPL、msime-dictionary 的来源与授权说明（`resources/licenses/msime-engine-dictionary-NOTICE.md`）和 SCOWL 声明；与 Linux 包给同一批词库带的声明相同（`platforms/linux/CMakeLists.txt` 的 `MSIME_NOTICE_SOURCES`）。包里的 `README.md` 由 `data-pack-README.md` 填上版本、大小和摘要，开头就写明引擎是 MPL-2.0 而这份数据不是，再逐个文件列出许可证（GPL-3.0 为主，n-gram 表 CC-BY-SA-4.0，拼音库里还有 CC-BY-4.0、MIT、WTFPL 的部分）和分发时要做的事。`msime-others.db` 不进包：§21 记着它的部分来源没有再分发授权。同一份内容另打一个 `.zip` 给 Windows 用户。

smoke 作业照 capi README 的「快速上手」逐字做一遍：在 Ubuntu 22.04 上解开 Linux 归档和数据包，用 pkg-config 和 cc 编归档里的 `examples/c/example.c`，在真实词库上打 `nihaoshijie`、选第一个候选上屏「你好世界」。示例原来最后一定要从个人词典读回这句，但真实词库里本来就有「你好世界」这个词，选它只调词频、不加个人词条，所以示例改成只在选中的是整句联想解出的句子（`MSIME_ENGINE_CANDIDATE_SENTENCE`）时才要求读回；为了不让这条放宽在夹具上悄悄生效，`c-example.sh` 另外核对夹具上的输出里确实有读回的这句。发布作业依赖 smoke。

### 快速上手

`crates/engine/examples/quickstart.rs`：命令行给数据目录（可选工作目录），`prepare_runtime_paths` 准备代次，持 `DictionaryAccess` 共享租约，`SessionOptions::new` 的库默认值建会话，打 `nihao`，打印 `display_snapshot` 的组字和候选，选第一个，打印上屏，结束会话、放租约、`shutdown()`。只用 crate 根上有文档的核心 API，不碰 `host`、`backend`、`ordering`、`web`。代次 id 取数据目录的名字，数据包换版本时目录名跟着换，不会沿用旧数据准备的代次。它不读仓库里的任何文件，所以进了 `crates/engine/Cargo.toml` 的 `include` 白名单随包发布（原来的理由「examples 读仓库里的词库」对它不成立），`scripts/test-engine-license.py` 把它列为必须进包的文件。engine README 的「快速上手」是它的节选，加上 `cargo add msime-engine` 和数据从哪里来；capi README 的「快速上手」给出下载哪几个文件、pkg-config、裸 cc、MSVC 和 CMake 四种编法和示例的输出。

## Alternatives considered

- **macOS 分成 arm64 和 x86_64 两个归档** — 最强的理由是每个归档只有一半大小，下载的人不用多带一个用不上的架构，验收也不依赖 Rosetta。没有采用：macOS 上的应用和输入法多半按 universal 发布，采用方拿到两个归档还得自己 lipo；而合成只是 workflow 里的一条命令，Depot 的 macOS runner 上有 Rosetta，两个架构都能在同一台机器上跑示例。
- **在 manylinux_2_28 容器里构建 Linux 库** — 最强的理由是 glibc 基线更低（2.28），覆盖 Debian 10、RHEL 8 这类老系统。没有采用：容器里要另装 rustup、CMake、Python、pkg-config 和 SQLite 头文件，C 示例的验收也要在容器里跑，与 CI 的 `engine-capi` 作业不再是同一套步骤；Ubuntu 22.04 的 2.35 已覆盖仍在支持期的主流发行版，真有更老的需求时再按 `build-linux-legacy.yml` 的做法加一个 legacy 归档。
- **数据包用 `msime-client-core` 的 `install_resources` 示例下载** — 它是仓库里给这个锁文件用的下载器，核对逻辑与各平台运行时相同。没有采用：它一次装下锁里的全部十二个文件（约 360 MB，含五笔、日文和神经模型），还要先编 reqwest 这一整套依赖；数据包只要五个文件，Python 脚本加 `download_retry.py` 几秒就完，采用方在本机也能直接跑。
- **在 workflow 里内联打包脚本** — `release-pack-tool.yml` 就是这样。没有采用：这里的打包步骤（改 `.pc`、改 install name、收许可证、生成声明和源码说明）三个平台都要跑，内联要写三遍或者写成一段难读的多平台 YAML；放进脚本还能在本机复现。

## Consequences

- **收益**：C 采用方下载一个归档和一个数据包就能编译、链接、打出第一个字，`.pc` 和 install name 不依赖解开的位置；每个归档带齐 MPL-2.0、编进引擎的数据与改编代码、第三方 crate 的声明和源码位置；数据包带齐它的数据许可证要求随附的文本，并在开头说明它不是 MPL-2.0。Rust 采用方 `cargo add msime-engine` 后照 README 的十几行就能跑。归档在上传之前就在三个平台上被解开、编译、运行过，数据包在真实词库上被 C 示例用过。
- **代价**：改 `crates/engine-capi/**` 的 PR 多占三台 runner，其中一台 macOS，release 构建比 CI 的调试构建慢。Linux 归档不支持 glibc 2.35 以前的系统。macOS 的库没有 Developer ID 签名和公证，浏览器下载的要先去掉隔离属性。数据包与 `desktop-dictionary.lock.json` 绑在一起：锁换到新的 `dict-v*`，下一次发布的数据包跟着换，锁里五个文件不来自同一个发布时打包失败。
- **没做的**：MSVC 的 `cl` 命令行只在 README 里，CI 在 Windows 上走的是 CMake；Linux arm64 和 Windows arm64 没有归档；数据包只有全拼、双拼一种，五笔、日文等方案的文件仍要从 `dict-v*` 发布自取。数据包的 `licenses/` 带拼音库单字读音所据 pinyin-data 和英文词库中英释义所据 ECDICT 的 MIT 全文；Windows-classic-samples 的 SampleIME 词表不进任何词库，不在其列。各平台安装包怎样带上词库里的 MIT 与 Apache-2.0 全文见 [`2026-10-29-dictionary-data-permissive-licence-texts.md`](2026-10-29-dictionary-data-permissive-licence-texts.md)。

## 相关笔记

按 engine-capi、release、data pack、数据包、预编译检索活跃笔记：[引擎的 C ABI](../architecture/2026-10-29-engine-capi.md) 是本篇打包的对象，它「没做的」里的二进制发布由本篇回答，部分重叠，互链；可嵌入引擎库的提案（`../../implemented/architecture/2026-10-29-embeddable-engine-library.md`）的 P5、P6 里的数据集与 Releases 两项由本篇落地，部分重叠；[Depot runner 默认](2026-10-27-depot-runners-by-default.md) 是 runner 选择的依据，部分重叠。没有被本篇吸收或取代的笔记。
