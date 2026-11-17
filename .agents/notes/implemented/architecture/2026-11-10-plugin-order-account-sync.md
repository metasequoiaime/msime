# Agent Note: 插件列表展示顺序通过账号标量字段跨桌面同步

Status: implemented

## Problem

#11076 的插件列表已经能在 Windows、macOS 和 Linux 设置页拖动卡片，但 `plugins.plugin_order` 只写入本机偏好。账号设置同步的值边界只接受布尔、整数、数字和字符串，直接把数组加入协议会扩大所有宿主的值类型契约；完全不接入账号同步又会违背 issue 对跨平台同步的要求。

## Decision

桌面宿主统一导出和应用 `plugins.plugin_order`。字段值使用 JSON 字符串，内容是 `kind/id` 字符串数组；空数组也写成 `[]`，这样用户恢复默认顺序时可以清除另一台设备留下的顺序。该字段只描述设置页卡片展示顺序，不改变插件加载、执行、候选处理或脚本授权顺序。

应用云端值前先解析为字符串数组，再限制最多 256 项、每项必须有非空的 `kind/id`、两部分都必须通过 `safe_id`、数组内不得重复。非法值只跳过 `plugins.plugin_order`，同一份文档里的其他设置继续应用；未知或尚未安装的插件键交给共享设置页按当前目录追加。服务端字段表按现有账号协议以 `string` 收录该键，客户端仍经字段表过滤，不绕过字段表写入。

## Alternatives considered

- **扩展 `AccountPreferenceValue`，让账号值直接支持数组**：数组更贴近本地 JSON，服务端也能直接校验元素。否定：账号协议当前明确只允许标量，移动端、字段表校验、合并和所有宿主都要同步扩大类型面；本需求只新增一个展示顺序字段，不值得改变公共边界。
- **只在本机保存顺序，不加入账号同步**：实现最小。否定：报告者明确要求拖动后的顺序跨平台同步，macOS、Windows、Linux 之间会继续出现设置页顺序不一致。
- **把每个插件位置拆成多个整数键**：可以继续使用标量字段。否定：插件集合动态变化、删除和插入都会产生大量字段，无法原子表达顺序，也会留下旧插件的位置键；一个 JSON 字符串能保持一次更新的完整顺序。

## Consequences

桌面三端使用同一个字段和校验规则，空顺序可以同步清除，非法云端数据不会阻断其他设置。账号字段的线格式仍是标量字符串，旧客户端会忽略字段表中新增但自身不认识的键。字段表未部署该键时，现有 `within_schema` 流程会过滤它，顺序同步要等服务端字段表收录后才生效。

## Verification

- `cargo test -p msime-client-services --lib account::settings_sync`：45 项通过，覆盖 Windows、Linux、macOS 三端往返，以及 JSON、重复项、非法路径的逐键跳过。
- `cargo fmt --all -- --check`：通过。
- UI 的拖动、类型检查和远程 quick 门禁沿用 PR #11222 已通过的结果。

## Related notes

- [字段表新增的共享偏好只由能改它的宿主导出和应用](2026-10-27-settings-sync-new-shared-keys-per-host.md)：沿用“只由桌面宿主导出和应用”的规则。
- [macOS 共享账号页补上设置同步](../bug-fix/2026-11-03-macos-account-settings-sync-entry.md)：macOS 共享设置页使用同一桌面同步入口。
