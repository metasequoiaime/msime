# macOS 迁移：来源测试断言逐条核对

[macos-feature-inventory.md](macos-feature-inventory.md) 回答「每个源文件去了哪」。这一份回答下一个问题：**来源用测试钉住的每一条行为，在这边是什么状态。** 五条比对轴里有两条出过货：参考窗口截图那条产出了 #3247（侧边栏分组与帮助页）与 #3258（快捷键页），这一条测试断言轴产出了 #3280（恢复默认设置）与 #3298（双拼方案菜单）；控制项标识、运行时偏好键与符号这三条没有新发现。五条轴的全貌见 [macos-parity.md](macos-parity.md) 的结论表。

范围是来源 `platforms/macos/tests` 下全部 93 条 `require(...)` 断言，按测试文件分组。核对方式（可重跑）：

```sh
ref=/path/to/MSIME-apple
grep -rhoE 'require\([^,]*,\s*"[^"]+"' "$ref"/platforms/macos/tests/*.mm "$ref"/platforms/macos/tests/*.cpp \
  | sed -E 's/.*"([^"]+)"/\1/' | wc -l   # 93
```

结论先写在前面：**91 条已对齐，1 条曾是真缺口（已补），1 条是刻意分歧。**

## PreferencesWindowTests.mm（40 条）

设置界面。绝大多数逐条对得上，以下四条值得单独记。

| 断言 | 状态 |
| --- | --- |
| The settings footer restore button was not found. | **曾缺，已补（#3280）。** 这是 93 条里唯一一个真缺口——参考每页底部都有「恢复默认设置」，目标只有「保存设置」。 |
| Selecting Shuangpin left the schema menu disabled. | **曾不一致，已改（#3298）。** 参考是 `_shuangpinSchemeButton.enabled = storedScheme == 1`，目标此前一直可改。 |
| The Wubi settings row remained visible after another scheme was selected. | 已对齐。参考 `_wubiSettingsRow.hidden = storedScheme != 2`，目标是五笔区块的 `draft.scheme === "wubi"` 门控。 |
| Bracket paging UI did not disable edge selection.（及其反向 Edge selection UI did not disable bracket paging.） | 已对齐，而且三条路径都堵住了：开翻页键关以词定字、开以词定字关对应翻页键、以及被占用的键组在单选里直接 `disabled`。所以 `Preferences::validate()` 的 `ConflictingKeyBindings` 在界面上走不到，只是存储层的兜底。 |

其余 36 条覆盖本地输入模式、皮肤卡片、候选窗预览、词库状态行、悬浮工具栏开关、学习数据清除确认、跟随光标与颜色覆盖、侧边栏导航项不可取消选中等，均有对应实现。两处是模型不同而非缺失：

- **标点**：参考用 `alwaysChinesePunctuation` / `alwaysEnglishPunctuation` 两个互斥的钉住标志加一个跟随态；目标用一个 `chinese_punctuation` 状态加独立的 `smart_punctuation` 开关。目标这侧更能表达（参考没有「钉成英文标点同时开智能标点」这种组合）。
- **翻译服务**：参考是单选 provider（账号 / 腾讯 / DeepLX），所以需要「DeepLX endpoint 行跟随选择」这条不变量；目标是三家各自独立的开关与凭据区块，各自的字段在各自区块里，等价保证。

## InputMenuTests.mm（16 条）

输入菜单。这 16 条断言钉的是来源菜单的条目集合、分隔位置与 selector，对应物是 `platforms/macos/src/input/InputMenu.h` 里的菜单构造器，由 CTest `input-menu` 覆盖；`InputMenu.h` 同时提供生效菜单使用的主题处理（`ApplyMetasequoiaMenuTheme` 按 `menu_theme` / `theme` 解析 dark/light/system）。

实际生效的输入菜单此后按 MSIME-Windows 的托盘菜单重排，由 `-[MSIMEInputController menu]`（`platforms/macos/src/input/InputController.mm`）构造：中文输入 / 英文输入 / 英文候选模式（⌃⇧E）/ 繁体输出 / 全角字符 / 中文标点 / 显示译文 / 输入方案（当前方案）▸ / 主题（当前主题）▸ / 悬浮工具栏 / 水杉表情面板… / 云剪贴板… / 水杉屏幕键盘… / 手写输入… / 开始/结束语音输入 / 水杉输入法设置… / 关于水杉输入法…。简繁输出是一个勾选项，输入方案与主题各是一个以当前选择命名的子菜单，词库从设置窗口进入。来源里单列的「检查更新…」「语音输入设置…」收进设置窗与悬浮工具栏，管理页统一进设置窗口。这份菜单的条目数、标题与 selector 由 `platforms/macos/tests/input/ShortcutTest.mm` 断言，CTest 目标 `shortcut`。

## InputControllerKeyRoutingTests.mm（14 条）

11 条是组合输出（你 / 你们 / 爷 / 你好 / 日期），由引擎驱动；目标的等价覆盖是 `platforms/macos/tests/input/ShortcutTest.mm` 的 `TestRealSessionComposition()`（#3240），走真引擎 → 真控制器 → 文本客户端。

- `Wubi auto-commit fired before the fourth code.` — `platforms/macos/src/core/WubiCommitPolicy.h` 的 `MSIMEShouldAutoCommitWubi`，控制器直接引用。（2026-10-08 起该头文件与分支已删：判定移进共享 `crates/input-runtime` 的 `Runtime::dispatch`，可由共享偏好 `wubi_auto_commit_unique` 关闭；当时那条断言守的是一段因共享路径先上屏而不可达的代码，见 `.agents/notes/implemented/feature/2026-10-08-wubi-auto-commit-unique-preference.md`。）
- `IMKCandidates does not support moveUp:/pageUp:.` — 这两条是参考在**记录平台限制**，说明它为什么自绘候选窗。目标同样自绘（`platforms/macos/src/candidate/CandidatePanel.mm`），结论已内化，没有要实现的东西。

## UninstallerTests.mm（12 条）

保留用户数据卸载、连用户数据一起删、重复卸载、bundle 已不在时清残留、报告去向、落入废纸篓。目标是 `crates/host-macos/native/uninstaller.mm` 的 `msime_macos_uninstall_input_source`，CTest 目标 `shared-uninstaller`。

## UpdateControllerTests.mm（5 条）

Sparkle 驱动就绪状态、手动检查激活 accessory UI 并转发给 Sparkle、两处缓存不得过期。目标 `platforms/macos/src/core/UpdateController.mm`，CTest 目标 `update-controller`。本仓库的 bundle 模板不声明 `SUFeedURL`（更新入口是共享 About 页），因此另钉住三路选择：有 feed 才启动 Sparkle；无 feed 的应用明确说明限制并在确认后打开固定官方发布页；非应用进程保持不可用。确认、取消和发布页打开失败均由替身覆盖，不在测试中打开浏览器或真实弹窗。

## 其余三个文件（6 条）

| 文件 | 断言 | 状态 |
| --- | --- | --- |
| InputSourceRegistrationTests | 注册收到已安装 bundle 的 URL；父输入源找到之前不得启用输入模式 | 目标 `platforms/macos/src/input/InputSourceRegistration.mm`，CTest 目标 `input-source-registration` |
| CandidateSelectionStateTests | 重置不得留下过期的引擎索引；分页夹具候选数够用 | 前一条由结构保证：选中项归共享运行时，宿主不再记索引，`crates/input-runtime/src/runtime.rs` 的 `refresh` 每次重建候选先把高亮归零（`CandidateSelectionState.h` 只被未参与构建的直连适配器使用，已随 C++ Engine 删除）。后一条是 CTest 目标 `candidate-pagination` |
| FloatingToolbarPanelTests | 关掉一个开关再打开要恢复按钮 | 已对齐 |
| FloatingToolbarPanelTests | **四个开关全关，齿轮还在** | **刻意分歧。** 参考的 `MetasequoiaFloatingToolbarItemKeys()` 只有四项，齿轮不可关；目标把齿轮也做成可开关，并多出表情与屏幕键盘两项。理由：目标的工具栏组件本来就更多，且关掉齿轮不困人——输入菜单里仍有「水杉输入法设置…」，手写与语音按钮恒常存在，工具栏不会变成空条。 |

## 这一轴覆盖的范围

这 93 条覆盖的是来源用测试钉住的行为。来源没写测试的行为不在其中，由另外两条轴补齐：文件与符号层见 [macos-feature-inventory.md](macos-feature-inventory.md)（111 个源文件、518 个函数与方法逐个定位），方法与结论见 [macos-parity.md](macos-parity.md)（截图、23 个控制项标识、27 个运行时偏好键，以及按来源注释里的理由逐条回验的行为层比对）。

刻意不跟的分歧只有上面那一条齿轮；两处模型差异（标点、翻译 provider）目标是超集，写在上面。
