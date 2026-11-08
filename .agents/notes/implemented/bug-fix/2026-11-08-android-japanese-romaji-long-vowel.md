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

JVM 冒烟覆盖键位顺序、原始布局不变、英文与本地模式退出。设备专项复用日语资源夹具和 instrumentation 入口，验证 `ko-hi-`、删除末尾长音后续输、单独长音上屏；`smoke.sh` 非 core 套件会实际调用。设备测试和真实截图需要 Android 模拟器，本机未安装模拟器，不能将 Java 编译称为设备验收。

## Verification

- 使用 Temurin JDK 17 与 Google Android API 35，原生宿主及 JVM 测试合计 879 份 Java 源码以 UTF-8、--release 17、-Xlint:all、-Werror 编译通过；日语、标准布局及分离布局 11 项针对性冒烟通过。设备测试清单中的 276 份源码编译通过。
- 全量 JVM 冒烟遇到 CandidateSkinStampSmoke 的原子替换戳断言失败；用未修改的主工作区原始两份源码单独编译复现了同一失败，不计为通过。
- npm run verify-notes 执行与 pnpm 同一条仓库脚本，通过三项校验；本机 pnpm 启动器指向不存在的 pnpm.exe。git diff --check 通过。
- scripts/verify-local.sh --quick 已运行但未通过，包含缺少 cargo、shasum、平台构建环境，以及契约检查失败；未逐一验证所有既有失败，不宣称全量门禁通过。完整本地日志保留在 target/android/japanese-romaji-test/quick.log。
- 本机没有 Android 模拟器或完整 APK 构建环境，未运行设备专项、未产出 APK，也未取得修改前后的真实设备截图。以上 Java 编译和 JVM 结果不能替代设备验收；不得据此宣称已装机验证。
