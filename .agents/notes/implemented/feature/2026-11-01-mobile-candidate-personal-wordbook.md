# Agent Note: 移动键盘的「加入生词本」

Status: implemented

## Problem

[桌面候选窗的「加入生词本」](2026-11-01-desktop-candidate-personal-wordbook.md)之后，iOS、Android 和鸿蒙的键盘还没有入口。三端的难处不一样：Android 键盘和设置在同一个应用目录里；iOS 键盘扩展只有开了完全访问才写得进与 App 共用的 App Group；鸿蒙从 API 12 起输入法扩展用独立沙箱，键盘根本写不到背单词所在的设置应用目录。

## Decision

三端都放在候选的长按菜单最后，标题「加入生词本」，只在这一行候选带着可上屏的释义时出现，和桌面的候选右键菜单同一个位置。

- Android：`ImeCandidates.showCandidateMenu` 在释义和词库操作之后加一项（id 3000，排序 1000），条件是这一行有可上屏的义项。`MSIMEInputService.addCandidateToPersonalWordbook` 在 `commonPhrasesWorker` 上调新的 JNI `personalWordbookAddRaw`（C 入口 `msime_client_personal_wordbook_add`），目录与打字统计、背单词相同，结果经 `notice` 显示在诊断行。读音取候选旁正在显示的音标。
- iOS：`candidateMenuElements` 在以词定字、释义和词库操作之后加一组（候选条和展开面板两处都有），`SharedUI/vocabulary/PersonalWordbookStore` 在后台队列调同一个 C 入口，目录是 `VocabularyReviewStore.defaultRoot`。没有完全访问时不调用，直接提示「开启「允许完全访问」后才能加入生词本」。iOS 键盘还不画音标，所以不带读音。
- 鸿蒙：`CandidateManagementAction.personalWordbookAction` 接在手机长按的整份菜单和 2in1 触屏长按的释义后面，动作里记着打开菜单时的候选原文和释义，点下去时与那一格现在的候选比，不一样就不收（菜单开着时候选栏可能重建）。键盘把词写进自己沙箱的 `files/state/keyboard-vocabulary/`（`PersonalWordbookPolicy`），这本书作为键盘的镜像条目 `personal-wordbook` 在完整体验模式下发布；设置应用回到前台导入时调新的同步 C 入口 `msime_client_personal_wordbook_merge`（`client-services::vocabulary::personal::merge_published`），只并进上次以后新出现或改过的词条，并把这一版存成「上一版」。
- 鸿蒙键盘那本是一个待交的队列：设置应用发布时在清单的 `applied` 里写明并到了哪一版（只写生词本的两条），键盘获得焦点和每次加词之前看到它与自己发布着的那一版相同、而且设置应用已经看到了自己最近一次的清走记录，就调 `msime_client_personal_wordbook_drain`（`drain_published`）把那一版里完全相同的词条清走，再把清走记录 `keyboard-vocabulary/drained.json`（`{drains, fingerprint}`，镜像条目 `personal-wordbook-drained`）的次数加一。设置应用看到一条没用过的清走记录、记录里的指纹就是自己存的上一版时丢掉上一版，这时键盘那本里的词都是之后加的，全部并进来；用过的记录按同一个相对路径存在设置应用里，每条只用一次，否则清走以后又加回同样的词时，之后存下的上一版内容相同，会被旧记录误当成已清走。所以用户在背单词里移出一个词以后，从键盘再加同一条释义是新加的，会回到背单词里。合并和清走的请求只带文件路径，生词本按单本书的上限（8 MiB）由共享层去读。设置应用那本已满、有词没并进来（`overLimit` 大于 0）时不记成已并过，键盘也就不清走，用户腾出地方后下次回到前台再并。
- 鸿蒙的提示：键盘发布时机除了原有的失焦和获得焦点，加入成功后也立即发布一次。基础访问模式下提示后面补一句「开启完整体验模式后在背单词里可见」，没有共享沙箱时补「仅保存在键盘里，暂时无法同步到背单词」（开了完整体验模式也到不了）。

鸿蒙的设置页（共享 React 页面经 ArkWeb）提供 `removePersonalWord`，背单词页因此显示生词本的说明和「移出生词本」；导出没有接，鸿蒙设置页里没有它。iOS 和 Android 的原生背单词页照常把生词本当一本书列出、复习和删除，没有导出和逐词移出。

## Alternatives considered

- **鸿蒙键盘直接把生词本镜像成一份，两边谁晚谁赢**：和偏好的镜像一样简单，键盘也能看到设置这边的删改。不用：生词本两边都会改（键盘加词，设置页移出、清空），按整份覆盖时，键盘加一个词就会把设置页刚移出的词带回来，反过来设置页的一次清空也会抹掉键盘还没交过去的词。只并新增的部分，两边的改动都留得住。
- **只在键盘回答「已在生词本里」时让条目重新发布一次（比如每次加入都写一个递增序号）**：不用握手。不用：生词本的文档里没有放序号的地方，得给词条另加字段或另写一份发件箱；而且键盘那本仍然只增不减，5000 词上限按键盘那本算、隐私残留也一直在。确认后清走能把这三件事一起解决，代价是两个进程之间多一条握手和一份只有指纹和次数的清走记录。
- **清走时整份删掉键盘那本，不按词条比**：少一个原生入口。不用：「加入生词本」在工作线程上写这本书，主线程上直接删文件会和它抢，而且会把交出去以后又在键盘里改过释义的词一起删掉；按词条比、在原生层取同一把词书目录锁做，两样都不会发生。
- **清走记录只写指纹，不带次数**：少一个字段。不用：指纹只看内容，清走以后又加回同样的词，生词本和清走记录的指纹都和上一轮一模一样，镜像认为没变，设置应用收不到，键盘却会按旧的确认马上再清一次，这个词就丢了。
- **鸿蒙键盘把词写进共享目录，由设置应用直接读**：少一次复制。不用：基础访问模式下键盘对共享目录只读，而且共享目录不是背单词的目录，设置应用一样要合并。
- **iOS 在没有完全访问时也调用，失败了再提示**：少一个判断。不用：失败时共享层只能回答存储出错，用户看到「无法加入生词本」不知道该去开完全访问。

## Consequences

- 三端开着候选释义的用户，都能从长按菜单把释义收进生词本；没有释义时菜单不变。
- 鸿蒙键盘那本只留着还没交出去的词，5000 词上限实际按设置应用那本算；设置应用并过、键盘下次获得焦点清走以后，键盘沙箱和共享沙箱里那本只剩之后加的词，设置应用下次回到前台时删掉存下的上一版。这一轮走完之前，在背单词里删书或移出一个词，不会立即删掉这几份副本，PRIVACY.md 写了。
- 基础访问模式下键盘不发布，词一直留在键盘沙箱里，开启完整体验模式后才交过去；模拟器上和签名不带 data-group-id 时没有共享沙箱，键盘加的词到不了背单词，只能看到提示里补的那一句，要清除应用数据或卸载才会删掉。
- 设置应用回到前台时的合并和键盘获得焦点时的清走都在主线程上同步调用原生层，要取词书目录的排他锁：工作线程上正在导入一份大词表时，主线程会等它做完。生词本平常很小，共享沙箱的导入本来也都在主线程上同步做，这里没有另起线程；真遇到卡顿再把合并改成异步，导入回调里等结果。
- 设置应用那本满了时，键盘那本里的词每次回到前台都会再并一次，直到腾出地方；这期间用户移出的、同在这一版里的词会被再并回来。
- iOS 的音标、鸿蒙和 iOS 的设置页导出都还没有。

## Verification

本机：`cargo test -p msime-client-services --lib vocabulary` 136 项通过（新增：设置这边移出的词在键盘清走以后再加回来能回到背单词；清走留下交出去以后又改过的词条；上一版读不懂或不在时照常合并），`cargo test -p msime-host-api --lib -- personal_wordbook` 2 项通过（合并、清走两个入口的路径校验和应答），两个 crate 的 `cargo clippy --lib --tests -- -D warnings` 无警告。PR 的 CI 里，`msime-client-services` 的整套单测由 `android-device-pr` 作业在 API 35 模拟器上跑（本 PR 通过）；`msime-host-api` 的这两项只在本机跑过，`ci-platforms.yml` 只跑 host-api 的 `windows_handwriting::`。Android：`platforms/android/check-host.sh` 通过，`CandidateManagementSmoke` 新增「加入生词本」的 id 不与释义和词库操作相撞、排在最后、只在有可上屏释义时出现。鸿蒙：`platforms/harmony/tests/run.sh` 和 `scripts/test-harmony-*.py` 通过，`personal-wordbook-policy.test.ts` 覆盖两种补充语、合并请求只带路径、已满时不记成已并过、清走记录每次都不同、键盘只在设置应用确认了它发布着的那一版并看到最近一次清走时才清走，`keyboard-logic.test.ts` 覆盖菜单顺序和点下去时核对候选；PR 的 CI 不编译 ArkTS（`HarmonyOS host` 作业只跑 `tests/run.sh`，hvigor 只在 `release-harmony.yml` 里），所以在本机用 DevEco 命令行工具 `ohpm install && hvigorw assembleHap --no-daemon` 编过一次，`CompileArkTS` 通过，改动的文件没有 ArkTS 警告（这次打包没有放原生库，只用来编译，没有安装）。iOS：`VocabularyReviewTests` 新增菜单只在有释义时出现的用例，由 PR 的 iOS CI 运行；菜单里这一组排在最后没有用例钉住，要实例化键盘控制器和整套候选状态。三端都没有在真机或模拟器上点过这个菜单项，鸿蒙的共享沙箱和清走的握手只能在带授权签名的真机上验证。
