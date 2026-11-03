# Agent Note: 回收用不上的工作词库代次和 Linux 换下的词库目录

Status: implemented

## Problem

#10735：Linux 用户反馈词库自动更新之后，旧版本的词库文件夹一直留着，越积越多。[词库更新回收用不上的旧版本](2026-10-31-dictionary-update-old-versions.md)只处理了 `dictionary-updates/generations/` 下的下载代次，词库按版本堆积的地方还有两处。

一、工作词库代次，全平台都有。宿主为每份资源集合在 `<状态目录>/user/dictionaries/<代次>` 准备一份工作词库：`msime-pinyin.db`（约 74 MB，并回拆开发布的五笔表后更大）、`msime-english.db`、n-gram 表的副本，回放过用户日志。代次是资源集合的 `generation()`（`prepare_shipped_host_configuration` 里的 `specification.generation()`）：用随包词库时是本安装包词库锁的代次，用在线更新时是下载版本的桌面代次，和 `generations/<代次>` 同名。这个目录下唯一的清理是引擎的 `sweep_abandoned_stages`，只删 `.incoming`、`.removing` 暂存目录，就绪的旧代次从来不删。于是每装一个在线词库版本、每升级一次换了词库锁，都多留一份；0.15.0 起下载代次已经会回收，它们对应的工作词库仍然留着，而且清单已经删掉，按版本逐个回收的办法再也找不到它们。

二、Linux 上用 `msime-linux-setup --update --download` 自己下载的词库（设置里「词库需要更新 → 立即更新」走的也是它）。`staged_resources` 在旧目录旁另建 `resources-<锁摘要>`，切换成功后旧目录故意留着，只提示「确认新词库正常后可以自行删除」，每次升级换锁就多一个。

## Decision

### 工作词库代次（host-api）

设置宿主的检查任务（`dictionary_updates::request` 的 `automatic`/`check` 线程）在 `updates::check` 结束于 `ready` 或 `current` 时，接着调用 `remove_unused_working_generations(state_root, baseline)`，时机与下载代次的回收相同。

留下两类：本安装包词库锁的代次（`baseline.generation()`），和下载版本还留着的那些。后者由 client-providers 的 `retained_generations` 回答：`installed.json` 的当前和可回退的上一个、`loaded.json` 记录的、以及 `generations/` 下还有清单 `<代次>.json` 的（有会话占用或删不掉、被 `remove_old_versions` 留下的版本都在其中）。清单只看文件名不读内容，多留只是多占空间。`generations/` 路径上有符号链接、读不了，或者 `installed.json`、`loaded.json` 在却读不出来时返回 `None`，这一轮什么也不删。

候选是 `user/dictionaries/` 下名字恰为 64 位十六进制、本身是真实目录（不是符号链接）的项；暂存目录（名字里带 `.`）和别的名字不碰。每个候选：

1. 只在这个代次目录上取独占的 `DictionaryAccess`（`try_maintenance(path, path)`，两个根去重后只有一把锁）。每个会话建立时都以共享方式持着它所用代次上的这把锁，设置里改词这类维护也持着；取不到就跳过，下一次检查再试。不碰 `user_data` 上那把锁：所有会话都持着它，取它的独占锁会让回收永远失败（上一篇否掉 `DictionaryAccess` 的理由正在这里，只锁代次目录就避开了）。
2. 持锁后重新问一次 `retained_generations`，避开取锁期间别的进程刚发布的新版本。
3. 改名成 `<代次>.<进程号和纳秒的十六进制>.removing` 认领，再 `remove_dir_all`。删到一半留下的目录由引擎下次准备代次时的 `sweep_abandoned_stages` 收尾，`.removing` 后缀与它的约定一致。Windows 上目录里有打开的文件时改名可能失败：先放掉自己的锁再试一次，还失败就留到下次。

代次里的内容都能从资源和用户日志重新准备出来，删掉一份用不上的代次不丢用户数据。

### Linux 换下的词库目录（msime-linux-setup）

`update_dictionaries` 在刷新确实切换了（`switched` 非空）且新目录不是原目录时，仍在 `hosts_held_off` 里（两个宿主的会话被请走、IBus 等过重读配置的 6 秒之后）调用 `remove_replaced_resources` 删掉换下的目录。只删当前用户自己下载的那份（复用 `updatable`）；随包词库的位置 `<前缀>/share/msime-client/resources` 一律不删，哪怕装在 `~/.local` 下、`updatable` 认它是用户自己的；符号链接、不再是目录的、与新目录是同一个的不碰；默认状态目录的 `runtime-options.json` 不是这次更新的那份、却还指着它（用 `--state` 另建过状态）或读不出来时也留下，提示用户确认后自己删。与锁一致的文件在新目录里是硬链接，删旧目录只去掉旧名字。删不掉不影响已经完成的切换。README 里「旧目录保留，确认后由用户自行删除」一句随之改写。

不加「保留几个版本」的设置。保留规则沿用上一篇：当前、可回退的上一个、输入法最近载入的、还有人在用的。

## Alternatives considered

- **在 `remove_old_versions` 里随每个被删的下载版本一起删 `user/dictionaries/<代次>`，持着同一把 `<代次>.lock`。** 删除点和下载代次完全对齐，不用另起一次扫描，会话租约一一对应、不必另取锁。但它只覆盖从此以后被删的下载版本：0.15.0 起已经删掉清单的版本，对应的工作词库再也没有人认领；随包词库换锁留下的代次没有下载清单，也覆盖不到。它还要让 client-providers 知道 `user/` 的布局，这是 host-api 准备宿主时定的。改成按「不再被留下」扫描，一条规则把三种来源都收掉。
- **旧随包代次的回收拆出去，只回收下载版本对应的工作词库。** 理由是升级后还没重启的旧版输入法进程没有下载代次的占用凭证，`loaded.json` 也只记最近一次加载。但这类进程的会话都持着工作词库上的共享 `DictionaryAccess`，取不到独占锁就不删，会话活着时是安全的；Linux 上每次发版都会换锁，不收这一类，问题里「越积越多」的大头就还在。剩下的风险见 Consequences。
- **按修改时间保留最近 N 天的代次。** 不需要任何锁。但输入进程可以连跑几周，代次也不是每次加载都会写，时间判断不了有没有人在用，也解释不了 N 为什么是这个数，与上一篇否掉同类做法的理由相同。
- **Linux 上扫掉 `resources-*` 里所有不是当前目录的。** 能顺带清掉这次修复之前留下的旧目录。但脚本只知道这一份状态记录的目录，另建的状态目录可能还指着其中某一个；只删这一次换下的那份，以前留下的在回复里告诉用户怎么判断再手动删。

## Consequences

- **收益**：`user/dictionaries/` 稳定在本安装包的代次、下载版本当前和上一个（以及输入法还没换上新版时在用的那个）对应的几份；0.15.0 起已经堆下的旧工作词库在下一次检查判定「已是最新」或装好新版本时一并收掉。Linux 上「立即更新」不再每次升级多留一个 `resources-*`。
- **代价与已知上限**：
  - 回收只跟着在线词库检查发生，关掉自动更新又不手动检查的用户，工作词库照旧不回收；只靠包管理器升级、从不检查的 Linux 用户同样。
  - 没有会话、此刻也不在维护的进程不受锁保护：升级后还没重启的旧版宿主若此刻一个会话都没有，它的旧随包代次可能被删，下一次建会话时 `DictionaryAccess::try_session` 找不到目录而失败，要等它重启或刷新配置后重新准备。升级时各平台通常会重启或替换输入进程（Linux 的 IBus 宿主在焦点变化时自行换新，Fcitx5 提示重启），这个窗口不长；以后若出现这类报告，应改为在设置宿主之外再加一道「本进程启动后由 `loaded.json` 记过」之类的确认。
  - 下载版本的工作词库沿用上一篇的保留范围：同一进程里没有会话却还拿着旧 `dictionaries` 路径的宿主（例如设置宿主的配置停在已被挤出保留范围的版本），下次用它时要重新准备。上一篇对下载代次本身已经接受了同一个前提。
  - 鸿蒙键盘在自己的沙箱里有一份状态目录，下载代次靠设置应用镜像过去，但键盘不跑检查任务，那里的工作词库仍然不回收。
  - Linux 上这次修复之前留下的 `resources-*` 不会被自动删除，用 `--state` 另建过状态、共用同一份下载目录的，默认状态目录之外的那些也不会被发现。

## Verification

- `cargo test -p msime-client-providers --lib dictionary_updates`：`retained_generations_follow_the_versions_left_on_disk` 装四个版本，断言有会话占用、最近载入、上一个、当前都在保留集合里；会话放开、再检查一次之后，被删的版本不再算；清单已删但 `loaded.json` 还记着的仍算；`installed.json` 读不出来时为 `None`。
- `cargo test -p msime-host-api --lib dictionary_updates`：`unused_working_generations_are_removed_unless_bundled_retained_or_in_use` 用真实准备出的随包代次，加上合成的几份代次：有清单的、`loaded.json` 记着的、有会话持着 `DictionaryAccess` 的、本安装包代次都留下，下载版本早已删掉的和上一个词库锁的被删；暂存目录、别的名字、指向外部目录的符号链接都不动；不留 `.removing`。会话放开、清单删掉之后下一次把它们收走；下载记录读不出来时一个都不删。
- `python3 platforms/linux/tests/core/setup_update.py`：切换成功后换下的目录被删，新目录里硬链接过来的文件仍可读，修复运行资源时换下的坏目录也删掉；默认状态目录还指着它时留下、轮到默认状态目录自己更新时删掉；符号链接不跟过去删；随包位置（`~/.local` 前缀）不删。改动前的脚本在第一条断言就失败。
- 真机验收留给设备轮：Linux 上连续装两个在线词库版本或换一次锁，再看 `~/.config/msime-client/user/dictionaries/`、`~/.config/msime-client/dictionary-updates/generations/` 和 `~/.local/share/msime-client/` 下剩下的目录，以及输入、回退是否正常。

## Related notes

[词库更新回收用不上的旧版本](2026-10-31-dictionary-update-old-versions.md)定义了下载代次的保留范围和回收时机，这里沿用它的保留规则，把回收延伸到工作词库代次。[代次暂存目录被中途杀掉的进程留下后不再挡住启动](2026-10-26-stale-generation-staging.md)定义了 `.removing` 认领后缀和 `sweep_abandoned_stages`，这里的删除复用它收尾。
