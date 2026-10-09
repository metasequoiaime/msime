# Agent Note: Linux 包把 IBus 和 Fcitx5 写成二选一依赖，不拆包

Status: implemented

## Problem

#6401：在 deepin 25 这类已经在用 Fcitx5 的系统上装 `.deb`，apt 会一并装上 IBus 和它的 GTK 模块等 11 个包，安装后多占约 68 MB。用户只想用系统默认的 Fcitx5。

各套打包的依赖是这样写的：

- 已发布的 0.11.0 把 `ibus (>= 1.5.20)` 和 `fcitx5 (>= 5.0.20)` 都写成了 Depends。
- develop 已经把 Fcitx5 降为 Recommends（#6305），IBus 仍是硬依赖。
- rpm 和 Arch 的 PKGBUILD 也都硬依赖两者，或者硬依赖 IBus。

issue 下有人提议把包拆成 IBus 和 Fcitx5 两个版本。为此拆开 0.11.0 的 `.deb` 看了两个前端各自专属的文件：

- IBus：`msime-linux-ibus` 加启动脚本和组件 xml，约 1.0 MB；
- Fcitx5：`libmsime-pinyin-fcitx5.so` 加两个 conf，约 1.3 MB；
- 共用部分：设置窗口、onnxruntime、Host API、MCP、离线释义和音效等，占了安装后 121 MB 里的其余全部。

所以拆包每个只省 1 MB 左右。多出来的那一百多 MB 来自依赖声明，不是包的内容。

## Decision

不拆包，把输入法框架写成二选一依赖：

- **`.deb`（`cmake/packaging.cmake`、PPA 用的 `debian/control`）**
  - 写成 `ibus (>= 1.5.20) | fcitx5 (>= 5.0.20)`。系统里已有哪个就算满足哪个；两个都没有时，apt 装写在前面的 IBus。
  - Fcitx5 从 Recommends 去掉：apt 默认安装推荐包，留着它会把 Fcitx5 拉进 IBus 系统。
  - Ubuntu 22.04 只有 Fcitx5 5.0.14，在那里由 IBus 满足这一项（#6305 的约束不变）。
  - 不带 Fcitx5 插件的构建（`MSIME_ENABLE_FCITX5=OFF`）仍只依赖 IBus。
- **`.rpm`（CPack 与 `rpm/msime.spec`）**
  - 写成 `(ibus >= 1.5.20 or fcitx5 >= 5.0.20)`。
  - 插件链接的 `libFcitx5*` 加进 `__requires_exclude`，这和 `.deb` 去掉 `libfcitx5*` 是同一个道理：插件只由 Fcitx5 加载，那时这些库一定在，自动生成的 Requires 只会把 Fcitx5 拉进 IBus 系统。
- **Arch（`msime`、`msime-bin`）**
  - `ibus`、`fcitx5` 从 depends 挪到 optdepends。
  - `fcitx5` 进 makedepends，因为源码包要编插件，二进制包要在打包时用 ldd 核对插件的库。
  - 源码包的 checkdepends 加 `ibus`，ctest 里有要起真 ibus-daemon 的用例。
- **核对**
  - `package-container.sh` 重新打包后，只核对 Depends 里不再有 `libfcitx5*`。原来它查的是「fcitx5」整个字样，现在二选一里本来就有这几个字。
  - `release-linux.yml` 逐包核对：Depends 只以这个二选一提到两个框架，Recommends 里没有 fcitx5。
  - `fcitx5_contract.py` 改为断言同一套规则。

辅助程序（`msime-linux-dictionary`、`-prepare`、`-online` 等）都链接 `msime-ibus-host`，进而链接 libibus。所以 `dpkg-shlibdeps` 仍会带上 `libibus-1.0-5`，rpm 也会要 IBus 的库。那只是一个几百 KB 的库，不会启动 IBus 守护进程，也不会抢输入法。

`msime-linux-setup --register` 不用改：系统里没有 `ibus` 时，`command_output` 把 OSError 变成 `RegistrationFailed`，打印手动步骤，退出码仍为 0；两个宿主都在跑时，它本来就以 Fcitx5 为准。

Gentoo 的 ebuild 不变：它没有单独的 libibus 包，`app-i18n/ibus` 连同守护进程都得装，辅助程序才链接得上；Fcitx5 本来就由 USE 开关决定。

## Alternatives considered

- **拆成 msime-ibus 和 msime-fcitx5 两个包，共用部分放进 msime-common。** 最强的理由是每个包只带自己框架的东西，概念上最清楚，rime 就是这样分的。不用它，是因为两个前端专属的文件加起来才 2 MB 多，拆包几乎省不了空间。代价却实实在在：deb、rpm、Arch、OBS、PPA、Nix 各自要改成多包，发布页多一倍安装包，用户还得先知道自己用的是哪个框架。
- **Fcitx5 留在 Recommends，只把 IBus 改成 Recommends。** 改动最小。不用它，是因为没有任何一个框架是硬依赖时，两个都没有的系统装上后用不了；而且 apt 默认装推荐包，IBus 系统会被拉进 Fcitx5，Fcitx5 系统也会被拉进 IBus。
- **把辅助程序从 libibus 上剥离，彻底不碰 IBus。** 能让 Fcitx5 用户连 libibus 也不装。不用它，是因为这要拆 `msime-ibus-host`，范围大，而 libibus 只是一个不会启动任何东西的库，不是 #6401 抱怨的问题。以后有必要再做。

## Consequences

- **收益**：
  - Fcitx5 系统装 `.deb` 或 `.rpm` 不再被拉进 IBus 守护进程和它的 GTK 模块，IBus 系统也不再被拉进 Fcitx5。
  - Arch 上两个框架都只是可选依赖。
- **代价**：
  - 两个框架都没有的系统，apt 和 dnf 会装 IBus，而不一定是用户想要的那一个。
  - Fcitx5 系统仍会装上 libibus 这个库。
  - Arch 上两个框架都不装也能装上本包，只是用不了。
- **验证**：
  - `fcitx5_contract.py`、`scripts/test-arch-gentoo-packaging.py`、`test-linux-distro-packaging.py`、`test-linux-editions.py` 通过。
  - `release-linux.yml` 的新核对拿五组 Depends 和 Recommends 跑过：新写法通过；0.11.0 和 develop 的旧写法、残留 `libfcitx5*`、Recommends 里有 fcitx5 都被拒绝。
  - 没有实际构建 `.deb`、`.rpm` 或 Arch 包，也没有在 deepin 上安装验证，这要等下一次发布构建和用户回报。
