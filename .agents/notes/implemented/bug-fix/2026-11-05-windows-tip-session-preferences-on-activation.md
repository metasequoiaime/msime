# Agent Note: Windows TIP 新建宿主会话时立即套上偏好快照，再次激活后屏障重新应用当前修订

Status: implemented

## Problem

[#10867](https://github.com/metasequoiaime/msime/issues/10867)（重复报告 [#10887](https://github.com/metasequoiaime/msime/issues/10887)）：Windows 上用小鹤双拼打 `qkkl` 选「情况」，上屏「情况kl」；`qigk` 选「奇怪」上屏「奇怪k」。只在从「英语 美式键盘」切到水杉拼音之后出现，出现后那个应用里一直如此，重开应用就好，重启输入法服务不好。维护者在引擎上复现过：双拼会话正常，把同样的按键交给全拼会话就一字不差地得到这两个结果。

Windows 版有两份引擎会话：候选窗由 Server 的会话画，行内组字和上屏由应用进程里 TIP 的宿主会话负责。按源码，TIP 那份会话停在全拼的路径是：

- TIP 的宿主会话由 `CCompositionProcessorEngine::SetupLanguageProfile` 按 `HostConfiguration::hostOptions` 建，`hostOptions` 来自状态目录的 `runtime-options.json`（`tsf/HostConfiguration.cpp` 的 `LoadConfiguration`）。这个文件是 Server 首次准备状态目录时写的，之后不改写，里面的 `preferences` 是当时的版本默认值，方案是全拼。用户选的方案只在偏好快照 `preferenceSnapshot` 里，要靠 `update_preferences` 套上去。
- `SetupLanguageProfile` 在会话无效时只建会话，不套快照；只有会话已经有效的分支和词库维护后重建的分支（`RefreshHostSessionForDictionaryMaintenance` 的 Restore）套快照。
- 建好之后靠 500ms 的偏好定时器（`TIMER_REFRESH_HOST_PREFERENCES`）套快照。Server 支持 `OrderedPreferences` 时，定时器只在 `configuration->revision > _preferenceBarrier.applied()` 时才发起一次有序应用。
- `Deactivate` 删掉整个引擎（宿主会话随之销毁），`_ClearDeferredKeyDowns` 调的 `PreferenceBarrier::reset()` 保留 `applied_`。同一个 TIP 对象再 `ActivateEx` 时建的是新的全拼会话，而屏障还记着旧会话套过的修订；用户不改设置，修订号就不会变大，新会话在这个进程里永远是全拼。新进程里屏障从 0 开始，所以重开应用就好；Server 那边的会话一直是双拼，所以重启服务不好。

这条链路的每一步都在源码里核对过，但 Windows 上「切键盘后 TSF 复用同一个 TIP 对象」这一点没有在真机上确认。

## Decision

- **新建宿主会话只走 `msime::tsf::create_host_session`（`tsf/HostSessionStart.h`）。** 它按宿主选项建会话，快照非空时立即 `update_preferences`。偏好套不上时会话照样可用，返回值只表示会话建没建起来。`CCompositionProcessorEngine::InitializeHostSession` 改为同时接收宿主选项和偏好快照并调用它，`SetupLanguageProfile` 的新建分支和词库维护的 Restore 分支都经过这里，Restore 分支原来自己套快照的几行随之去掉。
- **引擎随 `Deactivate` 销毁后，屏障忘掉已应用的修订。** `PreferenceBarrier::forget_applied()` 在 `reset()` 之外把 `target_` 和 `applied_` 清零，`Deactivate` 删掉引擎后紧接着调用。下次激活后第一轮偏好定时器把当前修订当作新目标，照新进程首次激活那样走一次有序应用（`_PollPreferenceTransition`），同时按快照更新 `Global::InputModeScheme` / `Global::InputModeIndicator`。会话在 `SetupLanguageProfile` 里已经套过同一份快照，这次重复应用不改变会话状态。
- **有序切换（#9598）的语义不变。** `forget_applied()` 之后屏障不在进行中，旧会话还在路上的请求回执（`request != 0`）照样被 `receive` 拒绝；之后的 `begin` / `sent` / `receive` / `complete` 与原来一样。`reset()` 本身不改：它在超时、配置缺失、Server 不支持有序偏好时也会调用，那些场合会话还在，保留 `applied_` 是对的。

## Alternatives considered

- **只改屏障，不在建会话时套快照。** 改动更小，偏好定时器 500ms 内就会套上。但这 500ms 里打的字仍按全拼组字；词库维护期间或配置读不出来时定时器那一路会放弃（`_PollPreferenceTransition` 的「没有可应用的对象」分支），会话就继续停在默认方案。建会话的地方本来就拿着快照，就地套上没有时序问题（新会话没有组字），所以两处都改。
- **只在建会话时套快照，不改屏障。** 会话本身对了，但 `Global::InputModeScheme` / `Global::InputModeIndicator` 只在有序应用完成时更新；而且屏障留着旧修订，就等于留着「这个会话已经应用过这个修订」的错误记录，以后再有路径依赖它会重蹈覆辙。在 `SetupLanguageProfile` 里也照抄一份模式计算可以补上前一点，但会让同一段「从快照算输入模式」的逻辑出现两份。
- **让 `runtime-options.json` 跟着偏好一起改写。** 准备流程明确不改写已存在的状态目录（`platforms/windows/PrepareHost.md`）；把偏好写进两处还会带来哪份为准的问题。

## Consequences

- 收益：同一应用里切换键盘再切回来，TIP 的宿主会话从建好的那一刻起就是用户的方案，与 Server 的会话一致；词库维护后的重建与首次建会话走同一个入口，以后加新的建会话路径也不会漏掉快照。
- 代价：再次激活后第一轮定时器会把同一份快照再套一次，并短暂进入有序屏障（期间按键排队），与新进程首次激活时一样。
- 测试：`platforms/windows/tests/input/host_session_preferences.cpp`（CTest `windows-host-session-preferences`）用真实引擎会话和合成词库验证：同一份全拼宿主选项，只建不套是全拼、`nihc` 选首选留下余码；带上双拼快照建是双拼、首选「拟好」整段上屏。`platforms/windows/tests/runtime/preference_transition.cpp` 补了 `forget_applied()` 的用例：同一修订重新触发应用、旧请求回执仍被拒绝、之后的有序切换照常。
- 未在 Windows 真机上验证 TSF 的 Deactivate/ActivateEx 复用路径，需要报告者在「英语 美式键盘」切到水杉拼音后复验。
