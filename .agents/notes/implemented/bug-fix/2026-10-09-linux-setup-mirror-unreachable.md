# Agent Note: Linux 安装脚本的镜像两个源都连不上时仍报「词库目录未改动」

Status: implemented

## Problem

cc1fc979a 让 `msime-linux-setup` 下载词库时先试 `https://dl.msime.app/gh/` 镜像、再试锁里的原地址。新加的 `fetch_verified` 把两个源的 `OSError` 和 `SystemExit` 一并收集，最后统一抛 `SystemExit`。原来网络失败抛的是 `OSError`，由 `fetch_missing` 接住，报「下载失败」并写出输入法正在读、这次没有改动的词库目录。改动之后这条路径接不到异常，`--update` 下载失败时只剩「目标目录未改动」，而这里的目标其实是暂存目录，用户看不出正在用的词库是否完好。

同一提交没有改 `linux-setup-update` 和 `linux-setup-resolution`，两者在 CI 上失败。前者还会真的去请求公网镜像（CI 日志里是 403），测试不再封闭。

## Decision

- `fetch_verified` 区分两种失败：两个源都只是连不上（`OSError`）时继续抛 `OSError`，交给 `fetch_missing` 报告，和加镜像之前一致；有一个源给了超限或 SHA-256 不符的内容时直接 `SystemExit` 中止，和以前「校验失败即中止」一致。
- `setup_update.py` 把复制到临时前缀的脚本里的 `MIRROR_PREFIX` 换成测试自己的本地 HTTP 服务器（`/mirror/<原地址>`），替换前断言常量还在，常量改名时测试会直接报错。这样测试不出网，还能单独让镜像缺文件或返回坏内容。
- 补上的用例：正常情况只请求镜像；镜像缺文件、镜像内容同长度但不符时回退原地址并成功；两个源都不通时报「下载失败」和正在用的词库目录。

## Alternatives considered

- 只把测试断言改成「目标目录未改动」：等于接受回归，用户在更新失败时失去「正在用的词库没动」这条信息。
- 给脚本加环境变量覆盖镜像地址：为测试在发行脚本上多开一个入口，而测试本来就复制了一份脚本，替换常量足够。
- 只对 GitHub 地址加镜像前缀：其他平台（client-core `mirrored`、Android `UpdateApi`、桌面端）都无条件拼前缀，Linux 单独例外会让各平台行为分叉。

## Consequences

镜像不可用时 `--download` / `--update` 的提示回到加镜像之前的样子，多列出两个源各自的错误。镜像回退逻辑第一次有了测试覆盖。

## Verification

- `python3 -I platforms/linux/tests/core/setup_update.py`：通过；脚本换回 develop 版本时在「两个源都不通」用例上失败。
- `python3 -I platforms/linux/tests/core/setup_resolution.py`：通过。
- `bash scripts/verify-local.sh --quick`（pre-push 门禁）
- `pnpm run verify-notes`
- `git diff --check`
