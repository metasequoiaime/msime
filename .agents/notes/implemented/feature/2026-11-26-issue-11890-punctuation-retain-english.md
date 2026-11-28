# Agent Note: #11890 中文标点按键保留英文

Status: implemented

## Problem

#11890 希望在中文标点模式下，按键级保留部分 ASCII 标点。报告者重点使用反引号和反斜杠，并希望不必频繁切换英文模式。契约第 6 版把反引号映射为 `·`，反斜杠映射为 `、`；反斜杠还与两个顿号开关共享键位，`punctuation_marks` 在两个顿号键都关时按「`\` 开」读，所以 `\` 和 `/` 不能同时保持 ASCII。

维护者在 issue 里给了两条路：A 先落共享规则，界面随原生设置迁移补；B 对共享 React 设置页开例外。维护者选了 A。报告者已接受第 1 点：`\` 设为保留英文后，两个顿号开关可以同时关。

## Decision

共享偏好 `preserve_english_punctuation` 记一组键，缺省为空、为空时不写进文档，行为与没有这个字段时逐字相同。集合里的键在中文标点下原样输出 ASCII，其余键照旧；不引入任何新字，所以宿主手抄表只需要少认几个键。只作用于中文标点表，日文表、「始终英文标点」锁和韩文等 ASCII 方案不看这个集合。

- **契约（第 7 版）**：`shared/contracts/punctuation/policy.json` 新增 `preserveEnglish` 段，正好是中文表会改写的 21 个键（`simple` 17 个、交替引号、嵌套书名号），顺序与这几段相同，`generate.py` 拒绝多一个或少一个；`policy.h` 生成 `preserve_english` 与 `is_preservable`。`/` 只是可关掉的顿号键，不在其中。
- **Engine**：`PunctuationMarks` 新增 `preserve_english`（`PreservedEnglish` 位集，第 i 位对应契约第 i 个键，保持 `Copy`）。`translate` 对集合里的键返回 `None`，不推进引号交替、不动书名号嵌套；`handle_punctuation` 在组字中先上屏组字再跟这个 ASCII 键（与韩文的 ASCII 标点同一条路），空闲时交回宿主。
- **client-core**：保存和读入都由 `validate` 校验，每项必须是契约里的单个 ASCII 键、不能重复，否则 `InvalidPreserveEnglishPunctuation`。`punctuation_marks` 是唯一解析处：`\` 在集合里时 `enumeration_comma_backslash` 为假，且不再参与「至少留一个顿号键」的归一。字段列进设置同步白名单。
- **host-api / View**：`apply_punctuation_marks` 交给引擎；View 的 `punctuation_marks.preserve_english_punctuation` 是单字符字符串数组，按契约顺序。`msime_client_smart_punctuation_arm` 不为集合里的键武装连按转中文。
- **宿主**：都只读 View（Windows TIP 读 Server 推来的帧），不自己解析偏好。共同规则是集合里的键当普通字符：不画中文键面、不跨过补出的后半个、不走智能标点直出和连按转中文；成对补全本来按上屏的字判断，ASCII 不补。各宿主额外的点：
  - `shared/input/EnglishModeOutput.h`（macOS、IBus、Fcitx5 英文模式下的中文标点）对这些键返回空。
  - macOS：`{` `}` 不再被宿主拦成「」，也不做花括号成对补全。
  - Linux：成对标点分支和跨过都跳过这些键，空格转换不武装。
  - iOS / Android / HarmonyOS：符号键面、字母角标、注音符号面板画 ASCII；iOS 123 层的中文键面对应的键被保留时按键面原样插入（Android、鸿蒙的 123 层本来就按字面上屏）。
  - Windows：Server 经新追加的 Worker 帧 `PreserveEnglishPunctuationChanged`（38，载荷 `"p"` 加键）推给 TIP，旧 TIP 丢掉这个帧照旧出中文标点；TIP 的 `GetPunctuation` 对这些键出键本身，`PairedPunctuationStepOverCandidate` 对键本身答 0，智能标点连按不把它替换成它自己。
- **设置界面**：iOS「标点」页、Android「表达 › 标点」、Windows「标点 › 更多选项」各加「保留英文的标点」，逐键开关 21 个键，按契约顺序写，全部取消时删掉这个键。macOS 的 AppKit 回退窗口冻结、共享 React 设置页只修缺陷，所以 macOS、Linux、HarmonyOS 没有入口，`scripts/test-preferences-field-parity.py` 的 `RUST_ONLY` 登记原因；迁移前可以直接编辑偏好文档启用。Windows 的 Win7 设置窗口不加。

## Alternatives considered

- 任意逐键映射（类似搜狗自定义标点）：表达力最强，但用户能填任意字符，六个宿主的补全、跨过、空格转换无法预先认识这些字，Windows TIP 还要经 IPC 传整张表；超出本需求，见 [标点样式](2026-10-13-punctuation-mark-styles.md)。
- 把反斜杠并入顿号开关：少一个字段，但无法表达「`\` 保留英文」与「谁出顿号」两个独立决定，也会改变既有偏好语义。
- 在 `packages/ui` 直接加多选入口（方案 B）：macOS 用户立刻可用，但违反「共享 React 设置页只修缺陷」，需要维护者对约定开例外；维护者选了 A。
- Windows 把集合塞进已有的 `PunctuationMarksChanged` 帧：少一个帧类型，但旧 TIP 严格校验 `"q<0|1>b<0|1>"` 后紧跟结束符，载荷变长会让它连引号样式一起丢掉；新帧追加在最后不影响旧 TIP。
- 引擎组字中对集合里的键返回 unhandled（与关掉的 `\` 一样）：改动更少，但这时组字还开着，ASCII 键是否上屏、组字怎么结束取决于各宿主；改为先上屏组字再跟 ASCII 键，行为由引擎一处定下。

## Consequences

- 收益：反引号、反斜杠等可以在中文标点下单独保持 ASCII，缺省用户逐字不变；`\` 与 `/` 可以同时保持 ASCII。规则集中在契约、引擎和 `client-core`，宿主只认一个集合，不认识新字。
- 代价：macOS、Linux、HarmonyOS 迁移到原生设置前没有界面入口，只能编辑偏好文档；因此本 PR 用 `Refs #11890`，有了合规入口后再关闭 issue。Windows 多一种 Worker 帧。
- 回归：契约测试（Rust、C++）钉住 21 个键；`client-core` 覆盖允许键、未知键、非 ASCII、多字符、重复、`\` 退出归一；引擎覆盖空闲、组字中、引号不推进交替、日文表不变；host-api 覆盖 View 报告与连按不武装；各宿主的纯规则测试（Linux `smart_punctuation_space.cpp`、Windows `punctuation_key_policy.cpp` / `tsf_config_frames.cpp` / `punctuation_settings.cpp`、Android 冒烟、鸿蒙 `keyboard-logic.test.ts`、iOS `PunctuationSettingsTests`、macOS `ShortcutTest`）覆盖各自的手抄表。
- 未在真机或真实编辑器里验收：各平台键盘与输入法的实际出字、设置界面的观感，见 PR 的验证段。
