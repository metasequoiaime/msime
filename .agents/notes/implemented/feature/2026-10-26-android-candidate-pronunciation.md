# Agent Note: Android 候选显示英文音标

Status: implemented

## Problem

Android 已有共享偏好 `candidate_pronunciation` 和离线查询 FFI，却没有设置入口、读音查询和绘制。发布包也没有 `en-phonetic.db`，直接接上 FFI 仍只会得到空结果。

## Decision

沿用 `resources/pronunciations.lock.json` 固定的 ECDICT 表及许可声明，把表装在 APK 的 `pronunciations/`，由 Bootstrap 解到活动 `resources` 的同级目录。表大小为 5,718,016 字节，NOTICE 为 1,275 字节，来源提交与两个产物的 SHA-256 均在锁中。构建只从锁定产物取文件，按键过程不下载。

设置页增加默认关闭的「显示英文音标」。宿主在开关开启且表存在时，从同一代候选中收集英文候选及英语释义的首个词组，在现有候选工作线程上调用 `msime_client_pronunciation_request`，只接纳当前会话及候选代次的回答。音标在候选的次要文字中独占一行，横向条限制单个按钮宽度并省略过长的次要文字，展开网格按格宽裁切。关闭后不查表、不挂音标、恢复原按钮布局。表缺失时设置页明确显示离线资源不可用。ECDICT 表只有词面到单个宽式音标的映射，不含义项或词性；多音词沿用共享层的固定读音，无法按释义消歧，界面说明不暗示消歧。

## Alternatives considered

直接复用背单词的 ECDICT 词书能省掉约 5.7 MB 包体。但词书 JSON 按课程拆分且没有供候选批量查询的 IPA 契约，复用它会在每次候选刷新时扫描无关数据，也会让两套 ECDICT 音标规则不一致；因此沿用桌面端的共享表。

## Verification

- 合成策略测试先因缺类失败，接入后通过；覆盖英文候选、英语释义、非英语目标和关开关、缺资源。
- `bash platforms/android/check-host.sh` 通过：JNI aarch64 交叉编译、280 个 JVM 冒烟测试、245 个设备测试源编译；此检查不产生 APK。
- `python3 scripts/fetch_gloss_tables.py --out target` 实取音标表与 NOTICE，两个 SHA-256 与锁一致；只读查询确认 `hello`、`world`、`love` 有宽式音标。
- `pnpm run verify-notes` 通过。完整 Android 模拟器装包、横向条和展开面板视觉验收仍待设备窗口。

## Consequences

Android 每版 APK 增加约 5.7 MB 数据库；瘦包的 135 MB 上限待实际 APK 构建核对。打开音标时，候选行增加一个次要文字行，最长按钮宽度限制在 240 dp；关闭后使用原布局。离线表不存在时设置页直接显示资源缺失提示，输入时没有网络请求。
