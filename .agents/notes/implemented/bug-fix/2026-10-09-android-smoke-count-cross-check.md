# Agent Note: Android 冒烟发现改为独立计数核对

Status: implemented

## Problem

`platforms/android/check-host.sh` 自动发现冒烟后，用一个写死的下限（最后是 174）防止发现循环的筛选或路径匹配坏掉，并要求「新增冒烟时把这个数一起调高」。结果每个新增冒烟的 Android PR 都要改同一行：2026-10-08 一天里 #5764、#5766、#5768、#5803 全部在这一行上和 develop 冲突，合进一个，其余的就要重新解决；各分支里的数字也早已各不相同（161、165、173、176）。写死的下限本身也只能发现「少于下限」，筛选误排除一部分冒烟、但总数仍高于下限时发现不了。

## Decision

去掉写死的数字，改为换一条路独立数一遍：在 `tests/` 目录里用 `find .` 只按文件名找 `*Smoke.java`，得到仓库内的相对路径，再用 `grep -vxE` 去掉同样的三处排除（`device/`、`core/NativeSmoke.java`、`settings/KeyboardGeometryStrictIntSmoke.java`），要求和发现循环得到的数量完全相等。两边都数出 0 也算失败，那说明 `tests/` 的位置错了。路径按相对路径匹配，沿用脚本里关于 /home/runner 的教训。

## Alternatives considered

- 保留下限、改成一个不随冒烟增减的粗略值（如 100）：不再冲突，但检查变得更弱，误排除几十个冒烟也发现不了。
- 用 `git ls-files` 计数：本地新加、还没 `git add` 的冒烟会让两边对不上，而已删除未提交的文件又仍在索引里，报错会让人困惑。
- 在 bash 里用 `globstar`：macOS 自带的 bash 3.2 不支持。

## Consequences

新增或删除冒烟不再需要改 `check-host.sh`，并行的 Android PR 不会再在这一行上冲突。检查比原来更严：少一个冒烟也会失败，并在报错里给出两边的数量。三处排除现在写在两个地方（发现循环的 `case` 和独立计数的 `grep`），以后增减排除要两处一起改，只改一处会让检查直接失败，而不是悄悄放过。这个改动本身会让当时还开着、改过下限那一行的 Android PR 最后再冲突一次。

## Verification

把发现循环和新检查单独截出来运行：正常情况数出 174 个并通过；把 `*Smoke.java` 的筛选改坏（数出 0 个）、多排除一个目录（数出 123 个）、把 `tests/` 指到不存在的目录（两边都是 0），三种情况都报错退出。完整的 `bash platforms/android/check-host.sh` 见 PR。
