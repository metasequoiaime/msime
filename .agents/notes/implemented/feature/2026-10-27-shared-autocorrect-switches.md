# Agent Note: 共享设置页拆分全拼纠错开关

Status: implemented

## Problem

共享输入设置页把全拼邻键纠错和全拼乱序纠错合并为一个开关。底层偏好本来就是两个独立字段，因此用户无法只关闭会把 `zhr` 判成 `zhe` 的邻键纠错，同时保留乱序纠错，或反过来。

## Decision

在 HarmonyOS 手机共享输入页分别显示“全拼邻键纠错”和“全拼乱序纠错”，各自只更新对应的 `quanpin` 字段，并保留默认开启行为。

## Alternatives considered

继续保留合并开关：实现简单且与 Android 页面一致，但无法满足已有独立偏好字段的可配置性，因此不采用。

## Consequences

用户可以独立关闭两类全拼纠错，设置保存后宿主仍沿用原有字段。桌面共享设置页的同一对开关放在「高级选项」的「全拼纠错」组，`qjn`、`zhr` 这类全辅音简拼被纠错抢首位的问题由引擎修复，见[全辅音简拼的字面整词排在全拼纠错前面](../bug-fix/2026-10-29-quanpin-jianpin-over-autocorrect.md)。
