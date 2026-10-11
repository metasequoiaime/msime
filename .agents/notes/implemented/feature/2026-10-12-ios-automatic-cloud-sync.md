# Agent Note: iOS 原生应用的自动云同步

Status: implemented

## Problem

iOS 原生应用「我的 › 同步」只有一页手动操作（「设置同步」上传或下载、「云词库」），页脚写着「在这台设备上手动上传或下载，不会在后台自动同步。」仓库负责人看到后问「？？？？不是自动的吗」：Android 早就有云同步开关（`home/CloudSync.java`、`account/SyncSwitch.java`、`account/SyncMergePolicy.java`），登录真实账号、打开开关后，每次回到前台按节流自动比对设置、常用语和词库。iOS 用户换手机、或在 iPad 和 iPhone 之间来回用时，只能记得每次手动上传、再到另一台手动下载。

鸿蒙不在本篇范围：输入法沙箱拦住了宿主和键盘之间的共享写入，同一套调度在那里接不上。

## Decision

照搬 Android 的规则，只在 iOS 的进程模型逼出差异的地方改，下文逐条写明差异和原因。

- **开关与进度（`CloudSyncState`，`SharedUI/core`）**：存在 App Group 的 `CloudSync/state.json`，读改写在同目录 `state.lock` 的 `flock` 里完成。字段与 `SyncSwitch` 对应：开关、绑定的账号 id 与登录、`bindingGeneration`、每个分类的游标和改动代数（`gen` / `clean`，不等即有待上传的改动）、本机收不下的云端常用语、上次尝试、上次成功、上次提示首次选择的时间、最近一次失败原因。
  - **差异：放 App Group 文件，不放宿主私有存储。** Android 的 `:ime` 进程经 `AccountSessionProvider` 的 `sync_dirty` 让主进程记改动；iOS 的键盘扩展没有能调用宿主的 IPC，只能写共享容器，所以状态文件本身放在 App Group，键盘直接给代数加一。
  - **差异：账号绑定在每次运行和打开「我的」时对账，不在登录流程里写。** iOS 有三条登录路径（验证码、Apple、Google）和三条退出路径（退出、注销、清除失效登录），都经 `BackendAccountSession`；会话里每次登录都有一个新的 `sessionID`，刷新令牌时不变。状态里记下（账号 id，`sessionID`），对账时与钥匙串里的当前会话比较：不同（换账号、退出后重新登录、已退出）就清空全部进度、关掉开关、`bindingGeneration` 加一，与 `SyncSwitch.clear` 加 `bindAccount` 的结果相同。匿名账号存在另一个会话里（`BackendAnonymousAccount`），`BackendAccountSession.shared` 只有真实登录，所以「只给真实账号同步」自然成立。
- **调度（`CloudSync`，App）**：与 `SyncMergePolicy.due` 相同的节流：距上次尝试满 5 分钟，或任一分类有待上传改动且满 30 秒；上次尝试时间在运行前写入；同一时刻只跑一次（主 actor 上的标志）。
  - **差异：前台触发点是 `scenePhase` 变为 `.active`**（`MetasequoiaImeApp`），对应 Android 的 `HomeActivity.onResume`。打开开关时不等节流立刻跑一次，与 `runNow` 相同。
  - 第一次同步某个分类而云端已有数据时，弹「云端已有同步数据」，选项「合并」「使用云端」，文案与 Android 相同；取消则什么也不做，5 分钟后回到前台再问。提示挂在应用根视图上，不依赖当时在哪个标签页。
  - 模式表、409 `revision_conflict` 时重新拉取并按「本机为后写」合并、最多重试 2 次、失败原因文案，全部放进 `shared/backend` 的纯函数 `BackendSyncPolicy`，与 `SyncMergePolicy` 和 `CloudSync.message` 一一对应，Swift 包测试直接覆盖。
- **设置**：沿用 `IOSCloudSettings.snapshot()` / `apply(_:)` 和 `BackendPreferencesClient`。上传 = 云端文档为底、本机值覆盖（`mergedPreferences`，只上传服务端字段表里有的键），上传后把结果应用回本机（云端独有的键落到本机，与 Android 相同）；下载直接 `apply`。上传前和应用前都去掉本地专属的键（隐私模式、开发者选项、诊断日志、语音贡献）。
  - 当前自定义键盘皮肤已经在文档里（`platform.ios.custom_keyboard_skin`），随设置同步。
  - **差异：不同步命名皮肤库。** Android 的 `platform.android.custom_keyboard_skins` 在服务端字段表里有，iOS 没有对应字段（msime-cloud `internal/account/preferences_fields.json` 里 iOS 只有 `custom_keyboard_skin` 单数），不新造字段。
  - 改动标记：共享偏好文档每次真正写入（`MetasequoiaInputSessionBridge.persistSharedPreferences`，应用的 `updateSharedPreferences` 和键盘的 `updateAndPersist` 都经过它）、按键反馈的开关和强度（应用两个设置页的开关、键盘菜单）、`CustomKeyboardSkinStore.save` 内容变了时，「重置所有设置」（文档由 Rust 整份替换，不经过上面那条路）时，给 `settings` 代数加一。同步自己应用云端设置、键盘把文档里的设计抄进 App Group（`GlobalThemePreference.mirror`）时用线程局部的「不记改动」（`CloudSyncState.suppressingMarks`）包住，免得这些写入又被当成本机改动。应用设置页的按键反馈开关经 `markingSettingsChange` 绑定记改动，而不是 `onChange`：同步应用云端值时 `@AppStorage` 也会变，`onChange` 分不出来源。
- **常用语**：新的 `BackendPhrasesClient`（GET/PUT `/v1/users/me/phrases`），解析与 `SyncApi.parsePhrases` 相同（最多 500 条、id 与正文必须是字符串、分组可空、位置非法时按顺序）。合并按 id、本机胜出，再按正文去重，截到 500 条，正文 1–2000 个 UTF-16 单元且不含 NUL；只有本机改动时的上传带上「本机收不下」的云端正文，免得它们被当成本机删除。本机经 `CommonPhrasesBridge`（`msime_client_common_phrases`）增删，只同步用户自己添加的（`pack == nil`）。本机存储只收 200 条、每条 1000 个 UTF-16 单元，超出和写入失败的正文记为「收不下」，状态行提示「云端有 N 条常用语超出本机上限，未下载但已保留」；删不掉的本机条目跳过，下一轮再对齐，与 Android 的 `applyPhrases` 相同。
  - **差异：没有「示例常用语」认领。** Android 预置示例常用语，同步前要排除和认领（`untouchedStarters` / `adoptStarters`）；iOS 的常用语存储没有预置条目，这一步不需要。
  - 改动标记放在 `CommonPhrasesBridge.add` / `remove`（只在 App Group 的那份上，测试传入的目录不算），同步自己的增删同样用「不记改动」包住，所以清标记直接比对运行前的代数，不需要 Android 那样加上「自己的写入次数」。
- **词库：只做探测设计，不实现。** Android 上传前在宿主进程调 `export_snapshot` 导出本机用户词库（`CloudSync.exportDictionarySnapshot`），合并前用 `count` 判断本机有没有自己的词。iOS 的用户词库由键盘扩展里的 Engine 持有，应用进程不能在键盘可能开着它时打开它，`PersonalDictionaryStore` 的注释写明了这一点：连文本导出都是应用写请求、键盘下次同步时代写。没有安全的导出路径，就不能上传；而只下载不上传是有害的：快照激活是整份替换，本机从没上传过的词会被云端快照覆盖掉。所以本次不让词库进自动同步，`CloudSyncState` 也不登记词库分类，「云词库」的手动上传下载照旧留在开关下面。将来要做时的设计：
  - 探测：与 Android 相同，`GET /v1/users/me/dictionary/changes?after=<游标>&limit=1`，有改动即云端变了（`BackendAccountClient.dictionaryChanges` 已有）。
  - 导出：在 `PersonalDictionaryState` 里加一个快照导出请求，形状同 `PersonalExportRequest`，由键盘的 `DictionarySnapshotWorker` 在没有输入会话时调 Engine 的 `export_snapshot` 写到 App Group，应用读到结果后再 `PUT /v1/users/me/dictionary/snapshot?revision=`；本机词数同样由键盘写进结果。
  - 下载交给 `DictionarySnapshotQueue`，合并经 `PersonalDictionaryStore.enqueueImport` 每批 128 条，个人词库队列有未应用的请求时推迟上传，与 Android 相同。
- **界面**：「我的 › 同步」的「云同步」行换成 Android 那样的开关加一行状态（「云同步已关闭」、最近一次失败原因、常用语收不下的提示、「云同步已开启 · 尚未同步」、「已同步 · 最近 N 分钟前」）；没有真实账号时置灰，写「登录后可用」。打开时提示「云同步已开启」并马上比对一次。下面一行「手动同步」进入原来那一页（设置同步、云词库）。资料卡的副标题照旧显示登录方式，没有改成 Android 那样的同步状态，状态只在同步行上。删掉原页脚「在这台设备上手动上传或下载，不会在后台自动同步。」和 `AccountSettingsView` 文档注释里「iOS 上没有自动同步」的说法。

## Alternatives considered

- **把状态放在 App Group 的 `UserDefaults`**：不用自己写文件锁，Android 的 `SharedPreferences` 也是键值。但应用和键盘两个进程都会给同一个代数加一，`UserDefaults` 跨进程的读改写不是原子的，并发时会丢掉一次加一；加上清标记要「代数仍等于 X 才写 clean」的比较并交换，键值存储做不到。仓库里 App Group 的其他共享状态（`DictionarySnapshotQueue`、`PersonalDictionaryStore`）也都是 JSON 文件加 `flock`。
- **在每条登录、退出路径里调用清空**：与 Android 的 `SignIn` 一一对应，退出那一刻就清。但 iOS 有六条路径分散在 `CodeLoginModel`、`SkinCommunityAPI` 和资料页，漏掉一条就会把旧账号的游标带进新账号；按会话对账只有一个入口，而且键盘或另一个进程改了钥匙串里的会话也能察觉。
- **词库先只做下载**：下载和合并在应用进程里都能做（`DictionarySnapshotQueue`、`PersonalDictionaryStore.enqueueImport`），看起来能先交一半。但没有上传时，首次「合并」之后本机的词永远到不了云端，增量轮次的「下载」又会整份替换本机词库，丢掉从没上传过的词。

## Consequences

- **收益**：iOS 用户打开开关后，设置（含当前自定义键盘皮肤、按键反馈）和常用语在设备间自动对齐，规则、文案和 Android 一致，跨平台的同一账号不会因为两边策略不同而互相覆盖。
- **代价：词库仍要手动同步。** 这是本次最大的缺口，要等键盘能代写快照导出（上面的设计）才能补上；在那之前状态行写「已同步」并不包含词库，「手动同步」行的副标题写明了「词库不自动同步」。
- **代价：应用写本机时与退出登录之间仍有极小的竞争窗口。** Android 在 `bindingLock` 里检查绑定并写本机；iOS 的本机写入会经共享偏好文档再回到状态文件记改动，持着状态锁写本机会自锁，所以只在每次写之前检查一次绑定（`CloudSyncRun.current`），窗口是一次本地写入的时长。
- **代价：设置文档的任何写入都会标记改动**，包括同步不上传的字段（例如工具栏布局）。偶尔多一次内容相同的 PUT，云端版本号加一，其他设备下一轮下载一份等价的文档；不会来回触发，因为应用云端内容不记改动。皮肤编辑器拖动颜色时每一帧都会写一次状态文件。
- **没有完全访问权限的键盘写不了 App Group**，键盘里的改动记不下来；这时它本来也写不了共享偏好文档，改动本身就没有落盘。
- 状态文件读不出来时按全新状态处理：开关回到关闭，下次打开时按首次同步整份比对，不会丢数据。

## Verification

- `swift test --package-path shared/backend`：189 个测试全部通过，其中新增 `BackendSyncPolicyTests`（模式表、节流、提示节流、本地专属键、409 重拉合并最多重试 2 次、失败文案、状态行、相对时间）和 `BackendPhrasesTests`（响应解析与拒收、PUT 发出去重后的列表、冲突识别、按 id 合并与按正文去重、500 条截断、收不下的正文随上传保留、按正文对齐本机）。
- MSIMEClientTests 在 iPhone 18 Pro Max 模拟器上全部通过：MSIMEKeyboardTests 703 个（7 个跳过）、MSIMEServiceTests 72 个、MSIMESharedTests 87 个。新增的 `CloudSyncStateTests`（跨实例持久化、开关要求账号并标记全部分类、只在开启且不在同步写入时记改动、只在上传的那一代清标记、新登录清空进度并作废旧一轮、读不出的文件按全新状态）和 `CloudSyncMarkingTests`（共享文档写入、按键反馈、键盘设计保存与镜像、常用语增删各自是否记改动）在其中。
- MSIMEApp 为模拟器编译通过；UI 测试 `testCloudSyncIsASwitchWithManualSyncBelowIt` 在未登录状态下截图确认「云同步」是置灰的开关、副标题「登录后可用」，下面是「手动同步」。
- 没有用真实账号在设备或模拟器上跑一轮完整的同步：手边没有可用的测试账号，登录后的开关、首次选择弹窗、两台设备之间的设置与常用语往返都没有实测。
