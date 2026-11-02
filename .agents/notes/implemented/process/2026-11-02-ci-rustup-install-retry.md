# Agent Note: nightly 与发版工作流的 rustup 安装重试

Status: implemented

## Problem

2026-11-01 的 Nightly builds（run `41353700583`）只因 Windows nightly / official-plugins 一个 job 变红：这个 job 跑在 `depot-ubuntu-24.04-4` 上，`rustup toolchain install` 取 `https://static.rust-lang.org/dist/channel-rust-1.97.1.toml.sha256` 时遇到 `dns error: failed to lookup address information: Temporary failure in name resolution`，立刻以退出码 1 结束，ci-red 单 #10608 因此一直开着。日志里 `syncing channel updates` 之后紧接着就是这条错误，没有任何重试，说明取 channel 清单这一步不受 rustup 自己的下载重试（`RUSTUP_MAX_RETRIES`）保护。

nightly 调用的六个发版工作流（`release-android`、`release-ios`、`release-macos`、`release-linux`、`release-harmony`、`release-windows`）和 `android-device.yml` 里，一共十处都是裸调 `rustup toolchain install` 或 `rustup target add`。`rustup target add` 在仓库根目录运行时，会先安装 `rust-toolchain.toml` 钉住的工具链，同样要取 channel 清单。任何一处碰到网络抖动，整个 nightly 或一次正式发版就失败。

## Decision

新增 `scripts/rustup-install.sh [目标三元组...]`：先回到仓库根目录，再执行 `rustup toolchain install`；给了目标时接着执行 `rustup target add <目标...>`。任一步失败，就把两步一起重来，重试前分别等 5、15、45 秒，一共四次尝试，四次都失败才以退出码 1 结束。工具链已经装好时，`rustup toolchain install` 只做一次本地检查，所以连同它一起重来没有额外代价。

上面十处中的九处 bash 调用都改成这个脚本。`release-windows.yml` 的「Build the Rust half」是 pwsh 步骤，就地写同样的等待序列（5、15、45 秒，共四次）。这样就不用管 Windows runner 上 `bash` 在 PATH 里会解析到哪个程序，所以不经过这个脚本。

只改 nightly 与发版这条路径；`ci-*.yml`、`codeql.yml` 和其余发版工作流里的 rustup 调用不动。

## Alternatives considered

- **设置 `RUSTUP_MAX_RETRIES`**：零代码，是 rustup 自带的旋钮；但这次失败时默认的重试一次也没发生，调大次数管不到 channel 清单这一步，所以不采用。
- **把仓库里全部约三十处 rustup 调用都换掉**：覆盖最全；但 PR 上的 CI 失败可以单独重跑一个 job，不会挂 ci-red 单，也不会拖住发版，改动面却要翻倍，跨十几个工作流。先只覆盖失败会挡住 nightly 和发版的这条路径；PR CI 上若也反复出现同类失败，再按同一个脚本扩展。
- **只修出事的那一个 job**：改动最小；但 nightly 的另外九处遇到同样的 DNS 抖动也会照样失败，下一次 nightly 红在别的平台上，还要再来一遍。不采用。

## Consequences

- nightly 和发版不再因为一两分钟内的解析或连接抖动失败；真正的配置错误（例如目标名写错）要多等约 65 秒才报出来，最后的报错信息会写明是在四次尝试后放弃的。
- `scripts/test-rustup-install.py` 用合成的 `rustup` 和 `sleep` 覆盖五种情况：一次成功、不带目标、工具链安装失败两次后成功、目标安装失败一次后成功、四次都失败放弃。同时核对等待序列、调用参数，以及脚本在仓库外被调用时仍回到仓库根目录。这个测试由 `scripts/run-checks.sh` 自动发现并执行。
- `actionlint` 检查改过的七个工作流、`shellcheck` 检查新脚本，都通过；pwsh 里的重试循环在 win-runner 的 pwsh 7.5 上用合成的 `rustup` 跑过：前两次失败、第三次成功，依次等了 5、15 秒。真实网络下的效果要看之后的 nightly。
- 同类做法见 [Android nightly 设备套件失败](../bug-fix/2026-10-28-android-nightly-device-failures.md) 里给 NDK 安装加的有限重试。
