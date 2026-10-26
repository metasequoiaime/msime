# Agent Note: Linux Release 附带 Arch Linux 二进制包

Status: implemented

## Problem

#9797：仓库已有 Arch 的源码包和 `msime-bin` AUR 定义，但 Linux Release 只发布 Debian、RPM 和 tar 包，测试人员仍需手动下载 RPM 或构建 PKGBUILD。

## Decision

Linux Release 在 x86_64 RPM 验证完成后，用 Arch 官方容器运行现有 `msime-bin` PKGBUILD，将同一 RPM 重新打成 `msime-bin-<版本>-1-x86_64.pkg.tar.zst`。该包作为发布页资产和 `SHA256SUMS` 的一部分上传；源码包与 aarch64 继续由 AUR 定义处理。发布流程只有在 Arch 包构建成功后才进入发布步骤，避免发布页缺少该资产。

## Alternatives considered

- **只继续发布 AUR 的 PKGBUILD**：维护者已有这条路径，但不能满足 issue 要求的预构建下载包。
- **在 Arch 容器重新编译完整源码**：会重复 RPM 已完成的 Rust、前端和资源构建，增加发布时间和失败面；`msime-bin` 本来就是同一 RPM 的官方重打包路线。
- **发布后再追加 Arch 资产**：发布成功后才发现重打包失败会留下不完整版本页，因此把 Arch job 放在 `publish` 之前。

## Consequences

Release 页面新增 x86_64 Arch 包；aarch64 用户仍通过 AUR 的架构包定义获得支持。构建依赖 Arch 官方镜像和 Docker，发布 job 的超时时间增加一个独立的 30 分钟 job。

## Verification

已通过 `bash -n platforms/linux/packaging/arch/build-bin-release.sh`、`git diff --check`；仓库 Linux release page 测试已同步更新，待运行完整 quick 门禁和 CI workflow validation。
