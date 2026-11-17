# Agent Note: Linux 标准首选项入口与设置窗口匹配

Status: implemented

## Problem

#11437 报告 GNOME 与 IBus 的首选项入口打不开设置，而直接运行 `msime-linux-settings` 正常。IBus 组件没有 `setup`，安装目录也没有 GNOME 按引擎名查找的 `ibus-setup-<engine>.desktop`。报告者观察到 Wayland 设置窗口的应用标识为 `msime-linux-desktop`，已有桌面文件名却是 `msime-linux`，窗口缺少 dock 图标关联。

## Decision

IBus 组件的 `setup` 指向现有设置启动器；有设置壳的构建同时安装隐藏的 `ibus-setup-<engine>.desktop`，两个入口复用同一启动器，不新建设置进程协议或窗口。已有桌面文件和隐藏入口都声明 `StartupWMClass=msime-linux-desktop`，匹配现有设置壳的窗口标识。

按既有 `edition_linux.py` 生成六个版本的首选项入口，命令、图标、名称与 IBus 引擎名跟随各自版本，设置壳可执行文件名及窗口标识保持现状。没有桌面设置壳的构建不安装隐藏入口，与已有应用入口的安装条件一致。

本轮检索：Linux 模式提示与菜单对齐笔记只涉及输入法菜单，不持有桌面入口协议；Linux 原生构建门禁笔记规定验证覆盖面，不规定产品入口，均不改。各版身份生成沿用现有机制，本篇只补标准首选项入口与窗口匹配。

## Alternatives considered

- **直接统一设置壳应用标识与桌面文件名**：能从源头匹配 Wayland 窗口，但需要同时处理多个版本的窗口身份、单实例及面板路由。现有窗口标识已被用户观测，本次用桌面集成元数据修复，不改变运行时身份。
- **只补组件 `setup`**：改动最少，IBus 设置工具可用；但 GNOME 的输入源首选项使用另一份桌面入口，仍然打不开，因此两个入口都补齐。

## Consequences

从系统输入源和 IBus 首选项都能到达现有设置启动器，不增加应用列表里的重复图标，不改偏好默认值。`StartupWMClass` 复用当前各版本相同的设置壳标识，多版本同时运行时的独立窗口身份仍是既有局限。

## Verification

`scripts/test-linux-editions.py` 检查六个版本的模板生成和身份隔离；`platforms/linux/tests/core/ibus_component_contract.py` 检查既有 IBus 描述契约。生成后的 XML 与 desktop 条目分别解析，核对引擎、命令、图标及隐藏属性。真实 GNOME/IBus 首选项点击与 dock 关联尚需 Linux 桌面验收，不把元数据校验当作桌面实测。
