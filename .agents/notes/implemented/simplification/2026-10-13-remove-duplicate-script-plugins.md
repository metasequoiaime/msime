# Agent Note: 脚本插件合集删掉与内置重复的三个插件

Status: implemented

## Problem

[脚本插件合集](../architecture/2026-10-12-script-plugin-collection.md)的五个插件里有三个做的是输入法已经内置的事：

- pangu-spacing：内置的「中英文之间加空格」开关（`crates/host-api/src/lib.rs` 的上屏处理，见[中英文之间加空格](../feature/2026-10-12-chinese-english-spacing.md)）。
- traditional-glyph：内置的繁体输出直接上屏繁体，注释只是把同一份 OpenCC 结果再写一遍。
- unicode-codepoint：内置的 U 本地模式（`crates/engine/src/local/unicode.rs`）。

维护者看过合集后的判断是这批插件几乎没有价值：用户装上以后得到的是已有功能的另一份。traditional-glyph 的模块还有约 127 KiB，按 `crates/script-runtime/src/limits.rs` 的内存总账要占约 16 MiB。

## Decision

- 删掉 `plugins/script/` 下这三个插件，以及只给 traditional-glyph 用的生成器 `tools/gen_s2t_table.py` 和 `assemble.py` 里 `s2t.bin` 的数据登记。
- 保留 rare-chars-last 和 hide-rare-chars：内置的 `character_set` 只管简繁，没有按常用字集过滤候选的功能。
- `docs/script-plugin-template/` 仍是 pangu-spacing：它是写法示例，不是给用户装的插件，README 改为指向内置开关。
- 以后往合集里加插件前，先核对内置功能清单（本地模式表 `crates/engine/src/session/input.rs`、`crates/engine/src/local/`、`crates/host-api/src/lib.rs` 的上屏处理），重复的不收。

## Alternatives considered

- **保留三个插件，在 README 里注明与内置重复。** 最强的理由是代码已经写好、测过，删掉是浪费。不用：合集的意义是给用户内置没有的东西，重复的插件只会让人以为是新功能；它们也不随输入法分发，留着只有维护成本。
- **连模板也换成别的例子。** 最强的理由是模板和内置功能重名。不用：模板只演示 `before_commit` 和 `rewrite_commit` 的写法，`msime-pack` 的测试还在解析它的夹具，换掉要连带改作者工具的测试。

## Consequences

- 合集只剩两个生僻字插件；`scripts/test-script-plugins.sh` 和 `test-script-plugin-data.py` 按目录和 `gen_*.py` 自动发现，不用改脚本。
- 用户已经导入的这三个包不受影响：插件是本地导入的，删掉的只是源码。
- `docs/third-party.md` 的 OpenCC 一行只剩常用字集用到的部分。

## Verification

- `MSIME_REQUIRE_SCRIPT_PLUGINS=1 MSIME_FETCH=1 scripts/test-script-plugins.sh`：剩下两个插件通过，生成数据检查只剩 `gen_common_chars.py` 并通过。
- `pnpm run verify-notes` 通过。

相关：[脚本插件合集](../architecture/2026-10-12-script-plugin-collection.md)。
