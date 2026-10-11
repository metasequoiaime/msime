# Agent Note: Windows 原生设置窗口按任意深的路径读写偏好

Status: implemented

## Problem

原生设置窗口要把剩下的设置页都做成原生页，不再拉起共享的 Tauri 设置界面。那些页要改的偏好嵌得比现在深：插件页的 `plugins.key_sound.enabled`、主题页的 `custom_theme.candidate_colors.surface`，AI 辅助页还要按来源存密钥（`ai_assistant.tokens[<origin>]`），而来源本身带点。`main.cpp` 里的 `PreferencesDocument` 只认「顶层」和「`parent.leaf` 下一层」两种键：`lookup` 和 `object_for_key` 只在第一个点处拆开，`a.b.c` 会被当成 `a` 下名为 `b.c` 的一项；也没有删除，`SetStrings` 一律排序去重，写不了顺序由用户决定的列表。

## Decision

- 路径规则放进只有头文件的 `platforms/windows/settings/PreferencePath.h`：`preference_path` 把点分路径拆成各段（空段即不合法，返回空表，调用方什么也不做）；`preference_entry_path` 把表的路径照常拆开，表项的键原样作为最后一段；`find_preference`、`set_preference`、`remove_preference` 沿路径读、写、删。
- 写入时缺的中间层（或挡路的非对象值）从随包默认值里同一路径的对象复制而来，默认值里也没有才是空对象。这是把原先只对第一层成立的规则（存储只整体接受一个嵌套对象）推广到每一层；删除不补中间层，读取随之回落到默认值。
- 走路径的逻辑对 JSON 实现是模板参数：窗口里用 WinRT `JsonObject`（`main.cpp` 的 `JsonPreferenceTree`），测试 `tests/ui/preference_path.cpp` 用一棵按引用共享的假树。
- `PreferencesDocument` 的 `Value`、`Default`、`SetValue` 改走这些函数，签名不变；新增 `Remove`、`SetOrderedStrings`（原样写入）、`Entry`/`SetEntry`/`RemoveEntry`（字符串表的一项）。`SetStrings` 仍排序去重，内部改调 `SetOrderedStrings`，现有调用方行为不变。

## Alternatives considered

- **在 `main.cpp` 里就地把 `lookup`、`object_for_key` 改成循环** — 改动最小；但那里的代码离不开 WinRT，CMake 的测试要在 MinGW 上也能编译，路径规则就没有单独的测试，而之后每个原生页都依赖它。
- **测试直接用 C++/WinRT 的 `Windows.Data.Json`** — 不用写适配层；但 MinGW 没有 C++/WinRT，CMake 的测试就只能在 MSVC 下编，和这个目录其他设置测试不一致。
- **表项也用点分路径，约定键里的点要转义** — 只要一个入口；但来源是 `https://api.example.com` 这样的串，转义规则要在每个调用方各写一遍，写错就把一个密钥拆进几层空对象里，存储校验还会拒掉整份文档。
- **让 `SetStrings` 不再排序** — 只留一个写列表的函数；但游戏程序列表、模糊音规则这些现有调用方依赖排序去重来得到稳定的文档，改了会让同样内容的保存产生不同的文档。

## Consequences

- 之后的原生页可以按任意深的路径读写和删除偏好，不需要各自拼 `JsonObject`。
- 现有键都不超过两层，读写结果与以前相同；`a.b.c` 这样的键现在按三层解释，原先会被当成 `a` 下名为 `b.c` 的一项，代码里没有这样用的地方。
- 不合法的路径（空段）静默不写；调用方传的都是常量，出错在测试和代码审查里能看出来。

## Verification

- `tests/ui/preference_path.cpp` 用 `cl /std:c++17 /W4 /WX` 单独编译并运行通过：拆分、表项键里的点、任意深读取、从默认值补中间层、挡路的非对象值、删除不补中间层。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`），没有启动窗口。
