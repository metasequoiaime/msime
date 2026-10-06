# Gentoo

本目录是一个最小的 overlay（`profiles/repo_name` 为 `msime`，`metadata/layout.conf` 以 `gentoo` 为 master），只有 `app-i18n/msime` 一个包：

- `msime-9999.ebuild`：跟踪 `develop` 的 live ebuild。克隆、`cargo vendor`、pnpm 安装与锁定资源下载都在 `src_unpack` 里完成，Portage 只在 live ebuild 的这一步放行网络；pnpm 经 `npx` 取 `package.json` 的 `packageManager` 钉住的版本。
- `msime.ebuild.in`：按版本发布的 ebuild 模板，由 `render.py` 渲染成 `msime-<版本>.ebuild`。构建时不联网：crate 由 `pycargoebuild` 按该版本的 `Cargo.lock` 列进 `CRATES`/`GIT_CRATES`（crate 许可证一并填入 `LICENSE`），随包资源按 `resources/*.lock.json` 的地址列进 `SRC_URI`（distfile 名带锁定 SHA-256 的前缀），`src_prepare` 把它们放到 fetch 脚本的输出目录，脚本只核对、不下载。桌面二进制嵌入的前端取自发布附带的 `msime-<版本>-frontend.tar.xz`（由 `platforms/linux/packaging/make-source-tarballs.sh` 生成；pnpm 依赖没法像 crate 那样逐个列进 `SRC_URI`）。

两份 ebuild 的构建步骤一致，对应 `platforms/linux/package-container.sh`：Host API、`msime-mcp` 与桌面二进制用 cargo.eclass 构建，`MSIME_ENABLE_PACKAGING=ON` 配置 CMake，`src_test` 跑与门禁相同的 ctest，`src_install` 结束前核对 Fcitx5 插件按 RUNPATH 找到的是本包里的 Host API。

- Rust：`RUST_MIN_VER` 等于 `rust-toolchain.toml` 钉住的版本（Gentoo 用系统的 `dev-lang/rust` 或 `rust-bin`，只能要求不低于它）。
- USE：`fcitx5`（默认开）控制 Fcitx5 插件。IBus engine 始终构建：顶层 CMake 把 `ibus-1.0` 列为必需，`msime-linux-setup` 与 Fcitx5 首次配置用到的 `msime-linux-prepare` 也和它一起构建。
- `pkg_postinst`/`pkg_prerm` 对应 `.deb` 的 postinst/prerm：在 systemd 系统上重启或停用 `msime-linux-setup` 启用过的用户单元，卸载时为每个用户运行 `msime-linux-setup --unregister`；OpenRC 系统上什么也不做。

## 每次发布后渲染

在 `linux-v<版本>` 标签的检出里、装有 `app-portage/pycargoebuild` 与 `dev-util/pkgdev` 的 Gentoo 环境中：

```sh
python3 platforms/linux/packaging/gentoo/render.py <版本> --out <overlay 检出>
cd <overlay 检出>/app-i18n/msime
pkgdev manifest            # 下载全部 distfile，生成 Manifest
pkgcheck scan --exit error
```

`render.py --no-crates` 只填资源与 Rust 版本、不跑 pycargoebuild，用来在没有 Gentoo 环境时检查模板；那样的结果不能发布。

容器里的检查（需要 docker）：

```sh
platforms/linux/packaging/gentoo/check-in-container.sh [<版本>]
```

它渲染版本 ebuild、用 `pkgcheck` 扫整个包，再把 live ebuild 跑完 `src_unpack`。完整的 `emerge` 不在其中：WebKitGTK、Fcitx5 与 IBus 在 stage3 容器里都要从源码编译。

## 发布

仓库不会自动发布。推荐维护一个独立的 overlay 仓库（例如 `metasequoiaime/gentoo-overlay`），每次发布：

```sh
git clone git@github.com:metasequoiaime/gentoo-overlay.git
cp -r platforms/linux/packaging/gentoo/{metadata,profiles} gentoo-overlay/
python3 platforms/linux/packaging/gentoo/render.py <版本> --out gentoo-overlay
cp platforms/linux/packaging/gentoo/app-i18n/msime/msime-9999.ebuild gentoo-overlay/app-i18n/msime/
cd gentoo-overlay/app-i18n/msime && pkgdev manifest && pkgcheck scan --exit error
cd ../.. && git add -A && git commit -m "app-i18n/msime: add <版本>" && git push
```

用户以 `eselect repository add msime git https://github.com/metasequoiaime/gentoo-overlay.git` 添加后 `emerge app-i18n/msime`。版本 ebuild 只有 `~amd64 ~arm64` 关键字，稳定分支的系统先放行：`echo 'app-i18n/msime ~amd64' > /etc/portage/package.accept_keywords/msime`（arm64 上写 `~arm64`），否则 Portage 报 `masked by: ~amd64 keyword`。
