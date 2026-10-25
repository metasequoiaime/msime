# Agent Note: Linux 词库下载声明项目客户端身份

Status: implemented

## Problem

#8005：`msime-linux-setup --update --download` 取国内镜像时返回 403，被迫回退 GitHub 原地址。2026-10-15 对锁文件 `dict-v2.0.24` 的公开资产做实测：Python 默认客户端标识收到 `error code: 1010`，项目标识 `msime-linux-setup` 则能下载同一镜像上的许可证文件。Cloudflare 的 [1010 说明](https://developers.cloudflare.com/support/troubleshooting/http-status-codes/cloudflare-1xxx-errors/error-1010/)将其归为客户端签名被阻止。直接 `urlopen(url)` 让 Python 默认标识进入所有词库下载请求。

镜像 Worker 的真实默认分支 `main` 固定提交 [f44e416f17e31d03e3c97d129f662e13b699343e](https://github.com/metasequoiaime/msime-web/blob/f44e416f17e31d03e3c97d129f662e13b699343e/workers/dl/index.mjs)已放行该词库仓的发布资产，当前返回正文也不同于 Worker 的 `Forbidden`。HEAD 不触发回源；合法但未缓存的文件可以返回 404，验收需执行实际 GET。

## Decision

在下载器唯一的 `fetch` 入口用 `urllib.request.Request` 设置 `User-Agent: msime-linux-setup`，让镜像和原地址都收到稳定的项目客户端身份。沿用 Python 标准库的超时、流式读取、大小上限、完整性校验和失败换源。仓库已有模型下载器声明项目 User-Agent，本入口采用相同惯例。

手动下载入口见[首次配置下载清单](../feature/2026-10-24-linux-dictionary-download-links.md)；它导出同一锁和镜像规则，自动下载的客户端身份仍由本篇管理。

## Alternatives considered

- **调整 Cloudflare Browser Integrity Check**：可以让使用 Python 默认标识的历史客户端一并恢复；但会修改站点安全策略，影响范围超过当前客户端缺少身份声明的问题，也不保证其他用户镜像接纳默认 Python 标识。
- **使用浏览器 User-Agent**：实测同样能取回文件，对常见浏览器放行规则适应面更广；但安装器有明确的项目身份，不应依靠易过期的浏览器版本字符串来维护下载。

## Verification

`python3 platforms/linux/tests/core/setup_update.py` 的真实安装器升级回归在旧实现下失败：本地镜像和原地址都因默认客户端标识返回 403。修复后整套升级回归通过，新用例确认只访问镜像、合成词库通过摘要校验并切换到新代次。

修复后的真实 `fetch_verified` 从国内镜像取回锁定的 `msime-pinyin.db`，共 73,723,904 字节，SHA-256 与 `resources/desktop-dictionary.lock.json` 一致；记录到一次请求，没有回退原地址。当前网络上的实际 GET 验证补上了 HEAD 对未缓存文件只能返回 404 的限制。

`python3 platforms/linux/tests/core/setup_resolution.py`、`setup_registration.py` 通过；`bash scripts/verify-local.sh --quick` 通过，其中 Linux 容器内原生宿主的 87 项测试全部通过，包含真实升级 HTTP 回归。

## Consequences

正常项目标识恢复当前镜像下载，客户端标识不带用户数据。标识只能声明来源，不能保证所有网络和镜像安全策略都会放行；不可达和校验失败仍按原规则回退。没有 Deepin 桌面环境，验证不代表已在报告者机器上安装输入法。

## Existing notes audit

检索活跃及被否笔记中的 `fetch_verified`、`download_prefixes`、`PROJECT_MIRRORS`、`MSIME_DOWNLOAD_MIRROR`、国内镜像和下载标识，没有管理本入口请求身份的现有笔记。

- [资源 generation 发布拒绝覆盖](2026-10-10-resource-generation-no-replace.md)负责共享安装器的文件发布原子性，与 HTTP 请求标识无关，保留。
- [Linux legacy 包](../feature/2026-10-09-linux-legacy-glibc228-deb.md)负责运行时与 Python 基线，部分重叠；本篇采用其 Python 3.7 可用的 `Request` API，与其互链。
- 语音模型导入、资源包换源与其他平台镜像笔记不管理 Linux setup 的请求头，保留。
