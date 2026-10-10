# Agent Note: 隐藏未选中的 Linux 辅助码插件菜单项

Status: implemented

## Problem

IBus 的辅助码方案菜单为了支持热更新，始终注册了一个 `HelpcodeSchemaPack` 子项。提交只把没有插件时的子项设为不可用；IBus 的不可用状态仍会显示菜单项，因此未选择插件时用户会看到没有名称的「插件：」空项。

## Decision

保留固定的子项键，继续让 IBus 在插件切换后通过 `update_menu_property` 更新它，但按当前插件是否为空设置 `ibus_property_set_visible`。没有插件时隐藏，有插件时显示并勾选；敏感性仍负责焦点、阻塞和保存期间的可操作状态。

## Alternatives considered

- 只保留 `sensitive=false`：它会阻止点击，却不会从 IBus 菜单隐藏空项，正是原缺陷。
- 在没有插件时不注册子项：插件由设置页热切换后，IBus 只能等下一次重新注册才出现标记，当前会话无法正确反映生效状态。

## Consequences

未选插件的 Linux IBus 菜单不再显示空的插件项；插件被选中或清除时仍使用同一个已注册键动态更新可见性，不改变 Fcitx5、Engine 或偏好持久化行为。

## Verification

先在未修复实现上运行 `python3 platforms/linux/tests/core/fcitx5_contract.py`，新增契约断言按预期失败；加入可见性更新后该检查通过。运行时烟雾测试新增断言，实际 Linux/IBus 容器门禁因本机 Docker daemon 未启动而未能执行。
