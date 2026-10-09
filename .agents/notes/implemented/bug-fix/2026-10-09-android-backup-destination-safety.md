# Agent Note: Android 备份导出保护已有目标

Status: implemented

## Problem

`LocalBackup.export` 使用 `ACTION_CREATE_DOCUMENT` 返回的 URI 写备份。文档提供方可能返回已有文档；代码用 `openOutputStream(uri, "wt")` 截断目标，并在复制失败时把 `written` 提前设为真，随后 `discardDocument` 会删除 URI。已有备份可能因此被截断后删除，导出失败造成数据损失。

## Decision

导出先以读流确认目标为空，再打开写流复制已完成的本地归档。非空目标在写入前拒绝，任何写入失败都保留提供方文档，不再自动删除。新建空文档若写入失败可能留下不完整文件，界面提示用户手动删除；这比删除用户原有备份安全。

## Alternatives considered

- **继续按字节数判断并在失败时删除**：能清理选择器新建的空文件，但无法区分空的已有文件，也会在写入失败后误删已有备份；因此不采用。
- **先写入临时 SAF 文档再重命名替换**：理论上可以避免部分写入，但 `ACTION_CREATE_DOCUMENT` 只给单个文档 URI，父文档和提供方的创建、移动、重命名能力并不稳定；引入提供方相关分支不能保证跨文件选择器安全，因此不采用。
- **把已有内容复制到应用缓存后失败时恢复**：仍依赖失败后的第二次写入，第一次截断或提供方故障时无法保证恢复，且会复制用户文件内容；因此不采用。

## Consequences

导出不会覆盖或删除非空目标，部分写入失败也不会触发删除。用户若选择空目标且写入中断，可能看到一个残留的不完整文件，需要手动删除后重试；成功导出路径和备份格式不变。

## Verification

`platforms/android/tests/settings/BackupDestinationWriterSmoke.java` 先证明非空目标不会打开写流且原内容不变，再证明空目标能接收归档；`bash platforms/android/check-host.sh` 通过（188 个 Android JVM 冒烟、55 个设备套件源编译、资源编译）。另运行 `git diff --check`、Gradle Android Java 编译和 Android API 35 专用模拟器原生宿主验收。
