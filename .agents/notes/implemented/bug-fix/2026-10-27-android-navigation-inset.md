# Agent Note: Android 输入法弹出时隐藏详情导航

Status: implemented

## Problem

部分 Android 11 厂商设备在输入法已经占用窗口底部时，`WindowInsetsCompat.isVisible(Type.ime())` 仍返回 false，导致详情页的分类导航栏显示在弹出的键盘上方。

## Decision

详情页固定导航同时使用输入法可见标志和输入法底部 inset 判断。只要任一信号表明输入法占用底部空间，就隐藏导航；两个信号都表示未占用时才恢复。策略保留原来的单参数接口，并为窗口接线提供带 inset 的重载。

## Alternatives considered

- 只依赖 `isVisible(Type.ime())`：报告者的 Android 11/vivo 设备已经证明该信号可能滞后，因此不能解决问题。
- 依据系统导航栏 inset 判断：系统导航栏与输入法占用区域不同，无法区分键盘弹出，不采用。

## Consequences

厂商延迟更新 IME 可见状态时，详情页导航仍会及时收起，键盘收起且 inset 清零后恢复。已增加 JVM 策略断言；未在报告者 vivo Android 11 真机上验收，需合并后复验。
