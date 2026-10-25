#!/usr/bin/env python3
"""检查符号名称表的副本没有在 msime 里被手改。

crates/engine/src/local/symbol_candidates.tsv 与 plugins/phrase_table/latex-symbols/symbols.tsv 是 msime-dictionary 的 sources/symbols/ 下两份表的逐字节副本，由 scripts/sync_symbol_tables.py 按 resources/symbol-tables.lock.json 同步。这里离线核对副本的大小和 SHA-256 等于锁，改副本而不改词库仓、或改了锁却没同步，都在这里失败；不联网。
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import sync_symbol_tables  # noqa: E402

problems = sync_symbol_tables.check(sync_symbol_tables.read_lock())
for problem in problems:
    print(problem, file=sys.stderr)
if problems:
    sys.exit(1)
print("symbol tables: copies match resources/symbol-tables.lock.json")
