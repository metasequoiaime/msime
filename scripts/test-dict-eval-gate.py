#!/usr/bin/env python3
"""检查 scripts/dict_eval_gate.py：五道硬门槛各自的判定、全拼表名和行数下限与 Rust 源码一致、评测集都归了类，以及用假的 convert_eval 端到端跑一遍（通过、门槛失败、跑不完三种退出码，参考项只报告不影响退出码，库损坏算跑不完）。不需要词库或评测二进制。"""
import hashlib
import importlib.util
import json
import os
import sqlite3
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
spec = importlib.util.spec_from_file_location("dict_eval_gate", ROOT / "scripts/dict_eval_gate.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)

# ---- 每一份评测集都归了类：新加的集合要先决定它算不算金标准 ----
listed = set(gate.GOLD_CASE_SETS) | set(gate.GOLD_TYPING_SETS) | set(gate.NOT_GOLD_SETS)
present = {path.name for path in (ROOT / "resources/eval").glob("*.tsv")}
assert present == listed, f"classify these eval sets in dict_eval_gate.py: unlisted {sorted(present - listed)}, missing {sorted(listed - present)}"

# ---- 全拼表名与 format.rs、行数下限与 product.rs ----
pattern = gate.quanpin_table_pattern()
initials = "abcdefghjklmnopqrstwxyz"
names = [f"tbl_{bucket}_{initial}" for bucket in [*map(str, range(1, 8)), "others"] for initial in initials]
assert len(names) == 184 and all(pattern.match(name) for name in names)
for name in ("tbl_8_a", "tbl_2_i", "tbl_2_u", "tbl_2_v", "tbl_0_a", "quick_parases", "sentence_excluded", "tbl_2_ab", "xtbl_2_a"):
    assert not pattern.match(name), name
assert gate.licensed_row_floor() == 800_000
with tempfile.TemporaryDirectory() as scratch:
    broken = Path(scratch) / "product.rs"
    broken.write_text("let minimum = 5;\n", encoding="utf-8")
    try:
        gate.licensed_row_floor(broken)
        raise AssertionError("a product.rs without the floor must be an error")
    except gate.GateError:
        pass

# ---- 金标准对：三种来源都取到，九键的码不当读音 ----
pairs = gate.gold_pairs()
assert ("车检", "chejian") in pairs, "must-outrank full rows give (higher word, input)"
assert ("阿爸", "aba") in pairs, "word sets give (gold, input)"
assert ("公司", "gongsi") in pairs, "typing sets give the tokens' word=reading"
assert not any(reading.isdigit() for _, reading in pairs), "nine-key digits are not readings"
assert all(not reading.count("'") for _, reading in pairs), "readings are canonical"
assert ("战略", "zhanlve") in pairs or all(not reading.endswith("lue") for _, reading in pairs), "lue is canonicalised to lve"


def make_pinyin_db(path, rows, offensive=None):
    connection = sqlite3.connect(path)
    tables = {}
    for key, value, weight in rows:
        table = f"tbl_{key.count(chr(39)) + 1}_{key[0]}"
        if table not in tables:
            connection.execute(f'create table {table} ("key" text, "jp" text, "value" text, "weight" integer default 0)')
            tables[table] = True
        connection.execute(f"insert into {table} values (?, '', ?, ?)", (key, value, weight))
    connection.execute("create table quick_parases (key text, value text, weight integer)")
    connection.execute("insert into quick_parases values ('chejian', '车检', 1)")
    if offensive is not None:
        connection.execute("CREATE TABLE sentence_excluded(value TEXT NOT NULL PRIMARY KEY, exempt INTEGER NOT NULL CHECK (exempt IN (0, 1))) WITHOUT ROWID")
        connection.executemany("insert into sentence_excluded values (?, ?)", offensive)
    connection.commit()
    connection.close()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def make_artifact(directory, rows, offensive=None, unlicensed=False, builder="a" * 40, dictionary="b" * 40):
    directory.mkdir(parents=True)
    make_pinyin_db(directory / "msime-pinyin.db", rows, offensive)
    manifest = {"source": {"commit": builder}, "custom_dictionary_commit": dictionary, "licensing": {"includes_unlicensed_inputs": unlicensed}}
    (directory / "msime-dictionary-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    sums = "".join(f"{sha256(directory / name)}  {name}\n" for name in ("msime-pinyin.db", "msime-dictionary-manifest.json"))
    (directory / "msime-SHA256SUMS.txt").write_text(sums, encoding="utf-8")


BASE_ROWS = [("che'jian", "车检", 1500), ("che'jian", "车件", 5895), ("a'ba", "阿爸", 100), ("gong'si", "公司", 9000), ("zhan'lve", "战略", 800)]

with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    make_artifact(scratch / "B", BASE_ROWS)
    # C 删掉了 车检（金标准）、加了一个不在任何集合里的词，权重改动不影响这道门槛。
    make_artifact(scratch / "C", [row for row in BASE_ROWS if row[1] != "车检"] + [("ji'ba", "鸡巴", 10)])
    result = gate.gold_pairs_gate(scratch / "B", scratch / "C", pattern)
    assert not result["passed"]
    assert [(item["word"], item["reading"]) for item in result["lost"]] == [("车检", "chejian")]
    assert any(source.startswith("must-outrank.tsv:") for source in result["lost"][0]["sources"])
    same = gate.gold_pairs_gate(scratch / "B", scratch / "B", pattern)
    assert same["passed"] and same["before"] == same["after"] >= 4
    # quick_parases 里的 车检 不算全拼表的行。
    make_artifact(scratch / "D", [row for row in BASE_ROWS if row[1] != "车检"])
    assert gate.present_pairs(scratch / "D" / "msime-pinyin.db", {("车检", "chejian"): []}, pattern) == set()

    # 行数下限：数全拼表，不数别的表；授权构建之外一律失败。
    floor = gate.row_floor_gate(scratch / "B", scratch / "C", 5, pattern)
    assert floor["passed"] and (floor["before"], floor["after"]) == (5, 5)
    assert not gate.row_floor_gate(scratch / "B", scratch / "C", 6, pattern)["passed"]
    make_artifact(scratch / "U", BASE_ROWS, unlicensed=True)
    unlicensed = gate.row_floor_gate(scratch / "B", scratch / "U", 1, pattern)
    assert not unlicensed["passed"] and "unlicensed" in unlicensed["problems"][0]

    # sentence_excluded 的并集：豁免写法前面加 !，没有这张表的一侧为空。
    make_artifact(scratch / "E", BASE_ROWS, offensive=[("鸡巴", 0), ("鸡巴肉", 1)])
    union = scratch / "union.txt"
    assert gate.write_shared_word_list(scratch / "B", scratch / "E", union) == 2
    assert [line for line in union.read_text(encoding="utf-8").splitlines() if not line.startswith("#")] == ["!鸡巴肉", "鸡巴"]


# ---- must-outrank 的棘轮 ----
def outcome(case_id, passed):
    return {"id": case_id, "passed": passed}


rows = [{"id": "a"}, {"id": "b"}, {"id": "c"}]
before = [outcome("a", True), outcome("b", False), outcome("c", False)]
ratchet = gate.must_outrank_gate(rows, before, [outcome("a", True), outcome("b", True), outcome("c", False)], strict=False)
assert ratchet["passed"] and len(ratchet["fixed"]) == 1 and len(ratchet["known_failures"]) == 1 and not ratchet["regressed"]
regressed = gate.must_outrank_gate(rows, before, [outcome("a", False), outcome("b", True), outcome("c", False)], strict=False)
assert not regressed["passed"] and [r["id"] for r in regressed["regressed"]] == ["a"]
strict = gate.must_outrank_gate(rows, before, [outcome("a", True), outcome("b", True), outcome("c", False)], strict=True)
assert not strict["passed"], "strict mode fails on a row failing on both sides"

# ---- 曝光门槛就是 vulgar_exposure 的 C ≤ B ----
def exposure(counts):
    return {"first_page_rows": 6, "keyboards": {"nine": {"summary": {"set_sha256": "x", **counts}}}}


low = exposure({"vulgar_first_page_inputs": 3, "vulgar_top1_inputs": 1, "english_offensive_first_page_inputs": 2})
high = exposure({"vulgar_first_page_inputs": 4, "vulgar_top1_inputs": 1, "english_offensive_first_page_inputs": 2})
assert gate.exposure_gate(high, low)["passed"]
worse = gate.exposure_gate(low, high)
assert not worse["passed"] and worse["counts"]["nine"]["vulgar_first_page_inputs"] == [3, 4]

# ---- 端到端：假 convert_eval 按 FAKE_MODE 出 dump ----
FAKE = r'''#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
def value(flag):
    return args[args.index(flag) + 1] if flag in args else None
resources, case_set, dump = value("--resources"), value("--set"), value("--dump")
limit = int(value("--limit") or 0)
mode = os.environ.get("FAKE_MODE", "same")
if mode == "crash":
    sys.stderr.write("boom\n")
    sys.exit(3)
candidate = "res-C" in resources
cases = []
for line in open(case_set, encoding="utf-8").read().splitlines():
    if not line or line.startswith("#") or line.startswith("id\t"):
        continue
    fields = line.split("\t")
    cases.append(fields)
if limit:
    stride = -(-len(cases) // limit)
    cases = cases[::stride]
with open(dump, "w", encoding="utf-8") as out:
    for index, fields in enumerate(cases):
        case_id, typed, gold = fields[0], fields[1], fields[2]
        texts = [gold, "其他", "别的"]
        if candidate and mode == "worse" and case_id.startswith("w-") and index % 10 == 0:
            texts = ["其他", "别的", "再一个", "还有", "又一个", "第六", gold]
        if candidate and mode == "worse" and case_id.startswith("x-") and index < 3:
            texts = [gold, "傻逼"]
        if case_id == "mo-0001":
            texts = ["车件", "车检"]
        if candidate and mode in ("worse", "reference") and case_id == "s-001":
            texts = ["其他", gold]
        out.write(json.dumps({"id": case_id, "input": typed, "gold": gold, "candidates": [{"text": t, "source": 0} for t in texts]}, ensure_ascii=False) + "\n")
open(value("--report"), "w").write("{}\n")
'''

with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    fake = scratch / "convert_eval"
    fake.write_text(FAKE, encoding="utf-8")
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    model = scratch / "sentence-model.safetensors"
    model.write_bytes(b"model")
    base_lock = scratch / "lock.json"
    base_lock.write_text(json.dumps({"source_commit": "c" * 40, "artifacts": [
        {"name": "msime-pinyin.db", "url": "https://example.invalid/a", "sha256": "0" * 64, "size": 1},
        {"name": "msime-dictionary-manifest.json", "url": "https://example.invalid/b", "sha256": "0" * 64, "size": 1},
        {"name": "sentence-model.safetensors", "url": "https://example.invalid/c", "sha256": sha256(model), "size": model.stat().st_size},
    ]}), encoding="utf-8")
    make_artifact(scratch / "B", BASE_ROWS)
    make_artifact(scratch / "C", BASE_ROWS, offensive=[("傻逼", 0)])

    def run_gate(mode, work, *extra, candidate="C"):
        env = dict(os.environ, FAKE_MODE=mode)
        command = [sys.executable, str(ROOT / "scripts/dict_eval_gate.py"), "run", "--baseline", str(scratch / "B"), "--candidate", str(scratch / candidate),
                   "--model", str(model), "--binary", str(fake), "--work", str(scratch / work), "--base-lock", str(base_lock), "--row-floor", "5",
                   "--jobs", "4", "--markdown", str(scratch / f"{work}.md"), "--json", str(scratch / f"{work}.json"), *extra]
        return subprocess.run(command, env=env, capture_output=True, text=True, timeout=600)

    same = run_gate("same", "same", "--limit", "500", "--arms", "model,engine")
    assert same.returncode == 0, same.stderr
    report = json.loads((scratch / "same.json").read_text(encoding="utf-8"))
    assert report["passed"] and report["limit"] == 500 and report["arms"] == ["model", "engine"]
    assert [g["name"] for g in report["gates"]] == ["gold-pairs", "must-outrank", "vulgar-exposure", "quanpin-words-v2", "row-floor"]
    outrank = report["gates"][1]
    assert outrank["passed"] and outrank["before_passed"] == outrank["after_passed"]
    assert "mo-0001" in {r["id"] for r in outrank["known_failures"]}, "a row failing on both sides is listed, not gated"
    assert set(report["gates"][3]["results"]) == {f"{arm}/{grid}" for arm in ("model", "engine") for grid in ("full", "nine", "fourteen")}
    assert report["gates"][3]["results"]["model/full"]["main"]["cases"] + (report["gates"][3]["results"]["model/full"]["same_code"]["cases"]) <= 500
    markdown = (scratch / "same.md").read_text(encoding="utf-8")
    assert "全部通过" in markdown and "等距子集" in markdown and "两边都失败" in markdown
    # 参考项：每一臂把每一份整句集跑一遍，--limit 不影响它们；目标码两种键盘都列出来。
    references = report["reference"]["sets"]
    assert [(item["arm"], item["set"], item["keyboard"]) for item in references] == [(arm, name, keyboard) for arm in ("model", "engine") for name, keyboard in gate.REFERENCE_RUNS]
    assert all(item["result"]["main"]["top1"]["fixed"] == item["result"]["main"]["top1"]["broken"] == 0 for item in references)
    sentences = next(item for item in references if item["set"] == "sentences-v1.tsv")
    assert sentences["result"]["main"]["cases"] == sum(1 for line in (ROOT / "resources/eval/sentences-v1.tsv").read_text(encoding="utf-8").splitlines() if line and not line.startswith("#") and not line.startswith("id\t"))
    targets = report["reference"]["targets"]
    assert {row["keyboard"] for row in targets} == {"full", "nine"} and ("nine", "2435422") in {(row["keyboard"], row["code"]) for row in targets}
    # 假 convert_eval 把金标准放第 1 位，所以只有金标准本身是粗俗词的三个九键码首屏有粗俗词，两边相同。
    assert {row["code"]: (row["before_offensive"], row["after_offensive"]) for row in targets if row["before_offensive"] or row["after_offensive"]} == {"2435422": (["别鸡巴"], ["别鸡巴"]), "435422": (["个鸡巴"], ["个鸡巴"]), "74224": (["傻逼"], ["傻逼"])}
    assert "参考（只报告，不进门槛）" in markdown and "粗俗目标码" in markdown

    # 只有参考项变差：报告里看得到，退出码仍是 0。
    reference = run_gate("reference", "reference")
    assert reference.returncode == 0, (reference.returncode, reference.stderr)
    report = json.loads((scratch / "reference.json").read_text(encoding="utf-8"))
    sentences = next(item for item in report["reference"]["sets"] if item["set"] == "sentences-v1.tsv")
    assert sentences["result"]["main"]["top1"]["broken"] == 1 and report["passed"]
    assert "top-1 broken（1" in (scratch / "reference.md").read_text(encoding="utf-8")

    worse = run_gate("worse", "worse", "--limit", "2000")
    assert worse.returncode == 1, (worse.returncode, worse.stderr)
    report = json.loads((scratch / "worse.json").read_text(encoding="utf-8"))
    failed = {g["name"] for g in report["gates"] if not g["passed"]}
    assert failed == {"vulgar-exposure", "quanpin-words-v2"}, failed
    assert "**失败**" in (scratch / "worse.md").read_text(encoding="utf-8")
    # 目标码 neirong（曝光集的 x-00001）在 C 的全拼首屏上多了一个粗俗词。
    neirong = next(row for row in report["reference"]["targets"] if (row["keyboard"], row["code"]) == ("full", "neirong"))
    assert neirong["before_offensive"] == [] and neirong["after_offensive"] == ["傻逼"], neirong

    crash = run_gate("crash", "crash")
    assert crash.returncode == 2 and "boom" in crash.stderr, (crash.returncode, crash.stderr)
    missing = run_gate("same", "missing", "--arms", "engine")
    assert missing.returncode == 2 and "--arms" in missing.stderr
    # 库文件损坏（摘要对得上，但不是 SQLite）：sqlite3.DatabaseError 不是门槛结论，退出码 2，不写报告。
    corrupt = scratch / "K"
    corrupt.mkdir()
    (corrupt / "msime-pinyin.db").write_bytes(b"not a database" * 100)
    (corrupt / "msime-dictionary-manifest.json").write_text((scratch / "C" / "msime-dictionary-manifest.json").read_text(encoding="utf-8"), encoding="utf-8")
    (corrupt / "msime-SHA256SUMS.txt").write_text("".join(f"{sha256(corrupt / name)}  {name}\n" for name in ("msime-pinyin.db", "msime-dictionary-manifest.json")), encoding="utf-8")
    broken = run_gate("same", "corrupt", candidate="K")
    assert broken.returncode == 2 and "DatabaseError" in broken.stderr, (broken.returncode, broken.stderr)
    assert not (scratch / "corrupt.md").exists()

print("dict_eval_gate: ok")
