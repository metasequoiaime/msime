# Agent Note: 云词库回车查询遵守组合输入和禁用状态

Status: implemented

## Problem

`CloudDictionaryQueryToolbar` 被个人云词条、完整目录和云端候选排序三个面板共用。输入框仅判断 `Enter` 就调用查询，中文组词确认可能提前查询尚未提交的搜索内容，查询按钮因 `busy` 或 `queryDisabled` 禁用时回车仍调用回调。

## Decision

共享输入框先判断回车、`busy`、`queryDisabled`、`nativeEvent.isComposing` 和 `keyCode === 229`。只有查询可用且回车不属于组合输入时调用回调。不阻止组合事件的默认行为，让输入法完成上屏。

## Alternatives considered

在三个调用面板的查询函数里判断组合状态能把控制集中在业务入口，但回调没有键盘事件，无法区分用户确认组词和明确查询；复制三份事件处理又会让共用组件的行为漂移。使用共享组件已有的禁用状态并在事件入口判断组合标记。

只检查 `isComposing` 更简单，但组合结束与最后一个键盘事件的顺序可能使标记提前变为 `false`。保留仓库其他输入框使用的 `keyCode === 229` 兼容判断，依据 [MDN 的组合输入事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。

## Testing

旧实现的四个回归用例均失败，均为回调被错误调用；组件测试覆盖两类组合回车和两类禁用状态不调用查询；结束组合或恢复启用后普通回车仍查询。`pnpm --filter @msime/desktop test` 指定查询栏及三个相关面板测试文件，19 项通过；前端类型检查和构建、改动文件 lint 与格式检查、笔记检查与本地 quick 门禁通过。quick 包含宿主 Rust 工作区、Android 目标编译和 Apple 桥接编译；Linux 容器、Windows 交叉编译和 HarmonyOS 原生打包因工具链未配置而跳过。本次验证为组件行为测试，没有宣称原生设备验收。

## Consequences

这次改变共享输入框的回车行为，不改变三个面板的查询协议。组词确认后需要一次独立回车或点击按钮来明确查询。

## 旧笔记归属

检索 `cloud-dictionary-query`、云词库查询与组合输入后，仅命中 Windows 屏幕键盘功能笔记；它讨论平台面板入口，与此事件约束无关。组合与回车的其他笔记讨论引擎和宿主按键，不归并到本篇。

[字体下拉导航](2026-10-14-font-family-ime-navigation.md) 部分重叠：使用相同的组合事件判断，但方向键、Esc 和字体选中状态由字体控件管理，查询栏只管理查询提交。
