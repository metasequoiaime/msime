# Agent Note: 修复共享界面的无效折行工具类

Status: implemented

## Problem

共享设置页、聊天、社区内容和键盘面板在 28 处用 `break-anywhere` 表达“长连续文本在容器内折行”。当前 Tailwind 不生成这个工具类，生产 CSS 没有对应规则。连续的路径、标识符或候选文字仍按 `overflow-wrap: normal` 排版，可撑出卡片或被裁掉。关于页系统信息的同类现象曾由独立改动修复，但其余位置仍保留无效类名。

## Decision

把共享 UI 源码中的 `break-anywhere` 全部换成 Tailwind 提供的 `wrap-anywhere`，包括直接作用在元素上的类和后代选择器变体。此工具类生成 `overflow-wrap: anywhere`，并让浏览器在计算 flex 项目的最小内容宽度时考虑单词内断行。现有尺寸、最大高度与滚动约束保持原样。

## Alternatives considered

- **继续使用 `break-anywhere`，再添加同名自定义 CSS**：改动文件少，但把不存在的 Tailwind 名称固化成项目私有约定，后续样式入口还要保证这条全局规则被加载。
- **逐处改成 `[overflow-wrap:anywhere]`**：效果正确，关于页已经这样使用；28 处重复任意属性比已有官方工具类更难辨认，也会让同一意图在源码中出现两种写法。
- **改成 `break-all`**：可强制长词折行，但会在无需折行时也允许词中断开，语义不如 `overflow-wrap: anywhere`。

## Verification

生产 CSS 在修改前没有 `.break-anywhere` 规则；180px 合成 flex 行中的连续文字把滚动宽度撑到 493px，浏览器计算样式为 `overflow-wrap: normal`。改用 `wrap-anywhere` 后，计算样式为 `anywhere`，文字分成 3 行，滚动宽度回到容器的 178px。桌面设置页生产构建与聊天、社区、系统信息的 18 项测试通过。

## Consequences

长连续内容会在可用宽度内折行，涉及聊天气泡、社区说明、文件路径、诊断文本和键盘面板等原本写有折行意图的位置。内容高度可能增加；已有最大高度或滚动容器继续限制对应区域。这个改动不改变字符串内容、复制结果或输入法算法。
