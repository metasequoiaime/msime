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
