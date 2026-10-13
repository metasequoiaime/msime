# 脚本插件模板：pangu-spacing

这是一个桌面脚本插件（`kind = "script"`）的完整例子：上屏前在中文和英文字母、数字之间加一个空格，「用Rust写插件」上屏为「用 Rust 写插件」。用 Rust 写，编译成 core wasm 模块，在输入法进程里由沙箱解释器运行。规则、能力和限制见 [docs/plugins.md 的「脚本插件」一节](../plugins.md#脚本插件kind--script)。

这个例子能编译、能通过校验、能在 `msime-pack run` 里看到它的改写和宿主给的结果。

## 文件

```text
script-plugin-template/
  Cargo.toml            独立的 crate（自带 [workspace]，不属于仓库的工作区），编译成 cdylib
  .cargo/config.toml    默认目标 wasm32-unknown-unknown，栈缩到 64 KiB
  src/lib.rs            插件本身
  sdk/msime_script.rs   ABI v1 的 Rust 绑定：单文件，只用 core
  sdk/msime_script.h    同一份 ABI 的 C 头文件
  pack/plugin.toml      清单；编译出的 pangu.wasm 放到它旁边，这个文件夹就是插件包
  fixture.jsonl         给 msime-pack run 的事件
```

仓库不提交编译出来的 `.wasm`，`.gitignore` 已经排除了它和 `target/`。

## 编译

需要 `wasm32-unknown-unknown` 目标。仓库根目录的 `rust-toolchain.toml` 把编译器固定在 1.97.1，只装了 rustfmt 和 clippy，不带这个目标，在仓库里编译要先给这个版本加上：

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.97.1
```

把模板复制到仓库外面单独开发时，给自己用的工具链加上同一个目标即可。然后：

```sh
cd docs/script-plugin-template
cargo build --release
cp target/wasm32-unknown-unknown/release/pangu_spacing.wasm pack/pangu.wasm
```

- Rust 默认给 wasm 留 1 MiB 的栈，线性内存因此至少 17 页，超过 `memory_pages` 的上限就会被拒绝（报 `memory must be 32-bit and its minimum must fit memory_pages`）。`.cargo/config.toml` 用 `-zstack-size=65536` 把栈缩到 64 KiB，模块只要 2 页，清单写 `memory_pages = 4`。
- 不需要 `wasm-bindgen`，也不能用 WASI：模块只能导入 `msime` 下的函数。
- 只有真正调用到的绑定函数才会成为 import。这个例子用到 `event_scalar`、`commit_text`、`rewrite_commit` 和 `log`，所以清单申请 `read_commit` 和 `rewrite_commit`；申请了没用到、或者用到了没申请，校验都会拒绝。

用 C 写的话，`sdk/msime_script.h` 的开头有编译命令（clang 自带 wasm32 后端，链接要 `wasm-ld`）。

## 校验和运行

```sh
cargo build -p msime-pack-tool --bin msime-pack
target/debug/msime-pack validate docs/script-plugin-template/pack
target/debug/msime-pack run docs/script-plugin-template/pack \
    --events docs/script-plugin-template/fixture.jsonl --grant all
```

`validate` 输出 `ok pangu-spacing script 1.0.0`。`run` 每个事件一行：

```text
{"seq":1,"event":"before_commit","status":"ok","fuel":6303,"init_fuel":2,"actions":[{"import":"rewrite_commit","result":"ok"}],"log":[{"level":"debug","text":"用 Rust 写插件"}]}
{"seq":2,"event":"before_commit","status":"ok","fuel":7115,"actions":[{"import":"rewrite_commit","result":"ok"}],"log":[{"level":"debug","text":"今天是 2026 年"}]}
{"seq":3,"event":"before_commit","status":"ok","fuel":10258,"actions":[],"log":[]}
{"seq":4,"event":"before_commit","status":"ok","fuel":215,"actions":[],"log":[]}
```

- 第 1、2 行：插件改写上屏，宿主接受（`ok`），`log` 里是改写后的文本。第 2 行的句号在尾巴里，插件只看到、只改写 body。
- 第 3 行：已经有空格，不改。
- 第 4 行：网址模式下上屏不可改写（`rewritable` 为 0），插件直接返回。
- 不写 `--grant` 时什么都不授予，`commit_text` 返回 `DENIED`，插件按原文上屏。
- fuel 的数值随编译器版本、目标和优化选项变化，只作参考；`before_commit` 每次最多 100 000。
- 这个例子为了好读，逐字处理并先对整段做 UTF-8 校验：body 超过约 100 个汉字就会耗尽 `before_commit` 的 fuel，在输入法里算一次失败。中英文之间加空格输入法已经内置（「中英文之间加空格」开关），这个模板只作写法示例；按字节扫描、给工作量设上限的写法见 [plugins/script 的 README](../../plugins/script/README.md#写插件时的-fuel)。

夹具的格式见 [docs/plugins.md](../plugins.md#用-msime-pack-run-试运行)。

## 安装

在设置的「插件」页用「导入文件夹」选 `pack` 文件夹（macOS、Linux 和 Windows）。导入后插件默认停用：在「脚本插件（实验）」分组里打开它，在授权对话框里勾选清单申请的能力后才会运行。
