#!/usr/bin/env python3
"""scripts/dict_artifact_diff.py 的比较规则：键怎么定、差异怎么分类、哪些差异不算、什么时候以 1 退出；以及 .github/workflows/dict-builder-artifact-diff.yml 的触发路径覆盖构建器用到的引擎文件。

夹具是测试里现建的两个小 SQLite 库和几份文本、二进制、JSON 文件，数据全是合成的，不读真实词库；路径检查只读仓库里的源码和 workflow 文本。作为契约检查在任何机器上都能跑。
"""

from contextlib import redirect_stderr, redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/dict-builder-artifact-diff.yml"
ENGINE = ROOT / "crates/engine/src"
BUILDER = ROOT / "crates/dict-builder/src"
# 只在发布构建以外的子命令里用到引擎的文件：它们用到的引擎接口不影响产物，不必触发这个 workflow。
SUBCOMMAND_ONLY = {"rime_ice_supplement.rs": "rime-ice-supplement 子命令，生成补充表供上游更新预览，不在发布构建里跑"}

SPEC = importlib.util.spec_from_file_location("dict_artifact_diff", Path(__file__).with_name("dict_artifact_diff.py"))
DIFF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIFF)

# 与拼音主库同形：没有主键和 UNIQUE，键是 key、jp、value，weight 是权重。
PINYIN_SCHEMA = 'CREATE TABLE tbl_2_a ("key" text, "jp" text, "value" text, "weight" integer default 0)'
# 与五笔库同形：UNIQUE(key, value)。
WUBI_SCHEMA = 'CREATE TABLE wubi86 ("key" TEXT NOT NULL, "value" TEXT NOT NULL, "weight" INTEGER NOT NULL DEFAULT 0, UNIQUE("key", "value"))'
# 与表情库同形：主键 emoji，sort_order 是权重类列，其余是内容列。
EMOJI_SCHEMA = "CREATE TABLE emoji (emoji TEXT NOT NULL, category TEXT NOT NULL, sort_order INTEGER NOT NULL, PRIMARY KEY (emoji)) WITHOUT ROWID"
METADATA_SCHEMA = "CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID"

BASE_PINYIN = [
    ("a'a", "aa", "啊啊", 500),
    ("a'ba", "ab", "阿爸", 300),
    ("a'bai", "ab", "阿白", 10),
    ("a'bo", "ab", "阿波", 20),
    ("a'bo", "ab", "阿波", 20),
]
BASE_WUBI = [("aa", "式", 20), ("aa", "戒", 10), ("bb", "子", 5)]
BASE_EMOJI = [("😀", "smileys", 1), ("🐱", "animals", 2)]


def write_db(path, pinyin=BASE_PINYIN, wubi=BASE_WUBI, emoji=BASE_EMOJI, extra=(), user_version=0):
    path.parent.mkdir(parents=True, exist_ok=True)
    con = sqlite3.connect(path)
    with con:
        con.execute(PINYIN_SCHEMA)
        con.execute(WUBI_SCHEMA)
        con.execute(EMOJI_SCHEMA)
        con.execute(METADATA_SCHEMA)
        con.executemany("INSERT INTO tbl_2_a VALUES (?, ?, ?, ?)", pinyin)
        con.executemany("INSERT INTO wubi86 VALUES (?, ?, ?)", wubi)
        con.executemany("INSERT INTO emoji VALUES (?, ?, ?)", emoji)
        con.execute("INSERT INTO metadata VALUES ('format_version', '1')")
        for statement in extra:
            con.execute(statement)
        con.execute(f"PRAGMA user_version = {user_version}")
    con.close()


MANIFEST = {
    "source": {"repository": "metasequoiaime/msime", "commit": "1" * 40, "dirty": False},
    "format_contract_commit": "1" * 40,
    "custom_dictionary_commit": "2" * 40,
    "features": ["pinyin", "wubi"],
    "files": {"msime-pinyin.db": {"sha256": "3" * 64, "size": 1}},
}


def write_tree(root, **db):
    write_db(root / "msime-pinyin.db", **db)
    (root / "msime-dictionary-manifest.json").write_text(json.dumps(MANIFEST, indent=1), encoding="utf-8")
    (root / "msime-SHA256SUMS.txt").write_text("0" * 64 + "  msime-pinyin.db\n", encoding="utf-8")
    (root / "msime-scowl_Copyright.txt").write_text("line one\nline two\n", encoding="utf-8")
    (root / "msime-bigram.bin").write_bytes(bytes(range(256)) * 4)


class DiffTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.base = self.root / "base"
        self.head = self.root / "head"
        write_tree(self.base)
        write_tree(self.head)

    def tearDown(self):
        shutil.rmtree(self.root)

    def run_diff(self, *extra):
        markdown = self.root / "report.md"
        report = self.root / "report.json"
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            status = DIFF.main(["--base", str(self.base), "--head", str(self.head), "--markdown", str(markdown), "--json", str(report), *extra])
        if status == 2:
            return status, None, None
        return status, json.loads(report.read_text(encoding="utf-8")), markdown.read_text(encoding="utf-8")

    def rewrite_head_db(self, **db):
        (self.head / "msime-pinyin.db").unlink()
        write_db(self.head / "msime-pinyin.db", **db)

    def table(self, report, name):
        return report["files"]["msime-pinyin.db"]["tables"][name]

    def test_identical_trees_report_no_change(self):
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 0)
        self.assertFalse(report["changed"])
        # 两次独立建库的字节可能相同也可能不同，两种都不能判为有差异。
        self.assertFalse(report["files"]["msime-pinyin.db"]["changed"])
        self.assertIn("没有差异", markdown)

    def test_pinyin_rows_are_classified_by_key_without_constraints(self):
        head = [
            ("a'a", "aa", "啊啊", 50),  # 改权重
            ("a'ba", "ab", "阿爸", 300),  # 不变
            ("a'bo", "ab", "阿波", 20),  # 重复次数 2 → 1：改内容
            ("a'bu", "ab", "阿布", 7),  # 新增
        ]  # 阿白 删除
        self.rewrite_head_db(pinyin=head)
        status, report, markdown = self.run_diff()
        self.assertEqual(status, 0, "informational by default")
        table = self.table(report, "tbl_2_a")
        self.assertEqual(table["key"], ["key", "jp", "value"])
        self.assertEqual((table["base_rows"], table["head_rows"]), (5, 4))
        self.assertEqual((table["added"], table["removed"], table["reweighted"], table["modified"]), (1, 1, 1, 1))
        self.assertEqual(table["added_samples"], [{"key": "a'bu", "jp": "ab", "value": "阿布", "weight": "7"}])
        self.assertEqual(table["removed_samples"][0]["value"], "阿白")
        self.assertEqual(table["reweighted_samples"][0]["base"], [{"weight": "500"}])
        self.assertEqual(table["reweighted_samples"][0]["head"], [{"weight": "50"}])
        self.assertEqual(table["modified_samples"][0]["base"], [{"weight": "20", "×": "2"}])
        self.assertIn("| `tbl_2_a` | 5 → 4 | 1 | 1 | 1 | 1 |", markdown)
        self.assertIn("`weight=500` | `weight=50`", markdown)
        self.assertEqual(self.run_diff("--expect-no-change")[0], 1)

    def test_unique_and_primary_keys_are_used(self):
        self.rewrite_head_db(
            wubi=[("aa", "式", 21), ("aa", "戒", 10), ("bb", "孑", 5)],
            emoji=[("😀", "faces", 1), ("🐱", "animals", 3)],
        )
        _, report, _ = self.run_diff()
        wubi = self.table(report, "wubi86")
        self.assertEqual(wubi["key"], ["key", "value"])
        self.assertEqual((wubi["added"], wubi["removed"], wubi["reweighted"], wubi["modified"]), (1, 1, 1, 0))
        emoji = self.table(report, "emoji")
        self.assertEqual(emoji["key"], ["emoji"])
        # sort_order 是权重类列；category 变了是改内容。
        self.assertEqual((emoji["added"], emoji["removed"], emoji["reweighted"], emoji["modified"]), (0, 0, 1, 1))
        self.assertEqual(emoji["modified_samples"][0]["base"], [{"category": "smileys", "sort_order": "1"}])

    def test_samples_list_heaviest_rows_first(self):
        base = [(f"k{i}", "k", f"词{i}", i) for i in range(20)]
        write_db(self.base / "msime-pinyin.db.tmp", pinyin=base)
        (self.base / "msime-pinyin.db.tmp").replace(self.base / "msime-pinyin.db")
        self.rewrite_head_db(pinyin=[])
        _, report, _ = self.run_diff("--json-samples", "3", "--samples", "3")
        removed = self.table(report, "tbl_2_a")["removed_samples"]
        self.assertEqual([row["weight"] for row in removed], ["19", "18", "17"])

    def test_schema_tables_and_header_changes(self):
        self.rewrite_head_db(
            extra=(
                "CREATE TABLE quick_parases (key TEXT NOT NULL, value TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 0, UNIQUE(key, value))",
                "INSERT INTO quick_parases VALUES ('dz', '地址', 1)",
                "CREATE INDEX idx_jp_2_a ON tbl_2_a(jp)",
                "ALTER TABLE emoji ADD COLUMN pinyin TEXT",
                "DROP TABLE metadata",
            ),
            user_version=3,
        )
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 1)
        db = report["files"]["msime-pinyin.db"]
        self.assertEqual(db["pragmas"], {"user_version": {"base": "0", "head": "3"}})
        self.assertIn("table quick_parases", db["schema"]["added"])
        self.assertIn("index idx_jp_2_a", db["schema"]["added"])
        self.assertIn("table metadata", db["schema"]["removed"])
        self.assertIn("table emoji", db["schema"]["changed"])
        self.assertEqual(db["tables"]["quick_parases"]["status"], "added")
        self.assertEqual(db["tables"]["metadata"]["status"], "removed")
        self.assertEqual(db["tables"]["emoji"]["columns_added"], ["pinyin"])
        self.assertIn("新表 1", markdown)
        self.assertIn("整表删除（1）", markdown)

    def test_schema_preserves_spaces_inside_quoted_values(self):
        for before, after in (("'two  words'", "'two words'"), ("'can''t  stop'", "'can''t stop'")):
            with self.subTest(before=before):
                for root, default in ((self.base, before), (self.head, after)):
                    with sqlite3.connect(root / "msime-pinyin.db") as con:
                        con.execute("DROP TABLE IF EXISTS review_terms")
                        con.execute(f"CREATE TABLE review_terms (value TEXT DEFAULT {default})")
                status, report, _ = self.run_diff("--expect-no-change")
                self.assertEqual(status, 1, "不同的默认字符串不能报告为相同定义")
                self.assertIn("table review_terms", report["files"]["msime-pinyin.db"]["schema"]["changed"])

    def test_schema_ignores_formatting_outside_quoted_values(self):
        for root, whitespace in ((self.base, " "), (self.head, "\n  ")):
            with sqlite3.connect(root / "msime-pinyin.db") as con:
                con.execute(f"CREATE TABLE review_terms (value{whitespace}TEXT DEFAULT 'two  words')")
        status, report, _ = self.run_diff("--expect-no-change")
        self.assertEqual(status, 0)
        self.assertFalse(report["changed"])

    def test_same_content_and_order_with_different_bytes_is_not_a_change(self):
        # 建一张表再删掉：多出空闲页，文件字节不同，行的内容和次序都不变。
        self.rewrite_head_db(extra=("CREATE TABLE scratch (x TEXT)", "INSERT INTO scratch VALUES ('x')", "DROP TABLE scratch"))
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 0)
        db = report["files"]["msime-pinyin.db"]
        self.assertFalse(db["bytes_identical"])
        self.assertFalse(db["changed"])
        self.assertIn("内容和行序相同（字节不同）", markdown)

    def test_reordered_rows_are_a_change(self):
        # 同码同权重的两个字只换了插入顺序：引擎按 weight DESC, rowid ASC 排，候选顺序跟着变了。
        base, head = [("aaaa", "工", 5), ("aaaa", "式", 5)], [("aaaa", "式", 5), ("aaaa", "工", 5)]
        (self.base / "msime-pinyin.db").unlink()
        write_db(self.base / "msime-pinyin.db", wubi=base)
        self.rewrite_head_db(wubi=head)
        query = 'SELECT value FROM wubi86 WHERE "key" = ? ORDER BY weight DESC, rowid ASC'
        candidates = []
        for root in (self.base, self.head):
            con = sqlite3.connect(root / "msime-pinyin.db")
            candidates.append([row[0] for row in con.execute(query, ("aaaa",))])
            con.close()
        self.assertEqual(candidates, [["工", "式"], ["式", "工"]], "the fixture must change what the engine shows")
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 1)
        wubi = self.table(report, "wubi86")
        self.assertEqual((wubi["added"], wubi["removed"], wubi["reweighted"], wubi["modified"], wubi["reordered"]), (0, 0, 0, 0, 1))
        self.assertEqual(len(wubi["reordered_samples"]), 1)
        self.assertIn("| `wubi86` | 2 → 2 | 0 | 0 | 0 | 0 | 1 |", markdown)
        # 拼音表没有约束、允许重复行：整表倒序，最少挪动 3 行：两行相同的 阿波 按出现次序配对，彼此的先后不变，挪的是另外三行。
        self.rewrite_head_db(pinyin=list(reversed(BASE_PINYIN)), wubi=base)
        _, report, _ = self.run_diff()
        self.assertEqual(self.table(report, "tbl_2_a")["reordered"], 3)
        self.assertEqual(self.table(report, "wubi86")["reordered"], 0)
        # WITHOUT ROWID 的表按主键存，插入顺序不影响次序。
        self.rewrite_head_db(emoji=list(reversed(BASE_EMOJI)))
        _, report, _ = self.run_diff()
        self.assertEqual(self.table(report, "emoji")["reordered"], 0)

    def test_order_ignores_rows_only_one_side_has(self):
        # 中间插进新行、删掉旧行，其余行的相对次序不变，不算改顺序。
        head = [BASE_PINYIN[0], ("a'b", "ab", "新", 1), BASE_PINYIN[1], BASE_PINYIN[3], BASE_PINYIN[4]]
        self.rewrite_head_db(pinyin=head)
        _, report, _ = self.run_diff()
        table = self.table(report, "tbl_2_a")
        self.assertEqual((table["added"], table["removed"], table["reordered"]), (1, 1, 0))

    def test_longest_increasing_subsequence(self):
        self.assertEqual(DIFF.longest_increasing([]), bytearray())
        self.assertEqual(DIFF.longest_increasing([1, 2, 3]), bytearray([1, 1, 1]))
        self.assertEqual(DIFF.longest_increasing([5, 1, 2, 9, 3, 4]), bytearray([0, 1, 1, 0, 1, 1]))

    def test_manifest_ignores_builder_provenance_and_file_hashes(self):
        manifest = json.loads(json.dumps(MANIFEST))
        manifest["source"]["commit"] = "9" * 40
        manifest["source"]["dirty"] = True
        manifest["format_contract_commit"] = "9" * 40
        manifest["files"]["msime-pinyin.db"]["sha256"] = "8" * 64
        (self.head / "msime-dictionary-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        (self.head / "msime-SHA256SUMS.txt").write_text("8" * 64 + "  msime-pinyin.db\n", encoding="utf-8")
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 0)
        entry = report["files"]["msime-dictionary-manifest.json"]
        self.assertFalse(entry["changed"])
        self.assertEqual(entry["ignored_differences"], ["files.msime-pinyin.db.sha256", "format_contract_commit", "source.commit", "source.dirty"])
        self.assertFalse(report["files"]["msime-SHA256SUMS.txt"]["changed"])
        self.assertIn("不同（由其他产物推出，不计入）", markdown)

        manifest["features"].append("english")
        (self.head / "msime-dictionary-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 1)
        self.assertEqual(report["files"]["msime-dictionary-manifest.json"]["samples"], [{"path": "features[2]", "base": None, "head": '"english"'}])
        self.assertIn("| `features[2]` | （无） | `\"english\"` |", markdown)

    def test_supplement_headers_ignore_only_the_builder_commit(self):
        for profile, label in (("wubi86", "86 五笔词组补充表"), ("wubi98", "98 五笔词组补充表"), ("wubixsj", "新世纪五笔词组表")):
            name = f"{profile}-supplement.txt"
            for root, commit in ((self.base, "1" * 40), (self.head, "2" * 40)):
                header = f"# {label}，由 msime 仓库提交 {commit} 的 crates/dict-builder/src/{profile}_supplement.rs 以固定命令生成。\n"
                (root / name).write_text(header + "合成词\taa\t5\n", encoding="utf-8")
        status, report, _ = self.run_diff("--expect-no-change")
        self.assertEqual(status, 0, "构建器提交号必然不同，不能让严格模式总是失败")
        self.assertFalse(report["changed"])
        path = self.head / "wubi86-supplement.txt"
        original = path.read_text(encoding="utf-8")
        for changed in (original.replace("\taa\t5", "\taa\t6"), original.replace("固定命令", "另一命令")):
            path.write_text(changed, encoding="utf-8")
            status, report, _ = self.run_diff("--expect-no-change")
            self.assertEqual(status, 1)
            self.assertTrue(report["files"][path.name]["changed"])
        path.write_text(original, encoding="utf-8")
        for root in (self.base, self.head):
            shutil.copy(root / path.name, root / "unrelated.txt")
        self.assertEqual(self.run_diff("--expect-no-change")[0], 1, "其他文本里的提交号不适用这条例外")

    def test_text_binary_and_missing_files(self):
        (self.head / "msime-scowl_Copyright.txt").write_text("line one\nline 2\nline three\n", encoding="utf-8")
        data = bytearray((self.base / "msime-bigram.bin").read_bytes())
        data[3] ^= 0xFF
        data[700] ^= 0x01
        (self.head / "msime-bigram.bin").write_bytes(bytes(data))
        (self.head / "msime-trigram.bin").write_bytes(b"\x00\x01")
        (self.base / "msime-japanese.dat").write_bytes(b"\x00MSJPDT1")
        status, report, markdown = self.run_diff("--expect-no-change")
        self.assertEqual(status, 1)
        files = report["files"]
        text = files["msime-scowl_Copyright.txt"]
        self.assertEqual((text["base_lines"], text["head_lines"], text["lines_added"], text["lines_removed"]), (2, 3, 2, 1))
        self.assertIn("+line 2", text["diff"])
        self.assertEqual(files["msime-bigram.bin"]["bytes_differing"], 2)
        self.assertEqual((files["msime-trigram.bin"]["status"], files["msime-japanese.dat"]["status"]), ("added", "removed"))
        self.assertIn("基准有、新构建没有", markdown)
        self.assertIn("2 个字节不同", markdown)

    def test_large_text_files_list_changed_lines_without_difflib(self):
        lines = [f"词{i}\tci{i}\t{i}" for i in range(DIFF.TEXT_DIFF_MAX_INPUT + 5)]
        (self.base / "wubi86-supplement.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
        lines[7] = "新词\txinci\t1"
        (self.head / "wubi86-supplement.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
        _, report, _ = self.run_diff()
        text = report["files"]["wubi86-supplement.txt"]
        self.assertEqual((text["lines_added"], text["lines_removed"]), (1, 1))
        self.assertEqual(text["diff"], ["-词7\tci7\t7", "+新词\txinci\t1"])

    def test_markdown_keeps_data_inside_code_spans_and_within_limit(self):
        noisy = [(f"x'{i}", "x", f"@someone | #1 `tick` {i}", i) for i in range(40)]
        self.rewrite_head_db(pinyin=BASE_PINYIN + noisy)
        _, _, markdown = self.run_diff("--samples", "40")
        line = next(l for l in markdown.splitlines() if "@someone" in l)
        self.assertTrue(line.startswith("- ``"), line)
        self.assertIn("\\|", line)
        _, _, short = self.run_diff("--samples", "40", "--max-markdown-chars", "1500")
        self.assertLessEqual(len(short), 1500)
        self.assertIn("报告超出长度上限", short)

    def test_unreadable_input_exits_2_with_a_report(self):
        markdown = self.root / "report.md"
        preamble = self.root / "preamble.md"
        preamble.write_text("- 新构建器：abc\n", encoding="utf-8")
        shutil.rmtree(self.head)
        self.assertEqual(self.run_diff("--preamble", str(preamble))[0], 2)
        self.assertIn("**没能比较**", markdown.read_text(encoding="utf-8"))
        self.head.mkdir()
        (self.head / "msime-pinyin.db").write_bytes(b"SQLite format 3\x00" + b"\xff" * 200)
        shutil.copy(self.base / "msime-dictionary-manifest.json", self.head)
        markdown.unlink()
        self.assertEqual(self.run_diff("--preamble", str(preamble))[0], 2)
        report = markdown.read_text(encoding="utf-8")
        self.assertTrue(report.startswith("## 词库产物差异\n\n- 新构建器：abc\n\n**没能比较**："), report)


def trigger_paths(event):
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    index = lines.index("    paths:", lines.index(f"  {event}:")) + 1
    paths = []
    while lines[index].startswith("      - "):
        paths.append(lines[index][len("      - "):])
        index += 1
    return paths


def refs_inputs():
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    start = lines.index("          inputs=(") + 1
    return [line.strip() for line in lines[start : lines.index("          )", start)]]


def covered(path, patterns):
    return any(path == p or (p.endswith("/**") and path.startswith(p[: -len("**")])) for p in patterns)


def module_file(segments):
    """engine 里与模块路径对应的最深的源文件（`zhuyin::layout::Kind` → zhuyin/layout.rs），第一段就不是模块时返回 None。"""
    found = None
    for depth in range(1, len(segments) + 1):
        base = ENGINE.joinpath(*segments[:depth])
        candidate = next((c for c in (base.with_suffix(".rs"), base / "mod.rs") if c.is_file()), None)
        if candidate is None:
            break
        found = candidate
    return found


def reexports():
    """lib.rs 里 `pub use` 导出到 crate 根的名字 → 定义它的模块路径。"""
    text = (ENGINE / "lib.rs").read_text(encoding="utf-8")
    names = {}
    for statement in text.split(";"):
        statement = " ".join(statement.split())
        start = statement.find("pub use ")
        if start < 0:
            continue
        statement = statement[start + len("pub use ") :]
        if "::{" in statement:
            prefix, items = statement.split("::{", 1)
            items = [item.strip() for item in items.rstrip("}").split(",") if item.strip()]
        else:
            prefix, items = "", [statement]
        for item in items:
            path, _, alias = item.partition(" as ")
            name = alias.strip() or path.split("::")[-1]
            names[name] = [p for p in prefix.split("::") if p] + path.split("::")[:-1]
    return names


def builder_engine_references():
    """crates/dict-builder/src 里每处 `msime_engine::…`（use 语句和代码里的全路径）：(文件名, 写法, 模块路径)。"""
    roots = reexports()
    found = []
    for source in sorted(BUILDER.rglob("*.rs")):
        if source.name in SUBCOMMAND_ONLY:
            continue
        text = source.read_text(encoding="utf-8")
        start = text.find("msime_engine::")
        while start >= 0:
            index = start + len("msime_engine::")
            segments, word = [], ""
            while index < len(text) and (text[index].isalnum() or text[index] in "_:"):
                if text[index] == ":":
                    if word:
                        segments.append(word)
                    word = ""
                else:
                    word += text[index]
                index += 1
            if word:
                segments.append(word)
            if segments:
                names = [segments]
            else:
                # `msime_engine::{a, b}`：花括号里每个名字各自是 crate 根上的导出。
                group = text[index + 1 : text.index("}", index)]
                names = [[item.strip().split("::")[0]] for item in group.split(",") if item.strip()]
            for path in names:
                modules = path if module_file(path) else roots.get(path[0], path)
                found.append((source.name, "::".join(path), modules))
            start = text.find("msime_engine::", index)
    return found


class WorkflowTriggerTest(unittest.TestCase):
    def test_pull_request_push_and_lock_gate_watch_the_same_paths(self):
        pull_request = trigger_paths("pull_request")
        self.assertEqual(pull_request, trigger_paths("push"))
        self.assertEqual([p.removesuffix("/**") for p in pull_request], refs_inputs())

    def test_every_watched_path_exists(self):
        for pattern in trigger_paths("pull_request"):
            path = ROOT / pattern.removesuffix("/**")
            self.assertTrue(path.is_dir() if pattern.endswith("/**") else path.is_file(), f"{pattern} does not exist; a renamed input silently stops triggering the artifact diff")

    def test_every_engine_file_the_builder_uses_triggers_the_diff(self):
        patterns = trigger_paths("pull_request")
        references = builder_engine_references()
        self.assertTrue(references, "no msime_engine reference found in crates/dict-builder/src")
        for name, written, modules in references:
            source = module_file(modules)
            self.assertIsNotNone(source, f"{name}: cannot resolve msime_engine::{written} to a file under crates/engine/src")
            relative = source.relative_to(ROOT).as_posix()
            self.assertTrue(covered(relative, patterns), f"{name} uses msime_engine::{written} from {relative}, which is not in the paths of {WORKFLOW.name}")

    def test_reference_resolution(self):
        self.assertEqual(module_file(["zhuyin", "layout", "Kind"]), ENGINE / "zhuyin/layout.rs")
        self.assertEqual(reexports()["wubi_phrase_code"], ["wubi", "code"])
        self.assertTrue(covered("crates/dict-builder/src/main.rs", ["crates/dict-builder/**"]))
        self.assertFalse(covered("crates/dict-builder-extra/x.rs", ["crates/dict-builder/**"]))


if __name__ == "__main__":
    unittest.main()
