# Agent Note: 移动端补齐多种双拼方案

Status: implemented

## Problem

#11343 要求 Android、iOS 和 HarmonyOS 的移动端输入方式增加搜狗、加加、紫光、智能 ABC、国标双拼。原有共享 Engine 只有智能 ABC 的部分支持，移动端入口只暴露小鹤、自然码、微软和首道；各宿主自行补表会造成键位、同步和统计不一致。

## Decision

把五套方案作为共享 Rust 的 `ShuangpinProfileKind` 追加值，并由共享 Engine 保存唯一的声母、韵母和零声母映射。智能 ABC 复用已有共享表；搜狗、加加、紫光、国标按公开方案说明独立转录为静态表，不把第三方配置文件复制进仓库。ABI 序号只追加，既有 0–5 保持不变。

Android、iOS 的原生设置追加对应触屏入口；三端键盘与共享偏好保留标题、短名、键位提示和统计来源。HarmonyOS 尚无原生设置页，撤回共享设置新增的可选入口，相关 PR 保持 Draft；共享页仅回显已经保存的新增 profile，并保留原生端已启用的触屏方案，不改写文档。搜狗、紫光与微软一样显示 `;` 韵母键；其余新增方案沿各自映射显示普通 26 键。新入口加入 Android、iOS 原生可选列表，不加入新装或旧文档的默认启用集合，用户显式打开后才显示。账号同步为四个新增云端双拼 profile 增加字段；智能 ABC 仍按已有本地兼容规则处理。

## Sources

搜狗、加加、紫光、智能 ABC 的键位参考公开 Rime schema；国标双拼参考公开键位表后独立转录。仓库只保存声韵母到键的结果和测试断言，不把第三方 schema 文件当作运行时输入或信任根。

## Alternatives considered

- **只在 Android、iOS 或 HarmonyOS 宿主内写映射**：会让同一份偏好在不同平台产生不同编码，也无法让桌面或账号同步识别新 profile，因此不采用。
- **只增加可编辑的自定义表入口**：不能满足用户直接选择常见方案的需求，且键位提示和统计无法稳定命名，因此不采用。
- **把新增方案加入默认启用列表**：会改变现有用户的快捷方案顺序和新装键盘数量；本 issue 只要求能力可达，故保留默认值并让用户显式启用。

## Consequences

共享偏好、Android/iOS 原生入口、账号 schema、键位提示和统计来源都认识五套新方案；HarmonyOS 原生选择入口尚缺，旧 profile 的序号和默认行为不变。新方案默认关闭，启用后三端使用同一套 Engine 映射。搜狗和紫光的分号键需要宿主显示与路由，其他方案不增加专用键。

## Verification

- `cargo test -p msime-engine shuangpin::profile --lib` 通过，覆盖五套表的选择、关键映射和分号键规则。
- `cargo check -p msime-client-core -p msime-client-services -p msime-host-api` 通过。
- 共享设置页只保留已有方案的可选顺序，并覆盖原生已存 profile 的禁用回显、保存保留和已启用触屏方案的展示；触屏方案回退与统计中文聚合测试保留；统计聚合包含四个新增云端方案，默认启用列表仍保持旧值。
- Android、HarmonyOS 真机/HAP 和 iOS 模拟器验收尚未在当前环境完成；PR 保持 Draft，并在验证段列出这些缺口。
