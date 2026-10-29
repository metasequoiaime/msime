# msime-path-trust

The one list of system symbolic links a storage path may pass through, and the walk that rejects every other link on the way to it. A dependency of `msime-engine`, the input engine of 水杉输入法 (Metasequoia IME); licensed under MPL-2.0 like the engine.

存储路径要做符号链接检查，防止被人放进去的链接把写入重定向到别处。但通往应用存储的路径上有些链接本来就属于操作系统：macOS 和 iOS 的 `/tmp`、`/var` 指向 `/private`，Android 应用自己的 mount namespace 里 `/data/user/0` 指向 `/data/data`。连它们一起拒绝，等于拒绝了该平台上的全部存储。这个 crate 是唯一列出这些系统链接、并核对每条链接唯一受信任目标的地方，`msime-engine` 和 msime 里逐层检查存储路径的 crate 都来这里查询。

只依赖 Unix 上的 `libc`（`geteuid`、`fstatat`，用来判断多链接文件所在的目录是否只有 root 或当前用户能写）。`test-support` 特性只给检查拒绝行为的其他 crate 的测试用。

## 许可证

按 [Mozilla Public License 2.0](LICENSE) 授权，不附 Exhibit B（「Incompatible With Secondary Licenses」），所以按 MPL-2.0 第 3.3 条仍能与 GPL 代码组合；Exhibit A 声明在 `LICENSE` 开头和 `src/lib.rs` 第一行。crate 里只有本项目自己的代码，没有第三方数据。仓库的其余部分仍是 GPL-3.0-only，见根目录的 [README](https://github.com/metasequoiaime/msime/blob/develop/README.md#许可证)。

## 贡献

向本 crate 提交的贡献按「GPL-3.0-only OR MPL-2.0」授权，规则与 `msime-engine` 相同，见 [crates/engine/README.md](https://github.com/metasequoiaime/msime/blob/develop/crates/engine/README.md#贡献)。
