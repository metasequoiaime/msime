# Agent Note: iOS 五笔码表改为按需资源

状态：已实现

## Problem

iOS 的 Simulator 归档仍把 `msime-wubi.db` 放进键盘扩展，单文件约 142.5 MB，占当前 219.82 MB 归档的约 65%。前一篇[全平台安装包按需资源瘦身](2026-11-07-all-platform-package-slimming.md)因当时 iOS 设置页没有把五笔资源包接入，只省略了日文词典组。

## Decision

iOS full 和 pinyin 版本把 `msime-wubi.db` 与日文词典一起列为按需资源组；wubi 版本仍随包携带码表。iOS 主 App 在用户选用五笔时请求已有的 `wubi` 资源包，键盘扩展只在码表随包存在或资源包已安装时列出五笔方案。资源包安装仍经共享 Rust 校验、原子发布和 App Group 状态目录，输入法进程不联网。

设置页没有码表时不打开五笔查码入口，避免在下载前对缺失资源发请求；下载完成并重新取得键盘焦点后，键盘刷新资源包状态并显示五笔。

## Alternatives considered

- 保留 iOS 随包五笔码表：改动最小，但首包继续承担约 142.5 MB，无法解决 iOS 体积问题。
- 只在 iOS 构建脚本里删除文件：会让键盘仍把五笔当作可用方案，缺失时回退且设置页没有下载入口，不能采用。
- 为 iOS 另做一套下载协议：没有必要，现有 `ResourcePack::Wubi`、C ABI 和 Swift 资源包页面已经覆盖同样的校验与进度语义。

## Validation

- `cargo test -p msime-client-core resources::tests::desktop_and_mobile_targets_allow_optional_groups --locked`：通过。
- `cargo test -p msime-client-core resources::tests::each_on_demand_group_is_left_out_on_its_own --locked`：通过。
- `cargo test -p msime-host-api wubi_reads_the_bundled_table_then_the_downloaded_pack_and_falls_back_without_either --locked`：通过。
- iOS Simulator 和真机包重打、资源包下载及五笔输入验收：待 CI 与真机完成。

## Consequences

iOS full 和 pinyin 的首包预计减少约 142.5 MB；五笔用户首次启用时需要下载同一份码表，下载完成后行为与完整包一致。App Store 的实际安装大小还会受 App Thinning 和压缩影响，最终以签名 IPA 和设备安装体积为准。
