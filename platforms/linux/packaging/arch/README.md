# Arch Linux（AUR）

两个 AUR 包的定义在这里维护，AUR 上的 git 仓库只是它们的拷贝：

> 两个 PKGBUILD 在第一次按 0.10.0 或更新的发布渲染之前停在 `pkgver=0.9.1`，只是占位，**不能直接推到 AUR**：`msime.install` 运行的 `msime-linux-setup --register` 从 0.10.0 才有，0.9.1 的源码还按已删除的 `langdict-v1.0.0` 取方言词库，源码包在 `prepare()` 就会失败。`render.py` 拒绝渲染 0.10.0 之前的版本。推送前确认 `pkgver` 就是这次要发布的版本。

- `msime/`：源码包。从 `linux-v<版本>` 标签的源码归档构建，Rust 用 `rust-toolchain.toml` 钉住的版本（经 `rustup`，不用系统的 `rust` 包），步骤与 `platforms/linux/package-container.sh` 相同：Release 的 Host API、`msime-mcp` 与带前端的桌面二进制，`MSIME_ENABLE_PACKAGING=ON` 配置的 CMake 构建（Fcitx5 插件与 IBus engine），`check()` 跑与门禁相同的 ctest，`package()` 结束前核对插件按 RUNPATH 找到的是本包里的 Host API、所有 ELF 都没有解析不到的库。随包资源由 `scripts/fetch_*.py` 按 `resources/*.lock.json` 下载并核对，不在 PKGBUILD 里另记哈希。
- `msime-bin/`：把同一发布的 `.rpm` 重新打包。`.rpm` 的 `/usr/lib64` 整体挪进 `/usr/lib`，RUNPATH 与 `msime_voice_paths.py` 里的相对路径改成 `lib`，Fedora 做成跨目录硬链接的许可证文件拆开；核对与源码包相同。

两个包内容一致，互相冲突，`msime-bin` 提供 `msime`。安装、升级与卸载时的用户单元处理在 `msime.install`，对应 `.deb` 的 postinst/prerm，两份逐字相同。主词库不随包，装好后每个用户运行一次 `msime-linux-setup --download`。

## 每次发布后更新

在 `develop` 的检出里（Arch 上，以普通用户运行，生成 `.SRCINFO` 要 `makepkg`），改完经普通 PR 合入 `develop`：

```sh
python3 platforms/linux/packaging/arch/render.py <版本> --sha256sums <发布的 SHA256SUMS>
```

它改写两个 PKGBUILD 的 `pkgver`、`pkgrel=1` 与校验值（源码归档自己下载计算，`.rpm` 取 SHA256SUMS 里的那行），并重新生成两份 `.SRCINFO`。`scripts/test-arch-gentoo-packaging.py` 核对 `.SRCINFO` 与 PKGBUILD 一致。

构建验证（需要 docker；Apple Silicon 上跑 linux/amd64 镜像）。在模拟下（Docker 主机不是 x86_64）`linux-replaced-program` 必然失败，脚本自动让 makepkg 跳过 `check()`、再自己跑排除了这一条的 ctest；这条测试只能在原生 x86_64 上验证：

```sh
platforms/linux/packaging/arch/check-in-container.sh msime      # 完整源码构建、ctest、namcap、安装与卸载
platforms/linux/packaging/arch/check-in-container.sh msime-bin
```

## 发布到 AUR

仓库不会自动推送。维护者在 AUR 注册 `msime` 与 `msime-bin`、上传 SSH 公钥后，每个包：

```sh
git clone ssh://aur@aur.archlinux.org/msime.git aur-msime
cp platforms/linux/packaging/arch/msime/{PKGBUILD,.SRCINFO,msime.install} aur-msime/
cd aur-msime
git add PKGBUILD .SRCINFO msime.install
git commit -m "Update to <版本>"
git push origin master
```

`msime-bin` 同理，换成 `ssh://aur@aur.archlinux.org/msime-bin.git` 与 `msime-bin/` 下的文件。AUR 只接受仓库根目录的文件，推送前确认 `.SRCINFO` 是 `render.py` 刚生成的。
