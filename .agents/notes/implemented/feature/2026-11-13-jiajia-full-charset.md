# Agent Note: 拼音加加辅助码同步到完整大字符集

Status: implemented

## Problem

本仓库随包的 `resources/helpcodes/jiajia_helpcode.txt` 是从 MSIME-Windows 原样带来的，只有 7968 个常用字。双拼/全拼的双码筛选按首尾字的码过滤，GBK 扩展区和词库里的生僻字没有码，输入辅助码时候选会直接消失。MSIME-Windows 已在 `f7bb1134` 把这张表扩到 21327 字；不同步的话，本仓库的生僻字覆盖会一直落后于 Windows。

## Decision

- **整表同步，不在这里重新生成**：`resources/helpcodes/jiajia_helpcode.txt` 与 MSIME-Windows `f7bb1134`（PR #1483）的表逐字节相同，21327 行，`sha256:657cc28534087c3e0efba856a0d02e9f476d59b3213263beff517b5c676aad99`。原有 7968 条一字未改，新增的 13359 条由源仓库的 `engine/helpcode/scripts/build_jiajia_helpcode.py` 按「前两个部件各取读音首字母」从 rime-radical-pinyin `2026.06.18` 的拆字数据生成。
- **覆盖与缺字**：GBK 字集（CP936 解码 20924 个汉字）覆盖 20901 个；23 个没有拆字数据的 GBK 字未收录，扩展 A 的 䱷 同样跳过。GBK 之外另含 426 个字符（扩展 A 133、扩展 B 46、扩展 C–G 170、兼容汉字 69、GB18030 独有的 8 个 BMP 字）。
- **测试跟着表的规模走**：`crates/engine/src/helpcode.rs` 与 `crates/engine/src/host/tests.rs` 的整表载入断言改为 21327；golden 场景 `bridge_jiajia_helpcode_filter.json` 的 `source` 行号改为新表里 拟/好/你 的实际行号（15864/18266/20850）。
- **来源记录同步**：`resources/helpcodes/NOTICE.md` 与 `docs/third-party.md` 的来源提交改为 `f7bb1134ef1c950f064e76b4ab81e98198fc857d`，摘要和行数改为上表；网页引擎的 NOTICE 副本同步同一段文本。
- **分发限制不变**：原有 7968 条里有一部分来自商业软件拼音加加 5.x 的 `fzm.bin`，本仓库的 GPL-3.0 仍不覆盖这张表；退出方式仍是删除 `HELPCODES` 里的 `jiajia` 行。

## Alternatives considered

- **不同步，保持 7968 字** — 代价最小，但本仓库会继续丢生僻字候选，和 Windows 端行为分叉。否定。
- **从这里已有的自然码/小鹤表借码** — 覆盖够，但语义是另一套方案，用户按加加规则输入会落空。否定。
- **把生成器移植到 `crates/dict-builder`，由本仓库重新生成** — 最强的理由是生成命令、摘要和规则都在一个仓库里，和 `wubi86-helpcode` 一样可复现，也能摆脱对 `pypinyin` 的依赖。这次没有采用：上游生成流程已经随 MSIME-Windows 的 13k 行数据一起交付，当前同步是数据搬运；移植要重新实现多拆法选择和俗称读音，属于另一个可单独评审的改动。若今后加加表在本仓库成为一等数据，优先做这件事。
- **保留旧断言，只校验前 7968 行** — 会让「整表载入」变成假的，新增字被解析器静默丢掉也发现不了。否定。

## Consequences

- **收益**：本仓库的加加辅助码覆盖与 Windows 一致，GBK 生僻字不再因为缺码从双码筛选中消失；来源、摘要和缺字都有记录。
- **代价与已知上限**：码表从约 55 KB 增到约 149 KB；新增字的码是按公开拆字数据生成的近似码，不保证与商业加加逐字一致，候选窗会显示实际码。23 个 GBK 字和 䱷 仍无码。
- **重访信号**：MSIME-Windows 再次更新这张表时同步并更新摘要；加加表需要在本仓库独立生成时，按上面的候选方案移植生成器。

## Verification

- 表：21327 行、149505 字节、无 BOM、LF、按码点降序、无重复，每行 `字=两个小写字母`；`好=nz`、`你=de`、`中=ks`、`国=ky`。
- 来源：`resources/helpcodes/jiajia_helpcode.txt` 的 SHA-256 与 NOTICE 记录一致；golden 场景引用的三行分别是 拟/好/你。
- 未运行 `cargo test -p msime-engine`：当前环境没有 `cargo`，Rust 侧只做了静态核对；由 CI 或本地 Rust 工具链验证整表载入断言。
