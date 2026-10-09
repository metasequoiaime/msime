# Agent Note: 发布页 .deb 只推荐 Fcitx5，让 Ubuntu 22.04 能装上并走 IBus

Status: implemented

## Problem

#6305：Ubuntu 22.04 上 `apt install ./msime-linux_<版本>_<架构>.deb` 失败，因为 Depends 里有 `fcitx5 (>= 5.0.20)`，22.04 只有 5.0.14。

0.11.0 的 `.deb` 里和 Fcitx5 有关的依赖有两处来源：

- `cmake/packaging.cmake` 在 `MSIME_ENABLE_FCITX5` 打开时手写进 Depends 的 `fcitx5 (>= 5.0.20)`；
- CPack 对包内每个 ELF 跑 `dpkg-shlibdeps`，从 Fcitx5 插件 `libmsime-fcitx5.so` 推出 `libfcitx5config6`、`libfcitx5core7`、`libfcitx5utils2`（均 `>= 5.0.21`，bookworm 的 shlibs 版本）。

把这个 `.deb` 的 Depends 逐项对照 22.04（jammy-updates）后发现，除了这四项都满足：glibc 2.35、libstdc++ 12、WebKitGTK 4.1 2.50、libsoup 3、IBus 1.5.26、Python 3.10。也就是说，22.04 上装不了是因为一个可选的前端，而 IBus 宿主在那里本来是能用的。

## Decision

- `packaging.cmake` 把 `fcitx5 (>= 5.0.20)` 从 `CPACK_DEBIAN_PACKAGE_DEPENDS` 移到 `CPACK_DEBIAN_PACKAGE_RECOMMENDS`。
- `package-container.sh` 在已有的 xz -9 重新打包循环里，用 `sed` 从 `DEBIAN/control` 的 Depends 去掉 `, libfcitx5*`，之后 Depends 里若仍有 `fcitx5` 就失败。CPack 没有按文件跳过 `dpkg-shlibdeps` 的设置，这是唯一不多一次解包重打的位置。
- `release-linux.yml` 的「Verify Linux packages」对每个版本的 `.deb` 断言：Depends 不含 `fcitx5`，Recommends 含 `fcitx5 (>= 5.0.20)`。
- `tests/core/fcitx5_contract.py` 静态钉住上面两处实现，以及插件条目 `fcitx5/msime.conf` 的 `[Addon/Dependencies] 0=core:5.0.20`。

## 为什么插件可以没有硬依赖地留在包里

插件条目声明了 `core:5.0.20` 依赖。在 Ubuntu 22.04 容器里装上系统的 Fcitx5 5.0.14 再装这个包，fcitx5 日志是 `Call loadAddon() with msime checkDependencies() returns 3`：在依赖检查这一步就拒绝了，不会 dlopen 这个链接了 5.0.21 符号的库，fcitx5 本身照常运行。没有装 Fcitx5 时，插件只是磁盘上一个没人加载的文件。所以这条条目依赖是「只推荐」能成立的前提，契约测试一并钉住了它。

apt 默认安装推荐包。Debian 12 上安装改写后的包，fcitx5 5.0.21 和 libfcitx5core7 照旧被一起装上，行为与以前相同；在 22.04 上，版本满足不了的推荐被 apt 跳过，不会报错。

## Alternatives considered

- **把 Fcitx5 插件拆成单独的二进制包 `msime-linux-fcitx5`**：这是 Debian 惯用的做法，`dpkg-shlibdeps` 按包分别计算，插件包的依赖自然准确，也不需要事后改 control。没有采用，是因为发布页的附件会从每个版本一个 `.deb` 变成两个（6 个版本 × 2 个架构）；设置页的更新检查按扩展名和包名挑附件，版本共存检查按每版一个 `.deb` 写；而且 Debian 12、Ubuntu 24.04 上用 Fcitx5 的大多数用户，从此要手动装两个文件才能得到今天一个文件的效果。拆包修好的是 22.04 这个少数场景，代价却落在多数场景上。
- **让插件兼容 Fcitx5 5.0.14，22.04 上也能用 Fcitx5**：对 22.04 用户最好。但插件依赖 5.0.20 才有的按绝对路径加载 Library（`fcitx5/CMakeLists.txt` 里有说明），所用 API 只在 5.0.21 上编译过，还要多一个 jammy 构建容器、多出一套包。这样的范围已经超出了「让包能装上」，等有人真需要 22.04 上的 Fcitx5 再单独做。
- **改到 ubuntu:22.04 容器里构建 `.deb`**：Depends 会跟着 jammy 的库版本走，但插件要对着 5.0.14 的头文件编译，问题就变成了上一条；而且基线变了，Debian 12 上的依赖也要重新验证。
- **在 CMake 里处理 shlibdeps（`CPACK_POST_BUILD_SCRIPTS` 改 control，或把 `SHLIBDEPS_EXECUTABLE` 换成加 `-x` 的包装脚本）**：好处是手工跑 `cpack` 也能得到同样的 Depends。前者每个 `.deb` 要多解包重打一次，而 `package-container.sh` 本来就要重打；后者要改 CPack 内部用的缓存变量，CPack 一升级就可能失效。

## Consequences

收益：发布页的 `.deb` 在 Ubuntu 22.04 上能用 apt 装上，水杉走 IBus。Debian 12 及更新的系统行为不变。

代价：

- 绕开 `package-container.sh` 手工跑 `cpack` 打出的 `.deb`，Depends 里仍有 `libfcitx5*`。这样的依赖更严格，但不会装出坏的包；发布只走 `package-container.sh`。
- 用 `--no-install-recommends` 安装的 Debian 12 用户不会再被自动装上 Fcitx5，只能用 IBus，除非自己装 Fcitx5。
- 22.04 上只有 IBus 可用。Debian 11、UOS 20 这类 glibc 更旧、没有 WebKitGTK 4.1 的系统仍然装不上（#6311），这次没有改变构建基线。

验证：

- 在 bookworm 容器里，对已发布的 `msime-linux_0.11.0_arm64.deb` 先套上 `packaging.cmake` 改动的等价修改，再逐字执行 `package-container.sh` 里的改写和检查。之后 Depends 里没有任何 Fcitx5 项，Recommends 末尾是 `fcitx5 (>= 5.0.20)`。
- 在 ubuntu:22.04（arm64）里，未装 Fcitx5 时 `apt-get install ./sim.deb` 成功，`/usr/bin` 下全部 `msime-*` 可执行文件（含 `msime-linux-ibus`、`msime-linux-desktop`）`ldd` 都能解析，只有插件缺 `libFcitx5*`。先装系统的 Fcitx5 5.0.14 再装这个包也成功，fcitx5 按上面的日志拒绝加载插件，进程一直运行到超时退出（124），没有崩溃。
- debian:bookworm 里安装改写后的包，fcitx5 5.0.21 作为推荐包被装上。
- 新的契约断言在改动前的实现上失败，在改动后的实现上通过。

尚未验证：

- 没有完整跑一遍 `package-container.sh`（要以 Release 模式编译 Host API、桌面二进制和全部版本）。改写命令与上面验证的那段逐字相同，下一次发布时由 `release-linux.yml` 的新断言把关。
- 没有在有图形会话的真实 Ubuntu 22.04 桌面上，用 IBus 实际打出候选。
- 只验证了 arm64 包；amd64 的 Depends 与 arm64 是同一组库，不同的只是架构限定。
