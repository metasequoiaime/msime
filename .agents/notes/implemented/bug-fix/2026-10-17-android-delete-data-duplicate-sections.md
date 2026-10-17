# Agent Note: Android 云端数据删除拒绝重复分类

Status: implemented

## Problem

Android `DeviceDataApi.deleteData` 只检查分类都属于允许集合，没有检查列表内重复项。共享 Rust 账号客户端的 `valid_data_sections` 要求非空、只出现一次；因此同一调用契约在 Android 上会把明显无效的请求发到服务端，才收到失败，且不同宿主的本地参数行为不一致。

## Decision

Android 在发送请求前执行“允许集合且不重复”的判定，并用冒烟测试固定重复分类被拒绝、唯一分类仍被接受的边界。`deleteData` 与公开的 `validDeleteSections` 共用这条规则。

## Alternatives considered

- 继续依赖服务端拒绝：实现改动最少，但会产生一次无意义网络请求，并保留 Android 与 Rust 的契约漂移。
- 只在 `ProfilePage` 构造固定列表时去重：不能保护公开的 `DeviceDataApi` 调用，也不能保证其他宿主调用路径遵守契约。

## Consequences

- `deleteData` 对空列表、未知分类和重复分类统一在本地抛出 `IllegalArgumentException`，唯一的允许分类列表仍正常发送。
- 只收紧本地参数校验；合法请求的 JSON 和网络路径不变。
- Android JVM 冒烟覆盖唯一、重复、未知和缺失分类，`check-host.sh` 通过并编译了设备套件；API 28 模拟器上的四个 Rust client-core 测试二进制也全部通过。
