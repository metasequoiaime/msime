# Agent Note: Android 大屏竖屏也可使用分离键盘

Status: implemented

## Problem

#9244 的折叠屏内屏竖放时不会分离。现有 `platform.android.split_keyboard` 只在大屏横屏生效，直接去掉方向判断会改变已开启该开关的用户原有竖屏布局。

## Decision

保留现有分离主开关，新增 `touch_keyboard_split_portrait` 布尔偏好，默认关闭。默认值、类型校验和持久化沿用 `client-core` 的共享偏好；目前只有 Android 原生设置页和宿主使用，不在其他平台设置页显示，也不增加账号同步字段映射。

大屏设置里提供「竖屏也分离」，需要先开启分离式键盘。只有主开关开启、当前窗口最小宽度不低于 600 dp、布局是 26 键一族或韩文，且横屏或竖屏选项开启时才分离。折叠到窄外屏后自动回到完整键盘，不清除偏好；再次展开时恢复。九键等网格、浮动键盘、独立数字面板仍沿用原有规则。几何快照比较纳入新选项，切换不重建输入会话、不结束组字。

## Related notes audit

[数字行与分离重复键](../../implemented/feature/2026-10-09-android-number-row-and-split-duplicates.md)部分重叠：本篇只扩展方向条件，断行、中缝、重复 G/V 与键宽不变。[Shift 就地改键面](../../implemented/bug-fix/2026-10-11-android-shift-relabel-in-place.md)仍允许真实分离状态变化重建键行。检索 proposed、implemented、rejected 没有发现已有竖屏分离提案。

## Alternatives considered

- 现有分离开关直接覆盖所有方向：不用增加偏好，但会改变原先只选择横屏分离的用户行为，保留独立的竖屏覆盖更明确。
- 按三星机型或铰链角度识别内屏：能表达特定折叠设备，但不同厂商的内外屏和多窗口状态不同。布局已经以当前窗口的 `smallestScreenWidthDp` 决定大屏，沿用它同时支持平板竖屏和折叠屏，不绑定机型。
- 在 Java 本地设置表新增布尔项：和分离主开关放在同一份文件里最直接，但新增设置的默认值和校验应归共享 Rust 层，避免继续扩张宿主里的独立配置规则。

## Consequences

折叠屏内屏与平板竖屏可以保持分离，窄外屏自动恢复完整键盘；已有横屏用户不改变竖屏布局。新选项依赖旧主开关，界面需要多一个子开关；默认值与类型规则在共享 Rust 中，主开关仍归既有 Android 本地设置，当前不扩展账号同步映射。

窗口尺寸变化可能让应用编辑器重建，设备测试重新聚焦后测量真实按键；开关热更新则在同一输入连接里检查组字保留，并按空格选出「你」。中文数字九宫格维持完整面板，扩展符号行分离，测试分别覆盖两者，不以已经废弃的横排数字偏好覆盖宿主规则。

模拟器窗口尺寸切换不等于三星 Z Fold5 真机验收，最终仍受厂商报告的窗口配置影响。新选项仅 Android 使用；未扩展 iPad 的分离方向设置。

## Testing

- `touch_keyboard_split_portrait` 在 macOS 的偏好覆盖清单中明确标为不适用，理由是 macOS 屏幕键盘不采用分离布局，设置页不提供此项。新增字段后 CI 的 `preference-coverage` 检出这项遗漏；补齐后脚本及同名 CTest 均通过。编译型 quick 本身不运行这项 CTest。

- 共享偏好回归先在没有新字段的实现上失败（缺省值读到 `Null`），加入字段后 `cargo test -p msime-client-core --locked --lib` 的 506 项全部通过；覆盖旧文档缺字段默认为关、布尔往返和字符串、数字、空值拒绝。
- `cargo clippy -p msime-client-core --locked --all-targets -- -D warnings` 通过。首次被既有 Vim 两项测试的 Default 后赋值挡住，移除本次新偏好仍复现；同文件中机械改成结构体初始化，断言不变。
- `bash platforms/android/check-host.sh` 的 256 项 JVM 冒烟和 JNI/Java 契约通过；`SplitKeyboardPolicySmoke` 覆盖主开关、方向覆盖、599/600 dp 边界及布局排除。
- 当前分支完整 Rust 原生库及发布用 Java 原生宿主经 `bash platforms/android/build-apk.sh <固定资源目录>` 构建，使用开发证书；设备测试 APK 单独经 `build-editor.sh` 编译。
- API 28 上，旧 #9275 构建的原生 APK（提交 `29d53a883760f4f2a9dc0bc12dcad174aa377b2b`，分离策略未改） 在「竖屏开关热更新」确定失败：开启新字段仍没有分离中缝。新 APK 的 `KeyboardHeightDeviceSmoke -e portrait-split true` 在 API 28、35 均报告 `MSIME_DEVICE_SMOKE_PASSED`，测量真实字母/符号键边界并验证原生设置布尔持久化、组字选词、中文数字九宫格不分离、窄外屏恢复、再次展开和关闭竖屏选项后的横屏兼容；结束恢复偏好、显示尺寸、密度和旋转。
- API 35 完整 `KeyboardHeightDeviceSmoke` 通过，覆盖原有高度预览、保存与重启、底部留白及 14 键/26 键高度。API 28 的补充 `smoke.sh --core` 通过输入、延迟批量组字、数字层重启、组字重启、中文 Shift、分词操作和网格拖动七项；之后停在既有「复制设备信息」剪贴板断言。旧 #9275 APK 用相同夹具、同一设备也在同一断言失败，记录为既有失败，不新增失败基线，也不将完整 core 冒烟计为通过。
- 原 PR 提交 `05f573986ffeacc181183acaa6d1b7fbdc0c71f9` 记录的 Mac Studio 完整 `bash scripts/verify-local.sh --quick`（`20261023-030641`）通过，包含仓库契约、Rust workspace、Android/wasm 目标、Android Java 宿主、Linux 设置壳与原生宿主（111 项测试）、macOS 原生宿主与共享 Apple 桥接层。该机缺少的 MinGW、HarmonyOS 打包前置和 Node 阶段由脚本显式跳过；本机另跑 `pnpm run verify-notes`、`cargo fmt --all --check`、偏好字段对齐和 `git diff --check` 通过。这份记录属于后续整合前的提交。
- 整合 `develop` 的 `e8ebe1fc2ab6e3d86ff2b3a1c3874101c855de61` 后，首轮 quick 的 Swift 豆包会话套件和语音语言目录导出超时，Linux polish 的 `/null-error` 用例失败，不能计为整合通过；该轮 Linux 原生 111 项及 macOS 编译完成。目录导出预编译成功后，完整 quick 重跑退出码 0，报告 `quick check passed`；仓库契约、Rust workspace、Android/wasm 目标、Android Java 宿主、Linux 设置壳与原生宿主（111 项测试）、macOS 原生编译和共享 Apple 桥接层通过。MinGW 与 HarmonyOS 打包前置缺失的阶段由脚本明确跳过。独立静态复审未发现阻塞项，设备源码清单保留了上游新增项。
