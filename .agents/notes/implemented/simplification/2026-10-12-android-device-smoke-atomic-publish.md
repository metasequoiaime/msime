# Agent Note: Android 设备验收共用原子发布方法

Status: implemented

## Problem

16 个 Android 设备验收类各自复制同一段 `AtomicFile` 写入和失败回滚代码。以后修改测试偏好的发布方式时，逐处同步容易遗漏。

## Decision

`DeviceSmoke.publish` 统一执行原子写入和失败回滚，子类继承并调用它。该方法为 `protected final`，避免设备验收类再次覆盖成不同语义；本次不改变测试准备的内容、调用顺序或恢复步骤。

## Alternatives considered

保留各类私有方法可以让每个测试文件独立阅读和修改，也无需触碰共同基类。但 16 处实现完全相同，而且所有类已经继承 `DeviceSmoke`，继续复制会让失败回滚逻辑产生分叉。

## Consequences

原子发布只有一个实现，测试类删去各自的重复方法和无用导入。共同基类新增一个受保护方法，今后若某项设备验收需要不同的写入策略，应明确另起方法，而不是修改所有测试的共享语义。Android Java 编译用于验证所有调用点；本次没有改变产品宿主的文件写入代码。
