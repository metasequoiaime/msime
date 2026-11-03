# Agent Note: Android nightly 各版本共用临时开发签名

Status: implemented
Archived: 2026-11-04

> 已被取代（2026-11-04）：见 [Android nightly 改用正式发布密钥签名](../../implemented/bug-fix/2026-11-04-android-nightly-release-key.md)。nightly 与 `publish=false` 的构建改用正式发布密钥签名，`development-signing-key` job 和 `ANDROID_NIGHTLY_KEYSTORE_*` secrets 都已删除。下文保留原文，供追溯当时的理由。

## Problem

Issue #10709 反馈 Android nightly 的各个版本包签名不一致。`release-android.yml` 的 `release` 是矩阵任务，每个 edition 在独立 runner 上构建；非发布构建没有正式密钥时，`signing.sh` 会在各自的 `HOME/.android/debug.keystore` 中生成开发密钥。每个 runner 都有自己的文件，因此 full、wubi、pinyin 等包的证书不同，用户安装或更新时会被 Android 当作不同签名，重新安装还会丢失 WebDAV 剪贴板和词库数据。

现有 nightly 流程明确禁止解码正式发布密钥，相关约束见[六个平台共用发布构建链的 nightly 产物](../../implemented/process/2026-10-14-six-platform-nightly-builds.md)。问题是开发密钥的作用域只覆盖单个 runner，而不是一次 workflow。

## Decision

`release-android.yml` 增加一个在 `publish=false` 时运行的 `development-signing-key` job。它从独立的 `ANDROID_NIGHTLY_KEYSTORE_BASE64`、`ANDROID_NIGHTLY_KEYSTORE_PASSWORD` secrets 解出固定的 PKCS12 开发密钥，检查 `msime-release` 别名后以 `actions/upload-artifact` 上传，保留一天；六个 `release` 矩阵任务下载同一个 artifact，并通过 `MSIME_ANDROID_RELEASE_KEYSTORE` 与同一密码交给现有 `signing.sh`。正式发布仍只从 `ANDROID_RELEASE_KEYSTORE_BASE64` 和正式密码解出正式密钥，nightly 没有任何正式凭据路径。nightly secret 必须在仓库中预先配置，不能在 workflow 中每次随机生成，否则跨 nightly 更新仍会换证书。

artifact 只在本次 workflow 内分发 nightly 开发密钥的临时副本，不把私钥提交仓库，也不把正式密钥复制到 nightly。固定 nightly 证书允许上一晚的包覆盖升级到下一晚；artifact 过期后不能复用它，这符合 nightly 的测试定位。

## Alternatives considered

- **让每个矩阵 runner 继续生成自己的 `debug.keystore`**：实现最简单，也保持本地开发回退逻辑；没有采用，因为 runner 隔离正是证书不一致的根因，无法覆盖安装。
- **nightly 直接使用正式发布密钥**：可以让 nightly 与正式包互相覆盖；没有采用，因为 nightly 的流程边界明确不接触无法更换的正式密钥，泄露测试构建会扩大正式发布风险。
- **把私钥提交仓库**：可以让矩阵任务读取同一把钥匙；没有采用，因为它会把开发签名私钥暴露在源代码历史中。nightly 使用专用 secret 保存密钥，artifact 只在一次运行期间暴露。
- **每次 workflow 随机生成并上传一把密钥**：可以解决同一次矩阵构建的 edition 不一致；没有采用，因为下一晚会换证书，用户仍无法覆盖升级。

## Consequences

- Android nightly 的各 edition 现在使用同一张临时证书，具备相同签名的必要条件，可以覆盖同一 `applicationId` 的上一份 nightly 包并保留应用数据；不同 edition 的 `applicationId` 仍由版本表决定，证书一致不改变它们能否互相安装。
- 正式发布路径和本地开发路径的默认值不变；本地仍由 `dev-keystore.sh` 管理自己的 `debug.keystore`。
- 共享密钥 artifact 是 workflow 的短期中间产物，发布任务不会把它上传到 release 页面；真正跨 nightly 保持不变的是专用 secret 中的证书，artifact 过期后不能复用它，这符合 nightly 的测试定位。
- 只做 workflow、契约脚本和文档改动，没有 Android APK 运行时资源或文件读写变更，因此本次不宣称真机验收；需要在下一次 nightly 中核对各 APK 的证书摘要并在设备上覆盖安装验证。

