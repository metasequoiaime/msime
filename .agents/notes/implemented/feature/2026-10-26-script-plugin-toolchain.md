# Agent Note: 仓库外写脚本插件的工具链：SDK crate msime-script-sdk、msime-pack new / build / pack 和 msime-pack 的发布工作流

Status: implemented

## Problem

桌面脚本插件的作者工具（[作者工具](2026-10-12-script-plugin-author-tools.md)）只在仓库里好用：SDK 是 `docs/script-plugin-template/sdk/` 下要复制走的两份文件，模板要整目录复制再改 `#[path]`，`msime-pack` 要先克隆仓库再 `cargo build -p msime-pack-tool`，打包靠仓库专用的 `plugins/script/tools/assemble.py`。维护者要的是第三方开发者不克隆这个仓库、不了解内部结构，也能写、测、打包脚本插件。

要回答的是：SDK 以什么形式交给作者，仓库里的插件怎样跟着用同一份；作者从零开始的那几步由谁来做；`msime-pack` 怎样交到作者手里；以及这些变化不能让官方包 uuid 的内容哈希变化（授权绑定在它上面，哈希一变所有用户都要重新授权）。

## Decision

**SDK 是独立的 crate `crates/script-sdk`，包名 `msime-script-sdk`。** 内容就是原来的 `msime_script.rs`，安全封装和模块划分不变，改成 crate 根（`#![no_std]`，去掉了单文件时期的 `#![allow(dead_code)]`）；C 头文件 `msime_script.h` 搬到同一个 crate 里，给 C、Zig 作者。没有加导出宏：`msime_abi`、`msime_on_event`、`#[panic_handler]` 就是 ABI 的全部入口，作者照 crate 文档里的最小例子写四个函数，宏省下的几行不值得把 ABI 藏起来（ARCHITECTURE 的「不过度抽象」）。

- 许可证是 MIT，`license` 字段单独写、不从工作区继承（工作区是 GPL-3.0-only）：SDK 要编进每个插件，作者用它写的插件不必因此变成 GPL。README「许可证」一节和 docs/third-party.md 把它列进 MIT 例外。
- 它是 Rust 工作区的成员，但不在 `default-members` 里。crate 只有常量、`unsafe extern "C"` 声明和薄封装，在主机目标上也能编译（clippy 在主机和 `wasm32-unknown-unknown` 上都干净），所以 `cargo clippy --workspace`、`cargo test --workspace` 不会因为它坏掉；`[lib]` 写 `test = false`、`doctest = false`，因为宿主函数只在 wasm 模块被输入法实例化时才有，在主机上链接测试二进制会找不到符号。常量和 import 列表仍由 `crates/pack-tool/tests/sdk.rs` 对照运行时逐项核对，路径改成读 `crates/script-sdk`。
- crates.io 元数据齐全（description、repository、homepage、readme、keywords、categories、`rust-version = "1.82"`，`include` 只带源码、头文件、README 和 LICENSE）。2026-10-26 查过 crates.io，`msime-script-sdk` 没有被占用。发布要维护者另行批准，这次不发布。

**仓库里的插件改用路径依赖。** `docs/script-plugin-template` 和 `plugins/script/{hide-rare-chars,rare-chars-last}` 依赖 `msime-script-sdk = { path = … }`，和发布到 crates.io 的是同一份源码；`plugins/script/README.md` 原来的「插件 crate 没有任何依赖」改为「只依赖 SDK」。这推翻了[脚本插件合集](../architecture/2026-10-12-script-plugin-collection.md)里「没有任何依赖、用 `#[path]` 引入 SDK」那一条，合集的其余决定不变。

**官方包 uuid 例外，仍用 `#[path]` 直接编进 SDK 的源文件。** 实测把 uuid 换成 crate 依赖后模块从 3281 字节变成 3279 字节：LTO 合并的顺序变了，import 段里四个 import 的排列、类型段的顺序和一个函数体跟着变，内容哈希随之改变。`#[path]` 指到 `crates/script-sdk/src/lib.rs`，编出的模块和改动之前逐字节相同（sha256 `57287707…`），`plugins/official-packs.json` 不动。作为私有模块，SDK 里没用到的公开项会报 `dead_code`、crate 根的 `#![no_std]` 会报 `unused_attributes`，`mod` 上用 `#[allow]` 收住，`plugins/script/uuid/src/lib.rs` 开头的注释写明原因和退出条件：下一次哈希本来就要变（升级编译器、改清单或源码）时，改成和别的插件一样依赖 SDK。

**`msime-pack new <id> [--hook candidates|command|before_commit] [--dir <path>]`（只在桌面）生成一个在仓库外独立构建的插件 crate。** 模板在 `crates/pack-tool/templates/`，用 `include_str!` 编进二进制。

- `Cargo.toml` 依赖 `msime-script-sdk = "0.1"`（`scaffold::SDK_REQUIREMENT`，`tests/author.rs` 核对它和 SDK crate 的版本一致），注释里给出改成路径依赖的写法；release 配置和模板一样（`opt-level = "s"`、LTO、`panic = "abort"`、strip）；自带空的 `[workspace]`，生成在别的工作区里面时不会被并进去。
- `.cargo/config.toml` 默认目标 `wasm32-unknown-unknown`、栈 64 KiB；`src/lib.rs` 是所选钩子的最小可用例子；`pack/plugin.toml` 是 schema 2，只申请这个例子导入的能力，`memory_pages = 2`；`pack/LICENSE.txt` 是写明必须替换的占位文字；`fixture.jsonl` 给 `msime-pack run`；README 写编译、试跑、打包、导入四步。
- 三个例子各演示一组能力：`command`（默认，`provide_commands` 加不需要能力的 `random_fill`：只打触发词时掷骰子，后面有字母时给出大写）、`candidates`（`read_candidates` 加 `annotate_candidates`：给单字加注释）、`before_commit`（`read_commit` 加 `rewrite_commit`：`...` 换成 `……`）。默认选 `command`，因为它只要一项能力、在 `/` 指令里看得见结果。
- id 按导入的同一条规则检查（`msime_client_base::text::safe_id`），内置插件的 id（`plugins::is_builtin`）直接拒绝；目标目录存在而且不空时拒绝。`/` 指令的触发词取 id 里的小写字母（最多 16 个），不到 2 个或者和内置指令重名时拒绝并提示换 id 或换钩子；标题取触发词的前 8 个字符。Cargo 包名不能有 `.`、不能以数字开头，包名把 `.` 换成 `-`，以数字开头时加 `plugin-`；模块名仍是 `<id>.wasm`。

**`msime-pack build [<dir>]`（只在桌面）。** 在 crate 目录里运行 `cargo build --release --target wasm32-unknown-unknown --message-format=json-render-diagnostics`：cargo 的进度和错误照常打在标准错误上，编出的 `.wasm` 从 JSON 消息里按 `manifest_path` 和 `crate_types` 取，不猜产物目录，`CARGO_TARGET_DIR` 和配置里的 `target-dir` 都不影响。模块复制成 `pack/<script.module>`（`module` 只能是一个文件名，带路径的拒绝），打印 `built pack/<module>: module N bytes, memory P/M pages`，P 是模块 memory 段的最小页数、M 是清单的 `memory_pages`，不相等时提示该改成多少（设了 `RUSTFLAGS` 时另外提示它让 `.cargo/config.toml` 的栈设置失效），最后按 `validate` 的规则检查 `pack/`。编译前在 crate 目录里问 `rustc --print sysroot`，sysroot 下没有 `lib/rustlib/wasm32-unknown-unknown` 就停下并打印 `rustup target add` 命令：在 crate 目录里问，`rust-toolchain.toml` 和 rustup 的目录覆盖同样算数，自己装的工具链也一样。cargo 用 `PATH` 上的那个，不用 `CARGO` 环境变量：在 `cargo run`、`cargo test` 底下它指向仓库那套工具链里的 cargo，会绕过 rustup 按插件目录选工具链，和问到的 rustc 不是同一套。

**`msime-pack pack [--builtin] [<dir>] [-o <zip>]`。** `<dir>` 是包文件夹（里面直接有 `plugin.toml`）或者 crate（包在 `pack/`）。先按 `validate` 检查文件夹，再把其中的非隐藏文件按名字排序写成 zip：顶层、时间 1980-01-01、权限 0644、不压缩，和 assemble.py 的包是同样的性质，同样的文件得到逐字节相同的压缩包；最后按 `validate` 检查这个压缩包。默认输出是包文件夹旁边的 `<id>-<version>.zip`（crate 的就是 crate 根），输出落在包文件夹里时拒绝，否则下一次打包会把它打进去。不限于脚本插件，任何类型的包都能打。写 zip 用 client-services 读写插件包已经在用的 `zip` 9.0（pack-tool 直接依赖同一个 crate，不开 deflate），`Cargo.lock` 没有新的第三方 crate；pack-tool 另外新增的两条依赖边是工作区里已有的 `toml` 和第一方的 `msime-client-base`。

**`.github/workflows/release-pack-tool.yml`，只能手动触发。** 构建 macOS（arm64 和 x86_64 两份 lipo 成 universal）、Windows x64、Linux x64（`x86_64-unknown-linux-musl` 静态链接，不挑发行版的 glibc），每个平台先跑一次 `msime-pack validate docs/plugin-template` 冒烟，再组装成带 GPL 许可证和工具 README 的压缩包加 `SHA256SUMS`；`publish` 为真时发布为 `pack-tool-vX.Y.Z`。版本号不写时取已有 `pack-tool-v` 的最大版本加一个补丁号（第一次是 0.1.0），按 AGENTS.md「发版」的约定 main 不加后缀、develop 加 `-beta`、其他分支加 `-alpha`，带后缀的一律 prerelease；永远 `--latest=false`，tag 已存在时失败，不替换。`update_check.rs` 只认 `<平台>-v` 前缀，这个 tag 不会被当成任何平台的版本。macOS 的二进制没有 Developer ID 签名和公证，发布说明写明用浏览器下载的要先 `xattr -d com.apple.quarantine`。

**assemble.py 不改。** 仓库里插件的许可证义务检查（`EMBEDDED_DATA`）、三名一致和许可证副本核对留在它那里；它自己写 zip，不调用 `msime-pack pack`。

**作者文档。** docs/plugins.md「脚本插件」开头的「想直接开始」指向新的「从零写一个插件」一节：装 Rust 和 wasm 目标、取得 `msime-pack`（发布页或 `cargo install --git`）、`new`、改代码和权限、`build`、`run`、`pack`、导入并授权。ABI 编号的出处链接改到 SDK crate。pack-tool 的 README 补了安装方式和三个子命令。

## Alternatives considered

- **SDK 继续是复制走的单文件，`msime-pack new` 把它的内容写进生成的 crate。** 最强的理由是没有发布 crate 这一步，也不用为 uuid 的哈希想办法。不用：作者手里的那份不会跟着 ABI 追加的编号更新，漂移只能等插件在输入法里读错字段才发现；crate 依赖让作者 `cargo update` 就拿到新编号，也是 Rust 作者默认会找的形式。
- **在 SDK 里提供一个导出宏，生成 `msime_abi`、`msime_on_event` 和 `#[panic_handler]`。** 最强的理由是每个插件开头那十几行完全一样。不用：这几个导出就是 ABI 本身，藏进宏之后作者看不到模块要求里写的签名，出错时也更难查；省下的行数不多。
- **uuid 也换成 crate 依赖，同时更新 `official-packs.json` 的哈希。** 最强的理由是所有插件写法一致，不留例外。不用：哈希一变，所有已经授权 uuid 的用户在下一个版本里都要重新授权，只为一个看不见的源码整理不值得；按 uuid 注释写的退出条件，等哈希本来就要变时一起改。
- **assemble.py 改成调用 `msime-pack pack` 写 zip，免得两份打包代码。** 最强的理由是确定性 zip 的规则只写一处。不用：`build-script-plugins.sh` 现在只编 wasm，调用 `msime-pack` 就要先在主机上编出整个 pack-tool（连带 client-services 和 wasmi），只为省下 Python 里十行标准库代码；两边的性质（排序、固定时间、不压缩、0644）写在 `src/archive.rs` 的文档里。两边的字节不相同：两个 zip 实现的头部字段写法不同，没有消费方依赖两者相同。
- **`new` 加一个 `--sdk-path` 选项，SDK 发布之前也能生成可用的 crate。** 最强的理由是 crate 发布之前，照文档走的作者第一步就编译不过。不用：多一个只在过渡期有用的选项；生成的 `Cargo.toml` 注释里已经写了路径依赖的写法，测试里也只是把那一行换掉。
- **`build` 用 `CARGO` 环境变量找 cargo。** 最强的理由是在 `cargo run` 底下能用上同一套工具链。不用：理由见 Decision，它会让 cargo 和 `rustc --print sysroot` 问到的不是同一套工具链。
- **Linux 版在 ubuntu-22.04 上用 glibc 编译。** 最强的理由是不用装 musl-gcc，也没有 musl 的分配器性能问题。不用：仍然排除 glibc 更旧的发行版，而 runner 镜像迟早下线；一个命令行工具静态链接最省事，性能差异对它无关紧要。

## Consequences

- 收益：作者装好 Rust 和 `msime-pack` 之后，`new`、`build`、`run`、`pack` 四步就能得到一个能导入的插件包，不需要这个仓库；SDK 跟着 ABI 追加的编号走 crates.io 更新。仓库里的插件和模板用的是和发布出去的同一份 SDK 源码。
- 代价：SDK 发布到 crates.io 之前，生成的 crate 依赖一个还不存在的版本，照文档走的第一次 `build` 会在解析依赖时失败；`msime-pack` 的发布同样要维护者批准后手动触发。两件都在 PR 里写了命令，等批准。
- uuid 是唯一仍用 `#[path]` 的插件。SDK 的源文件改了，uuid 编出的字节可能跟着变；`build-official-plugins.sh --check` 会在 PR 里把这件事变红，和以前一样。
- `crates/script-sdk/*` 加进了 CI「Script plugins」作业的触发路径和 `verify-local.sh` 的不触发平台阶段的路径、clippy 清单。
- `tests/author.rs` 要 `wasm32-unknown-unknown` 目标才能跑完 build、run、pack 那一项：没有时打印 skip，`MSIME_REQUIRE_SCRIPT_PLUGINS=1`（CI 的 Script plugins 作业）时失败。它在临时目录里编三个生成的 crate，每个约 1 秒。
- 发布工作流没有运行过（只过了 actionlint 和 shellcheck），musl 目标的交叉编译、Windows 上的构建和 lipo 都要第一次手动触发时才知道。macOS 二进制没有公证，作者要手动去掉隔离属性；要签名就照 `publish-macos-voice-runtime.yml` 加 Developer ID 和公证步骤。

## Verification

- `rbuild cargo test -p msime-pack-tool --locked`（`TMPDIR` 指到 Workspace 卷，Studio 的系统盘满了）：lib 单测 1 项（memory 段最小页数）、`tests/author.rs` 6 项全部通过，其中 `every_scaffold_builds_validates_runs_and_packs` 在装了 wasm 目标的 1.97.1 上编了三个生成的 crate，`build` 都输出 `memory 2/2 pages` 和 `ok <id> script 0.1.0`，`run` 的输出逐项核对，两次 `pack` 字节相同，文件夹和压缩包的内容哈希相同。
- `rbuild cargo clippy -p msime-pack-tool --all-targets --locked -- -D warnings`、`cargo clippy -p msime-script-sdk --all-targets -- -D warnings`、`cargo clippy -p msime-script-sdk --target wasm32-unknown-unknown -- -D warnings`、`cargo fmt --check`，模板的 `lib.rs` 另外过 `rustfmt --check`。
- 手动：Studio 上 `msime-pack new demo-dice`、`build`、`pack`；`RUSTUP_TOOLCHAIN=stable`（没有 wasm 目标）时 `build` 停在 `rustup target add` 提示、退出 1；`-o pack/x.zip` 被拒绝。
- uuid：改动前后 `bash scripts/build-script-plugins.sh` 编出的 `uuid.wasm` sha256 都是 `57287707ced024ed713aed604b4e7737f51e231ec003529d4f07706adedc54b8`；换成 crate 依赖的那一版是 `23241a0b…`（3279 字节），据此保留 `#[path]`。
- `actionlint`（`SHELLCHECK_OPTS=--severity=warning`）检查 `release-pack-tool.yml` 和改过的 `ci-platforms.yml`，无输出。
- `bash scripts/test-script-plugins.sh`：三个插件和 `expected.jsonl` 一致（uuid 另按 RFC 9562 核对），生成数据那一步因 Studio 没有缓存 Unihan zip 打印 skip；`bash scripts/build-official-plugins.sh --check` 通过，uuid 的内容哈希等于锁定值。
- `bash scripts/verify-local.sh --quick`：Rust 工作区、android 目标、wasm 目标的检查、actionlint 和契约检查通过；`android host java` 和 `macos build` 两项因 Studio 系统盘写满（`No space left on device`）失败，与这次改动无关，没有在别的机器上重跑；Windows、HarmonyOS 和 Linux 容器检查在 Studio 上跳过。

相关：[作者工具](2026-10-12-script-plugin-author-tools.md) 是 `run`、SDK 和模板的出处；[脚本插件合集](../architecture/2026-10-12-script-plugin-collection.md) 的「没有任何依赖」被这里改为「只依赖 SDK」；[官方插件包随桌面安装包分发](../architecture/2026-10-13-official-plugin-packs-in-desktop-installers.md) 是 uuid 哈希锁的出处。
