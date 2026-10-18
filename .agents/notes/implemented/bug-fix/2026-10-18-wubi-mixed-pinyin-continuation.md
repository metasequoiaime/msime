# Agent Note: 五笔四码碰撞时保留拼音续输

Status: implemented

## Problem

五笔拼音混输输入五个以上字母时，前四个字母可能恰好也是五笔码。例如 `guiyi` 在输入 `guiy` 后，五笔表已有精确候选，运行时在第五个字母之前顶字，导致完整拼音永远到不了查询层。更新词库不能修复这个按键路由问题。

## Decision

引擎根据当前混输候选的完整拼音判断下一小写字母是否能继续读音前缀：忽略音节分隔符，逐字节匹配已有输入和后续字母。判断不分配字符串、不查新词库、不修改候选或学习状态，拼音候选排在第几页不影响判断。运行时只询问引擎，在可续输时跳过顶字；引擎按相同判断临时放开四码长度限制，完整拼音再走原有查询。

混输关闭、Shift 字母、本地模式、专用英文和无法继续任何读音的字母沿用顶字规则。简拼与纠错候选不能仅凭候选存在就放开长度，必须匹配真实完整读音前缀。候选保留各自的生产方案，选择拼音候选仍按拼音学习和上屏。

## Related decisions

[混输开关命名](../feature/2026-10-09-wubi-mixed-pinyin-naming.md)持有候选合并和偏好命名，本决定替换其中“只有五笔表未回答才接受第五码”的长度规则。[顶字保留后续按键](2026-10-10-wubi-top-commit-tail.md)继续约束实际执行顶字时的尾部合并。

检索活跃笔记：上述两篇部分重叠并互链；[四码唯一自动上屏](../feature/2026-10-08-wubi-auto-commit-unique-preference.md)与[拼音反查](../feature/2026-10-11-wubi-reverse-lookup.md)分别持有第四码自动上屏和混输关闭时的 z 引导规则，本次不改变它们；没有完全吸收或过时的提案。

## Alternatives considered

- 混输时全部禁用顶字：不需要判断拼音，但会使已经能流畅连打的五笔码也不能顶字，改变范围过大。
- 在运行时解析拼音或再查词库：能独立决定第五个字母，但复制引擎规则、增加按键查询成本，且会与候选的实际读音漂移。
- 只要列表中有拼音候选就放开：实现更短，但简拼、纠错候选也会截获不相关的下一个五笔码。使用真实读音前缀收窄冲突范围。

## Consequences

五笔 86 与 98 共用续输判断，无需更换词库格式或改六个平台宿主。碰撞前缀确实能继续当前拼音候选时，输入优先按拼音延长；想选该五笔候选时仍可空格或选词。续输判断使用当前引擎持有的候选集合，未进入该集合的读音不会改变顶字行为。

## Verification

使用合成候选的运行时回归先在旧实现失败，再覆盖五笔 86/98、混输开关、可续输小写字母、不相关字母、Shift 字母与拼音候选选择，分页设为每页一行以确认后页读音仍可续输。真实词库测试只在本机执行，不把个人目录或学习记录写进提交。

`cargo test -p msime-engine --lib --locked`：2298 passed、40 ignored；`cargo test -p msime-input-runtime --lib --locked`：320 passed；`cargo fmt --all -- --check`、`pnpm run verify-notes` 与 `bash scripts/verify-local.sh --quick` 通过。本机工作词库在五笔 86/98 下已验证既有混输回归；本次同一构建还完成 macOS Host API、macOS bundle 构建与 `platforms/macos/scripts/install.sh` 安装，输入源注册检查确认五笔模式已启用。自动化文本工具对 TextEdit 使用无障碍文本注入，不能把它的原始文本结果当作 IMK 候选窗口证据。

后续发现 `bian` 的拼音候选本身只有完整音节 `bian`，而“编译”的长词要等到第二个音节输入后才出现，原来的候选全读音匹配因此仍会在 `y` 处截断。现在在已有完整拼音候选的边界上，只要下一个字母是合法拼音音节前缀也允许续输；这覆盖 `bianyi`，同时仍要求当前列表有拼音行，不改变纯五笔码的四码上限。回归测试 `bianyi_continues_after_the_bian_wubi_code` 固定该路径。

同一边界还可能出现在两个完整音节之后，例如 `qigu` 既是五笔整码也是 `qi'gu` 的拼音输入；候选保存的规范读音带有音节分隔符，边界判断必须在比较时忽略 `'`，否则输入 `qiguai` 的第五个字母仍会被截掉。回归测试 `mixed_wubi_starts_a_new_pinyin_syllable_after_a_four_letter_code` 固定这一条续输路径。

`bash scripts/verify-local.sh --quick` 通过：Rust 工作区、Android 目标与宿主 Java、WASM、Linux 设置壳、Linux 原生宿主（90 项测试）、macOS 原生宿主、共享 Apple 桥与 Windows IPC 两种架构均通过；Windows 完整交叉编译因其他构建占用共享锁而跳过，HarmonyOS 原生编译因缺少 staged 输入而跳过。未进行系统输入法替换或发布。
