# msime-script-sdk

Rust bindings for the desktop script plugin ABI v1 of 水杉输入法 (Metasequoia IME): `no_std`, no dependencies, for plugins compiled to `wasm32-unknown-unknown`.

水杉输入法桌面脚本插件 ABI v1 的 Rust 绑定。脚本插件是一个 core wasm 模块，在 macOS、Windows 和 Linux 的输入法进程里由沙箱解释器运行，按用户逐项授予的能力读组字、上屏文本和候选，改写上屏、重排、隐藏或注释候选，或者在自己声明的 `/` 指令里给出候选。

## 用法

最快的开始方式是用 `msime-pack` 生成一个插件 crate，它已经依赖这个 SDK：

```sh
rustup target add wasm32-unknown-unknown
msime-pack new my-plugin
cd my-plugin && msime-pack build
```

手写的话，`Cargo.toml` 里：

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
msime-script-sdk = "0.1"
```

插件自己导出 `msime_abi`、`msime_on_event`，可选导出 `msime_init`，并写 `#[panic_handler]`，最小的例子在 crate 文档开头。模块按能力域分组，和清单的 `permissions` 一一对应；只有真正调用到的函数才会成为模块的 import，用到哪一组就申请哪一项能力。

`msime_script.h` 是同一份 ABI 的 C 头文件，给 C 和 Zig 作者，开头有编译命令。

## 文档

清单、能力、钩子、限制、隐私规则和 `msime-pack` 的用法见 msime 仓库的 [docs/plugins.md「脚本插件」一节](https://github.com/metasequoiaime/msime/blob/develop/docs/plugins.md)。编号只会追加，不会改值；每个常量和 import 都有测试对照输入法的运行时逐项核对。

## 许可证

MIT，全文见 [LICENSE](LICENSE)。SDK 会编进每个插件，用它写的插件不受 msime 仓库其余部分的 GPL-3.0-only 约束。
