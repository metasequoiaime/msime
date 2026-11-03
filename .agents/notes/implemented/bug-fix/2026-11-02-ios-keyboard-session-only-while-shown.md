# Agent Note: iOS 键盘只在可见期间持有引擎会话

Status: implemented

## Problem

2026-11-02 维护者在 iPhone XR（iOS 18.7.11）上验收 #10616 时，在水杉 App 的设置页和试用页之间来回切换，键盘扩展被 RunningBoard 以 `0xdead10cc` 结束（#10634）：进程只活了约 7 秒，被杀时所有线程都空闲，没有线程在做 I/O。说明不是「正在写时被挂起」，而是有描述符在线程之外长期持着 App Group 里的锁。

引擎会话从建立到销毁一直持着三把 App Group 里的锁：`user_data` 和 `dictionaries` 下各一个 `.msime-dictionary-access.lock` 的共享租约（host-api 的 `create_session` 经 `DictionaryAccess::try_session` 取得），以及资源是下载代次时 `generations/<代次>.lock` 的共享锁（client-providers 的 `dictionary_updates::lease`）。原来的代码有三处缺口：

- 取得过早。`KeyboardViewController` 的存储属性 `session = MetasequoiaInputSessionBridge()` 在控制器创建时执行，桥的 `init` 当场建会话。系统为输入框建了控制器、键盘却还没出现（或出现被打断）时，锁已经在手上。
- 只有一个释放点。会话只在 `viewWillDisappear` 里由 `suspendDictionarySession` 销毁，没有 `viewDidDisappear` 兜底，扩展级通知只注册了 `NSExtensionHostWillResignActive`，而且不放会话。
- 释放会静默失败。`suspendDictionarySession` 在还有组字时返回 false，会话和锁都留着，调用方既不重试也不记日志。按代码看 iOS 上走不到这一步：键盘不设高亮，`finishComposition` 总从第 0 行结束，引擎的 `finish_composition` 逐段上屏到没有组字为止（唯一提前返回的「📅日期」入口从不排在第 0 行），九键的 `finish` 最后总是 `Cancel`。但这一点靠的是引擎和运行时（插件行、快捷模式前缀）各条结束路径的细节，不是桥的契约，所以仍按会失败处理。

另有一条概率较低的路径：收起之后才到的 `viewDidAppear` 会重启两秒一轮的词库维护定时器，有活动的云词库快照请求时 `DictionarySnapshotWorker.tick` 可能重新取得 `worker.lock`。

## Decision

锁的持有期不超过键盘的可见期：会话在 `viewWillAppear` 里建、在 `viewWillDisappear` 里销毁，控制器创建时只准备选项。

- `MetasequoiaInputSessionBridge.init` 多一个 `startsSuspended` 参数。为 true 时 `msime_client_prepare_host` 成功后直接置 `suspended`，不建会话。键盘控制器用 `startsSuspended: true` 构造；`viewWillAppear` 本来就会同步 `resumeDictionarySession()` 并写 `dictionary_resumed ms=`，第一次出现和以后每次出现走同一条路。
- 挂起且选项已准备好（`recordsStateForNextSession`）时，桥只记状态、不报诊断：`updatePreferences` 把改动写进 `options["preferences"]` 并返回 true，下一个会话由 `engineOptions` 带上它；网格、全角、中文标点经 `applySessionState` 记在桥上、返回空快照，`createFocusedSession` 建会话时重放（AI 凭据和隐私标记原本就这样记、这样重放）。`viewDidLoad` 在键盘出现之前就按文档设一遍方案、网格、全角和学习偏好，所以这一条是必需的：否则方案切换报「九键模式切换失败」、网格不记下，建出来的会话回到 26 键。准备失败（选项为空）时照旧报失败。
- `applyInputContext` 在会话挂起时不再 `render(discardComposition())`：没有组字可丢，问会话只会拿回「输入运行时尚未准备完成。」画在键盘上。
- 控制器加 `isPresented`，`viewWillAppear` 置 true、`viewWillDisappear` 置 false。`idleForMaintenance` 要求它为 true，`viewDidAppear` 在它为 false 时不开始维护、不起定时器。
- 释放集中在 `releaseDictionarySession()`：`suspendDictionarySession()` 返回 false 时记 `dictionary_suspend_retry`，取消剩下的组字再试一次（用户的输入已经由 `finishComposition` 上屏，取消的只是不随它结束的那一截）；仍然失败记 `dictionary_suspend_failed`。宿主能走到的输入里没找到在 `finishComposition` 之后还留着组字的，这条退路是防御性的，控制器层面没有用例能触发它；它依赖的两步由桥的单测守着（见 Verification）。`viewDidDisappear` 在 `isPresented` 为 false 时再调一次，作为第二道保险。
- 真机验收用的诊断：`NSExtensionHostDidEnterBackground` 时记 `host_background session_held= presented=`，控制器释放时还持着会话记 `controller_released session_held=1`。`session_held` 看桥的 `holdsSession`（句柄非零），不看「没挂起」：运行时准备失败时 `init` 在置 `suspended` 之前就返回了，桥既没挂起也没有会话，按「没挂起」记会让准备失败的键盘在日志里显示成持锁，把重访信号变成误报。

## Alternatives considered

- **先上真机确认是哪条路径再改** — 分诊最初的建议，理由是根因还没用设备日志证实。但三处缺口单看代码就能确认，修法也不随复现结果变化；单测能证明控制器出现前、收起后都取得到独占锁。真机复现留作修完之后的验收，靠上面补的诊断行判断还有没有残余路径。
- **宿主进后台时也销毁会话** — 能覆盖「宿主进后台却不调 `viewWillDisappear`」这条路径。代价是回到前台时键盘可能不再收到 `viewWillAppear`，会话不在，按键只剩诊断，需要在 `WillEnterForeground` 里按 `isPresented` 重建，改动面和风险都更大。这次只记 `host_background` 诊断，真机上看到 `session_held=1` 再做。
- **桥在第一次按键时自己建会话** — 能让不调 `viewWillAppear` 就打字的旧单测不用改。但任何在键盘隐藏时到达的调用（`textDidChange` 一类的回调、`applyInputContext` 里的取消）都会把锁取回来，正是这次要堵的那一类路径。单测改为按真实的生命周期先调 `viewWillAppear`。
- **挂起时让所有 `dispatch` 都返回空快照** — 一处改完，不用逐个改设置状态的调用。但挂起时按键必须如实报「会话不在」（`testSuspendReleasesDictionaryAccessAndResumeStillConverts` 守的就是这个），而空快照会把角引号、空心括号这些键面状态重置成默认值。所以只放过只改会话状态的那几个调用。

## Consequences

- **收益**：只建过控制器、从没出现的键盘不持任何 App Group 锁；收起之后晚到的 `viewDidAppear` 不再重启维护；释放失败第一次有了重试和日志。
- **代价**：建会话的时机从控制器创建挪到第一次 `viewWillAppear`，首帧前的同步开销总量不变，但 `keyboard_loaded` 的 `session=` 现在只量准备选项，会话的内存算进 `keyboard_shown`。挂起期间改的偏好只记在桥上，不经 `msime_client_update_preferences` 校验；校验推迟到建会话时（`create_session` 同样 `validate()`），宿主写出不合法的值时表现为 `dictionary_resume_failed`，而不是改的那一下被拒。这些改动都来自宿主自己的代码（方案名、钳过范围的几何、学习开关），同样的值在键盘可见时也要过会话的校验。
- **代价**：直接构造 `KeyboardViewController`、不调 `viewWillAppear` 就打字的单测要补上这一步。PR 第一次跑 CI 时有 47 个这样的用例全部拿到「输入运行时尚未准备完成。」（九键、邮箱后缀、快捷短语光标、韩文、日文九键等），都按真实生命周期补了 `viewWillAppear(false)`，没有改产品代码去迁就它们。补完之后又露出一处真回归：`updateSchemeButton` 在 `viewDidLoad` 里向会话取双拼键面提示表，会话不在时取到空表，第一次出现的双拼键盘不画键位提示（`testTheShuangpinKeyHint*` 两条用例）。现在会话在 `viewWillAppear` 建好后重取提示表（`refreshShuangpinKeyHints`）；表不在 `LetterCaseInputs` 里，表变了要作废 `appliedLetterCaseInputs` 才会重画。凡是在 `viewDidLoad` 里向会话视图取值并缓存在控制器上的状态都有同样的风险，`sharedPreferences` 一类读桥上选项的不受影响。
- **重访信号**：真机诊断日志里出现 `dictionary_suspend_failed`、`host_background session_held=1` 或 `controller_released session_held=1`，说明还有路径在键盘不可见时持着会话。

相关决定：[收起时释放词库快照租约](2026-10-14-ios-snapshot-worker-lock-release.md) 处理的是快照工作者的 `worker.lock`；[低端机功耗](2026-10-27-ios-keyboard-low-end-power.md) 记了每次收起都销毁会话、出现时重建的代价；[崩溃诊断归因](2026-10-25-ios-crash-diagnostic-session-attribution.md) 列过 0xdead10cc 的候选路径，这一篇补上了会话本身的那一条。

## Verification

- `MSIMEKeyboardTests/NineKeyKeyboardTests/testABridgeThatStartsSuspendedHoldsNoLockUntilResumed`：只准备不建会话的桥在 `resume` 之前、`suspend` 之后都能对两个 `.msime-dictionary-access.lock` 取得独占 flock；挂起时选的小鹤双拼（会话有小鹤键面提示，`nihc` 打出「你好」）和九键在建出来的会话里生效。
- `MSIMEKeyboardTests/NineKeyKeyboardTests/testSuspendReleasesDictionaryAccessAndResumeStillConverts`：`releaseDictionarySession` 退路依赖的两步。组字没结束时 `suspendDictionarySession` 返回 false，会话还在（`holdsSession`）、两个 `.msime-dictionary-access.lock` 都取不到独占锁；`cancel` 之后再挂起成功，锁都放掉。控制器里「失败就取消再试」那三行本身没有用例触发，理由见 Decision。
- `MSIMEKeyboardTests/NineKeyKeyboardTests/testABridgeWhoseRuntimeFailedToPrepareHoldsNoSession`：资源目录为空、准备失败的桥 `holdsSession` 为 false，诊断里的 `session_held` 不会把它记成持锁。
- `MSIMEKeyboardTests/NineKeyKeyboardTests/testKeyboardHoldsTheDictionaryLeaseOnlyWhileItIsShown`：控制器 `loadViewIfNeeded` 加 `applyInputContext` 之后不持锁、不显示诊断，`viewWillAppear` 后持锁并能打出「你好」，`viewDidAppear` 后两秒维护定时器在走；`viewWillDisappear` 后定时器停、不持锁，晚到的 `viewDidAppear` 不重启定时器、不取回锁。
- `MSIMEKeyboardTests/NineKeyKeyboardTests/testABridgeThatStartsSuspendedHoldsNoGenerationLeaseUntilResumed`：把随包资源复制成 `dictionary-updates/generations/<64 位十六进制>` 这样的下载代次交给挂起起步的桥，`resume` 之前、`suspend` 之后都能对 `generations/<代次>.lock` 取得独占 flock，会话存在时取不到。Rust 侧 `a_session_on_a_downloaded_generation_holds_it_until_destroyed` 守着同一把锁从建会话持有到销毁。
- 真机：iPhone XR 上按 #10634 的操作（设置页与试用页之间快速来回切换，同时唤出、收起键盘）做 50 次以上，Console 里不应再出现 `dead10cc`，诊断日志里不应出现上面三条重访信号。
