# Agent Note: Android 下载临时文件拒绝硬链接

Status: implemented

## Problem

更新 APK 和云端词库快照下载都把响应写入固定的 `.part` 文件。`NOFOLLOW_LINKS` 能拒绝符号链接，却不会阻止预先放置的硬链接；打开文件时的截断会改写更新目录之外的同一 inode。

## Decision

更新下载使用 `CREATE_NEW` 打开固定临时名，已有符号链接、硬链接或其它文件都会被拒绝，失败清理后再尝试下一个源。快照下载先允许删除安全的旧普通单链接文件，再用 `CREATE_NEW` 创建新临时文件；已有非单链接文件直接拒绝。两条路径继续使用原子改名发布完整文件。

## Alternatives considered

- 只增加 `NOFOLLOW_LINKS`：仍会跟随硬链接。
- 仅检查 `nlink` 后继续使用 `CREATE`：检查与打开之间仍有竞态。
- 直接复用固定临时文件并截断：保留原有硬链接覆盖问题。

## Consequences

下载不会通过预先存在的临时文件硬链接改写目录外文件；普通中断残留仍会被清理，镜像失败后仍可切换到 GitHub，成功文件仍以原子替换发布。

## Verification

新增 Android JVM 冒烟用例覆盖更新 APK 和云快照的临时文件硬链接。`bash platforms/android/check-host.sh` 通过，运行 187 个 Android JVM smoke、编译 55 个 device-suite 源文件及全部服务契约检查。
