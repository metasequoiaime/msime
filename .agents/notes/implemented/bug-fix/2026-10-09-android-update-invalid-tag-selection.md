# Agent Note: Android 更新过滤非法发布标签

Status: implemented

## Problem

更新列表允许任意非空 `tag`。`pick` 会先按版本号选中最高条目，而 `update` 随后才拒绝不符合下载路径规则的标签；因此一个带非法标签的高版本条目会让可下载的低版本更新被错误地报告为不可用。

## Decision

`pick` 在比较版本前复用下载 URL 使用的标签规则，并跳过空版本或非法标签。这样坏的元数据不会遮蔽其它可用发布；最终的 `update` 校验仍保留。

## Alternatives considered

- 只在 `update` 中校验标签：非法的高版本仍会先被 `pick` 选中，从而遮蔽可用更新。
- 只在 `parseReleases` 中过滤：直接调用 `pick` 的路径仍可把非法条目选出来，选择逻辑的安全边界不完整。

## Consequences

合法发布的选择和下载地址保持不变。非法发布条目被忽略，检查更新会继续寻找下一个可下载版本。

## Verification

- `UpdateApiSmoke` 新增非法高版本标签回归检查并通过。
- `git diff --check`
- `bash platforms/android/check-host.sh`
