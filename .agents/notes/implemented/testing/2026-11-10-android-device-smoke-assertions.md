# Agent Note: Android 设备冒烟按语言键目标和弹窗类型断言

Status: implemented

## Problem

完整 Android 设备冒烟的 `ChineseLanguageReturnDeviceSmoke` 在 API 35 报告找不到中文返回键，实际键盘仍显示日语 9 键。轮换其他语言打开时，日语语言键的键面按契约显示当前语言字形「あ」，要切到的中文入口写在 `contentDescription`；测试却要求节点文字是「中」，把显示字形和目标动作混在了一起。

同一次运行的 14/18 键角标测试在关闭滑动输入符号后报告弹出了字母选项，实际窗口是组字中的「14 键拼音选择」读音条。读音按钮也使用「输入 …」的读屏前缀，测试的全局查找把主键盘里的读音条误当成附着的长按选项弹窗。

## Decision

语言键回归改为按精确的目标 `contentDescription` 查找「切换到全拼 9 键」或「切换到全拼 14 键」，保留返回原入口的行为断言，不再要求当前语言键面的文字等于目标文字。字母选项回归只在 `TYPE_APPLICATION_ATTACHED_DIALOG` 窗口中查找「输入 …」节点，读音选择条留在主键盘窗口时不计为弹窗。

## Alternatives considered

- **修改产品让日语语言键显示「中」**：会推翻 #9770 已确定的「键面显示当前语言、读屏说明显示动作」契约，且会让轮换停在日语时的状态字形失去一致性，不采用。
- **把所有「输入 …」读屏文案改成不同前缀**：会扩大无障碍文案变更范围，且没有必要；测试应按窗口类型识别自己的弹窗，不改产品文案。

## Consequences

完整矩阵重新运行时，两个断言分别验证语言键确实能回到原中文入口、以及关闭滑动或组字时没有打开字母选项；不会再被当前语言字形或拼音读音条误报。完整 nightly/发版仍保留这两项真实设备回归。

## Verification

- `git diff --check`
- `bash -n platforms/android/tests/device/smoke.sh`
- `actionlint .github/workflows/android-device.yml .github/workflows/ci-platforms.yml`（相关工作流未改，本次仅作门禁复核）
- 未在本机启动 Android 模拟器；需要在 nightly 或 Release Android 的完整设备矩阵上确认两项回归。
