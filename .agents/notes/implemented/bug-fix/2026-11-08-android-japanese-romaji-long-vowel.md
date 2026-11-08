# Agent Note: 安卓日语罗马字键盘直接提供长音键

Status: implemented

## Problem

日语 26 键只有字母，用户没有可见的长音入口。N 的减号角标走 `commitLetterHint`，直接提交 ASCII 减号；组字中长按和滑动又按普通点按处理，不能把它当作日语长音入口。引擎本身已经支持把 `-` 保留为读音中的「ー」，九键的「わ」右滑也已经走这条路径。用户明确要求修改 26 键罗马字键盘。

## Decision

参考 [Google 官方日语 QWERTY 示意图](https://www.android.com/intl/ja_jp/articles/243/)，第二行 L 右侧增加「ー」，第二行取消半键缩进，与首行一样十格。第三行、键盘高度、九键和出厂设置保持原样。

`JapaneseRomajiLayout` 负责在日语、非英文直输且非本地模式时扩展字母行。`ImeLetterRows` 把长音键交给 `character('-', false)`，不结束组字、不走字面提交，也不受 Shift 和滑动符号开关影响。切换中英文或本地模式时重建键行，防止日语和中文共用标准布局编号导致长音键残留或缺失。分离式键盘沿用扩展后的键位权重切分。

## Historical audit

- [日语九键五向提示](../feature/2026-10-10-android-japanese-flick-guide.md)与[日语九键连点](../feature/2026-10-23-android-japanese-toggle-input.md)属于九键交互，本次不改。
- [日语九键符号入口](2026-11-01-android-japanese-nine-key-symbol-key.md)解决符号面板可达性；长音必须能接入读音，不能用符号面板直接提交代替。

## Alternatives considered

只把 N 的减号角标改为长音，能保持字母键数量，但组字中的候选态会禁用这条手势，且入口依然需要发现长按。单独显示长音键能在连续输入中稳定到达，因此采用可见键。

## Consequences

可以在罗马字之间插入长音而不打断组词，第二行每键由原来的九键加两侧缩进改为十键，其他行不挤占宽度。普通英文模式没有这个键。N 的减号快捷符号仍保留字面含义。

JVM 冒烟覆盖键位顺序、原始布局不变、英文与本地模式退出。设备专项复用日语资源夹具和 instrumentation 入口，验证 `ko-hi-`、删除末尾长音后续输、单独长音上屏；`smoke.sh` 非 core 套件会实际调用。设备测试在 Docker 中的 API 35 专用模拟器上运行，截图取自真实产品 APK 和合成编辑器。修改前后都先输入并清空同一段短读音，使键面处于一致的稳定状态。

## Verification

- Docker Ubuntu 24.04、JDK 21、NDK r28c、固定 Rust 1.97.1 构建完整 APK，包含 arm64-v8a 与 x86_64；原生库架构、依赖白名单、JNI 导出和 16 KB 对齐检查通过，APK 的 apksigner 与 zipalign 检查通过。
- Linux 的完整 Android host 检查通过：344 项 JVM 冒烟、276 份设备套件源码以及 JNI、清单、资源和契约检查。此前 Windows 上复现的 CandidateSkinStampSmoke 文件替换断言在 Linux 上通过。
- API 35、1080×2400 专用模拟器先装修改前基线，再覆盖安装修改版。旧版确认没有独立长音键；新版通过系统输入主路径，以及日语 `ko-hi-` 连续组字、删除末尾长音再续输、清空后单独长音上屏的实际输入验证。
- 本地保留修改前、修改后、连续组字、单独长音上屏四张原尺寸截图。产物和日志在主工作区 `target/android/japanese-long-vowel/`，设备证据见 `device-acceptance.log`，宿主检查见 `android-host-check.log`。
- 测试包版本 0.7.1，使用开发签名，不能覆盖正式签名版本；报告者已确认共存测试包可安装且长音正常；未记录其手机型号与系统版本，也未完成 API 28 或整个设备回归套件验收。
- 补齐容器的 Linux ALSA 开发包后，`scripts/verify-local.sh --quick` 通过，包含契约检查、共享 Rust workspace 的 `cargo check` 和 Android host 检查。`npm run verify-notes` 的三项校验及 `git diff --check` 通过。

问题跟踪：[#11123](https://github.com/metasequoiaime/msime/issues/11123)。
