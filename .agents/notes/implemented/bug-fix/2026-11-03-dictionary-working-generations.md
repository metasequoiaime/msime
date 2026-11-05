# Agent Note: 回收用不上的工作词库代次和 Linux 换下的词库目录

Status: implemented

## Problem

#10735：Linux 用户反馈词库自动更新之后，旧版本的词库文件夹一直留着，越积越多。[词库更新回收用不上的旧版本](2026-10-31-dictionary-update-old-versions.md)只处理了 `dictionary-updates/generations/` 下的下载代次，词库按版本堆积的地方还有两处。

一、工作词库代次，全平台都有。宿主为每份资源集合在 `<状态目录>/user/dictionaries/<代次>` 准备一份工作词库：`msime-pinyin.db`（约 74 MB，并回拆开发布的五笔表后更大）、`msime-english.db`、n-gram 表的副本，回放过用户日志。代次是资源集合的 `generation()`（`prepare_shipped_host_configuration` 里的 `specification.generation()`）：用随包词库时是本安装包词库锁的代次，用在线更新时是下载版本的桌面代次，和 `generations/<代次>` 同名。这个目录下唯一的清理是引擎的 `sweep_abandoned_stages`，只删 `.incoming`、`.removing` 暂存目录，就绪的旧代次从来不删。于是每装一个在线词库版本、每升级一次换了词库锁，都多留一份；0.15.0 起下载代次已经会回收，它们对应的工作词库仍然留着，而且清单已经删掉，按版本逐个回收的办法再也找不到它们。

二、Linux 上用 `msime-linux-setup --update --download` 自己下载的词库（设置里「词库需要更新 → 立即更新」走的也是它）。`staged_resources` 在旧目录旁另建 `resources-<锁摘要>`，切换成功后旧目录故意留着，只提示「确认新词库正常后可以自行删除」，每次升级换锁就多一个。

## Decision

### 工作词库代次（host-api）

设置宿主的检查任务（`dictionary_updates::request` 的 `automatic`/`check` 线程）在 `updates::check` 结束于 `ready` 或 `current` 时，接着调用 `remove_unused_working_generations(state_root, baseline, configured)`，时机与下载代次的回收相同。`configured` 是发来这次请求的 HostOptions 里 `dictionaries` 的文件名，即宿主配置记着的工作词库代次，在起线程前取好。

留下三类：本安装包词库锁的代次（`baseline.generation()`），宿主配置记着的那一代（`configured`），和下载版本还留着的那些。后者由 client-providers 的 `retained_generations` 回答：`installed.json` 的当前和可回退的上一个、`loaded.json` 记录的、以及 `generations/` 下还有清单 `<代次>.json` 的（有会话占用或删不掉、被 `remove_old_versions` 留下的版本都在其中）。清单只看文件名不读内容，多留只是多占空间。`generations/` 路径上有符号链接、读不了，或者 `installed.json`、`loaded.json` 在却读不出来时返回 `None`，这一轮什么也不删。

宿主配置记着的那一代不能只靠下面的锁保护。Linux 上用户自己下载的词库在包升级换锁之后，`msime_client_refresh_host` 报 `dictionary_outdated`，两个宿主按设计继续用 `runtime-options.json` 记着的旧锁代次，直到用户点「立即更新」；这一代既不是新的随包代次，也不是下载版本。而 Fcitx5 每次失焦都销毁会话，建会话时只按 `dictionaries` 取 `DictionaryAccess::try_session`，不重新准备代次，没有焦点的那一刻谁也不持锁。设置应用在窗口获得焦点、每小时都发自动检查，只靠锁的话这一代迟早被删，之后每次建会话都失败，重启宿主也准备不回来（刷新照样报 `dictionary_outdated`），用户完全打不了字。发来请求的设置应用每次请求都现读 `runtime-options.json`：Linux 是 Tauri 设置应用的 `DictionaryHostOptions::snapshot`，Android 是原生设置的 `DictionaryUpdates.request`（`HostOptionsPolicy.readRuntimeOptions`），Windows 是原生设置窗口的 `runtime_options_document`（Server 每次启动用 `publish_prepared_dictionaries` 把准备好的代次写回这份文件）。拿到的就是输入法此刻在用的那份；包升级后、`fcitx5 -r` 之前还在跑的旧插件同理，它建会话时读的也是这份文件。macOS 的设置应用用启动时准备好的文档，记着的是那一刻的代次，最坏是在设置应用重启前多留一份；iOS 在进程里自己拼 HostOptions。

候选是 `user/dictionaries/` 下名字恰为 64 位十六进制、本身是真实目录（不是符号链接）的项；暂存目录（名字里带 `.`）和别的名字不碰。每个候选：

1. 只在这个代次目录上取独占的 `DictionaryAccess`（`try_maintenance(path, path)`，两个根去重后只有一把锁）。每个会话建立时都以共享方式持着它所用代次上的这把锁，设置里改词这类维护也持着；取不到就跳过，下一次检查再试。不碰 `user_data` 上那把锁：所有会话都持着它，取它的独占锁会让回收永远失败（上一篇否掉 `DictionaryAccess` 的理由正在这里，只锁代次目录就避开了）。
2. 持锁后重新问一次 `retained_generations`，避开取锁期间别的进程刚发布的新版本。
3. Unix 上改名成 `<代次>.<进程号和纳秒的十六进制>.removing` 认领，再 `remove_dir_all`（`claim_then_remove`）。删到一半留下的目录由引擎下次准备代次时的 `sweep_abandoned_stages` 收尾，`.removing` 后缀与它的约定一致。改名失败就放掉锁、保留原目录，留到下次检查重新取锁再试。不能在放掉锁之后立即重试：其他输入进程可能在这个空档取得共享租约，之后的改名和删除会破坏正在使用的代次。
4. Windows 上不改名（`empty_then_remove`）。租约的锁文件 `.msime-dictionary-access.lock` 就在代次目录里，持锁期间它的句柄一直开着，NTFS 不允许改名还有打开文件的目录，第 3 步在 Windows 上每次都会失败，代次永远收不掉（最初的实现就是这样，评审时发现）。改成持锁时就地清空：先删 `.ready`，这一代在引擎看来从此不再就绪；删 `.ready` 失败就什么也不动。再删锁文件以外的全部内容，然后放锁，最后删锁文件和已经空了的目录。放锁之后删的只剩一个没有数据的锁文件：锁文件原则上不能删，是怕两个进程各自锁住不同的文件、以为都独占了同一份数据，而这里数据已经在持锁时删光，谁之后取到这把锁也只得到一个不就绪的空壳。
5. 空壳必须收干净。引擎准备代次时，同名目录存在而没有 `.ready` 就重新暂存，再把暂存目录改名过去；目标非空时改名必然失败，这一代就再也准备不出来。放锁之后又有进程打开了锁文件、删不掉时会留下这样的空壳，所以候选和持锁后的复查都只保护就绪的代次：本安装包的、宿主配置的、下载版本留着的同名目录没有 `.ready` 时照样回收。引擎发布代次是整目录改名、`.ready` 已在里面，64 位十六进制名下没有 `.ready` 的目录不会是准备到一半的代次。

代次里的内容都能从资源和用户日志重新准备出来，删掉一份用不上的代次不丢用户数据。

### Linux 换下的词库目录（msime-linux-setup）

`update_dictionaries` 在刷新确实切换了（`switched` 非空）且新目录不是原目录时，仍在 `hosts_held_off` 里（两个宿主的会话被请走、IBus 等过重读配置的 6 秒之后）调用 `remove_replaced_resources` 删掉换下的目录。只删当前用户自己下载的那份（复用 `updatable`）；随包词库的位置 `<前缀>/share/msime-client/resources` 一律不删，哪怕装在 `~/.local` 下、`updatable` 认它是用户自己的；符号链接、不再是目录的、与新目录是同一个的不碰；在线词库更新的下载代次（状态目录下 `dictionary-updates/` 里的，取 `runtime-options.json` 所在目录和它记的 `preferences_directory` 两处）不碰：它归当前用户、可写，`updatable` 认它，但它有清单、代次锁和校验标记，脚本 `rmtree` 会让 `installed.json` 和 `<代次>.json` 指着不存在的目录，`retained_generations` 又因为清单还在一直留着对应的工作词库，统一交给宿主的检查任务按 `remove_old_versions` 回收，输出提示它由词库更新回收、不要手动删；默认状态目录的 `runtime-options.json` 不是这次更新的那份、却还指着它（用 `--state` 另建过状态）或读不出来时也留下，提示用户确认后自己删。与锁一致的文件在新目录里是硬链接，删旧目录只去掉旧名字。删不掉不影响已经完成的切换。README 里「旧目录保留，确认后由用户自行删除」一句随之改写。

不加「保留几个版本」的设置。保留规则沿用上一篇：当前、可回退的上一个、输入法最近载入的、还有人在用的。

## Alternatives considered

- **在 `remove_old_versions` 里随每个被删的下载版本一起删 `user/dictionaries/<代次>`，持着同一把 `<代次>.lock`。** 删除点和下载代次完全对齐，不用另起一次扫描，会话租约一一对应、不必另取锁。但它只覆盖从此以后被删的下载版本：0.15.0 起已经删掉清单的版本，对应的工作词库再也没有人认领；随包词库换锁留下的代次没有下载清单，也覆盖不到。它还要让 client-providers 知道 `user/` 的布局，这是 host-api 准备宿主时定的。改成按「不再被留下」扫描，一条规则把三种来源都收掉。
- **旧随包代次的回收拆出去，只回收下载版本对应的工作词库。** 理由是升级后还没重启的旧版输入法进程没有下载代次的占用凭证，`loaded.json` 也只记最近一次加载。但这类进程的会话都持着工作词库上的共享 `DictionaryAccess`，取不到独占锁就不删，会话活着时是安全的；会话之间的空档（Fcitx5 失焦即销毁会话）由宿主配置记着的那一代补上。Linux 上每次发版都会换锁，不收这一类，问题里「越积越多」的大头就还在。剩下的风险见 Consequences。
- **只回收由 `loaded.json` 在本进程启动后记录过的代次。** 能挡住任何还没被记录的使用者。但 `loaded.json` 只记最近一次，宿主配置停在旧锁代次（`dictionary_outdated`）时它记的正是那一代，换个时刻又会被别的加载覆盖，判断不了「还有没有宿主指着它」；直接看宿主配置更准，也不需要新的记录。
- **改名失败后先释放自己的访问锁，再立即重试改名。** Windows 上锁文件的打开句柄可能影响目录改名，释放锁后重试看起来能提高回收成功率。但释放租约后其他进程就能取得共享租约，此时重试会把正在使用的代次改名并删除。回收失败只多占一份空间，留到下次持锁再试，不能用失去互斥来换回收成功。
- **Windows 上把锁文件挪到代次目录之外。** 改名时目录里没有打开的句柄，Unix 和 Windows 能用同一条认领路径。但锁文件的位置是引擎租约的契约，所有宿主和各个版本都按「根目录里的 `.msime-dictionary-access.lock`」取锁，挪走等于换一把锁，新旧版本进程之间的互斥随之失效，代价远大于这一处回收。
- **Windows 上放锁后再改名认领。** 改名本身能成功，但放锁与改名之间别的进程可以取得共享租约开始用这一代，改名和删除就会破坏它，与上面「改名失败后放锁重试」被否掉的理由相同。持锁清空之后再放锁，空档里能被取到的只剩空壳。
- **按修改时间保留最近 N 天的代次。** 不需要任何锁。但输入进程可以连跑几周，代次也不是每次加载都会写，时间判断不了有没有人在用，也解释不了 N 为什么是这个数，与上一篇否掉同类做法的理由相同。
- **Linux 上扫掉 `resources-*` 里所有不是当前目录的。** 能顺带清掉这次修复之前留下的旧目录。但脚本只知道这一份状态记录的目录，另建的状态目录可能还指着其中某一个；只删这一次换下的那份，以前留下的在回复里告诉用户怎么判断再手动删。

## Consequences

- **收益**：`user/dictionaries/` 稳定在本安装包的代次、下载版本当前和上一个（以及输入法还没换上新版时在用的那个）对应的几份；0.15.0 起已经堆下的旧工作词库在下一次检查判定「已是最新」或装好新版本时一并收掉。Linux 上「立即更新」不再每次升级多留一个 `resources-*`。
- **代价与已知上限**：
  - 回收只跟着在线词库检查发生，关掉自动更新又不手动检查的用户，工作词库照旧不回收；只靠包管理器升级、从不检查的 Linux 用户同样。
  - Linux `dictionary_outdated`（自己下载的词库在升级换锁后没更新）这种状态下，宿主配置记着的旧锁代次一直留着，直到「立即更新」切换成功、配置换到新代次后的下一次检查才回收。保护只看发来请求的那份配置：用 `--state` 另建过状态、或设置应用读的配置文件与输入宿主读的不是同一份（`MSIME_CLIENT_STATE_DIR` 这类部署）时，另一份配置记着的代次只受会话锁保护，没有会话的空档里可能被删，删后那份配置要「立即更新」或重新准备才能再用。
  - 宿主配置之外、没有会话、此刻也不在维护的进程不受保护：macOS、Windows 上升级后还没重启的旧版宿主若此刻一个会话都没有，而它的旧随包代次也不是设置应用读到的那一代，可能被删，下一次建会话时 `DictionaryAccess::try_session` 找不到目录而失败，要等它重启后重新准备。Windows 安装程序升级时先停掉输入法进程、装完再启动新的，这个窗口只在安装过程中；macOS 上旧输入法进程在换到新程序之前同样有这个窗口，没有核实它有多长，以后若出现这类报告，应让输入宿主自己的配置也参与保留。Linux 两个宿主都按 `runtime-options.json` 建会话，由上一条覆盖。
  - 下载版本的工作词库沿用上一篇的保留范围：同一进程里没有会话却还拿着旧 `dictionaries` 路径的宿主（例如设置宿主的配置停在已被挤出保留范围的版本），下次用它时要重新准备。上一篇对下载代次本身已经接受了同一个前提。
  - 鸿蒙键盘在自己的沙箱里有一份状态目录，下载代次靠设置应用镜像过去，但键盘不跑检查任务，那里的工作词库仍然不回收。
  - Linux 上这次修复之前留下的 `resources-*` 不会被自动删除，用 `--state` 另建过状态、共用同一份下载目录的，默认状态目录之外的那些也不会被发现。
  - Linux 上在线版本生效时 `resources` 指着 `dictionary-updates/generations/<代次>`，换锁后「立即更新」换下的是这个下载代次。脚本不删它（见上文），它要等宿主的检查任务把它挤出保留范围（`installed.json` 换了当前和上一个、`loaded.json` 也不再记它）才回收。

## Verification

- `cargo test -p msime-client-providers --lib dictionary_updates`：`retained_generations_follow_the_versions_left_on_disk` 装四个版本，断言有会话占用、最近载入、上一个、当前都在保留集合里；会话放开、再检查一次之后，被删的版本不再算；清单已删但 `loaded.json` 还记着的仍算；`installed.json` 读不出来时为 `None`。
- `cargo test -p msime-host-api --lib dictionary_updates`：`the_generation_the_host_configuration_records_is_kept_without_a_session` 按旧锁真实准备出宿主配置，再换一份锁，没有任何会话：带着配置记着的代次调用时这一代留下，不带时被删，证明保住它的正是这条规则。`unused_working_generations_are_removed_unless_bundled_retained_or_in_use` 用真实准备出的随包代次，加上合成的几份代次：有清单的、`loaded.json` 记着的、有会话持着 `DictionaryAccess` 的、本安装包代次都留下，下载版本早已删掉的和上一个词库锁的被删；暂存目录、别的名字、指向外部目录的符号链接都不动；不留 `.removing`。会话放开、清单删掉之后下一次把它们收走；下载记录读不出来时一个都不删。
- `cargo test -p msime-host-api --lib emptying_reclaim_removes_the_generation_and_leftover_shells`：直接走 Windows 的回收：就绪的旧代次（带子目录）被清空删掉；只剩锁文件、没有 `.ready` 的空壳即使名字是本安装包的代次也被收掉；就绪的宿主配置代次原样留下。Windows CI 的 `Build and test Windows 7 handwriting host` 一步跑这一条和其余三条回收用例，在 NTFS 上验证锁文件句柄与目录删除的真实行为；此前那一步只跑 `windows_handwriting::`，回收用例从没在 Windows 上执行过。
- `cargo test -p msime-host-api --lib failed_generation_claim_keeps_the_lease_until_rename_returns`：注入系统拒绝改名的结果，所有回收尝试都用真实 `DictionaryAccess` 检查独占租约阻止新会话；失败后只尝试一次、原目录内容仍可读且仍可建立共享会话，下一轮成功认领后可回收。旧实现释放租约后立即重试，第二次尝试时共享租约已可取得，测试失败。
- `python3 platforms/linux/tests/core/setup_update.py`：切换成功后换下的目录被删，新目录里硬链接过来的文件仍可读，修复运行资源时换下的坏目录也删掉；默认状态目录还指着它时留下、轮到默认状态目录自己更新时删掉；符号链接不跟过去删；随包位置（`~/.local` 前缀）不删；换下的是 `dictionary-updates/generations/<代次>` 时目录和清单都留着，输出提示由词库更新回收（这一条用上一版脚本跑，`b.db` 已被删掉而失败）。改动前的脚本在第一条断言就失败。
- 真机验收留给设备轮：Linux 上连续装两个在线词库版本或换一次锁，再看 `~/.config/msime-client/user/dictionaries/`、`~/.config/msime-client/dictionary-updates/generations/` 和 `~/.local/share/msime-client/` 下剩下的目录，以及输入、回退是否正常。

## Related notes

[词库更新回收用不上的旧版本](2026-10-31-dictionary-update-old-versions.md)定义了下载代次的保留范围和回收时机，这里沿用它的保留规则，把回收延伸到工作词库代次。[代次暂存目录被中途杀掉的进程留下后不再挡住启动](2026-10-26-stale-generation-staging.md)定义了 `.removing` 认领后缀和 `sweep_abandoned_stages`，这里的删除复用它收尾。
