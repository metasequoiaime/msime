# Agent Note: Linux Release 附带 Arch Linux 二进制包

Status: implemented

## Problem

#9797：仓库已有 Arch 的源码包和 `msime-bin` AUR 定义，但 Linux Release 只发布 Debian、RPM 和 tar 包，测试人员仍需手动下载 RPM 或构建 PKGBUILD。

## Decision

Linux Release 在 x86_64 RPM 验证完成后，用 Arch 官方容器运行现有 `msime-bin` PKGBUILD，将同一 RPM 重新打成 `msime-bin-<版本>-1-x86_64.pkg.tar.zst`。该包作为发布页资产和 `SHA256SUMS` 的一部分上传；源码包与 aarch64 继续由 AUR 定义处理。发布流程只有在 Arch 包构建成功后才进入发布步骤，避免发布页缺少该资产。

#10336 要求 nightly 同样提供 Arch 包。Arch 构建与 `inputs.publish` 脱钩，只以源码是否带打包脚本为条件，普通构建也上传同一 artifact；nightly 按完整 `.pkg.tar.zst` 后缀收集、加构建标识、校验并发布，失败时沿用平台原有的保留旧包规则。包内版本号与正式版一致，不另推 AUR、不改变输入框架的可选依赖。

打包暂存同时携带 `msime.install`，容器里的普通用户经 `makepkg --syncdeps` 安装 PKGBUILD 声明的依赖，随后执行原有 RUNPATH 与动态库校验。省略安装脚本会直接无法打包，跳过依赖安装则无法通过动态库校验。

## Alternatives considered

- **只继续发布 AUR 的 PKGBUILD**：维护者已有这条路径，但不能满足 issue 要求的预构建下载包。
- **在 Arch 容器重新编译完整源码**：会重复 RPM 已完成的 Rust、前端和资源构建，增加发布时间和失败面；`msime-bin` 本来就是同一 RPM 的官方重打包路线。
- **发布后再追加 Arch 资产**：发布成功后才发现重打包失败会留下不完整版本页，因此把 Arch job 放在 `publish` 之前。

## Consequences

Release 页面新增 x86_64 Arch 包；aarch64 用户仍通过 AUR 的架构包定义获得支持。构建依赖 Arch 官方镜像和 Docker，发布 job 的超时时间增加一个独立的 30 分钟 job。

## Verification

已通过远端 `bash scripts/verify-local.sh --quick`、本机 `actionlint .github/workflows/release-linux.yml`、`bash -n platforms/linux/packaging/arch/build-bin-release.sh`、`git diff --check` 与笔记校验。`test-linux-release-page.py` 19 项、`test-nightly-packaging.py` 4 项、`test-nightly-partial-publishing.py` 27 项通过；本机 release-page 测试需将 Homebrew coreutils 的 GNU 工具目录加入 PATH。新增回归检查实际暂存的安装脚本、RPM 字节和摘要、完整扩展名、连续两轮替换及失败保留旧包。容器入口在测试中由替身检查，不代表真实 makepkg、pacman 安装或 Arch 桌面验收；远端没有 Docker，本轮未触发任何发布或 nightly workflow。
