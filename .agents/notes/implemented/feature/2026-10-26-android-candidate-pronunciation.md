# Agent Note: Android 候选显示英文音标

Status: implemented

## Problem

Android 已有共享偏好 `candidate_pronunciation` 和离线查询 FFI，却没有设置入口、读音查询和绘制。发布包也没有 `en-phonetic.db`，直接接上 FFI 仍只会得到空结果。

## Decision

沿用 `resources/pronunciations.lock.json` 固定的 ECDICT 表及许可声明，把其中 `built` 的数据库和 NOTICE 登记为共享 `pronunciations` 资源包。表大小为 5,718,016 字节，NOTICE 为 1,275 字节，来源提交与两个产物的 SHA-256 均在锁中。Android APK 不随包放这两件文件；设置页只在用户打开音标且缺资源时列出「离线英文音标词典」下载行，用户点「下载」才经共享资源包安装器取锁定 URL、核对长度和摘要，再原子发布到 `state_root/resource-packs/pronunciations/`。打开开关不自动联网，按键过程只读本机文件。

设置页增加默认关闭的「显示英文音标」。宿主在开关开启且资源包已安装时，从同一代引擎候选和直输英语建议中收集英文候选及英语释义的首个词组，在现有候选工作线程上调用 `msime_client_pronunciation_request`，把 `state_root/resource-packs` 作为显式补充资源根，只接纳当前会话及候选代次的回答。音标在候选的次要文字中独占一行，横向条限制单个按钮宽度并省略过长的次要文字，展开网格按格宽裁切；直输建议复用同一批查询与字号、宽度策略。关闭后不查表、不挂音标、恢复原按钮布局。表缺失时设置页明确显示离线资源未安装及下载入口。ECDICT 表只有词面到单个宽式音标的映射，不含义项或词性；多音词沿用共享层的固定读音，无法按释义消歧，界面说明不暗示消歧。

## Alternatives considered

**随 APK 携带完整表**让用户开启开关后即刻离线使用，也沿用 macOS 的打包方式。首版实现已用真实 full arm64 瘦包检验：APK 为 136,865,171 字节，超过固定 135,000,000 字节上限 1,865,171 字节；表自身在 APK 中压缩后仍占 2,343,614 字节。不能为音标提高瘦包上限，也不为腾空间删除其他既有资源，因此改为用户显式安装固定资源包。

**直接复用背单词的 ECDICT 词书**能省掉新增的资源包。但词书 JSON 按课程拆分且没有供候选批量查询的 IPA 契约，读音使用另一个转写格式；复用它会在候选刷新时扫描无关数据，也会让两套音标规则不一致，因此继续使用桌面端的共享表。

## Verification

- 合成策略测试先因缺类失败，接入后通过；覆盖英文候选、英语释义、非英语目标和关开关、缺资源。
- `bash platforms/android/check-host.sh` 通过：JNI aarch64 交叉编译、280 个 JVM 冒烟测试、245 个设备测试源编译；此检查不产生 APK。
- `python3 scripts/fetch_gloss_tables.py --out target` 实取音标表与 NOTICE，两个 SHA-256 与锁一致；只读查询确认 `hello`、`world`、`love` 有宽式音标。
- 资源包锁测试先因缺 `Pronunciations` 失败，接入后通过；校验包 ID、来源提交、两个文件的 URL、长度和 SHA-256。
- `bash scripts/verify-local.sh --quick` 首轮发现 JNI 方法校验清单遗漏，补齐后复跑通过。
- 改为资源包后重打 full arm64 瘦包为 134,530,276 字节，低于 135,000,000 字节上限，APK 内没有 `assets/pronunciations/`。
- API 28 模拟器上四个共享 crate 的设备测试通过；原生 APK 已装包，核心冒烟前两个输入用例通过。设置页实际显示缺包提示，打开开关后显示「需下载 5.7 MB」及「下载」；点击下载后状态自动更新，下载行消失。横向条的英语直输 `hello`、`hellos` 各自在词下显示音标；中文模式 `ai` 的 `AI` 候选在横条和展开网格均显示 `/aɪ/`，没有覆盖相邻候选；关闭开关后建议恢复单行。
- `pnpm run verify-notes`、`actionlint` 通过；新版 `--quick` 已通过契约、Rust workspace、Android target 和 Android host，因磁盘低于 50 GiB 时进入新的 Linux Docker 构建树而中止。

## Consequences

Android APK 不增加音标数据库；第一次使用时用户须显式下载约 5.7 MB 的固定离线资源包。打开音标且资源存在时，候选行增加一个次要文字行，最长按钮宽度限制在 240 dp；关闭后使用原布局。未安装时设置页显示缺失和下载入口，输入时没有网络请求。
