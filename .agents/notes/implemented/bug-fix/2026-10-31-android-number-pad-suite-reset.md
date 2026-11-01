# Agent Note: Android 数字九宫格套件清理遗留符号面板

Status: implemented

## Problem

`Native Platform CI` 在 develop 的 API 28 和 API 35 设备验收中，`NumberPadDeviceSmoke` 进入英文数字九宫格用例时仍停在前一个套件留下的符号面板。测试等待语言键，实际无障碍树只有符号面板，导致数字九宫格用例超时；后续候选、标点和十四键套件继续运行时也继承了错误的键盘状态，形成连锁失败并打开 ci-red #10421。

## Decision

`NumberPadDeviceSmoke.openEditor` 在启动合成编辑器并聚焦文本框后，在每次等待语言键面时轮询当前输入法窗口；若异步重绑期间出现「关闭符号面板」就点击返回条，直到目标字母键面可见后再开始断言。清理只放在该设备套件的编辑器入口和语言键等待，不改变产品运行时的面板生命周期，也不修改公共 `DeviceSmoke` 启动流程。

## Alternatives considered

- **修改公共 `restartIme`，强制停止输入法进程。** 能更彻底地清理状态，但会改变所有设备套件的启动时序，重新引入 Android 9 绑定延迟和跨套件副作用，影响范围超过 #10421。
- **让 `smoke.sh` 在每个 instrumentation 前统一关闭面板。** 设备套件的面板状态由测试自身创建，脚本无法可靠访问无障碍节点；把 UI 清理塞进 shell 层也会掩盖各套件未恢复状态的问题。
- **只增加等待时间。** 等待不会把符号面板变成字母面，仍会超时，不能修复失败原因。

## Consequences

数字九宫格套件从任意前置键面进入时都会先恢复到字母层，即使符号面板在输入法异步重绑后才出现也会被清理。API 28 和 API 35 的后续套件不再因首个语言键而连锁失败。测试多执行无障碍轮询和可能的一次返回点击；轮询每拍先清 UiAutomation 缓存（API 34+ 的 `find` 不再逐节点刷新），关闭键只点本拍查到的节点且不阻塞，避免读到已拆掉的面板后空等 15 秒误报。生产代码与用户可见行为不变。PR 阶段只跑 API 35 设备任务（API 28 的 `android-device` 在 PR 上跳过），API 28 以合并后 develop 的设备任务为准；develop 上 API 28 在 `verifyPasswordNumberPad` 的密码数字框（inputType 18）失败是另一个问题，本篇不覆盖。主机检查和笔记门禁照常保留。
