# Agent Note: 只读打开的多链接文件在可信目录里放行

Status: implemented

## Problem

#6027 让引擎的资源读取入口拒绝硬链接数大于 1 的文件，#6030 对 SQLite 数据库做了同样的事，client-core 和 host-api 的只读入口也是同一条规则。规则本身不区分文件在哪个目录里。

有些部署方式会把安装目录里的文件合并成硬链接：

- NixOS 的 `nix-optimise`、`auto-optimise-store` 会把 store 里内容相同的文件合并成硬链接。
- ostree 系统的 `/usr` 本身就是以硬链接部署的。

在这些系统上，下面这些都被拒绝了：

- 离线手写模型；
- 内置按键音；
- 随包词库和 `en-phonetic.db` 这类只读数据库；
- `edition.json`。

用户什么都没做，过一夜手写就不能用了（#6386）。开了 store 去重的机器上，`linux-handwriting-local-model` 这项 ctest 也会失败，包装不上。

## Decision

只读打开时，链接数大于 1 的文件在下面这种情况下照常读：文件所在的目录属于 root 或当前用户，并且组和其他用户都不可写。其他情况照旧拒绝。会写入的路径不受影响，仍然只接受单链接，包括可写数据库、锁文件和 journal。

依据是：拒绝硬链接要防的是别人在我们读取的目录里放一个指向别处 inode 的链接，让我们把不属于这里的文件当作资源或私有数据读进来，甚至复制进用户的代次目录。要放下这样的链接，必须能写这个目录。只有 root 或当前用户能写的目录里，多链接文件只可能是 root 或用户自己建的，例如包管理器去重、ostree 部署、`cp -al` 备份。写入的规则不同：写一个多链接文件会改到链接另一端的文件，这与目录是否可信无关。

判断只写一份，放在 `msime-path-trust` 里，有三种用法：

- **`multi_link_is_trusted(file, path)`：给先按路径打开、再检查的入口用。**
  - 调用方：engine 的 `open_file_no_follow`、client-core 的 `file_lock::open_private_file`、host-api 的 `bounded_file::open_private`。
  - 做法：以句柄打开父目录，核对属主和权限；再用 `fstatat` 在这个目录句柄下按名字查到的 inode，必须就是手里这个文件的 inode。这样检查的就是这个文件所在的目录，不会是检查之后换上来的另一个。
- **`multi_link_in_directory_is_trusted(directory)`：给 client-core 的 `open_private_file_at` 用。**
  - 文件本来就是相对目录句柄用 `openat` 打开的，所以只需检查这个目录。
- **`multi_link_path_is_trusted(path)`：给 SQLite 用。**
  - SQLite 只能按路径打开，所以检查也按路径做，与原先的单链接检查处在同一个检查与打开之间的窗口。
  - engine 新增 `sqlite_read_only_path_no_follow`，`SQLITE_OPEN_READ_ONLY` 的调用点改用它：
    - 汉字库、五笔码表、`others.db`、发音库和释义库；
    - 本地模式词库；
    - 准备代次时作为来源的随包数据库。
  - 语言词库用的 `sqlite_path_no_follow_allow_parent_symlinks` 本来就只读，也按这条规则。
  - `sqlite_path_no_follow` 仍给会写入的数据库用，继续只接受单链接。

`msime-path-trust` 为此在 unix 上依赖 libc（工作区里已有的同一版本），用来调用 `geteuid` 和 `fstatat`；这两处用函数级的 `#[allow(unsafe_code)]`，并写明 SAFETY。Windows 等没有属主和权限位的平台一律不信任多链接文件，行为不变。

ctest `linux-handwriting-local-model` 改为先把模型复制到构建目录，再识别这份副本。Nix 构建沙箱里，store 的属主显示为 nobody，任何按属主判断的规则在那里都不会放行。复制出来的是新的单链接文件。

## Alternatives considered

- **只信任 root 专属的位置。** 文件和目录都属于 root，且组和其他用户不可写时才放行，与 `is_root_only_link` 同一思路。最强的理由是放宽得最少。不用它，是因为它漏掉单用户 Nix（store 属于用户自己），也漏掉用户自己建的硬链接。而这两种情况里，链接同样只可能是用户自己放的，信任当前用户并不多打开攻击面。
- **只读的安装资源不检查链接数。** 最强的理由是覆盖面最大，连构建沙箱也不用改，实现也最简单。不用它，是因为安装目录不一定只有 root 能写。`2026-10-09-local-asr-private-file.md` 已经否定过「只依赖安装器」的做法：资源目录被别人写了，他们就能用硬链接让我们读到他们读不到的私有文件，`copy_private_file` 还会把它复制进用户的代次目录。
- **放宽所有检查，包括写入路径。** 写一个多链接文件会改到链接另一端，这个后果与目录是否可信无关。所以写入路径不放宽。

## Consequences

- **收益**：Nix store 去重、ostree 部署之后，手写、按键音、随包词库和 `edition.json` 照常可用；开了 `auto-optimise-store` 的机器能完成构建。被别人能写的目录里的硬链接仍然被拒绝。
- **代价**：umask 为 002 的用户，自己的目录是组可写的，里面的硬链接仍然被拒绝，与改动前相同。是否按「用户私有组」放宽，没有在这次决定。多用户 Nix、单用户 Nix 和 ostree 只按代码路径和单元测试确认，没有在真实系统上验证。
