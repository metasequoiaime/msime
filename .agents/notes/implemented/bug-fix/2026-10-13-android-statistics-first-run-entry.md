# Agent Note: Android 统计页提供首次开启入口

Status: implemented

## Problem

打字统计默认关闭且首次没有统计文件时，`StatisticsFragment` 的模型为空，右上角菜单直接返回，页面只有「还没有记录」提示。用户看不到开关，也无法从统计页开始记录，表现像统计功能损坏。

## Decision

空统计页直接显示「统计记录」卡片和「开启」按钮。按钮调用共享统计存储的 `set_enabled(true)`，成功后沿用现有模型刷新和统计菜单；默认仍保持关闭，不在用户未同意时记录输入。

## Alternatives considered

把统计默认改成开启能消除空状态，但会改变隐私同意边界，因此保留关闭默认。只在右上角菜单增加入口仍要求用户猜菜单位置，不能解决首次页面没有开关的问题。

## Consequences

首次用户能在当前页面明确开启统计，说明文字解释数据从下一次输入开始累计且只保存聚合计数。已有统计或读取失败路径保持原来的右上角菜单和刷新行为。

## Verification

Android host Java 检查、quick 门禁和签名 APK 构建通过；统计页使用现有 `HostStore.setStatisticsEnabled` 跨进程写入路径。
