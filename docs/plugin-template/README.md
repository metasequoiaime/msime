# 插件模板

这是一个能直接通过校验的最小按键音效包（`kind = "sound"`，`mode = "keys"`），作为写新插件的起点。

- `plugin.toml`：清单。复制整个文件夹后，把文件夹名和 `id` 改成同一个新 id，再改 `name`、`version`、`license`、`author`、`description`。
- `key.wav`：普通键的音效，`[sounds] default` 指向它。
- `space.wav`：空格键的音效。
- `README.md`：说明文字。插件里允许放 `.txt` 和 `.md` 说明文件，单个不超过 64 KiB。

两段音频是 22050 Hz、单声道、16 位 PCM 的短促正弦波，按 CC0-1.0 提供，可以直接替换成自己的录音。音效包只接受 WAV。

改完后校验：

```sh
cargo build -p msime-pack-tool --bin msime-pack
target/debug/msime-pack validate docs/plugin-template
```

输出 `ok example-keys sound 1.0.0` 就可以在设置的「插件」页里用「导入文件夹」安装。完整的格式、限制和其他类型见 [docs/plugins.md](../plugins.md)。

## 其他类型

这个文件夹本身就是一个包，包里不能有子文件夹，所以其他类型的模板不放在这里：

- 短语表（`phrase_table`）和符号集（`symbol_set`）只有清单，把上面的 `[sounds]` 和两段音频换成 [docs/plugins.md](../plugins.md) 对应一节里的 `[[phrases]]` 或 `[[groups]]` 即可，`mode` 一行也要删掉。
- 辅助码表（`helpcode`）和单词本（`wordbook`）各需要一个数据文件：辅助码表在 `[helpcode] table` 里点名一个 `.txt`，单词本在 `[wordbook] file` 里点名一个 `.tsv`。单词本的 `id` 只能用小写字母、数字和 `-`，不超过 59 个字符。
- 每种类型能通过校验的完整例子在 `crates/client-core/tests/fixtures/plugin-packs/valid/` 下，按 `<类型>-<用例>` 命名；`invalid/` 下是会被拒绝的例子。
