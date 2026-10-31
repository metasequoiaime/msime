# Agent Note: 移动键盘的「加入生词本」

Status: implemented

## Problem

[桌面候选窗的「加入生词本」](2026-11-01-desktop-candidate-personal-wordbook.md)之后，iOS、Android 和鸿蒙的键盘还没有入口。三端的难处不一样：Android 键盘和设置在同一个应用目录里；iOS 键盘扩展只有开了完全访问才写得进与 App 共用的 App Group；鸿蒙从 API 12 起输入法扩展用独立沙箱，键盘根本写不到背单词所在的设置应用目录。

## Decision

三端都放在候选的长按菜单最后，标题「加入生词本」，只在这一行候选带着可上屏的释义时出现，和桌面的候选右键菜单同一个位置。

- Android：`ImeCandidates.showCandidateMenu` 在释义和词库操作之后加一项（id 3000，排序 1000），条件是这一行有可上屏的义项。`MSIMEInputService.addCandidateToPersonalWordbook` 在 `commonPhrasesWorker` 上调新的 JNI `personalWordbookAddRaw`（C 入口 `msime_client_personal_wordbook_add`），目录与打字统计、背单词相同，结果经 `notice` 显示在诊断行。读音取候选旁正在显示的音标。
- iOS：`candidateMenuElements` 在以词定字、释义和词库操作之后加一组（候选条和展开面板两处都有），`SharedUI/vocabulary/PersonalWordbookStore` 在后台队列调同一个 C 入口，目录是 `VocabularyReviewStore.defaultRoot`。没有完全访问时不调用，直接提示「开启「允许完全访问」后才能加入生词本」。iOS 键盘还不画音标，所以不带读音。
- 鸿蒙：`CandidateManagementAction.personalWordbookAction` 接在手机长按的整份菜单和 2in1 触屏长按的释义后面。键盘把词写进自己沙箱的 `files/state/keyboard-vocabulary/`（`PersonalWordbookPolicy`），这本书作为键盘的镜像条目 `personal-wordbook` 在完整体验模式下发布；设置应用回到前台导入时调新的同步 C 入口 `msime_client_personal_wordbook_merge`（`client-services::vocabulary::personal::merge_published`），只并进上次以后新出现或改过的词条，然后照常把这一版复制下来，作为下一次比较的「上一版」。键盘发布时机除了原有的失焦和获得焦点，加入成功后也立即发布一次。没有共享沙箱或在基础访问模式下，提示后面补一句「开启完整体验模式后在背单词里可见」。

鸿蒙的设置页（共享 React 页面经 ArkWeb）提供 `removePersonalWord`，背单词页因此显示生词本的说明和「移出生词本」；导出没有接，鸿蒙设置页里没有它。iOS 和 Android 的原生背单词页照常把生词本当一本书列出、复习和删除，没有导出和逐词移出。

## Alternatives considered

- **鸿蒙键盘直接把生词本镜像成一份，两边谁晚谁赢**：和偏好的镜像一样简单，键盘也能看到设置这边的删改。不用：生词本两边都会改（键盘加词，设置页移出、清空），按整份覆盖时，键盘加一个词就会把设置页刚移出的词带回来，反过来设置页的一次清空也会抹掉键盘还没交过去的词。只并新增的部分，两边的改动都留得住。
- **键盘在设置应用确认合并之后清空自己那本，当成队列用**：键盘那本不会一直变大，也不受 5000 词上限的影响。不用：要读设置应用清单里的 `applied`、核对指纹、在键盘里再调一次删除，多一条两个进程之间的握手；而键盘那本只是一份发往设置应用的记录，长到 5000 词之前不影响任何东西。代价写在下面。
- **鸿蒙键盘把词写进共享目录，由设置应用直接读**：少一次复制。不用：基础访问模式下键盘对共享目录只读，而且共享目录不是背单词的目录，设置应用一样要合并。
- **iOS 在没有完全访问时也调用，失败了再提示**：少一个判断。不用：失败时共享层只能回答存储出错，用户看到「无法加入生词本」不知道该去开完全访问。

## Consequences

- 三端开着候选释义的用户，都能从长按菜单把释义收进生词本；没有释义时菜单不变。
- 鸿蒙键盘那本生词本只增不减：键盘里累计加满 5000 个词之后，键盘这边会一直回答「生词本已满」，即使设置页那本已经移出过很多词。要解决就得做上面那个确认后清空的握手。
- 鸿蒙在模拟器上和签名不带 data-group-id 时没有共享沙箱，键盘加的词到不了背单词，只能看到提示里补的那一句。
- iOS 的音标、鸿蒙和 iOS 的设置页导出都还没有。

## Verification

`cargo test -p msime-client-services --lib vocabulary` 128 项通过（新增 `merge_published` 用例：设置这边移出的词不会因为键盘再发布而回来，上一版读不懂时照常合并），`cargo test -p msime-host-api --lib -- personal_wordbook` 2 项通过。Android：`platforms/android/check-host.sh` 通过（319 项 JVM 冒烟、设备套件源文件编译、契约检查），`scripts/test-android-jni-names.py` 通过。鸿蒙：`platforms/harmony/tests/run.sh` 通过，新增 `personal-wordbook-policy.test.ts`（请求的目录、提示和补充语、镜像条目只由键盘在完整体验模式下发布、合并请求），`keyboard-logic.test.ts` 更新了菜单顺序的断言；`apps/harmony` 的 `tsc --noEmit` 和 `scripts/test-harmony-*.py` 通过。ArkTS 的 `.ets` 编译和 iOS 的 Xcode 构建交给 PR 的 CI；iOS 新增 `VocabularyReviewTests.testPersonalWordbookAddsAGlossTheReviewPageLists`。三端都没有在真机或模拟器上点过这个菜单项，鸿蒙的共享沙箱只能在带授权签名的真机上验证。
