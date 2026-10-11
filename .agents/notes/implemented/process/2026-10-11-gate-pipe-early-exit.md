# Agent Note: 门禁脚本不把输出用管道交给 rg -q / grep -q

Status: implemented

## Problem

门禁脚本都开着 `set -o pipefail`。`rg -q` 和 `grep -q` 读到第一处命中就退出，上游的 printf、sed 或 rg 若还有没写完的输出，再写就收到 SIGPIPE（`printf: write error: Broken pipe`），整条管道按上游的非零状态判为失败。是否触发取决于上游把输出分成几次写、下游何时退出，所以同一份源码时过时不过：`platforms/android/check-host.sh` 的「android host java」阶段因此让 pre-push 钩子和 CI 偶发失败，重跑又过。本机复现：把两万行经 `printf | rg -q '^1$'` 交给首行命中的 `rg -q`，pipefail 下 100 次全部失败。

## Decision

门禁会跑到的 shell 里，判断变量内容一律用 here-string（`rg -q pattern <<< "$var"`），判断命令输出时先整段取进来（`grep -q pattern <<< "$(command)"`）。`check-host.sh` 的这类守卫有三十多处，都是 `rg -A … | rg -q` 或 `sed -n … | rg -q` 的形状，改用脚本里的 `rg_q_stdin`：它先 `cat` 读完标准输入，再交给 `rg -q`，上游永远写得进去，守卫的写法和判断内容不变。

`scripts/test-gate-pipe-early-exit.py` 扫描门禁 shell，发现单个 `|` 后接带 `-q` 或 `--quiet` 的 rg、grep 就失败。范围是 `scripts/verify-local.sh`、`scripts/run-checks.sh`、`.githooks/*`、`platforms/*/check-*.sh`、`platforms/*/tests/run.sh`，加上它们和 PR 上运行的 workflow（`ci*.yml`、`contracts.yml`、`quality.yml`）用路径点名的 `.sh`，逐层展开。只在手动触发的发版 workflow 里用到的脚本不扫。

## Alternatives considered

- 去掉 `-q`、改成 `| rg … >/dev/null`：改动最小，本机实测也会读完输入。但它看上去只是 `-q` 的啰嗦写法，下一个人顺手「简化」回 `-q` 就又回到原样，扫描也不好区分哪种 `>/dev/null` 是有意的；here-string 把「先拿到完整内容再判断」写在了形状上。
- `grep -c` 或 `rg -c` 读完全部输入再比较计数：能避开提前退出，可每一处都要改成比较输出，不命中时还要处理退出码 1，三十多处守卫的读法全部变样。
- 只在出过事的那一行改：修好了已知的一处，但同样形状的守卫还有几十处，其中输出更长的随时可能成为下一处；没有扫描，新写的守卫也会照旧写成管道。

## Consequences

守卫多读一次输入、多起一个 `cat`，对几 KB 的源码片段可以忽略。here-string 在输入末尾补一个换行，和原来的 `printf '%s\n'` 等价；`$(…)` 去掉末尾换行后再补回一个，按行匹配的结果不变。

扫描只认字面上的 `| rg`、`| grep`，经变量或函数间接调用的不在范围内；`| head` 这类同样提前退出的下游也不在范围内，现有的几处都带 `|| true` 且只取输出。

## Verification

`ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh` 改后通过（204 个 JVM smoke、设备套件编译和全部源码守卫）。`rg_q_stdin` 的命中、不命中、空输入、取反四种情形与原 `rg -q` 一致；两万行输入在 pipefail 下 100 次全部成功，原写法 100 次全部失败。扫描对 develop 上的 `check-host.sh` 报出 20 个逻辑行，对改后的 19 个门禁脚本报零。
