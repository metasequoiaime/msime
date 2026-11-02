# Agent Note: 三端 26 键默认上滑输入角标

Status: implemented

## Problem

用户明确要求 Android、iOS、鸿蒙 26 键上滑输入符号。Android 和鸿蒙已有方向选择但默认下滑；iOS 只有下滑和总开关，缺少上滑方向及设置入口。

## Decision

三端未保存方向时默认上滑，沿所选方向严格超过 14 dp、pt 或 vp 后，松手输入现有角标表对应字符。复用现有手势、预览、输入和取消路径，不新增符号映射；用户明确保存的下滑方向和关闭选择继续生效。iOS 增加本机方向字段、设置控件、键盘出现时刷新缓存及恢复全部设置时清理字段。

Android 恢复已有的非 ASCII 字面提交路径：`commitNineKeyLiteral` 仅将单个 ASCII 字符交给英文纠错入口，避免 `¥`、`…`、`、` 被其 ASCII 上限丢弃；不扩展实体键共用的英文入口契约。复用完整英文方向专项验证。长按夹具先检查上滑大写选项，再输入会打开邮箱候选的 `@`，避免用候选态错误验收空闲态手势。

本次用户直接指定上滑，授权改变这一项默认值；其他默认值不动。键面上画不画角标另由「显示字母键角标」决定，见[字母键角标可以隐藏](2026-11-02-letter-corner-hints-switch.md)。Android 候选态门控、反方向字母选项及各端长按路径继续按原规则工作，九键与假名 flick 不受影响。

## 历史笔记审计

- [Android 字母滑动选项](../../implemented/bug-fix/2026-10-21-android-letter-option-gesture-settings.md)：部分取代，仅默认方向由下滑改为上滑；保留方向选择、候选态门控和标点路由。
- [iOS 与鸿蒙全平台设计](../../implemented/feature/2026-10-07-ios-harmony-all-platform-design.md)：部分取代字母角标默认下滑的决定，其他设计保持。
- [滑行输入引擎](../../implemented/feature/2026-10-07-glide-typing-engine.md)：无关，本次单键竖直手势仍沿用既有滑行仲裁。
- [Android 角标标点路由](../../implemented/feature/2026-10-12-android-letter-hint-chinese-punctuation.md)：部分重叠，英文非 ASCII 角标按既有决定恢复字面路径；原先的 #9483 修复在当前基线中缺失，已有完整专项可复现。
- 检索命中的九键、14 键、数字层、标点样式、英文长按和候选点击笔记不拥有 26 键默认方向，不修改。

## Alternatives considered

- **只在 iOS 增加上滑选择，保留三端默认下滑**：保持既有默认，但用户直接要求三端上滑输入符号，因此将未设置方向的默认统一上滑，明确存储的选择仍保留。
- **无条件覆盖已保存方向**：所有设备立即切为上滑，但会丢掉用户主动设置的下滑手感，因此只改变缺省方向。

## Verification

- 远端 `bash scripts/verify-local.sh --quick` 两轮通过，包含 Android 原生编译与鸿蒙真实 HAP 编译；`pnpm run verify-notes` 通过。
- 鸿蒙 `bash platforms/harmony/tests/run.sh` 通过；共享设置页组件 16 项测试、TypeScript 类型检查及改动文件的格式和 lint 通过。
- iOS 原生测试与 App/UI tests 构建通过；iOS 27 专用模拟器中 `LetterCornerHintTests` 7 项通过，方向保存、重启回读、总开关联动的 UI 测试通过。未在 iOS 真机验收扩展触摸。
- Android API 28 专用模拟器的缺省上滑标点套件和 `letter-swipe` 方向、开关、取消、反向选项、候选态门控套件通过。
- Android 真机已覆盖安装保留数据的 0.6.0 包，三次独立连续触摸分别上滑 Q/A/M 输入 `1`、`@`、`/`；每次含 8 个 MOVE 且只提交一次，结束后移除合成编辑器并恢复原输入法。
- 鸿蒙签名 HAP 已覆盖安装，真实设置控件保留原下滑选择并可切换、保存上滑；在应用内空白试用框三次独立上滑 Q/A/M 分别输入 `1`、`@`、`/`，均只提交一次，结束时清空合成内容且未发送。
- Android `english-swipe` 完整专项通过：26 个角标分别覆盖上下两个方向，包含 `¥`、`…`、`、`，并检查取消、关闭及符号后普通字母；修正夹具顺序后的 `letter-hold` 专项通过。

## Consequences

已有 Android、鸿蒙配置若明确保存下滑，升级后仍按下滑工作。iOS 原来没有方向字段，升级后使用新默认上滑。
