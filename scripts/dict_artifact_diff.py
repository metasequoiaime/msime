#!/usr/bin/env python3
"""比较两次词库构建的产物目录（msime-dict-build 的 --out），逐个文件、逐张表写出差异报告。

`.github/workflows/dict-builder-artifact-diff.yml` 用 PR 的构建器和基准构建器，对同一份 msime-dictionary 数据各构建一次，再用本脚本比较两份产物。报告只是信息：默认有没有差异都以 0 退出；给了 `--expect-no-change` 时，任何内容差异都以 1 退出。

各类文件怎么比：

- SQLite 库（`*.db`，或文件头是 SQLite 魔数）：比较库头的几个 pragma、`sqlite_master` 里的建表和索引语句，再逐表比较行。每张表先定出「键列」：声明的主键，没有主键就取第一个 UNIQUE 约束，都没有就取除权重类列（`WEIGHT_COLUMNS`）以外的全部列。行按多重集合比较（同样的行出现两次与出现一次不同）。差异分成新增（键只在新库）、删除（键只在旧库）、改权重（键两边都有，只有权重类列变了）和改内容（键两边都有，别的列或重复次数变了）。另外比较两边都有的行的相对次序（rowid 次序），挪了位置的行记为改顺序：引擎在同权重的候选之间按 rowid 排，只改插入顺序也会改变候选顺序。逐字节相同的库直接记为相同，不再逐表比较；内容、行序、库头和定义都相同而字节不同（页布局、空闲页不同）的库不算差异。
- JSON：展平成路径到值，逐路径比较。`IGNORED_JSON_PATHS` 列出的路径只报告、不算差异：manifest 里的构建器提交和 `files`（产物的摘要，已由逐文件比较覆盖）。
- 校验和列表（`*SHA256SUMS*`）：由其他产物推出，只报告是否相同，不算差异。
- 其余可按 UTF-8 解码且不含 NUL 的文件按文本比较（行数、增删行数、开头一段 unified diff），否则按二进制比较（大小、SHA-256、等长时不同的字节数）。

退出码：0 完成；1 给了 `--expect-no-change` 且有差异；2 无法比较（目录不存在、库打不开），这时给了 `--markdown` 也会写一份说明原因的报告。

usage: dict_artifact_diff.py --base <dir> --head <dir> [--markdown <file>] [--json <file>] [--preamble <file>] [--expect-no-change] [--samples N] [--json-samples N] [--sample-tables N] [--max-markdown-chars N]
"""
import argparse
from array import array
from bisect import bisect_left
from collections import Counter
import difflib
import hashlib
import json
from pathlib import Path
import re
import sqlite3
import sys

# 名字是这些的列视为权重类：只有它们变了的行记为「改权重」，没有主键和 UNIQUE 约束的表也不拿它们当键。
WEIGHT_COLUMNS = frozenset({"weight", "sort_order", "frequency", "freq", "count", "score", "rank", "cost"})
# 每个文件里只报告、不算差异的 JSON 路径（前缀匹配）。构建器提交天然不同；`files` 是产物的大小和摘要，逐文件比较已经覆盖，SQLite 库内容相同而字节不同时也不该因此判为有差异。
IGNORED_JSON_PATHS = {
    "msime-dictionary-manifest.json": ("source.commit", "source.dirty", "format_contract_commit", "files"),
}
# 内置表的键：`sqlite_stat1` 没有约束，按 (tbl, idx) 配对，统计值变了记为改内容。
KEY_OVERRIDES = {"sqlite_stat1": ["tbl", "idx"]}
# 比较的库头 pragma。
PRAGMAS = ("user_version", "application_id", "page_size", "journal_mode", "encoding", "auto_vacuum")
SQLITE_MAGIC = b"SQLite format 3\x00"
# 多重集合比较时附加的重复次数列。
COUNT_COLUMN = "__msime_diff_n"
# 比较行次序时附加的列：同样的行第几次出现，和原表的 rowid。
OCCURRENCE = "__msime_diff_occurrence"
ROWID = "__msime_diff_rowid"
CHUNK = 1 << 20
TEXT_DIFF_LINES = 40
# 两边都不超过这么多行才算 unified diff。
TEXT_DIFF_MAX_INPUT = 20000
VALUE_LIMIT = 80
# 引号内的字符串、标识符和注释整段保留，只折叠它们之外的排版空白。
SQL_PARTS = re.compile(r"""'(?:[^']|'')*'|"(?:[^"]|"")*"|`(?:[^`]|``)*`|\[[^\]]*\]|--[^\n]*(?:\n|$)|/\*[\s\S]*?\*/|\s+""")
# 这三份生成文本的首行记录构建器提交；仅这个来源字段不属于词库内容。
SUPPLEMENT_PROFILES = {"wubi86", "wubi98", "wubixsj"}


def quote(name):
    return '"' + name.replace('"', '""') + '"'


def normalize_schema_sql(sql):
    if sql is None:
        return None
    return SQL_PARTS.sub(lambda match: " " if match[0].isspace() else match[0], sql).strip()


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            digest.update(block)
    return digest.hexdigest()


def same_bytes(a, b):
    if a.stat().st_size != b.stat().st_size:
        return False
    with a.open("rb") as left, b.open("rb") as right:
        while True:
            x = left.read(CHUNK)
            if x != right.read(CHUNK):
                return False
            if not x:
                return True


def is_sqlite(path):
    with path.open("rb") as handle:
        return handle.read(len(SQLITE_MAGIC)) == SQLITE_MAGIC


def kind_of(name, paths):
    if "SHA256SUMS" in Path(name).name:
        return "checksums"
    if name.endswith(".db") or all(is_sqlite(p) for p in paths):
        return "sqlite"
    if name.endswith(".json"):
        return "json"
    for path in paths:
        data = path.read_bytes() if path.stat().st_size <= 64 * CHUNK else None
        if data is None or b"\x00" in data:
            return "binary"
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            return "binary"
    return "text"


def list_files(root):
    return {p.relative_to(root).as_posix(): p for p in sorted(root.rglob("*")) if p.is_file()}


# ---- SQLite ----


def table_columns(con, schema, table):
    return [row[1] for row in con.execute(f"PRAGMA {schema}.table_info({quote(table)})")]


def key_columns(con, schema, table, columns):
    """主键，没有就取第一个 UNIQUE 约束，都没有就取除权重类列以外的全部列。"""
    if table in KEY_OVERRIDES:
        return KEY_OVERRIDES[table]
    info = list(con.execute(f"PRAGMA {schema}.table_info({quote(table)})"))
    primary = [row[1] for row in sorted((r for r in info if r[5] > 0), key=lambda r: r[5])]
    if primary:
        return primary
    for index in con.execute(f"PRAGMA {schema}.index_list({quote(table)})"):
        # index_list 的列：seq, name, unique, origin, partial。
        if index[2] and index[3] in ("u", "pk") and not index[4]:
            names = [row[2] for row in con.execute(f"PRAGMA {schema}.index_info({quote(index[1])})")]
            if names and all(name is not None for name in names):
                return names
    rest = [c for c in columns if c.lower() not in WEIGHT_COLUMNS]
    return rest or list(columns)


def render_value(value):
    if isinstance(value, bytes):
        text = "x'" + value[: VALUE_LIMIT // 2].hex() + ("…'" if len(value) > VALUE_LIMIT // 2 else "'")
    else:
        text = "NULL" if value is None else str(value)
    if len(text) > VALUE_LIMIT:
        text = text[:VALUE_LIMIT] + "…"
    return text


def row_dict(columns, row):
    out = {c: render_value(v) for c, v in zip(columns, row) if c != COUNT_COLUMN}
    count = row[columns.index(COUNT_COLUMN)] if COUNT_COLUMN in columns else 1
    if count != 1:
        out["×"] = str(count)
    return out


def compare_table(con, table, base_columns, head_columns, samples):
    columns = [c for c in base_columns if c in head_columns]
    result = {
        "base_rows": con.execute(f"SELECT COUNT(*) FROM main.{quote(table)}").fetchone()[0],
        "head_rows": con.execute(f"SELECT COUNT(*) FROM head.{quote(table)}").fetchone()[0],
        "columns_added": [c for c in head_columns if c not in base_columns],
        "columns_removed": [c for c in base_columns if c not in head_columns],
    }
    keys = [c for c in key_columns(con, "main", table, base_columns) if c in columns] or columns
    weights = [c for c in columns if c not in keys and c.lower() in WEIGHT_COLUMNS]
    others = [c for c in columns if c not in keys and c not in weights] + [COUNT_COLUMN]
    result["key"] = keys
    listed = ", ".join(quote(c) for c in columns)
    grouped = "SELECT {cols}, COUNT(*) AS {n} FROM {schema}.{table} GROUP BY {cols}"
    for name, left, right in (("added", "head", "main"), ("removed", "main", "head")):
        con.execute(f"DROP TABLE IF EXISTS temp.{name}")
        con.execute(
            f"CREATE TEMP TABLE {name} AS "
            + grouped.format(cols=listed, n=quote(COUNT_COLUMN), schema=left, table=quote(table))
            + " EXCEPT "
            + grouped.format(cols=listed, n=quote(COUNT_COLUMN), schema=right, table=quote(table))
        )
    key_list = ", ".join(quote(c) for c in keys)
    on = " AND ".join(f"x.{quote(c)} IS y.{quote(c)}" for c in keys)
    other_list = ", ".join(quote(c) for c in others)
    con.executescript(
        f"""
        CREATE INDEX temp.added_key ON added({key_list});
        CREATE INDEX temp.removed_key ON removed({key_list});
        DROP TABLE IF EXISTS temp.changed;
        DROP TABLE IF EXISTS temp.structural;
        CREATE TEMP TABLE changed AS SELECT {key_list} FROM temp.removed INTERSECT SELECT {key_list} FROM temp.added;
        CREATE INDEX temp.changed_key ON changed({key_list});
        CREATE TEMP TABLE structural AS
            SELECT {key_list} FROM (SELECT {key_list}, {other_list} FROM temp.removed EXCEPT SELECT {key_list}, {other_list} FROM temp.added)
            UNION
            SELECT {key_list} FROM (SELECT {key_list}, {other_list} FROM temp.added EXCEPT SELECT {key_list}, {other_list} FROM temp.removed);
        CREATE INDEX temp.structural_key ON structural({key_list});
        """
    )
    row_columns = columns + [COUNT_COLUMN]
    selected = ", ".join(f"x.{quote(c)}" for c in row_columns)
    order = ", ".join(f"x.{quote(c)}" for c in keys)
    # 有 weight 列时样例按权重从高到低列，排在前面的候选最影响手感；改权重的行取新旧两边较大的那个。
    rank = next((c for c in weights if c.lower() == "weight"), None)
    rows_order = f"x.{quote(rank)} DESC, {order}" if rank else order
    if rank:
        new, old = (f"IFNULL((SELECT MAX(y.{quote(rank)}) FROM temp.{side} y WHERE {on}), 0)" for side in ("added", "removed"))
        changed_order = f"MAX({new}, {old}) DESC, {order}"
    else:
        changed_order = order

    def only(name):
        where = f"NOT EXISTS (SELECT 1 FROM temp.changed y WHERE {on})"
        count = con.execute(f"SELECT COUNT(*) FROM temp.{name} x WHERE {where}").fetchone()[0]
        rows = con.execute(f"SELECT {selected} FROM temp.{name} x WHERE {where} ORDER BY {rows_order} LIMIT ?", (samples,))
        return count, [row_dict(row_columns, row) for row in rows]

    result["added"], result["added_samples"] = only("added")
    result["removed"], result["removed_samples"] = only("removed")
    structural = f"EXISTS (SELECT 1 FROM temp.structural y WHERE {on})"
    for name, condition in (("reweighted", "NOT " + structural), ("modified", structural)):
        result[name] = con.execute(f"SELECT COUNT(*) FROM temp.changed x WHERE {condition}").fetchone()[0]
        picked = list(con.execute(f"SELECT {key_list} FROM temp.changed x WHERE {condition} ORDER BY {changed_order} LIMIT ?", (samples,)))
        entries = []
        match = " AND ".join(f"{quote(c)} IS ?" for c in keys)
        for key in picked:
            entry = {"key": {c: render_value(v) for c, v in zip(keys, key)}}
            for side, source in (("base", "removed"), ("head", "added")):
                rows = con.execute(f"SELECT {', '.join(quote(c) for c in row_columns)} FROM temp.{source} WHERE {match}", key)
                entry[side] = [{c: v for c, v in row_dict(row_columns, row).items() if c not in keys} for row in rows]
            entries.append(entry)
        result[name + "_samples"] = entries
    result["reordered"], result["reordered_samples"] = compare_order(con, table, columns, samples)
    result["changed"] = bool(
        result["added"] or result["removed"] or result["reweighted"] or result["modified"] or result["reordered"]
        or result["columns_added"] or result["columns_removed"]
    )
    return result


def has_rowid(con, schema, table):
    sql = con.execute(f"SELECT sql FROM {schema}.sqlite_master WHERE type = 'table' AND name = ?", (table,)).fetchone()[0]
    return "WITHOUT ROWID" not in " ".join((sql or "").upper().split())


def longest_increasing(sequence):
    """严格递增的最长子序列：返回与输入等长的 bytearray，属于子序列的位置是 1（耐心排序，O(n log n)）。"""
    tails, previous = [], array("q", [-1]) * len(sequence)
    tail_values = []
    for index, value in enumerate(sequence):
        slot = bisect_left(tail_values, value)
        if slot:
            previous[index] = tails[slot - 1]
        if slot == len(tails):
            tails.append(index)
            tail_values.append(value)
        else:
            tails[slot] = index
            tail_values[slot] = value
    kept, index = bytearray(len(sequence)), tails[-1] if tails else -1
    while index >= 0:
        kept[index] = 1
        index = previous[index]
    return kept


def compare_order(con, table, columns, samples):
    """两边都有的行，相对次序变了几行。

    引擎在同权重的候选之间按 rowid（即插入顺序）排：五笔的查询写明了 `ORDER BY weight DESC, rowid ASC`，拼音按 key 的索引扫，同键的行也按 rowid 出来。所以内容相同、只是行的次序变了的库，候选顺序可能已经变了。这里把两边都有的行（重复的行按出现次序逐个配对）按旧库的 rowid 排好，取它们在新库里的 rowid，最长递增子序列以外的行就是挪了位置的行，个数是让新库次序与旧库一致最少要挪动的行数。只有一边有的行不参与。WITHOUT ROWID 的表按主键存，次序由内容决定，不比较。
    """
    if not (has_rowid(con, "main", table) and has_rowid(con, "head", table)):
        return 0, []
    listed = ", ".join(quote(c) for c in columns)
    for side, schema in (("order_base", "main"), ("order_head", "head")):
        con.execute(f"DROP TABLE IF EXISTS temp.{side}")
        con.execute(
            f"CREATE TEMP TABLE {side} AS SELECT {listed}, ROW_NUMBER() OVER (PARTITION BY {listed} ORDER BY rowid) AS {OCCURRENCE}, "
            f"rowid AS {ROWID} FROM {schema}.{quote(table)}"
        )
    con.execute(f"CREATE INDEX temp.order_head_rows ON order_head({listed}, {OCCURRENCE})")
    on = " AND ".join([f"x.{quote(c)} IS y.{quote(c)}" for c in columns] + [f"x.{OCCURRENCE} = y.{OCCURRENCE}"])
    pairs = con.execute(f"SELECT x.{ROWID}, y.{ROWID} FROM temp.order_base x JOIN temp.order_head y ON {on} ORDER BY x.{ROWID}")
    base_rowids, head_rowids = array("q"), array("q")
    for base_rowid, head_rowid in pairs:
        base_rowids.append(base_rowid)
        head_rowids.append(head_rowid)
    kept = longest_increasing(head_rowids)
    moved = kept.count(0)
    if not moved:
        return 0, []
    # 样例写「旧库第几行 → 新库第几行」，名次只数两边都有的行，新增和删除的行不占名次。
    head_order = array("q", sorted(head_rowids))
    entries, i = [], kept.find(0)
    while i >= 0 and len(entries) < samples:
        row = con.execute(f"SELECT {listed} FROM main.{quote(table)} WHERE rowid = ?", (base_rowids[i],)).fetchone()
        rank = bisect_left(head_order, head_rowids[i]) + 1
        entries.append({"row": {c: render_value(v) for c, v in zip(columns, row)}, "base_rank": i + 1, "head_rank": rank})
        i = kept.find(0, i + 1)
    return moved, entries


def one_side_table(con, schema, table, samples):
    columns = table_columns(con, schema, table)
    rows = con.execute(f"SELECT * FROM {schema}.{quote(table)} LIMIT ?", (samples,))
    return {
        "rows": con.execute(f"SELECT COUNT(*) FROM {schema}.{quote(table)}").fetchone()[0],
        "samples": [{c: render_value(v) for c, v in zip(columns, row)} for row in rows],
    }


def compare_sqlite(base, head, samples):
    report = {"kind": "sqlite", "bytes_identical": same_bytes(base, head)}
    if report["bytes_identical"]:
        report["changed"] = False
        return report
    con = sqlite3.connect(base.resolve().as_uri() + "?mode=ro", uri=True)
    try:
        con.execute("ATTACH DATABASE ? AS head", (head.resolve().as_uri() + "?mode=ro",))
        report["pragmas"] = {}
        for pragma in PRAGMAS:
            left = con.execute(f"PRAGMA main.{pragma}").fetchone()
            right = con.execute(f"PRAGMA head.{pragma}").fetchone()
            if left != right:
                report["pragmas"][pragma] = {"base": render_value(left and left[0]), "head": render_value(right and right[0])}

        def master(schema):
            return {
                (row[0], row[1]): normalize_schema_sql(row[2])
                for row in con.execute(f"SELECT type, name, sql FROM {schema}.sqlite_master")
            }

        left, right = master("main"), master("head")
        report["schema"] = {
            "added": sorted(f"{t} {n}" for t, n in right.keys() - left.keys()),
            "removed": sorted(f"{t} {n}" for t, n in left.keys() - right.keys()),
            "changed": sorted(f"{t} {n}" for (t, n) in left.keys() & right.keys() if left[(t, n)] != right[(t, n)]),
        }
        tables = {}
        for kind, name in sorted(left.keys() | right.keys(), key=lambda k: k[1]):
            if kind != "table":
                continue
            if (kind, name) not in right:
                tables[name] = {"status": "removed", **one_side_table(con, "main", name, samples)}
            elif (kind, name) not in left:
                tables[name] = {"status": "added", **one_side_table(con, "head", name, samples)}
            else:
                table = compare_table(con, name, table_columns(con, "main", name), table_columns(con, "head", name), samples)
                table["status"] = "changed" if table["changed"] else "same"
                tables[name] = table
        report["tables"] = tables
        report["changed"] = bool(
            report["pragmas"] or any(report["schema"].values()) or any(t["status"] != "same" for t in tables.values())
        )
    finally:
        con.close()
    return report


# ---- 其他文件 ----


def flatten(value, prefix=""):
    """展平成「路径 → 标量」；数组按下标展开（`features[3]`），新增或删掉一项只报那一项。"""
    if isinstance(value, dict) and value:
        out = {}
        for key, item in value.items():
            out.update(flatten(item, f"{prefix}.{key}" if prefix else str(key)))
        return out
    if isinstance(value, list) and value:
        out = {}
        for index, item in enumerate(value):
            out.update(flatten(item, f"{prefix}[{index}]"))
        return out
    return {prefix: value}


def compare_json(name, base, head, samples):
    left = flatten(json.loads(base.read_text(encoding="utf-8")))
    right = flatten(json.loads(head.read_text(encoding="utf-8")))
    ignored = IGNORED_JSON_PATHS.get(Path(name).name, ())

    def skip(path):
        return any(path == p or path.startswith(p + ".") for p in ignored)

    differing = sorted(p for p in left.keys() | right.keys() if left.get(p, KeyError) != right.get(p, KeyError))
    entries = [
        {"path": p, "base": render_value(json.dumps(left[p], ensure_ascii=False)) if p in left else None,
         "head": render_value(json.dumps(right[p], ensure_ascii=False)) if p in right else None}
        for p in differing
    ]
    counted = [e for e in entries if not skip(e["path"])]
    return {
        "kind": "json",
        "bytes_identical": same_bytes(base, head),
        "differences": len(counted),
        "ignored_differences": [e["path"] for e in entries if skip(e["path"])],
        "samples": counted[:samples],
        "changed": bool(counted),
    }


def compare_text(base, head):
    if same_bytes(base, head):
        return {"kind": "text", "bytes_identical": True, "changed": False}
    left = base.read_text(encoding="utf-8").splitlines()
    right = head.read_text(encoding="utf-8").splitlines()
    profile = base.name.removesuffix("-supplement.txt")
    if profile in SUPPLEMENT_PROFILES and base.name == f"{profile}-supplement.txt" and left and right:
        header = re.compile(r"^# .*，由 msime 仓库提交 (?P<commit>[0-9a-f]{40}) 的 crates/dict-builder/src/" + re.escape(profile) + r"_supplement\.rs ")
        matches = [header.match(lines[0]) for lines in (left, right)]
        if all(matches):
            for lines, match in zip((left, right), matches):
                lines[0] = lines[0][:match.start("commit")] + "<构建器提交>" + lines[0][match.end("commit"):]
            if left == right:
                return {"kind": "text", "bytes_identical": False, "ignored_differences": ["generator_commit"], "changed": False}
    lc, rc = Counter(left), Counter(right)
    added, removed = rc - lc, lc - rc
    # difflib 在最坏情况下是平方复杂度，大文件（五笔补充表这类几十万行的中间文件）只按多重集合列出增删的行。
    if max(len(left), len(right)) <= TEXT_DIFF_MAX_INPUT:
        diff = list(difflib.unified_diff(left, right, "base", "head", lineterm="", n=1))
    else:
        diff = [f"-{line}" for line in left if line in removed] + [f"+{line}" for line in right if line in added]
    return {
        "kind": "text",
        "bytes_identical": False,
        "base_lines": len(left),
        "head_lines": len(right),
        "lines_added": sum(added.values()),
        "lines_removed": sum(removed.values()),
        "diff": [line[:200] for line in diff[:TEXT_DIFF_LINES]],
        "diff_truncated": len(diff) > TEXT_DIFF_LINES,
        "changed": True,
    }


def compare_binary(base, head):
    report = {
        "kind": "binary",
        "base_size": base.stat().st_size,
        "head_size": head.stat().st_size,
        "base_sha256": sha256(base),
        "head_sha256": sha256(head),
    }
    report["bytes_identical"] = report["base_sha256"] == report["head_sha256"]
    report["changed"] = not report["bytes_identical"]
    if report["changed"] and report["base_size"] == report["head_size"]:
        differing = 0
        with base.open("rb") as left, head.open("rb") as right:
            for x in iter(lambda: left.read(CHUNK), b""):
                y = right.read(CHUNK)
                differing += sum(1 for a, b in zip(x, y) if a != b)
        report["bytes_differing"] = differing
    return report


def compare(base_dir, head_dir, samples):
    left, right = list_files(base_dir), list_files(head_dir)
    files = {}
    for name in sorted(left.keys() | right.keys()):
        if name not in right:
            files[name] = {"kind": "missing", "status": "removed", "changed": True, "base_size": left[name].stat().st_size}
            continue
        if name not in left:
            files[name] = {"kind": "missing", "status": "added", "changed": True, "head_size": right[name].stat().st_size}
            continue
        kind = kind_of(name, (left[name], right[name]))
        if kind == "sqlite":
            report = compare_sqlite(left[name], right[name], samples)
        elif kind == "json":
            report = compare_json(name, left[name], right[name], samples)
        elif kind == "text":
            report = compare_text(left[name], right[name])
        elif kind == "checksums":
            identical = same_bytes(left[name], right[name])
            report = {"kind": "checksums", "bytes_identical": identical, "changed": False}
        else:
            report = compare_binary(left[name], right[name])
        report["status"] = "changed" if report["changed"] else "same"
        files[name] = report
    return {"files": files, "changed": any(f["changed"] for f in files.values())}


# ---- Markdown ----


def code(value):
    """表格单元里的代码片段：数据里的 @ 和 # 进了代码片段，GitHub 就不会当成提及或引用。"""
    text = str(value).replace("\n", "⏎").replace("\r", "")
    fence = "`"
    while fence in text:
        fence += "`"
    text = text.replace("|", "\\|")
    padded = f" {text} " if text.startswith("`") or text.endswith("`") or fence != "`" else text
    return f"{fence}{padded}{fence}"


def absent(value):
    return "（无）" if value is None else code(value)


def describe(values):
    return ", ".join(f"{k}={v}" for k, v in values.items()) or "（无其他列）"


def sqlite_markdown(name, report, sample_tables):
    lines = [f"### `{name}`", ""]
    if report["bytes_identical"]:
        return lines + ["逐字节相同。", ""]
    tables = report["tables"]
    changed = {n: t for n, t in tables.items() if t["status"] != "same"}
    base_rows = sum(t.get("base_rows", t.get("rows", 0) if t["status"] == "removed" else 0) for t in tables.values())
    head_rows = sum(t.get("head_rows", t.get("rows", 0) if t["status"] == "added" else 0) for t in tables.values())
    lines.append(f"{len(tables)} 张表，{len(changed)} 张有变化；总行数 {base_rows} → {head_rows}。" + ("" if report["changed"] else "内容和行序都相同，只是文件字节不同。"))
    lines.append("")
    for pragma, values in report["pragmas"].items():
        lines.append(f"- 库头 `{pragma}`：{code(values['base'])} → {code(values['head'])}")
    for label, key in (("新增对象", "added"), ("删除对象", "removed"), ("定义改变", "changed")):
        if report["schema"][key]:
            lines.append(f"- {label}：" + "、".join(code(x) for x in report["schema"][key]))
    if report["pragmas"] or any(report["schema"].values()):
        lines.append("")
    if not changed:
        return lines
    lines += ["| 表 | 行数 | 新增 | 删除 | 改权重 | 改内容 | 改顺序 |", "| --- | --- | --- | --- | --- | --- | --- |"]
    for table_name, t in changed.items():
        if t["status"] in ("added", "removed"):
            rows = f"新表 {t['rows']}" if t["status"] == "added" else f"整表删除（{t['rows']}）"
            lines.append(f"| {code(table_name)} | {rows} | | | | | |")
        else:
            lines.append(f"| {code(table_name)} | {t['base_rows']} → {t['head_rows']} | {t['added']} | {t['removed']} | {t['reweighted']} | {t['modified']} | {t['reordered']} |")
    lines.append("")
    ranked = sorted(
        (n for n, t in changed.items() if t["status"] == "changed"),
        key=lambda n: -sum(changed[n][k] for k in ("added", "removed", "reweighted", "modified", "reordered")),
    )
    for table_name in ranked[:sample_tables]:
        t = changed[table_name]
        lines.append(f"<details><summary><code>{table_name}</code> 样例（键：{', '.join(t['key'])}）</summary>")
        lines.append("")
        if t["columns_added"] or t["columns_removed"]:
            lines.append(f"列：新增 {t['columns_added']}，删除 {t['columns_removed']}；行只按两边共有的列比较。")
            lines.append("")
        for label, key in (("新增", "added_samples"), ("删除", "removed_samples")):
            if t[key]:
                lines.append(f"{label}：")
                lines.append("")
                lines += [f"- {code(describe(row))}" for row in t[key]]
                lines.append("")
        for label, key in (("改权重", "reweighted_samples"), ("改内容", "modified_samples")):
            if t[key]:
                lines += [f"{label}：", "", "| 键 | 旧 | 新 |", "| --- | --- | --- |"]
                for entry in t[key]:
                    old = "；".join(describe(r) for r in entry["base"])
                    new = "；".join(describe(r) for r in entry["head"])
                    lines.append(f"| {code(describe(entry['key']))} | {code(old)} | {code(new)} |")
                lines.append("")
        if t["reordered_samples"]:
            lines += ["改顺序（两边都有的行里的名次，按旧库顺序列）：", "", "| 行 | 旧名次 | 新名次 |", "| --- | --- | --- |"]
            lines += [f"| {code(describe(e['row']))} | {e['base_rank']} | {e['head_rank']} |" for e in t["reordered_samples"]]
            lines.append("")
        lines += ["</details>", ""]
    if len(ranked) > sample_tables:
        lines += [f"另有 {len(ranked) - sample_tables} 张有变化的表没有列样例，见 JSON 报告。", ""]
    return lines


def file_markdown(name, report, sample_tables):
    kind = report["kind"]
    if kind == "missing":
        size = report.get("base_size", report.get("head_size"))
        return [f"### `{name}`", "", ("基准有、新构建没有" if report["status"] == "removed" else "新构建新增") + f"（{size} 字节）。", ""]
    if kind == "sqlite":
        return sqlite_markdown(name, report, sample_tables)
    lines = [f"### `{name}`", ""]
    if kind == "json":
        if report["ignored_differences"]:
            lines.append("不计入差异的字段：" + "、".join(code(p) for p in report["ignored_differences"]) + "。")
            lines.append("")
        if report["differences"]:
            lines += [f"{report['differences']} 个字段不同：", "", "| 字段 | 旧 | 新 |", "| --- | --- | --- |"]
            lines += [f"| {code(e['path'])} | {absent(e['base'])} | {absent(e['head'])} |" for e in report["samples"]]
            lines.append("")
        elif not report["ignored_differences"]:
            lines += ["相同。", ""]
        return lines
    if kind == "text":
        lines.append(f"行数 {report['base_lines']} → {report['head_lines']}，新增 {report['lines_added']} 行，删除 {report['lines_removed']} 行。")
        lines += ["", "~~~~diff", *report["diff"], "~~~~", ""]
        if report["diff_truncated"]:
            lines += ["（diff 只列开头一段。）", ""]
        return lines
    size = f"{report['base_size']} → {report['head_size']} 字节"
    detail = f"，{report['bytes_differing']} 个字节不同" if "bytes_differing" in report else ""
    return lines + [f"二进制内容不同：{size}{detail}，SHA-256 `{report['base_sha256'][:12]}` → `{report['head_sha256'][:12]}`。", ""]


def markdown(result, preamble, sample_tables, limit, expect_no_change):
    files = result["files"]
    changed = [n for n, f in files.items() if f["changed"]]
    lines = ["## 词库产物差异", ""]
    if preamble:
        lines += [preamble.rstrip(), ""]
    if expect_no_change:
        lines += ["本次声明产物不应变化（`expect-no-artifact-change`）：" + ("**有差异，检查失败。**" if result["changed"] else "没有差异。"), ""]
    lines += ["| 文件 | 类型 | 结果 |", "| --- | --- | --- |"]
    labels = {"sqlite": "SQLite", "json": "JSON", "text": "文本", "binary": "二进制", "checksums": "校验和", "missing": "—"}
    for name, f in files.items():
        if f["kind"] == "missing":
            status = "新增" if f["status"] == "added" else "缺失"
        elif f["changed"]:
            status = "**有差异**"
        elif f["kind"] == "checksums":
            status = "相同" if f["bytes_identical"] else "不同（由其他产物推出，不计入）"
        elif f.get("bytes_identical", True):
            status = "相同"
        else:
            status = "内容和行序相同（字节不同）" if f["kind"] == "sqlite" else "相同（只有不计入的字段不同）"
        lines.append(f"| `{name}` | {labels[f['kind']]} | {status} |")
    lines.append("")
    for name in changed:
        lines += file_markdown(name, files[name], sample_tables)
    for name, f in files.items():
        if f["kind"] == "sqlite" and not f["changed"] and not f["bytes_identical"]:
            lines += file_markdown(name, f, sample_tables)
    text = "\n".join(lines) + "\n"
    if len(text) > limit:
        cut = text.rfind("\n", 0, max(0, limit - 200))
        text = text[: max(cut, 0)] + "\n\n（报告超出长度上限，后面的内容截掉了，完整内容见 workflow artifact 里的 JSON 报告。）\n"
    return text


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--base", type=Path, required=True, help="基准构建器的产物目录")
    parser.add_argument("--head", type=Path, required=True, help="新构建器的产物目录")
    parser.add_argument("--markdown", type=Path, help="写出 Markdown 报告")
    parser.add_argument("--json", type=Path, help="写出 JSON 报告")
    parser.add_argument("--preamble", type=Path, help="插在 Markdown 报告标题后的一段文字")
    parser.add_argument("--expect-no-change", action="store_true", help="任何内容差异都以 1 退出")
    parser.add_argument("--samples", type=int, default=5, help="Markdown 里每张表每类差异列几行样例")
    parser.add_argument("--json-samples", type=int, default=100, help="JSON 里每张表每类差异列几行样例")
    parser.add_argument("--sample-tables", type=int, default=8, help="每个库最多给几张表列样例")
    parser.add_argument("--max-markdown-chars", type=int, default=60000, help="Markdown 报告的长度上限（PR 评论上限是 65536 个字符）")
    args = parser.parse_args(argv)
    preamble = args.preamble.read_text(encoding="utf-8") if args.preamble else ""

    def cannot_compare(reason):
        # 无法比较时也写一份报告，作业摘要和 PR 评论说清楚原因，不会留着上一次的结果。
        print(f"cannot compare {args.base} and {args.head}: {reason}", file=sys.stderr)
        if args.markdown:
            args.markdown.parent.mkdir(parents=True, exist_ok=True)
            parts = ("## 词库产物差异", preamble.rstrip(), f"**没能比较**：{code(reason)}")
            args.markdown.write_text("\n\n".join(part for part in parts if part) + "\n", encoding="utf-8")
        return 2

    for directory in (args.base, args.head):
        if not directory.is_dir():
            return cannot_compare(f"not a directory: {directory}")
    try:
        full = compare(args.base, args.head, max(args.samples, args.json_samples))
    except (sqlite3.Error, OSError, ValueError) as error:
        return cannot_compare(error)
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(full, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    if args.markdown:
        short = trim_samples(full, args.samples)
        args.markdown.parent.mkdir(parents=True, exist_ok=True)
        args.markdown.write_text(
            markdown(short, preamble, args.sample_tables, args.max_markdown_chars, args.expect_no_change), encoding="utf-8"
        )
    changed = [n for n, f in full["files"].items() if f["changed"]]
    print(f"{len(full['files'])} files compared, {len(changed)} with differences" + (": " + ", ".join(changed) if changed else ""))
    return 1 if args.expect_no_change and full["changed"] else 0


def trim_samples(result, samples):
    """Markdown 用的副本：每类样例只留前 `samples` 行。"""
    trimmed = json.loads(json.dumps(result))
    for f in trimmed["files"].values():
        if "samples" in f:
            f["samples"] = f["samples"][:samples]
        for t in f.get("tables", {}).values():
            for key in [k for k in t if k.endswith("samples")]:
                t[key] = t[key][:samples]
    return trimmed


if __name__ == "__main__":
    sys.exit(main())
