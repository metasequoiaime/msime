# Agent Note: OBS 的 Arch 仓库只在提交新版本时构建

Status: implemented

## Problem

#10515：Arch/Omarchy 用户从 OBS 的 `home_msime_Arch` 源升级 `msime-bin 0.15.0-1` 时，pacman 报 `Maximum file size exceeded`，下载失败。

- pacman 把数据库里的 `%CSIZE%` 当作 libcurl 的 `CURLOPT_MAXFILESIZE`，下载到的文件比它大才会报这个错。镜像同步到一半只会让文件变小，所以出错时拿到的是同名、内容不同、更大的另一份包。
- 2026-11-02 实测，`home_msime_Arch.db` 里 `msime-bin-0.15.0-1` 的 `%CSIZE%` 是 76093983（72.57 MiB，与报告截图一致），`%BUILDDATE%` 是 2026-10-31 23:21:01 UTC；download.opensuse.org 跳转到的镜像上同一文件的 Content-Length 也是 76093983。现在源和镜像一致，这次的失败已经不再复现。
- 同名不同内容的包从哪来：`msime-bin` 的 pkgrel 固定为 1（`platforms/linux/packaging/arch/render.py` 与 PKGBUILD），OBS 给 Arch 重建时不改 pkgrel；`publish.sh` 生成的项目 meta 没给仓库设 `rebuild`，OBS 默认 `transitive`，`Arch:Extra` 里任一构建依赖更新都会触发重建。每次 makepkg 都写进新的构建时间和 `.BUILDINFO`，压缩后大小随之变化。`linux-v0.15.0` 在 10-31 14:23 UTC 发布，Fedora_43 的包 15:58 已发布，Arch 的包却是 23:21 构建的；Debian_Unstable 的包 11-01 05:08 又重建过一次，说明这个项目确实在被依赖变更触发重建。
- 还没证实的一环：没有 OBS 账号权限查 `osc jobhistory home:msime msime Arch x86_64`，不能确认 0.15.0-1 是否真的先后构建过两次。另一种可能是用户本地 `/var/cache/pacman/pkg` 里留着旧下载的残片。根因因此只算推断，维护者核对构建历史后再定。

## Decision

`platforms/linux/packaging/obs/repositories.txt` 的 Arch 行加上 `rebuild=local`，`publish.sh` 生成 `<repository name="Arch" rebuild="local">`，Arch 仓库只在包的源码（即每次发版提交的文件）变化时构建，依赖包更新不再触发重建。

- `msime-bin` 只是把发布页的 `.rpm` 重新打包，构建依赖更新后重建出的包没有实际变化，关掉依赖触发的重建对它只有好处：同一个 `msime-bin-V-1` 在源里只会有一份。
- `repositories.txt` 的格式随之扩展：名字后面的列里，含 `=` 的当作 `<repository>` 的属性，只接受 `rebuild=transitive|direct|local`，其他属性名、取值或多于一个路径覆盖列都让 `publish.sh` 在改 OBS 之前报错退出；不含 `=` 的照旧是路径覆盖，两种列顺序不限。
- 生效时机：随下一次 Linux 发版由 `publish-linux-obs.yml` 写入项目 meta。不为 0.15.0 手动重跑：改 meta 有可能再触发一次 Arch 构建，正好又制造一份同名的包。

## Alternatives considered

- **每次重建自动递增 pkgrel。** 最强的理由是它从版本号上根除同名不同内容，镜像和缓存都能按版本区分。不用它，是因为 OBS 的 Arch 构建配方没有像 RPM 的 release 自增那样的机制，PKGBUILD 又和 AUR 共用同一份渲染结果，为 OBS 单独改 pkgrel 会让两边走岔。
- **Debian/Ubuntu/Fedora 仓库也设 `rebuild=local`。** 理由是 Debian_Unstable 也在被反复重建，`.deb` 同样同名。不用它，是因为这些仓库从源码编译，依赖库的 soname 变了必须重建，否则装上就链接失败；RPM 重建时 release 会自增，不会同名。
- **让用户绕开 MirrorCache 直接走 OBS 源站。** 镜像同步的时机不是我们能控制的，而且只是缩短不一致的窗口，同名不同内容的包依然存在。

## Consequences

- **收益**：同一个版本的 `msime-bin` 在 OBS 的 Arch 源里只构建一次，不会再因为依赖更新产生同名不同内容的包；如果 #10515 确实是这个原因，以后不会再犯。
- **代价**：Arch:Extra 的构建环境变化（比如 makepkg 本身的修复）不会自动反映到已发布的版本上，要等下一次发版。`msime-bin` 不编译任何东西，这个代价可以忽略。
- **仍然存在的缺口**：根因未经 OBS 构建历史证实；如果 jobhistory 显示 0.15.0-1 只构建过一次，这次的直接成因要改写成用户本地缓存，`rebuild=local` 仍然防住依赖触发重建这一类问题。`publish-linux-obs.yml` 不等 OBS 构建结果的缺口（见 [2026-10-10-obs-arch-allsource-aarch64-rpm.md](2026-10-10-obs-arch-allsource-aarch64-rpm.md)）依然存在。
- **验证**：
  - 用替身 `osc` 和一份 `/distributions` 夹具跑 `publish.sh`：生成的项目 meta 里只有 Arch 仓库多了 `rebuild="local"`，与改动前的输出逐行比对没有其他差异，Ubuntu 的路径覆盖照旧；`rebuild=never`、`block=never`、两个路径覆盖列都报错退出，且没有走到 `osc meta prj`。
  - `shellcheck` 通过。
  - 需要真实环境：下一次 Linux 发版后用 `osc meta prj home:msime` 确认 Arch 仓库带 `rebuild="local"`，之后几天用 `osc jobhistory home:msime msime Arch x86_64` 确认每个版本只构建一次，并对比 `home_msime_Arch.db` 的 `%CSIZE%` 与镜像上包的 Content-Length。
