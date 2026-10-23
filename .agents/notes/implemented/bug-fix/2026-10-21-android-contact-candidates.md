# Agent Note: Android 联系人计数与姓名候选

Status: implemented

## Problem

#9000 中，联系人导入后设置页一直显示零条，普通拼音输入也找不到姓名。设置页把 `load_mentions` 的数组 `value` 当成带 `entries` 的对象读取，读取失败又显示成零条；共享宿主仅在 `@` 名单模式开启时装载联系人。已有引擎只生成手机号候选，和姓名第一、姓名加手机号第二的要求不符。

## Decision

设置页严格读取数组，失败时明确显示读取失败。共享宿主独立于 `@` 开关读取本机名单，名单模式的触发规则保持原样。普通全拼与双拼组字匹配含手机号的本地联系人，生成相邻的姓名、姓名加手机号两行，使用 `Contact` 来源替换字典中的同名行。完整命中先于前缀命中；联系人在混排与固定位置之后置顶，引擎与宿主在线座位整理均保留开头的完整联系人组，异步云候选、AI 行及固定英文都不会插入姓名与手机号之间。联系人快照不向句子重排器提供候选文本。编辑光标前缀时不生成会结束整段组字的联系人候选。选择联系人整段上屏，不进入调频、造词、释义或个人上下文。名单更新时刷新正在组字的普通候选。

Android 普通条与展开面板共用右上角小人角标，文字保持原始姓名或组合内容，无障碍描述报告联系人来源。装饰候选绘制复用同一份转换后的候选文字，整合时不能引用已移除的旧标签变量。导入、刷新、清空保持显式操作，输入进程只读本地快照，不查系统电话本。

## Existing note audit

- [原联系人导入决定](../feature/2026-10-17-android-contacts-import.md)：部分取代其手机号单行候选与装载条件，保留显式权限、本机快照与共享存储的决定，双方互链。
- [插件文件戳](2026-10-11-plugin-table-stamp-inode.md)：部分重叠，保留修改时间、长度、inode 检查。
- [名单目录锁](2026-10-09-mentions-lock-directory.md)：存储边界相关，锁与原子保存不变。
- `2026-10-15-scoped-file-lock-release`、`2026-10-13-host-api-cloud-disable-invalidates-candidates`：无关，不修改锁租约或云开关的失效规则。

## Alternatives considered

在 Android 宿主直接生成两行可减少 Rust 改动，但会复制匹配排序规则，展开面板与分页候选的身份也无法统一，因此交给引擎生成完整候选。保留文字前的小人标记可复用旧显示方法，但不满足右上角角标要求，因此普通条与展开面板均独立绘制角标。

## Consequences

联系人明确占据最高优先级，会把原本的固定位置和云候选向后移动。名单仍与 `@` 共享，普通候选只读取带手机号的行；不修改磁盘格式和权限请求时机。模拟器验收不能证明 vivo Android 11 真机的电话本权限行为。

## Verification

引擎单测 2377 项、golden 31 项通过，运行时单测 327 项通过；覆盖全拼、简拼、双拼、同名词典去重、云候选回包、两行顺序、完整上屏、不学习，以及名单刷新、清空和光标前缀边界。原始代码在新增引擎、宿主装载和运行时分组用例上均失败，修复后对应用例通过。三个修改的 Rust crate 的 clippy 通过。

Host API 单测 910 项通过、5 项忽略；`vocabulary_boundary_imports_reviews_and_reports_one_whole_status` 的明日到期数量预期 2、实际 3。同一失败在恢复本任务修改前的基线上复现，不增加已知失败豁免。

Android API 28、35 的完整版与拼音版原生安装包包含本分支编译的 Rust/JNI 库，四组联系人专项均通过：真实电话本 provider 写入合成姓名与保留号码，实际点击导入、刷新和清空，核对计数、JNI 的姓名与组合来源、普通候选条与展开面板的完整上屏和右上角标，退出恢复名单和偏好。`UiAutomation.executeShellCommand` 不解释 shell 引号，合成姓名直接作为无空格参数传递；更新与清理使用单参数 SQL 条件，避免把额外引号写进姓名后得到无效的拼音验收。

两个 API 上的 `build-core-test.sh` 均通过，覆盖 client-base、client-core、client-providers 和 client-services 在应用沙箱中的存储测试，沿用脚本规定的忽略与过滤。`scripts/verify-local.sh --quick` 通过，包括 Android Java/JVM/NDK、Rust/WASM、Linux 原生 94 项和 macOS 原生编译；Windows 与 HarmonyOS 因对应工具链不可用跳过。
