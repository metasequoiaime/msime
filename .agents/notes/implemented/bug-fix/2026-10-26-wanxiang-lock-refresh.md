# Agent Note: 更新万象 LTS 语法模型锁

Status: implemented

## Problem

万象 LTS 发布在同一下载 URL 下替换了 `wanxiang-lts-zh-hans.gram`，仓库锁文件仍记录旧文件的长度和 SHA-256，导致按需下载后完整性校验失败。#11212 的同类报告对应 2026-11-10 再次替换的资产；当前在线目录也仍是旧摘要，因此不仅 macOS 0.55.0 的内嵌锁会失败，新版客户端读取旧目录同样会拒绝新字节。

## Decision

保留 LTS 标签和已有 `source_commit`，把上游 Release API 当前资产的 `size`、`digest` 与完整下载文件逐字节哈希核对后，同时更新 `resources/grammar-models.lock.json` 和 `resources/grammar-models.catalog.json`。下载器继续执行长度和 SHA-256 双重校验，不放宽完整性检查，也不在仓库内提交 418 MB 模型文件。

## Alternatives considered

- 固定不可变 release URL：当前上游没有为该资产提供新的不可变下载地址，因此本次不能采用。
- 取消或放宽校验：会削弱资源包完整性保证，拒绝。

## Consequences

已有旧缓存因摘要不匹配会被重新下载；新下载内容可通过锁定值校验。上游继续覆盖 LTS 资产时仍需再次刷新锁，长期可考虑托管不可变副本。

下载及已安装身份按[语法模型保留本地快照](../feature/2026-10-26-grammar-model-local-snapshot.md)执行：新下载必须匹配在线目录中的摘要和长度，已有完整快照继续离线可用。此前[可变资源下载](2026-10-26-wanxiang-auto-update.md)的完整下载机制仍在，但不能据此跳过当前目录的摘要检查。macOS 0.55.0 仍使用旧内嵌锁，需要升级到包含新锁的客户端；支持在线目录的版本在目录更新后即可重新下载。

## Verification

- 2026-11-10 的 GitHub Release 资产 `729644630` 返回 `sha256:3457b69f5fe0fc4f5d211c878bbfa2e2361b90639952c9845c8e31e9d047cde6`、`418071596` 字节，更新时间为 `2026-11-10T01:22:43Z`。
- 完整下载官方文件后，本机计算的 SHA-256 和长度与 API 完全一致。文件头为 `Rime::Grammar/1.0`，数组 `104517888` 单元，偏移为 `4`，数组结束位置恰好等于文件大小；未把模型文件提交入仓库。
- 上游 `LTS` 标签仍指向 `c78463a521aee2681db6cd6424a75a9b413237a3`，所以保留 `source_commit` 和下载地址。
- `cargo test -p msime-client-providers --lib resource_packs::tests::`：41 项通过；覆盖目录来源、通道、锁回退、快照和损坏模型拒绝。
- `rbuild bash scripts/verify-local.sh --quick`：通过，运行 `20261110-163421`；部分平台工具链缺失时由门禁按既有规则跳过，不代表所有平台装机验收。
- `git diff --check`。
