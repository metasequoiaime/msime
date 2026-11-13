# Agent Note: 移动端普通 26 键自定义双拼

Status: implemented

## Problem

共享偏好和 Engine 已经支持 `shuangpin_custom_profile` 及逐项校验，但移动端只有九键双拼的自定义页面。Android、iOS 和 HarmonyOS 无法在「语言与方案」里编辑、导入、导出或切换普通 26 键自定义表；键盘收到 `custom` 时也无法从共享层取键位提示。

## Decision

共享 `host-api` 提供无状态的 `msime_client_shuangpin_custom_layout` 接口，输入方案名和可选表，返回实际生效方案、规范表、带位置的校验问题以及字母键提示。自定义表不合法时按小鹤运行，和会话创建时的既有回退一致；接口不落盘，编辑器每次修改都可以调用。

Android 原生设置页和 iOS 原生设置页都直接使用这份接口完成编辑、导入、导出与保存。HarmonyOS 设置桥暴露同一计算接口，继续复用共享设置页已有的自定义双拼编辑器。所有平台仍通过 `PreferencesStore` 保存 `shuangpin_profile` 和 `shuangpin_custom_profile`，不改变内置方案或默认值。

## Alternatives considered

直接在 Android、iOS 或 ArkTS/React 中复制 Engine 的缺失韵母、重复按键和零声母规则，能少改 C ABI，但会产生三套会漂移的校验和回退行为，和现有九键双拼编辑器的宿主契约冲突，因此不采用。

## Consequences

设置页和键盘提示共享同一个表及问题格式，导出的 JSON 可在 Android、iOS 与 HarmonyOS 间互用。新增了跨平台 C ABI 和原生文件编辑入口；HarmonyOS 真机、iOS 真机和 Android 设备上的完整输入验收需要设备条件，当前只完成代码级、共享接口和宿主静态验证，PR 验证段必须保留这些缺口。
