# Agent Note: 万象改取数据仓库 rime-wanxiang-data，reweight 不再读 lianxiang、renming

Status: implemented

## Problem

[reweight 阶段](2026-11-04-wanxiang-reweight-stage.md)读的 17 份万象词库来自配置仓库 amzxyz/rime-wanxiang 的 `v18.0.16`（`7ab73ee`）。作者把词库数据移到了数据仓库 [amzxyz/rime-wanxiang-data](https://github.com/amzxyz/rime-wanxiang-data)（`wanxiang` 分支，`dicts/`），配置仓库的 `dicts/` 不再更新。msime-dictionary#140 把 `sources/pinyin/wanxiang/` 换成数据仓库 `6b317736a8f90bb6dddf1423dbd1f0b83d4bf764` 的版本，但数据仓库的 `dicts/` 已经没有 `lianxiang.dict.yaml`、`renming.dict.yaml`：构建器的 `reweight::IMPORTED` 写死了这两份，`load` 按 `upstream.lock.json` 逐份取文件，缺一份构建就失败。

## Decision

- `IMPORTED` 去掉 `lianxiang`、`renming`，剩 13 份；`RECORDED_ONLY` 仍是 `cuoyin`、`mixed`，登记的全套是 15 份。第一步的规则和三个文件系数不变，只是按新数据重算共有行；万象不再收录的行保留 #84 按 `7ab73ee` 改编进源文件的值。
- `curation/pinyin-reweight.txt` 的 `renming` 设置取消：词库不在了，比例乘不到任何一行。写了它时 `Settings::parse` 报错并说明原因，不静默接受，免得维护者以为人名还在被压。`duoyin`、`unattested-cap`、`cap-leader-minimum` 不变。msime-dictionary 目前没有这份文件，不影响任何产物。
- `resources/dictionary-sources.lock.json` 的 `rime-wanxiang` 引用改成数据仓库的地址与提交，`rime-ice-supplement` 同批改成 `927e1cf`（与 msime-dictionary#140 一起换的补充表）。
- 署名：`rime-wanxiang-CC-BY-4.0.txt` 以数据仓库提交为主，同时保留 `7ab73ee` 与原 15 份词库的记录，因为当前发布的 `dict-v2.1.2` 和之后仍留在源文件里的 #84 权重都来自它；两次提交的 `LICENSE` 逐字节相同（CC BY 4.0），数据仓库另有 `LICENSE-SCOPE.md` 写明词库按它执行。三份总览、`docs/third-party.md` 和 `web-engine-NOTICE.md` 的嵌入副本同步改。署名的总体做法见 [rime-wanxiang 权重署名](../process/2026-10-29-rime-wanxiang-weights-attribution.md)。
- 笔画库的署名不改：`msime-stroke.db` 直接读 `single-chars.txt` 存的字频，不经过 `reweight`，这次没有换这份文件，它的权重仍然只来自 `7ab73ee`。

## Alternatives considered

- **保留 `lianxiang`、`renming` 的旧副本，其余 15 份取数据仓库。** 不用改构建器，权重变化最小。但这样同一个上游名下混着两个提交的文件，`upstream.lock.json` 只能记一个提交，记录就是假的；旧副本也不会再更新。msime-dictionary 那边的同步脚本也明确拒绝这样补齐。
- **保留 `renming` 设置、文件缺失时忽略。** 改动更少，以后上游若再加回人名表能直接用。但一个不起作用的设置会让 `curation/` 看起来在做事，与「每一条清洗都必须匹配到东西」的约定相反；真加回来时连同 `IMPORTED` 一起改回即可。

## Consequences

- 合入 msime-dictionary#140 之后，共有行的权重随数据仓库的词频变化，不再与 `7ab73ee` 逐行吻合；`lianxiang`（联想词）、`renming`（人名）只在这两份里出现的词不再有万象权重和「万象收录」的佐证，`unattested-cap` 打开时它们可能被当成碎片。打开第二步之前要按新数据重跑 `cleanse-report` 复核。
- `places-supplement` 的万象下限也少了这两份，`sources/pinyin/places.txt` 要在构建器升级后重新生成。
- 本地验证：`crates/dict-builder` 在 `dd32caf` 与 develop 之间没有改动，在 `dd32caf` 上套用同一份改动跑 `cargo test -p msime-dict-builder`，321 项通过；失败的 3 项（粤拼、注音、笔画的 `the_engine_reads_what_is_written_and_the_bytes_repeat`）在不带改动的 `dd32caf` 上同样失败，与本次无关。`scripts/test-language-data-notices.py` 通过。develop 上新增的 `llama-cpp-rs` 依赖在这台 Windows 机器上取子模块时路径过长，无法在 develop 检出上直接编译，由 PR 的 CI 补跑。
