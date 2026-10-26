# Agent Note: 更新万象 LTS 语法模型锁

Status: implemented

## Problem

万象 LTS 发布在同一下载 URL 下替换了 `wanxiang-lts-zh-hans.gram`，仓库锁文件仍记录旧文件的长度和 SHA-256，导致按需下载后完整性校验失败。

## Decision

保留 LTS 标签和已有 `source_commit`，只按上游 Release API 当前资产的 `size`、`digest` 更新 `resources/grammar-models.lock.json`。下载器继续执行长度和 SHA-256 双重校验，不放宽完整性检查，也不在仓库内提交 418 MB 模型文件。

## Alternatives considered

- 固定不可变 release URL：当前上游没有为该资产提供新的不可变下载地址，因此本次不能采用。
- 取消或放宽校验：会削弱资源包完整性保证，拒绝。

## Consequences

已有旧缓存因摘要不匹配会被重新下载；新下载内容可通过锁定值校验。上游继续覆盖 LTS 资产时仍需再次刷新锁，长期可考虑托管不可变副本。

## Verification

- GitHub Release API 返回 `sha256:e34500ab44626dc039a0641792ecba98ab63307646eb3d29186be56608ffe094` 和 `418078764` 字节。
- `git diff --check`。
