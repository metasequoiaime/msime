# Agent Note: 云候选固定席位

Status: implemented

## Problem

云候选在网络回包到达前不出现在候选列表。回包随后插入第二席，会把用户正在看的本地候选整体后移，数字键或空格可能选中另一行。

## Decision

共享 Engine 在允许云候选的组字中预留一个 `CandidateSource::CloudPending` 行，固定在云候选席位。云回包仍由在线批处理写入 `CloudSuggestion`，它替换同一席位；没有结果时占位保留到组字结束。关闭云候选时立即移除占位。Android、iOS 和 HarmonyOS 隐藏该内部占位行，等真实云候选返回后再显示，避免把 `☁`、加载符号或云字样放进移动端候选栏；Runtime、插件锁定规则和各宿主候选呈现都把该来源当作在线占位行处理，禁止学习、改字和候选管理。

## Alternatives considered

- **只在宿主候选栏末尾画加载图标**：改动小，但候选数字位置仍会在回包时移动，无法解决误选。
- **为云候选预留多个空席**：能覆盖多条回答，但云协议只有一个云候选席位；多余空席会挤掉本地词并造成分页跳动。
- **只在回包后冻结当前高亮**：高亮不动仍不能保证数字键对应的文本不变，且各宿主的候选身份映射会分叉。

## Consequences

Android、iOS 和 HarmonyOS 在云请求期间不显示占位行，真实云候选返回后才出现在候选栏；三个移动宿主都把触控、硬件数字键、候选维护操作和展开面板的显示序号重新映射回 Engine 原始索引，避免隐藏占位行后错选。共享占位行仍会增加一个候选席位并参与页数计算，云请求失败时仍会占位到组字变化。来源值追加为 15，保留已有候选来源的 ABI 数字。

验证包括 `cargo test -p msime-engine --lib session::tests::cloud_answer_replaces_its_fixed_pending_seat`、`cargo test -p msime-input-runtime --lib --no-default-features`（318 项）和 `cargo check -p msime-host-api --lib`。
