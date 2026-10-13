# 脚本插件合集

这里是一组拿来就能用的桌面脚本插件（`kind = "script"`）：每个插件是一个用 Rust 写的 `no_std` crate，编译成 core wasm 模块，由输入法进程里的沙箱解释器运行。它们按 [docs/plugins.md 的「脚本插件」一节](../../docs/plugins.md#脚本插件kind--script)的规则写成，用的是 [docs/script-plugin-template](../../docs/script-plugin-template/) 里的 SDK。

- 只在桌面运行：macOS、Windows 和 Linux 的输入法进程。iOS、Android 和鸿蒙不认这个类型。
- 导入后默认停用，要在设置的「插件 → 脚本插件（实验）」里打开，并在授权对话框里逐项勾选能力才会运行。
- 授权文件 `<数据目录>/plugins/script-grants.json` 在用户可写的目录下，不签名：同一用户下的其他程序能伪造一条授权，让脚本在输入法里运行。只装你信得过的脚本包；这里的包都从本仓库的源码构建，构建和测试脚本也在仓库里。
- 不能发布到社区库，只能本地导入。
- 插件没有设置项、没有持久存储，也不能联网、读写文件或看时间；它只能经授权的函数读当前这一次的上屏文本或候选窗口，在宿主允许的范围内改写、重排、隐藏或注释。

## 插件

| id | 名称 | 钩子 | 能力 | 许可证 | 模块 | 内存 |
| --- | --- | --- | --- | --- | --- | --- |
| [hide-rare-chars](hide-rare-chars/) | 隐藏生僻字 | `candidates` | `read_candidates`、`filter_candidates` | MIT AND Unicode-3.0 AND Apache-2.0 | 6.9 KiB | 2 页 |
| [rare-chars-last](rare-chars-last/) | 生僻字靠后 | `candidates` | `read_candidates`、`rerank_candidates` | MIT AND Unicode-3.0 AND Apache-2.0 | 12.4 KiB | 2 页 |
<!-- 插件表：每个插件一行，插在这条注释之上，按 id 字母序排列。模块大小和内存页数取 scripts/build-script-plugins.sh 打印的值。 -->

几个插件一起启用时，执行顺序在「脚本插件（实验）」里调整，合成规则见 docs/plugins.md：重排是排在后面的插件优先，隐藏取并集，同一行的注释后面的覆盖前面的。落到这两个插件上：rare-chars-last 和 hide-rare-chars 二选一，两个都开时被隐藏的行不再显示，靠后白做。

## 构建和测试

需要 `rust-toolchain.toml` 钉住的 1.97.1 加上 `wasm32-unknown-unknown` 目标：

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.97.1
```

```sh
scripts/build-script-plugins.sh [<id>...]     # 只构建和组装
scripts/test-script-plugins.sh [<id>...]      # 构建、校验、按夹具运行并核对
scripts/test-script-plugins.sh --update <id>  # 把运行结果写回 <id>/expected.jsonl
```

- 构建产物都在仓库根的 `target/script-plugins/` 下：`cargo/` 是 cargo 的产物目录，`<id>/` 是可以导入的插件包文件夹，`<id>-<version>.zip` 是同样内容的压缩包（条目排序、时间固定、不压缩，同样的源码得到逐字节相同的压缩包）。
- `test-script-plugins.sh` 先编出 `target/debug/msime-pack`，然后对每个插件：`msime-pack validate` 校验文件夹和压缩包；用 `fixture.jsonl` 跑 `msime-pack run --grant all`，输出去掉 `fuel`、`init_fuel` 后必须和 `expected.jsonl` 逐行相同，每一行的 fuel 不能超过钩子单次上限的一半（`candidates` 62 500，`before_commit` 50 000）；再不授予任何能力跑一遍，每个事件都不能有动作。最后跑 `scripts/test-script-plugin-data.py`，确认生成的数据文件没有漂移；`common-chars.bin` 要用锁定的 Unihan zip 重新生成，缓存里没有 zip 时这一项打印 skip，设 `MSIME_FETCH=1` 才下载。
- 没有 wasm 目标时测试脚本打印 `skip` 并以 0 退出；CI 设 `MSIME_REQUIRE_SCRIPT_PLUGINS=1`，缺目标就失败，生成数据的检查打印 skip 也算失败。CI 同时设 `MSIME_FETCH=1` 并缓存 Unihan zip，所以 `common-chars.bin` 在 CI 上每次都核对。
- CI：`.github/workflows/ci-platforms.yml` 的「Script plugins」作业在 `plugins/`、模板与 SDK、`crates/script-runtime`、`crates/pack-tool`、这几个脚本、`resources/licenses/Unicode-License-V3.txt` 或 `resources/character-glosses.lock.json` 改动时运行 `test-script-plugins.sh`。

## 导入

在设置的「插件 → 我的插件」里「导入文件夹」选 `target/script-plugins/<id>/`，或「导入 .zip」选对应的压缩包。然后在「脚本插件（实验）」里打开它，勾选清单申请的能力。清单或模块改了一个字节，授权就作废，要重新授权。

## 目录约定

```text
plugins/script/
  .cargo/config.toml    所有插件共用：目标 wasm32-unknown-unknown、64 KiB 栈、release 配置、产物目录
  data/                 多个插件共用的生成数据（提交进仓库）和它的来源说明
  shared/               多个插件共用的 Rust 源码，用 #[path] 引入
  tools/                生成器（gen_*.py）和构建、测试用的辅助脚本
  <id>/
    Cargo.toml          [package] name 就是 id；自带空的 [workspace]；crate-type = ["cdylib"]；不写 profile
    src/lib.rs          #![no_std]，用 #[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"] 引入 SDK
    data/               只有这个插件用的生成数据（提交进仓库），生成器同样放在 tools/gen_*.py
    pack/               插件包里除模块以外的全部文件：plugin.toml、LICENSE.txt、需要时的 NOTICE.txt 和第三方许可证
    fixture.jsonl       给 msime-pack run 的事件
    expected.jsonl      去掉 fuel 之后的期望输出
    README.md           行为、边界情况、最坏情况的 fuel
```

- 目录名、`Cargo.toml` 的包名和 `plugin.toml` 的 `id` 必须相同；清单的 `module` 写 `<id>.wasm`，构建时把编出的模块改成这个名字放进包里。源码树里不放 `.wasm`，`.gitignore` 排除了它、`target/` 和 `Cargo.lock`。
- 插件 crate 没有任何依赖；不要加。
- `memory_pages` 写模块实际需要的最小页数（构建脚本会打印，例如 `memory 2/2 pages`）：64 KiB 的栈占第一页，静态数据从第二页开始。
- 包里最多 16 个文件，说明文件（`.txt`、`.md`）每个不超过 64 KiB，文件名只用 ASCII 字母、数字、`_`、`-` 和 `.`。
- 带第三方数据的插件把许可证原文放进 `pack/`：`Unicode-License-V3.txt` 复制自 `resources/licenses/Unicode-License-V3.txt`，`OpenCC-LICENSE.txt` 复制自 `crates/client-core/data/opencc/LICENSE`，组装时逐字节核对。`NOTICE.txt` 写明数据来源、版本和生成器。
- 许可证义务由组装脚本按模块实际编进的数据核对（`tools/assemble.py` 的 `EMBEDDED_DATA`）：它顺着 `#[path]` 找到 crate 的全部源文件，收集 `include_bytes!` 的文件。编进 `data/common-chars.bin` 的包必须带 `Unicode-License-V3.txt` 和 `OpenCC-LICENSE.txt`，`license` 含 `Unicode-3.0` 和 `Apache-2.0`，`NOTICE.txt` 原文包含 `data/common-chars-NOTICE.txt`；编进 `traditional-glyph/data/s2t.bin` 的必须带 `OpenCC-LICENSE.txt`、`license` 含 `Apache-2.0`。每个包都要有 `LICENSE.txt`、`license` 含 `MIT`。新的数据文件先在 `EMBEDDED_DATA` 里登记义务，否则组装失败。
- 插件代码用 MIT，全文在 `plugins/script/LICENSE`，每个包的 `pack/LICENSE.txt` 是它的副本；仓库其余部分是 GPL-3.0-only，这个目录和模板是写明的例外（README 的「许可证」一节）。生成数据的来源和许可证写在 docs/third-party.md 的「脚本插件合集」一节。

## 共用的数据

- `data/common-chars.bin`：常用字集，给生僻字类插件用。《通用规范汉字表》8105 字（Unihan 17.0.0 的 `kTGH`）并上它们的繁体形（Unihan `kTraditionalVariant` 和 OpenCC `STCharacters.txt`），共 11 049 字，4 KiB。由 `tools/gen_common_chars.py` 生成，来源说明在同目录的 `common-chars-NOTICE.txt`。读法在 `shared/common_chars.rs`：`is_rare`、`is_kana` 和按字节扫描一行的 `classify`。`classify` 是判定规则的参照实现，按 fuel 的计法太贵（每个常用汉字约 200–290、每个 ASCII 字节约 35），扫满一个 20 行的窗口会耗尽 `candidates` 的 fuel：hide-rare-chars 只调用 `is_han`、`is_common`、`is_kana`，自己在 `src/scan.rs` 里按字节跳过；rare-chars-last 在 `src/rare.rs` 里把数据文件在编译时展开成查表，判定和 `classify` 逐字相同。
- 重新生成：`python3 plugins/script/tools/gen_common_chars.py`（第一次会把锁定的 Unihan zip 下载到 `target/script-plugins/cache/` 并校验 sha256）。`--check` 只比较不写入；缓存里没有 zip 时打印 skip。

## 写插件时的 fuel

fuel 是脚本唯一的硬限制，超出就算一次失败，最近 50 次投递里失败 3 次插件就被停用。实测 wasmi 的计法比「一条指令一个单位」更苛刻，写插件时按下面几条来：

- `local.get`、`local.set` 也各算一条。
- 在函数入口和每个循环的开头，按函数体（或循环体）里词法上的全部指令一次扣完，不管实际走哪个分支、是否提前返回；`block` 和 `br_if` 不另起计数。所以把逐字节执行的循环单独放进一个很小的 `#[inline(never)]` 函数，只在少数情况下用到的代码也各自拆成不内联的函数。
- SDK 的 `text()` 会对整段做 UTF-8 校验，在解释器里每个汉字要上百 fuel。逐字扫描时用 `commit::bytes`、`candidates::bytes`，宿主给的总是合法 UTF-8。
- 每次调用宿主函数先扣 100，每复制 4 字节再扣 1；`log` 也一样，每个事件写一行就够。
- 给每个插件定一个自限的工作量（读多少字节、看多少处、查多少次表），超出时放弃剩下的部分或整次操作，然后在夹具里放一个最坏情况的事件卡住它。
