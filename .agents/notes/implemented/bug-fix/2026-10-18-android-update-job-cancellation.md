# Agent Note: Android 自动更新任务正常接收下载取消

Status: implemented

## Problem

系统调用 `UpdateJobService.onStopJob` 会中断更新线程。`UpdateApi.download` 的进度回调检测中断后抛出 `CancellationException`，下载器完成临时文件清理后继续抛出，但任务只捕获 `UpdateApi.Failure`。取消因而成为未捕获异常；Android 的默认未捕获异常处理器会终止应用进程。

平台依据：[JobService](https://developer.android.com/reference/android/app/job/JobService) 要求在 `onStopJob` 后停止任务；[AOSP RuntimeInit](https://android.googlesource.com/platform/frameworks/base/+/android12-release/core/java/com/android/internal/os/RuntimeInit.java) 的 `KillApplicationHandler` 终止出现未捕获异常的应用进程。产品的遥测异常处理器仍会转交平台处理器。

## Decision

在任务工作线程内单独捕获 `CancellationException`，把系统取消作为正常停止，不发送安装通知，也不套用网络错误的重试判断；调度器仍根据 `onStopJob` 返回的 `true` 决定重新调度。

## Alternatives considered

- 在 `UpdateApi` 内吞掉取消：所有调用方都不用捕获，但下载器没有合法安装文件可以返回，而且关于页也依赖异常中止下载；取消应由任务入口接收。
- 捕获所有 `RuntimeException`：能防住取消，但会同时隐藏程序错误。仅接收明确约定的取消异常。

## Verification

`python3 scripts/test-android-update-job-cancellation.py` 从产品源码提取真实启动、停止方法并编译运行，以同步信号控制下载停止；修复前因取消逃出线程失败，修复后通过。取消不逃出线程、不通知，成功通知和网络错误退避保持原样。测试不联网、不读写安装包。

## Consequences

主机测试替换 Android 平台及更新 API 边界，只验证任务的控制流，不代表真实系统调度或 APK 安装验收。与[关于页视图生命周期](2026-10-11-android-about-update-view-lifecycle.md)部分相关，但该笔记处理页面状态，本笔记处理后台任务异常，两者保留各自归属。
