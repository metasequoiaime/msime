# Agent Note: Windows 设置页整句模型行的状态读取与重画

Status: implemented

## Problem

#10839：Windows 0.4.1 上「神经联想模型」装好以后，过一阵再打开设置又显示「下载」；点「下载」像没反应，多点几次才显示已安装。

原生设置页的神经联想模型和三种语法模型行（`platforms/windows/settings/main.cpp` 的 `sentence_pack_row`）在工作线程上调用 `msime_client_resource_packs` 读状态，读回、安装开始和结束、删除开始和结束后都调 `redraw_input()` 重画。`redraw_input()` 只在 `current_page_ == "input"` 时重画，而这几行所在的「输入」页 id 是 `"typing"`（`SettingsNavigation.h` 的 `pages`），`"input"` 只是路由别名，`current_page_` 永远不会等于它。于是：

- 打开设置时状态还没读回，行按「没装」画出「下载」；读回 `installed` 后不重画，页面一直停在「下载」，直到别的操作让整页重画。
- 点「下载」后「正在下载和校验模型…」和「取消」画不出来，安装结束也不重画，看上去点了没反应；用户再点又发一次安装。

另外读取失败（FFI 报错、内容解析不了、状态目录不可用）时页面不记失败，`sentence_packs_requested_` 已置位，本进程不再重读，行一直按「没装」画「下载」。

`resource_packs::list` 只看 `<state_root>/resource-packs/<id>/` 的标记文件和文件属性，没有删除逻辑。现有证据不支持「模型文件真的被删掉了」；用户机器上的文件是否还在，没有拿到现场确认。

## Decision

- 新增 `platforms/windows/settings/SentencePackModel.h`（不依赖 Windows 头文件）：`host_page` 是这几行所在的页面 id `"typing"`；`Catalog` 记状态读到哪一步（`unknown` / `known` / `failed`，加 `reading`、`stale`）；`row_view` 决定每一行的副标题和按钮。
- `redraw_input()` 改名 `redraw_sentence_packs()`，比较 `current_page_ == spk::host_page`。
- 状态没读回时行写「正在读取模型状态…」，不给「下载」「从文件导入」；读失败写「无法读取模型状态，请重试。」，只给「重试」（automation id `sentence-model-retry-<pack>`）。失败后不自动重读，免得失败、重画、再读一直转；点「重试」或再次进入「输入」页时重读。
- 同一时间只发一次读取。读的途中又要读（安装或删除结束、进页）时记 `stale`，读回后再读一次，避免先发的读取后回来、用旧结果盖掉新结果。读回的列表先放进局部变量，解析成功后才替换 `sentence_pack_statuses_`。
- 每次进入「输入」页都重读一次（与辅助码表插件的 `helpcode_packs_stale_` 同一处），已读回的状态在重读期间照常显示。

## Alternatives considered

- 只把 `"input"` 改成 `"typing"`：一行改动就能让读回的状态和安装进度画出来，是本次的主修复；但首屏仍会在读回前闪一下「下载」，读失败仍永久显示「下载」。读回前给「下载」正是用户会去点的那个按钮，所以连同读取状态一起改。
- 读失败后在下一次画页面时自动重读：不用加「重试」按钮；但失败的回调本身会重画，重画又触发读取，状态目录坏掉时会一直调 FFI。改成等用户点「重试」或重新进页。
- 点「下载」前若列表说已安装就先刷新：能挡住重复安装，但根因是行画错了；行在状态未知时不给「下载」后，这个分支没有入口。

## Consequences

- `tests/ui/sentence_pack_model.cpp`（CMake 目标 `windows-sentence-pack-model`）检查 `host_page` 是真实页面 id、`"input"` 路由打开的就是它，以及读取状态流转和每种状态下的按钮。把 `host_page` 改回 `"input"` 时这个测试失败。
- `main.cpp` 本身是 C++/WinRT，只能在 Windows CI 上编译；页面上的实际显示要在 Windows 上看。
- 状态读回前多出「正在读取模型状态…」这一屏，读取很快时一闪而过。

## Note audit

[octagram 语法模型](../../implemented/feature/2026-10-21-octagram-grammar-model.md)部分重叠：那篇定下 Windows 原生页模型行的下载、导入、删除和不自动安装，仍然成立；本篇只改状态读取、重画页面和读回前后的按钮，不改安装流程。
