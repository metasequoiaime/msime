# Agent Note: OBS 的 Arch 包同时放两个架构的 .rpm

Status: implemented

## Problem

#6687：用 `install.sh` 或按 README 添加 OBS 的 pacman 源，装到的 `msime-bin` 还是 0.10.1，Linux 0.11.0 已经发布两天了。OBS 上其余仓库（Tumbleweed、Fedora 43/44、Debian Testing/Unstable、Ubuntu 24.04/26.04）都已经是 0.11.0，只有 Arch 停在旧版。

- 0.11.0 的 Release Linux 里 `obs / obs` 是绿的，`publish.sh` 提交成功（`Committed revision 8`）。
- OBS 上 `home:msime/Arch/x86_64` 的 `msime` 构建失败。日志里 `makepkg` 已经打完 `msime-bin 0.11.0-1`，接着又跑了一次 `makepkg`，去下载 `msime-linux-0.11.0-1.aarch64.rpm`，构建机不联网，`Could not resolve host: github.com`，整个构建算失败，pacman 源保留上一次成功的 0.10.1。
- 第二次 `makepkg` 来自 obs-build 的 `build-recipe-arch`：先 `makepkg -so`，再 `makepkg -ef`，再 `makepkg -ef --allsource` 打源码包。`--allsource` 要取齐 PKGBUILD 里所有架构的 source，不只是构建机这个架构的。
- `msime-bin` 的 PKGBUILD 从 0.11.0 起才有 `source_aarch64`（0.11.0 是第一个发布 aarch64 `.rpm` 的版本），而 `publish.sh` 一直只往 OBS 包里放 x86_64 的 `.rpm`。0.10.x 的 PKGBUILD 只有 x86_64，所以之前没出事。

## Decision

OBS 包里放 PKGBUILD 列出的每个架构的 `.rpm`：

- `publish-linux-obs.yml` 从发布页多下载 `msime-linux-VERSION-1.aarch64.rpm`。0.11.0 之前的发布没有这个文件，`gh release download` 对匹配不到的模式不报错（拿 `linux-v0.10.1` 实测退出码 0）。
- `publish.sh` 在渲染出的 `msime-bin/PKGBUILD` 有 `source_aarch64=` 时把 aarch64 的 `.rpm` 加进上传列表，并像其他输入一样缺了就失败；没有这一行（0.11.0 之前的标签）时只放 x86_64 的。按 PKGBUILD 判断而不是按版本号判断，因为发布脚本取自触发运行的提交，PKGBUILD 取自 `linux-vX.Y.Z`，两者可以不同代。

文件完整性不在这里另做：`.rpm` 的校验值由 `render.py` 从 SHA256SUMS 写进 PKGBUILD，OBS 上 `makepkg --allsource` 会按 `sha256sums_x86_64` 与 `sha256sums_aarch64` 逐个核对。

## Alternatives considered

- **给 OBS 渲染一份只有 x86_64 的 PKGBUILD。** 最强的理由是 OBS 的 Arch 仓库只有 x86_64，上传少 30 MB，源码包也小一半。不用它，是因为 OBS 和 AUR 现在用的是同一份渲染结果，为 OBS 再改一份就多了一条要维护、会和 AUR 走岔的路径；OBS 将来给 Arch 加 aarch64 时还得再改回来。
- **在 OBS 项目配置或 prjconf 里关掉 Arch 的源码包。** obs-build 的 Arch 配方把 `--allsource` 写死在构建命令里，和二进制包用 `&&` 串着，没有开关能只跳过它。

## Consequences

- **收益**：OBS 的 Arch 构建不再因为缺 aarch64 的 source 失败，pacman 源能跟上发布。
- **代价**：每次提交多传一个约 30 MB 的 `.rpm`，OBS 上的 Arch 源码包同时带两个架构的 `.rpm`。
- **仍然存在的缺口**：`publish-linux-obs.yml` 只负责提交，不等 OBS 的构建结果，OBS 上某个仓库构建失败时发版流程照样是绿的。这次就是靠用户报告才发现的。
- **验证**：
  - 在断网的 `archlinux` 容器里用 0.11.0 的 SHA256SUMS 渲染 PKGBUILD，按 OBS 的顺序跑 `makepkg -od` 与 `makepkg -efd --allsource`。只放 x86_64 的 `.rpm` 时，报出和 OBS 一样的 `Failure while downloading ... aarch64.rpm`；两个都放时，两组 sha256 都通过，源码包里有两个 `.rpm`。
  - 用替身 `osc` 跑 `publish.sh`：PKGBUILD 有 `source_aarch64` 时提交里有两个 `.rpm`；没有时只有 x86_64 的；有这一行但缺 aarch64 的 `.rpm` 时报 missing 并失败。
  - `shellcheck` 与 `actionlint` 通过。没有在 OBS 上真正提交，要等合并后手动触发 Publish Linux to OBS 重新提交 0.11.0。
