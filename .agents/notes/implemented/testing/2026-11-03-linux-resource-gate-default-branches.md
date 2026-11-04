# Agent Note: Linux 资源宿主门禁跑两个宿主的默认分支

Status: implemented

## Problem

#10723 要的是让 IBus `ibus-engine-smoke <词库>` 和 Fcitx5 `fcitx5-native-test <词库>` 的默认分支（按住说话、组合键取消、迟到结果都在这里）进入 CI，并修掉挡在语音段前面的断言。[资源宿主集成门禁](2026-11-03-linux-resource-native-smoke.md)（#10724）注册了 `ibus-native-context`，新建了 `Linux native host resource integration` job，但 job 设了 `MSIME_RESOURCE_SMOKE_ONLY=1`，容器脚本在这个模式下只跑 `ibus-custom-voice-hotkey` 和 `fcitx5-custom-voice-hotkey` 两个 `--voice-hotkey` 参数分支。两个默认分支在 CI 里一次都没跑过。真机验收在资源容器里把它们完整跑了一遍：IBus 停在 `Candidate auxiliary text did not expose the preedit caret`，Fcitx5 停在 `the view reports corner quotes and hollow brackets`，`check-container.sh` 在更早的 Rust 单测 `typing_statistics::batch_tests` 就失败了。

## Decision

资源宿主 job 仍用 `MSIME_RESOURCE_SMOKE_ONLY=1`（只跑要词库的宿主用例，不进需要完整用户会话的安装后桌面验收），但这个模式下 IBus 跑 `ctest -R '^ibus-'`，也就是默认分支 `ibus-native-context` 加上各参数分支；Fcitx5 跑 `fcitx5-native-context` 和 `fcitx5-custom-voice-hotkey`。`ibus-translation-scope`、`ibus-chaizi` 不再按这个模式排除，CMake 也就不再读 `MSIME_RESOURCE_SMOKE_ONLY`。`engine_smoke.cpp` 里按这个环境变量跳过候选拼音行光标断言的分支删掉。`ibus-native-context` 的超时从 120 秒放到 300 秒：它串起全部 IBus 宿主用例，`ctest -j6` 的负载下会超过两分钟。

默认分支和两个参数分支挡路的断言逐条判过是产品缺陷还是用例过时：

- IBus 候选拼音行光标：产品缺陷。拼音行画的是分词后的 `ni'hao`，宿主只在它等于按键原文时才画光标，所以全拼多音节时光标从不出现。改成读引擎的 `View.preedit_caret`，见 [Linux 候选拼音行的组字光标](../bug-fix/2026-11-03-linux-candidate-preedit-caret.md)；用例期望从 `ni|hao` 改成 `ni|'hao`。
- IBus 智能标点影子前文：夹具问题。语音重建会话那段为了不跑主循环，直接调类方法 `set_content_type` 设了隐私字段，libibus 缓存的 ContentType 没变；随后经 D-Bus 设回 `(FREE_FORM, 0)` 被 libibus 当成没变、不转给宿主，宿主后面一直停在隐私字段里，不记影子前文。改成同样直接调类方法换回普通字段。真实 IBus 的 ContentType 都经 D-Bus 到达，不会这样失步。
- IBus 屏幕键盘一行把下面的 `!key(IBUS_period)`（不处理、不上屏）改成处理并上屏 `.`：#8402 实现的就是后者，同一个提交里上面那条 Unicode 数字用例也这样写，这一条写反了。
- IBus 关掉临时日文后 Shift+R、关掉拆字后 Shift+C：用例过时。#8809 起关着的快捷模式把大写字母留给拼音组字（引擎的 `disabled_shift_shortcuts_leave_every_capital_available_for_pinyin`），只有「Shift+字母进入快捷模式」总开关才交还应用。改成核对 Shift+R/C 再打几个字母后候选窗辅助行有没有「日文」「拆字」模式标签，开着时先确认标签在。拆字分支还把资源夹具显式写下的 `chaizi=false` 拿掉，验的才是缺键时的桌面默认。
- IBus 屏幕键盘空格上屏：时序。第一次画出 `nihao` 之后首选还会再换一次，读早了就和空格上屏的那条对不上；等候选连续 300 ms 不变再读。英文模式录音那段也改成等语音预编辑收起，不读同一轮的值。
- IBus 翻译范围：夹具开了混排 Emoji，第二个候选是不带译文的 👋。这个分支关掉混排 Emoji。
- Fcitx5 标点样式：用例过时。`View.punctuation_marks` 自 #9241 起多了两个顿号字段，整对象相等永远不成立；改成只比引号和括号。
- Fcitx5 读音：用例过时。#10507 起读音按 `pronunciation_key`（`语言\n原文`）记，夹具的回答还用裸词键。
- Fcitx5 韩文汉字训音：用例过时。#9854 起候选词翻译默认「仅当前高亮」，韩文汉字行和中文一样要译，高亮行后面多一段译文占位，只核对前两段。
- `typing_statistics::batch_tests`：测试隔离问题，develop 上就有。攒着的批次放在进程共用的 `PENDING_COMMITS` 里，三个用例调 `flush_pending_commits`（或后台版）会把并行用例攒着的批次提前写出。调它的用例拿模块里一把 `RwLock` 的写锁，其余用例拿读锁。

## Alternatives considered

- **资源 job 跑完整的 `check-container.sh`（不设 smoke-only）。** 覆盖最全，安装后桌面验收和 Fcitx5 的真实窗口、画布用例也在 CI 里跑。但验收容器里 Fcitx5 的画布、chrome、Wayland 用例在别的机器上失败过十九项，原因和词库无关；安装后验收还要完整用户会话。把它们和默认分支绑在一个门禁上，会让 #10723 要守的语音段又被无关失败挡住，这正是这条 issue 要解决的问题。
- **只跑 `ibus-native-context` 和 `fcitx5-native-context` 两条。** 最贴 issue 的字面要求，CI 时间最少。但 IBus 其余的资源参数分支在同一次构建里，串行跑完也只多几分钟，而 `ibus-translation-scope`、`ibus-chaizi` 正是因为不在任何门禁里才过时的；放进来能防同样的事再发生。Fcitx5 的其余用例要真实窗口，不放。
- **把过时断言改成只打印。** #10704 验证时就是这样临时看语音段的，但留在仓库里会让前面的回归在 CI 里假绿，issue 明确要求修掉。

## Consequences

两个宿主的默认分支和 IBus 的全部资源参数分支每次 Linux 相关 PR 都会跑，按住说话、组合键取消、迟到结果这些宿主级行为有了门禁；IBus 资源步骤串行跑要多几分钟。代价是这些长用例对时序更敏感：这次已经把能看到的读早问题改成等条件，再出现时按同样的办法改，不靠调大全局超时。`MSIME_RESOURCE_SMOKE_ONLY` 的名字沿用，含义变成「只跑要词库的宿主用例」。

## Verification

在 Studio 的 OrbStack（arm64 Debian 容器）里用仓库的 `platforms/linux/tests/tools` 镜像：`ibus-engine-smoke /resources` 默认分支、`--translation-scope`、`--chaizi` 通过；`ctest -R '^ibus-' -j6` 连跑两次、`fcitx5-native-context` 通过；按 CI 的环境变量跑 `check-container.sh <词库>` 和 `--fcitx5` 通过。`typing_statistics` 用 `--test-threads=32` 连跑 40 次：develop 的用例 35 次失败，改后 0 次。还没有在 x86_64 runner 上跑过，以 PR 的 CI 为准。
