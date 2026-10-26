# Agent Note: 脚本插件第二批能力的设置界面：授权分组、持久存储的删除确认、设置表单和数据大小

Status: implemented

## Problem

[脚本插件第二批能力](../../proposed/architecture/2026-10-26-script-plugin-capabilities-v2.md)的运行时和宿主已经能处理 `insert_candidates`、`persist_data`、清单设置项、随包数据和 `extended` 指令，设置页的 FFI（`script_plugin_state`、`set_script_settings`、`clear_script_data`）也有了，但共享设置页和 Windows 原生设置页都还停在第一批能力上：

- 能力列表只有 8 项。共享页的 `sortCapabilities` 只保留认识的能力，申请了新能力的包在授权对话框和详情页里悄悄少了这两项，用户勾不到也看不见；Windows 的 `sort_capabilities` 同样丢掉它们。社区页把它们列成「未知能力」并说安装会失败。
- 互斥规则只有一对。`persist_data` 和 `rewrite_commit` 不能同时授予，页面不知道，勾上之后要等宿主以 `plugin_script_grant_invalid` 拒绝。
- 维护者接受 `persist_data` 的条件是单独一项、默认不勾、红字说明。
- 宿主写下一份不含 `persist_data` 的授权就删掉插件存下的数据，包括撤销全部授权的「启用」开关（`set_script_grant(enabled=false, granted=[])`）和更新之后重新授权时没有再勾它。页面上这几个动作都没说会删数据。
- 详情页没有设置项、数据大小和「清除数据」。

## Decision

**能力列表、依赖和互斥按运行时补齐。** `SCRIPT_CAPABILITIES` 按 `Capability::ALL` 的顺序到 10 项，`insert_candidates` 依赖 `read_candidates`。`EXCLUSIVE_CAPABILITIES` 改成两对（`rewrite_commit`×`read_composition`、`persist_data`×`rewrite_commit`），`exclusivePartner` 换成返回全部互斥项的 `exclusivePartners`：`rewrite_commit` 同时和两项互斥，勾上它时两项都让开，复选框说明写第一项勾着的那个。Windows 的 `PluginPageModel.h` 逐条照搬，文案一字不差。

**授权对话框按「读文字」「改文字」「记住数据」分三组**（`CAPABILITY_GROUPS`）。读文字是三项读取；改文字是四项修改、`provide_commands` 和 `insert_candidates`，这两项都是往候选里放插件自己的文字；`persist_data` 单独一组。分组只影响显示，依赖和互斥照旧跨组生效。

**`persist_data` 的标题是「长期保存数据」，说明就是设计笔记里的红字原文**（`PERSIST_DATA_WARNING`），对话框里用危险色画。它从不预先勾上：对话框的初值只在授权有效且启用时取现在授予的，内容变了之后重新授权、停用之后再启用，初值都是空的；「调整授权」时它照现在的授权显示。

**会删数据的写入都先确认。** 判断规则是 `grantDeletesStoredData`，和宿主的 `script_grants::set` 一致：写下的授权不含 `persist_data`，而授权文件里那一条（不论内容哈希）授予过它，或者 `storage_bytes` 大于 0。对话框里第一次点确认只出一行红字「这次授权不包括「长期保存数据」，插件已存的数据（N KB）会被删除，不能恢复」，按钮换成「删除数据并授权」，再点才写；改了任何勾选就要重新确认。「启用」开关关掉时用页面的确认对话框，按钮是「停用并删除数据」。「清除数据」也先确认。

**详情页加「设置」和「数据」两组。** 设置表单由清单 `settings` 生成：布尔是开关，选项是下拉框，数字是带 `min`、`max` 的数字框，文本是带 `max_length` 的输入框（`max_length` 按 UTF-16 码元计，和 JS 字符串长度、Windows 的 `std::wstring` 长度一致）。改完点「保存设置」整份写入 `set_script_settings`。范围和长度页面先拦；宿主以 `plugin_script_setting_invalid` 拒绝时，从 `detail` 里的「设置项 <key>」认出是哪一项，原因写在那一项下面，认不出时写在表单底部。「数据」一组有「自带数据 N KB」（`data_bytes`）和「已存数据 N KB」（`storage_bytes`）加「清除数据」；后者在授权文件里那一条授予过 `persist_data`、或者存储里有数据时显示，所以更新之后还没重新授权时也能清。KB 向上取整，有数据就至少 1 KB。状态在打开详情和授权文件 `revision` 变化时重读：撤销 `persist_data` 会删掉存储。

**说明文字随清单变。** 声明了 `arguments = "extended"` 指令的插件，「提供 / 指令」的说明把「之后打的字母会交给」换成「之后打的字母以及数字和运算符号会交给」，「只取决于这些字母和随机数」换成「这些字符」。有设置项时对话框说明插件能读到设置值、不需要授权；没有申请任何能力的插件，那句「只能知道发生了什么事件」按清单接上「读自己包里的数据」「读你在设置项里填的值」。

**Win7 设置窗口用同一份模型。** `PluginScriptsForm.h` 的能力列表每项前面写组名，静态文本画不了红色，`persist_data` 的说明前面加「注意：」；授权确认框里带上红字原文和删除说明，撤销、清除数据同样先确认；设置表单是「标题：值」的列表加一个下拉框或输入框和「保存设置」，宿主拒绝时保留草稿并标出那一项。

**新失败码有中文说明**：`plugin_script_setting_invalid`（设置没有保存：值不符合插件的要求）、`plugin_script_settings_format`（设置文件是更新版本写的）、`storage`（插件的数据读写失败）。

## Alternatives considered

- **设置项改一下就保存，不要「保存设置」按钮。** 最强的理由是别的设置页大多即改即存，用户不用记得点保存。不用：`set_script_settings` 是整份替换，每写一次 `revision` 加一、输入法下一次聚焦重建插件；文本框每打一个字写一次，没打完的值还会被宿主当成不合规拒绝。开关和下拉单独即存、文本和数字另存又会让同一个表单有两种保存方式。
- **对话框里的删除确认也用页面的确认对话框。** 最强的理由是和「停用」「清除数据」一致。不用：授权对话框本身是模态的，再叠一层模态，两层的焦点、Escape 和遮罩点击都要另外协调（Windows 的 `ContentDialog` 同一时间只能开一个）；在原对话框里把按钮换成「删除数据并授权」，说明就在用户刚取消的那一项下面。
- **「启用」开关关掉时只停用、保留授权和数据。** 最强的理由是不用确认，数据也不会因为一次误关就没了。不用：开关的说明一直是「关闭会撤销全部授权」，改语义要同时改授权初值、列表标记和两个平台的测试，超出这次的范围；宿主侧停用（`enabled=false`、能力不变）本来就不删数据，以后要做「暂停」可以单独加一个控件。

## Consequences

- 申请了新能力的包在两个平台的授权对话框、详情页和社区页里都能看到、勾到，规则和宿主一致，不再要等宿主拒绝。
- 每个会删数据的动作多一次确认。代价是对话框里删数据要点两次确认。
- 设置表单不即时生效：用户要点「保存设置」，输入法在下一次切换输入框时才用上新值，表单上方的说明写着这一点。
- 「设置项 <key>」是从宿主的中文 `detail` 里认出来的，宿主改了这句话的格式，错误就退回写在表单底部，不会丢。
- 验证：共享页 `apps/desktop/tests/settings/script-plugin-capabilities-v2.test.tsx` 覆盖依赖和两对互斥、分组和红字、重新授权时不预先勾选、删除前的二次确认、扩展指令的说明、设置表单的保存和逐项报错、数据大小与清除确认、关开关时的确认，以及社区页不再把新能力当未知能力。Windows 的规则由 `platforms/windows/tests/ui/plugin_page_model.cpp` 覆盖，WinUI 页只在 PR 的 Windows CI 里编译和走页面冒烟。真实设置应用里的点选没有在本机跑过。
