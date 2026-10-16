# Agent Note: 鸿蒙设置页选择和编辑九键双拼方案

Status: implemented

## Problem

#6572 的九键双拼已经接入鸿蒙键盘，但共享设置页只能选择布局，不能选择米旮旯 2、辜氏 2 低重码或编辑自定义表。Android 与 iOS 已有这条用户路径。

## Decision

鸿蒙的共享输入页在选择九键双拼后提供内置方案选择和自定义表编辑、JSON 导入导出。复用现有 26 键编辑器的异步操作、草稿及文件处理，只参数化声母集合、数字规则、内置模板和说明。表的合法性与内置映射经既有 `msime_client_nine_key_shuangpin_layout` 计算，ArkWeb 桥只转发 JSON，不复制引擎规则。

只写 `nine_key_shuangpin_profile` 与 `nine_key_shuangpin_custom_profile`，不覆盖 26 键双拼表。无桥接能力的宿主不显示入口。非法导入保留原草稿；异步校验未完成时不能保存或导出。

## Alternatives considered

- 单独复制一个九键编辑器：隔离 26 键行为最直接，但会复制已有的异步过期保护、文件上限、确认与导出流程；参数化已有编辑器能共用这些保障。
- 在 React 中复制三套内置表与数字校验：省一次桥接，但方案表与运行时可能漂移；既有无状态 C ABI 已同时提供模板和错误，无需再维护一份。

## Verification

设置、编辑器与触屏方案选择相关回归 80 项通过，包含原 26 键编辑器 27 项；新增九键回归覆盖模板、23 声母、0 键、非法导入不覆盖草稿、导出文件名、异步过期结果、设置保存只改九键字段和能力缺失。桌面及鸿蒙类型检查、鸿蒙网页构建、桥接契约检查、完整 HAP ArkTS 编译通过。HAP 复用 #7835 经 #7836 保留的生成原生库与固定词库资源，本片没有改 Rust/NAPI；未重新构建这些原生库。当前无可调用的鸿蒙设备桥，未宣称设备或系统键盘验收。

本地 `verify-local.sh --quick` 的契约、笔记、Rust 工作区、Android 目标及鸿蒙 HAP 检查通过；Windows 交叉构建期间磁盘剩余空间低于 512 MiB，资源守卫停止本轮进程组，完整 quick 未通过。Android Java 宿主按改动范围跳过，Linux 设置外壳因 Docker 不可用跳过，不能将这些项目计为通过。

## Consequences

鸿蒙可以选择和配置数字表，26 键表独立保留。两种编辑器共用文件上限、草稿、异步过期保护和宿主导出，维护参数定义的成本小于复制整套交互。保存沿用已有自动保存流程和共享偏好通知，不改变鸿蒙沙箱契约；真实设备与安装验收仍待完成。

## Existing notes audit

- [九键双拼](../../implemented/feature/2026-10-10-nine-key-shuangpin.md)部分重叠，继续持有编码、来源和 C ABI，本篇补齐鸿蒙设置入口；不增加辜氏 2 普通版的内置表。
- [自定义双拼](../../implemented/feature/2026-10-10-custom-shuangpin-profile.md)部分重叠，复用编辑器操作，不混用 26 键与数字表的校验。
- [iOS 九键双拼](../../implemented/feature/2026-10-16-ios-nine-key-shuangpin.md)部分重叠，保持三移动端表格式和校验一致。
