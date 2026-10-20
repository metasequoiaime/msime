# Agent Note: Android 常用符号只显示实际使用记录

Status: implemented

## Problem

#8991 希望 Android「符号 → 常用」初始为空，只记录用户输入过的符号。当前没有使用记录时也显示 30 个默认标点，使用几个符号后仍被默认内容补满，不能反映个人习惯。

## Decision

Android「常用」只显示现有记录经校验、去重后的内容，不再追加默认标点。没有记录时仍默认打开「常用」，显示现有空态提示。历史记录保留，不清空存储；中文标点继续可以从「中文」分类使用。上屏成功后记录、最多 30 项、隐私输入不记录，以及不即时重排正在显示的网格等规则保持。

## Historical audit

- [移动端默认补满](../../implemented/feature/2026-10-16-mobile-symbol-common-filled.md)：部分取代其 Android 展示决定，保留 iOS 的规则与原先避免空白首屏的理由，双方互链。
- [Android 符号使用记录](../../implemented/feature/2026-10-08-android-symbol-panel-recents.md)：记录规则仍成立；不恢复其没有记录时改开「中文」的旧入口行为，双方互链。

## Alternatives considered

- 保留默认内容补满：首次打开就能直接使用常见标点，避免空白首屏；但这正是 #8991 希望移除的内容，固定分类已经提供这些标点。
- 没有记录时默认打开「中文」：新用户不必手动切类就能使用符号；但默认页会随历史有无变化，保留稳定的「常用」入口和现有提示更符合本次要求。

## Verification

- `check-host.sh` 通过：250 项 JVM 冒烟、196 个设备套件源文件编译，以及 Java/API、清单和资源检查；`SymbolPanelModelSmoke` 覆盖初始为空、有记录时只显示记录、重复记录、长度与数量限制及分类入口。
- 原生 Android 宿主 `assembleFullRelease` 通过。在 API 28（`emulator-5566`）与 API 35（`emulator-5580`）安装后，`SymbolPanelLayoutDeviceSmoke` 均通过：空态、两条记录不被补满、1000/720/1000 宽度、连续上屏、滑动切类与重新打开后的空态恢复。
- 同一设备检查在 API 28 原有 0.5.0 安装包上先失败于「空常用应仅显示提示，不应填充默认符号」，覆盖安装本分支的宿主后通过。
- Mac Studio 上 `pnpm run verify-notes` 和 `scripts/verify-local.sh --quick` 均通过；quick 包括仓库契约、Rust workspace、Android 目标编译、Android Java 宿主与共享 Apple 桥接层，其他平台按改动范围跳过。
- 设备验收包使用本分支 Java 宿主，原生库及资源取自官方 `android-v0.5.0` APK（提交 `d61258b1f5dd21959c13c6df7c72ac3a1ec7dbc7`，SHA-256 `924d1727b9ecd1099c7a96d1acaadec4a9cfc95c8a54d2f075e21c2a0a3238a1`），剔除会与 Gradle 生成内容重复的 `assets/dexopt` 后重新打包，以编译机的开发证书签名。此次未修改原生库、资源与记录存储，不把该包验收当作当前分支完整 Rust 构建或发布签名验收。

## Consequences

常用列表能够准确反映实际使用，不再混入自动补位的标点。新用户首次打开需要切到其他分类才能输入符号；默认标点不会再作为使用记录的视觉补位。iOS 的补满行为仍保持，由另一项明确的平台需求决定是否调整。
