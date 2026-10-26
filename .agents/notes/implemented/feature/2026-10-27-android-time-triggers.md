# Agent Note: Android 时间触发词

Status: implemented

## Problem

Android 的时间指令只有日期、时间、星期和农历。用户输入季度、年内天数、倒计时、年内周数或生肖时，没有对应的动态候选。

## Decision

在引擎的内置日期时间指令中增加 `jidu`、`tianshu`、`daojishi`、`zhou`、`shengxiao` 和 `shu` 六个触发词。结果由传入的本地日期计算：季度显示年份和季度；天数与倒计时显示已完成天数及包含当天的剩余天数；周数使用 ISO 周；生肖按公历年份的地支计算。引擎、无状态 backend 和 client-services 的内置触发词契约保持同步。

## Alternatives considered

- 只在 Android 宿主里拼接文本：会让共享引擎、桌面宿主和无状态接口的指令行为不一致，因此不采用。
- 把六项合并为一个通用模板：无法表达周数和生肖的计算规则，也会削弱已有触发词的精确过滤，因此不采用。

## Consequences

新增触发词可在所有使用引擎日期时间查询的宿主使用，并通过 backend 接口获得相同结果。结果依赖调用方传入的本地日期，闰年和年末周数按日历计算。Android 真机设备验收未执行；已运行 Rust 单元测试、client-services 编译和仓库笔记门禁。
