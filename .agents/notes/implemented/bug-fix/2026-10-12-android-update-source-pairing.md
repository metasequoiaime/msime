# Agent Note: Android 更新按来源配对校验文件与 APK

Status: implemented

## Problem

`UpdateApi.download` 先从镜像或 GitHub 取得第一份可用的 SHA-256 文件，再按镜像、GitHub 的顺序下载 APK。镜像校验文件过期或被污染时，GitHub 上正确的 APK 仍按镜像 SHA 核对而被拒绝；反过来，镜像 SHA 也可能让不符合 GitHub 校验文件的 APK 通过下载阶段。

## Decision

每次来源尝试都先读取同一来源的校验文件，再核对该来源的 APK。镜像的校验文件缺失、格式错误、网络失败或 APK 摘要不符时，下一次尝试重新读取 GitHub 校验文件。缓存中的已下载文件也只按当前尝试取得的 SHA 复用。`DOWNLOAD_LOCK` 覆盖这整个流程，以免并发下载改写同一目录。

## Alternatives considered

- 只在镜像 APK 校验失败后重新获取 GitHub SHA：能处理摘要不符，但镜像校验文件缺失或格式错误时仍需额外分支，且来源配对规则分散。
- 继续使用第一份可用的 SHA：少一次网络请求，却无法保证 GitHub APK 与 GitHub 发布的校验文件配套。

## Consequences

GitHub 回退会多请求一次自己的校验文件；镜像可用时仍不访问 GitHub。镜像与 GitHub 的校验信息互不混用，失败的 `.part` 文件继续清理，安装前的签名和版本检查保持原有约束。

## Verification

Android JVM 冒烟覆盖镜像校验文件过期后回退成功，以及镜像 SHA 不能替代 GitHub SHA 放行 APK。`bash platforms/android/check-host.sh` 通过，运行 215 个 JVM smoke 并编译 86 个设备套件源文件。
