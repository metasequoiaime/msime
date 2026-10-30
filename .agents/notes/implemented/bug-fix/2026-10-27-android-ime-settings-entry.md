# Agent Note: 在系统输入法列表中提供水杉设置入口

Status: implemented

## Problem

Android 的系统输入法列表没有水杉的设置入口。六种发行版的 `method.xml` 都没有声明 `android:settingsActivity`，系统通过 `InputMethodInfo.getSettingsActivity()` 读到空值，因此不会给这一项提供打开应用设置页的动作。#9916 同时报告启动卡顿和耗电；这两项与入口缺失是独立现象，不能由本改动解决。

## Decision

所有 Android 发行版的 `<input-method>` 元数据都指向现有的 `app.msime.android.home.HomeActivity`。该 Activity 在各发行版的清单中已导出，系统按输入法包名和此完整类名构造显式 Intent；打开后由既有首页提供设置。各发行版统一声明，避免拼音版修好而其他版本仍缺入口。

## Alternatives considered

- **只给首页增加一个 Intent filter**：可以让外部应用通过动作找到首页，但 Android 系统输入法列表读取的是 `<input-method>` 的 `settingsActivity` 字段，单独改首页的 filter 不会补上设置入口，因此不用。

## Consequences

支持该入口的系统设置页可以直接打开水杉应用。设备厂商仍可自行决定设置行的呈现形式；本改动不改变输入法进程常驻策略、首次唤起时延或电池消耗。

## Verification

逐个解析六份 `method.xml`，确认 `settingsActivity` 指向已导出的首页；运行 `bash platforms/android/check-host.sh` 和 `pnpm run verify-notes`。本机磁盘低于新构建门槛，尚未在 Android 模拟器或报告者的 vivo 系统设置页验收入口显示。
