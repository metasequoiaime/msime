# Agent Note: 发布后自动上传蒲公英

Status: implemented

## Problem

发布工作流只把 Android 安装包放进 GitHub Release，测试用户还需要手动下载再上传蒲公英。蒲公英已有官方 GitHub Action，仓库需要在发布完成后复用已验证的安装包自动上传，同时不能把 API Key 写入仓库或让没有密钥的 fork 发布失败。

## Decision

`release-android.yml` 在 GitHub Release、设备验收都成功后增加 `pgyer` 矩阵 job，为六个 Android edition 各下载一个 APK，再调用固定到 `v2.0.0` 提交的 `PGYER/pgyer-upload-app-action`。流程从仓库 Secret `PGYER_API_KEY` 读取密钥，并把版本和 edition 写入蒲公英更新说明。

密钥为空时上传步骤跳过并写 notice，以保留 fork 和没有配置蒲公英的构建能力；密钥存在时 Action 上传失败会让对应 job 失败。上传发生在 GitHub Release 成功之后，避免蒲公英失败时留下一个半发布的 GitHub Release，也不回滚已经发布的 GitHub Release。

iOS 暂不接入：当前公开 Release 产物是未签名的模拟器 tarball，不能作为蒲公英支持的 IPA；真机 IPA 只在 TestFlight 分支流程的临时目录中生成。桌面平台的 DMG、EXE、DEB、RPM 也不属于蒲公英支持的安装包格式。

## Alternatives considered

- 直接在工作流里用 curl 实现蒲公英三步上传：可以减少对第三方 Action 的依赖，但会重复维护凭证获取、上传和轮询逻辑；官方 Action 已封装这些步骤，因此不选。
- 用蒲公英 CLI：需要在 runner 上额外安装 Node.js CLI，并维护运行时；现有 Action 能直接消费已下载的 APK，因此不选。
- 让缺少 `PGYER_API_KEY` 的运行失败：能更早暴露配置遗漏，但会破坏 fork 和只验证 GitHub Release 的发布；保留跳过并输出 notice。

## Consequences

- 配置 `PGYER_API_KEY` 后，正式版和 beta 版的 Android 六个 APK 会自动进入蒲公英，并在 job summary 显示各自下载页。
- Android 每个 edition 独立上传，蒲公英按包的 application ID 维护对应应用；上传 job 与设备验收和 GitHub 发布绑定，任一步前置失败都不会上传。
- 第三方 Action 通过完整 commit SHA 固定，后续升级 v2 需要显式更新 SHA 并重新核对输入输出。
