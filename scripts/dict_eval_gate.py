#!/usr/bin/env python3
"""词库 PR 评测的硬门槛（dict-cleanse 方案 4.3）：一份候选词库 C 对比基线 B，一次跑完全部硬门槛，写出 Markdown 和 JSON，退出码表示过没过。

为什么要有它：msime-dictionary 合入 main 就发布，约 24 小时后用户自动收到。评测工具（`eval_compare.py`、`must_outrank.py`、`vulgar_exposure.py`）各自只回答一个问题，要在词库仓库的 PR 上挡住发布，需要一个入口把它们按同一份资源、同一套参数跑完，并把结论合成一个退出码和一条 PR 评论。这个脚本只编排和判定，不改任何评测口径：判定规则全部来自那几个脚本。

五道硬门槛，任何一道失败退出码 1：

1. 金标准对不丢（`gold-pairs`）：评测集里的（词, 读音）对，B 的全拼表里有的，C 里一个不少。词级集和整句集取（金标准, 输入），打字流集取 tokens 列的 `词=读音`，must-outrank 取全键盘行的（应靠前的词, 输入）。九键、14 键的输入是有歧义的码，不构成读音，不取。
2. must-outrank（`must-outrank`）：在 B 上通过的行，C 上必须仍然通过。两边都失败的行是已知问题，逐条列出但不卡（见下面「为什么 must-outrank 是棘轮」）；`--must-outrank-strict` 要求全部通过。
3. 粗俗首屏曝光 C ≤ B（`vulgar-exposure`）：`vulgar_exposure.py` 的三项计数（粗俗首屏输入数、粗俗首选输入数、英文冒犯词首屏输入数）在全键盘和九键上都不得增加。两臂用同一份词表：引擎内嵌的表，加上 B 和 C 的 `msime-pinyin.db` 里 `sentence_excluded` 表（词库仓库 `curation/offensive-words.txt` 写进去的）的并集，再加 `--extra-words`。
4. quanpin-words-v2 配对比较（`quanpin-words-v2`）：全拼、九键、14 键三臂，`eval_compare.py` 的 `--gate` 规则（非同码桶里 top-1 或 top-5 的 broken > fixed 且 McNemar p < 0.05 即失败）。
5. 全拼行数下限（`row-floor`）：C 是授权构建（manifest 的 `licensing.includes_unlicensed_inputs` 为 false），全拼表总行数不低于构建器 `product::verify` 的下限（从 `crates/dict-builder/src/product.rs` 读，改下限要在构建器 PR 里说明理由）。

参考项（方案 4.3「只报告」的那几项，不影响退出码）：sentences-v1、九键长句、particles（三种键盘）、neutral 四份整句集在每一臂上的配对比较，以及方案点名的粗俗目标码（2435422、435422、74224、63476，内容、尴尬、试图 打全的码）在 B、C 首屏上的粗俗词，针对性清洗批次要靠它确认目标码已经清零。ctx-v1 和 typing_sim 只能在 Studio 上跑，不在这里；五笔、英文的 top-N 差异清单和逐表逐行的产物差异由 workflow 另外调用 `dictionary_topn_diff.py`、`dict_artifact_diff.py` 生成。

所有 `convert_eval` 都加 `--mixed-english`（产品出厂设置，九键第 2 位的英文只在它开着时出现）和出厂每页 6 个。挂模型的一臂（`model`，产品默认：模型在桌面锁里）跑全部门槛；`--arms model,engine` 再加一臂不挂模型，只跑 quanpin-words-v2，量词库权重本身。`--limit N` 把 quanpin-words-v2 换成 `convert_eval --limit` 的等距子集（B、C 同一批用例），用于时间不够时；其余门槛总是全量。

为什么 must-outrank 是棘轮：方案 4.3 写的是「全部通过」，但第一批 6 行在当前发布上全部失败（都是排了修复阶段的已知问题）。照字面卡，每个与之无关的投稿 PR 都会红，门禁只会被跳过。棘轮保证已经修好的不再坏，还没修的逐条列在报告里；全部转绿后它就等于「全部通过」。

用法（在 msime 仓库根目录）：

    python3 scripts/dict_eval_gate.py run --baseline <B 的发布附件目录> --candidate <C 的构建产物目录> \\
        --model <sentence-model.safetensors> --work <工作目录> --markdown <报告.md> --json <报告.json> \\
        [--binary target/release/examples/convert_eval] [--jobs 8] [--limit N] [--arms model,engine] \\
        [--preamble <开头.md>] [--extra-words <表>] [--must-outrank-strict] [--row-floor N]

两个目录都要有 `msime-SHA256SUMS.txt` 和它列出的文件（B 用 `gh release download` 下全部主附件，C 是构建器的 `--out`）。退出码：0 全部通过，1 有门槛失败，2 没能跑完（输入不全、评测二进制失败、库打不开或脚本自己出错）。1 只在全部门槛都算出结果、且有一道失败时出现。
"""
from __future__ import annotations

import argparse
import concurrent.futures
import json
import re
import sqlite3
import sys
import time
import traceback
from pathlib import Path

import eval_compare
import eval_keys
import must_outrank
import resource_lock_from_sums
import vulgar_exposure

ROOT = eval_keys.ROOT
EVAL = ROOT / "resources/eval"
V2_SET = EVAL / "quanpin-words-v2.tsv"
PRODUCT_RS = ROOT / "crates/dict-builder/src/product.rs"
FORMAT_RS = ROOT / "crates/engine/src/format.rs"
BASE_LOCK = ROOT / "resources/desktop-dictionary.lock.json"
GRIDS = ("full", "nine", "fourteen")
ARMS = ("model", "engine")
FIRST_PAGE_ROWS = must_outrank.DEFAULT_FIRST_PAGE_ROWS
# 每一次 convert_eval 都带的参数：混输英文是产品出厂设置，首屏按出厂每页 6 个。
COMMON_ARGUMENTS = ["--mixed-english", "--first-page-rows", str(FIRST_PAGE_ROWS)]

# 金标准对取自哪些评测集。`id input gold syllables tags [context]` 格式的集合取（金标准, 输入）；打字流集取 tokens 列。`resources/eval/` 下每一份 .tsv 都必须出现在这三份清单之一，`scripts/test-dict-eval-gate.py` 检查：新加的集合要先决定它算不算金标准。
GOLD_CASE_SETS = (
    "quanpin-words-v1.tsv",
    "quanpin-words-v2.tsv",
    "sentences-v1.tsv",
    "sentences-v2.tsv",
    "sentences-neutral-v1.tsv",
    "sentences-nine-key-v1.tsv",
    "sentences-particles-v1.tsv",
)
GOLD_TYPING_SETS = ("typing-chat-v1.tsv", "typing-tatoeba-v1.tsv")
# 不取金标准对的 .tsv：曝光输入集的第 3 列只是生成这串码的参照词，读音审核表没有金标准；must-outrank 单独处理。
NOT_GOLD_SETS = ("exposure-quanpin-v1.tsv", "exposure-nine-key-v1.tsv", "typing-readings-reviewed.tsv", "must-outrank.tsv")
# 方案 4.3 的参考项：（整句集, 键盘），每一臂都跑，只报告。键盘按各集合自己的用途：九键长句只在九键上，particles 三种键盘都打（与 `verify-local.sh` 的 particles、particles-nine-key、particles-fourteen-key 一致）。
REFERENCE_RUNS = (
    ("sentences-v1.tsv", "full"),
    ("sentences-nine-key-v1.tsv", "nine"),
    ("sentences-particles-v1.tsv", "full"),
    ("sentences-particles-v1.tsv", "nine"),
    ("sentences-particles-v1.tsv", "fourteen"),
    ("sentences-neutral-v1.tsv", "full"),
)


class GateError(Exception):
    """没能跑完门槛（输入不全、评测二进制失败）：退出码 2，不是「门槛失败」。"""


# ---- 全拼表与行数下限 ----


def quanpin_table_pattern(format_rs: Path = FORMAT_RS) -> re.Pattern:
    """全拼表名的正则，从 `msime_engine::format` 的 `MAXIMUM_NUMBERED_SYLLABLES`、`SHIPPED_INITIALS`、`OVERFLOW_BUCKET` 读，与构建器的 `msime::quanpin_tables` 同一份定义。"""
    text = format_rs.read_text(encoding="utf-8")
    maximum = re.search(r"pub const MAXIMUM_NUMBERED_SYLLABLES: usize = (\d+);", text)
    initials = re.search(r'pub const SHIPPED_INITIALS: &str = "([a-z]+)";', text)
    overflow = re.search(r'pub const OVERFLOW_BUCKET: &str = "([a-z]+)";', text)
    if not (maximum and initials and overflow):
        raise GateError(f"{format_rs}: cannot find MAXIMUM_NUMBERED_SYLLABLES, SHIPPED_INITIALS or OVERFLOW_BUCKET")
    buckets = "|".join([str(n) for n in range(1, int(maximum.group(1)) + 1)] + [overflow.group(1)])
    return re.compile(f"^tbl_(?:{buckets})_[{initials.group(1)}]$")


def licensed_row_floor(product_rs: Path = PRODUCT_RS) -> int:
    """授权构建（不含无授权输入）的全拼行数下限：`product::verify` 里 `if complete { … } else { N }` 的 N。"""
    found = re.search(r"let minimum = if complete \{ ([\d_]+) \} else \{ ([\d_]+) \};", product_rs.read_text(encoding="utf-8"))
    if not found:
        raise GateError(f"{product_rs}: cannot find the quanpin row floor (let minimum = if complete {{ … }} else {{ … }};)")
    return int(found.group(2).replace("_", ""))


def quanpin_tables(connection: sqlite3.Connection, pattern: re.Pattern) -> list:
    names = [name for (name,) in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")]
    return sorted(name for name in names if pattern.match(name))


def open_read_only(database: Path) -> sqlite3.Connection:
    if not database.is_file():
        raise GateError(f"{database} does not exist")
    return sqlite3.connect(f"file:{database}?mode=ro", uri=True)


def quanpin_row_count(database: Path, pattern: re.Pattern) -> int:
    connection = open_read_only(database)
    try:
        return sum(connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0] for table in quanpin_tables(connection, pattern))
    finally:
        connection.close()


def row_floor_gate(baseline: Path, candidate: Path, floor: int, pattern: re.Pattern) -> dict:
    manifest = json.loads((candidate / resource_lock_from_sums.MANIFEST).read_text(encoding="utf-8"))
    unlicensed = manifest.get("licensing", {}).get("includes_unlicensed_inputs")
    before = quanpin_row_count(baseline / "msime-pinyin.db", pattern)
    after = quanpin_row_count(candidate / "msime-pinyin.db", pattern)
    problems = []
    if unlicensed is not False:
        problems.append(f"the candidate manifest says licensing.includes_unlicensed_inputs = {json.dumps(unlicensed)}; only a licensed build may be released")
    if after < floor:
        problems.append(f"quanpin rows {after} < floor {floor}")
    return {"name": "row-floor", "passed": not problems, "problems": problems, "floor": floor, "before": before, "after": after}


# ---- 金标准对 ----


def read_set_rows(path: Path) -> list:
    """`id input gold …` 格式的行（同 `convert_eval` 的 `load`：跳过 `#`、空行和 `id\\t` 开头的表头）。"""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("id\t"):
            continue
        rows.append(line.split("\t"))
    return rows


def gold_pairs(eval_dir: Path = EVAL) -> dict:
    """评测集里的（词, 规整后的读音）对，值是取自哪些集合（`集合:id`，每对只记前三处）。"""
    pairs: dict = {}

    def add(word: str, reading: str, where: str) -> None:
        if not word or not reading:
            return
        sources = pairs.setdefault((word, eval_keys.canonical_reading(reading)), [])
        if len(sources) < 3:
            sources.append(where)

    for name in GOLD_CASE_SETS:
        for fields in read_set_rows(eval_dir / name):
            add(fields[2], fields[1], f"{name}:{fields[0]}")
    for name in GOLD_TYPING_SETS:
        path = eval_dir / name
        header = None
        for line in path.read_text(encoding="utf-8").splitlines():
            if not line or line.startswith("#"):
                continue
            fields = line.split("\t")
            if header is None:
                header = fields
                if "tokens" not in header:
                    raise GateError(f"{path}: the header has no tokens column")
                continue
            row = dict(zip(header, fields))
            for token in row.get("tokens", "").split(" "):
                # `~，` 是原样上屏的标点；其余是 `词=读音`。
                word, sign, reading = token.partition("=")
                if sign and not token.startswith("~"):
                    add(word, reading, f"{name}:{row['id']}")
    for row in must_outrank.load_fixture(eval_dir / "must-outrank.tsv"):
        if row["keyboard"] == "full" and row["should_rank_higher"] not in ("", "-") and not row["should_rank_higher"].isascii():
            add(row["should_rank_higher"], row["input"], f"must-outrank.tsv:{row['id']}")
    return pairs


def present_pairs(database: Path, wanted: dict, pattern: re.Pattern) -> set:
    """`wanted` 里在这份词库全拼表中出现的（词, 规整读音）对。"""
    words = {word for word, _ in wanted}
    found = set()
    connection = open_read_only(database)
    try:
        for table in quanpin_tables(connection, pattern):
            for key, value in connection.execute(f'SELECT key, value FROM "{table}"'):
                if value in words:
                    pair = (value, eval_keys.canonical_reading(key))
                    if pair in wanted:
                        found.add(pair)
    finally:
        connection.close()
    return found


def gold_pairs_gate(baseline: Path, candidate: Path, pattern: re.Pattern, eval_dir: Path = EVAL) -> dict:
    wanted = gold_pairs(eval_dir)
    before = present_pairs(baseline / "msime-pinyin.db", wanted, pattern)
    after = present_pairs(candidate / "msime-pinyin.db", wanted, pattern)
    lost = sorted(before - after)
    return {
        "name": "gold-pairs",
        "passed": not lost,
        "pairs": len(wanted),
        "before": len(before),
        "after": len(after),
        "gained": len(after - before),
        "lost": [{"word": word, "reading": reading, "sources": wanted[(word, reading)]} for word, reading in lost],
    }


# ---- 资源目录与 convert_eval ----


def stage_resources(artifact: Path, out: Path, name: str, model: Path | None, base_lock: Path) -> tuple:
    """用 `resource_lock_from_sums.py` 给一份产物目录建临时锁和只含锁里文件的资源目录；`model` 为 None 时是不挂模型的一臂。返回（资源目录, 锁）。"""
    base = json.loads(base_lock.read_text(encoding="utf-8"))
    try:
        lock, _ = resource_lock_from_sums.build_lock(artifact, base, "https://dry-run.invalid/", model is None, None)
        lock_path = out / f"{name}.lock.json"
        lock_path.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")
        directory = out / name
        resource_lock_from_sums.stage(lock, artifact, model, directory)
    except SystemExit as error:
        raise GateError(f"staging {artifact} as {name}: {error}") from error
    return directory, lock_path


class Job:
    """一次 `convert_eval`：哪份资源、哪份集合、哪种键盘、dump 写到哪里。"""

    def __init__(self, label: str, resources: Path, lock: Path, case_set: Path, keyboard: str, dump: Path, extra: list) -> None:
        self.label = label
        self.resources = resources
        self.lock = lock
        self.case_set = case_set
        self.keyboard = keyboard
        self.dump = dump
        self.extra = extra
        self.seconds = 0.0


def run_jobs(jobs: list, binary: Path, parallel: int) -> None:
    """并行跑全部 `convert_eval`（它自己只用一个核）。任何一个失败就是 GateError：门槛没跑完，不能当作通过或失败。"""

    def one(job: Job) -> Job:
        started = time.monotonic()
        try:
            eval_keys.run_convert_eval(binary, job.resources, job.lock, job.case_set, job.dump, job.dump.with_suffix(".report.json"), job.keyboard, job.extra)
        except SystemExit as error:
            raise GateError(f"{job.label}: {error}") from error
        job.seconds = time.monotonic() - started
        print(f"done {job.label} in {job.seconds:.0f}s", file=sys.stderr, flush=True)
        return job

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, parallel)) as pool:
        for future in concurrent.futures.as_completed([pool.submit(one, job) for job in jobs]):
            future.result()


# ---- 三道跑评测的门槛 ----


def must_outrank_gate(rows: list, before: list, after: list, strict: bool) -> dict:
    """棘轮：B 上通过、C 上失败的行算退步，门槛失败；`strict` 时 C 上任何失败都算。"""
    previous = {result["id"]: result for result in before}
    regressed, fixed, known = [], [], []
    for result in after:
        was = previous[result["id"]]["passed"]
        if was and not result["passed"]:
            regressed.append(result)
        elif not was and result["passed"]:
            fixed.append(result)
        elif not was and not result["passed"]:
            known.append(result)
    failing = [result for result in after if not result["passed"]]
    passed = not (failing if strict else regressed)
    return {
        "name": "must-outrank",
        "passed": passed,
        "strict": strict,
        "rows": len(rows),
        "before_passed": sum(result["passed"] for result in before),
        "after_passed": sum(result["passed"] for result in after),
        "regressed": regressed,
        "fixed": fixed,
        "known_failures": known,
    }


def offensive_rows(database: Path) -> list:
    """一份词库 `sentence_excluded` 表的行，写成 `offensive_words.txt` 的格式（豁免写法前面加 `!`）；没有这张表时为空。"""
    connection = open_read_only(database)
    try:
        exists = connection.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='sentence_excluded'").fetchone()
        if not exists:
            return []
        return [("!" if exempt else "") + value for value, exempt in connection.execute("SELECT value, exempt FROM sentence_excluded")]
    finally:
        connection.close()


def write_shared_word_list(baseline: Path, candidate: Path, path: Path) -> int:
    """B、C 两份 `sentence_excluded` 的并集写成一张表，两臂都用它判定，计数才可比。返回行数。"""
    rows = sorted(set(offensive_rows(baseline / "msime-pinyin.db")) | set(offensive_rows(candidate / "msime-pinyin.db")))
    path.write_text("# B 与 C 的 sentence_excluded 并集，scripts/dict_eval_gate.py 生成\n" + "".join(row + "\n" for row in rows), encoding="utf-8")
    return len(rows)


def exposure_gate(before: dict, after: dict) -> dict:
    problems = vulgar_exposure.compare_baseline(after, before)
    counts = {}
    for keyboard, values in after["keyboards"].items():
        counts[keyboard] = {name: [before["keyboards"][keyboard]["summary"][name], values["summary"][name]] for name in vulgar_exposure.GATED}
    return {"name": "vulgar-exposure", "passed": not problems, "problems": problems, "counts": counts}


def v2_gate(results: dict, alpha: float) -> dict:
    """`results` 是 {(臂, 键盘): eval_compare 的比较结果}。"""
    failures = []
    for (arm, grid), result in results.items():
        for failure in eval_compare.gate_failures(result, [1, 5], alpha):
            failures.append(f"{arm} {grid}: {failure}")
    return {"name": "quanpin-words-v2", "passed": not failures, "problems": failures, "alpha": alpha, "results": {f"{arm}/{grid}": result for (arm, grid), result in results.items()}}


# ---- 参考项（只报告） ----


def reference_dump(arm: str, set_name: str, keyboard: str) -> str:
    return f"reference-{arm}-{Path(set_name).stem}-{keyboard}.jsonl"


def exposure_targets(before: dict, after: dict) -> list:
    """方案点名的目标码在 B、C 首屏上的粗俗词和英文冒犯词。只报告：不是每个 PR 都是针对性清洗批次，C ≤ B 已经由曝光门槛保证。"""

    def offensive_at(values: dict) -> dict:
        found = {}
        for hit in [*values["vulgar"], *values["english_offensive"]]:
            found.setdefault(hit["input"], []).append(hit["word"])
        return found

    rows = []
    for keyboard, values in after["keyboards"].items():
        previous = before["keyboards"][keyboard]
        was = {target["input"]: target for target in previous["targets"]}
        old_hits, new_hits = offensive_at(previous), offensive_at(values)
        for target in values["targets"]:
            rows.append({
                "keyboard": keyboard,
                "code": target["code"],
                "for": target["for"],
                "before_offensive": old_hits.get(target["input"], []),
                "after_offensive": new_hits.get(target["input"], []),
                "before_first_page": was.get(target["input"], {}).get("first_page", []),
                "after_first_page": target["first_page"],
            })
    return rows


# ---- 编排 ----


def run(args: argparse.Namespace) -> dict:
    arms = [arm for arm in args.arms.split(",") if arm]
    if not arms or any(arm not in ARMS for arm in arms) or arms[0] != "model":
        raise GateError(f"--arms takes model optionally followed by engine (model first: it runs every gate), got {args.arms}")
    if args.model is None or not args.model.is_file():
        raise GateError("--model must name sentence-model.safetensors: the product arm runs with the model")
    if args.binary is None or not args.binary.exists():
        raise GateError(f"{args.binary} does not exist; build it with `cargo build --release -p msime-input-runtime --example convert_eval`")
    work = args.work
    work.mkdir(parents=True, exist_ok=True)
    pattern = quanpin_table_pattern()
    floor = args.row_floor if args.row_floor is not None else licensed_row_floor()
    started = time.monotonic()

    gates = [gold_pairs_gate(args.baseline, args.candidate, pattern), row_floor_gate(args.baseline, args.candidate, floor, pattern)]

    staged = {}
    for side, artifact in (("B", args.baseline), ("C", args.candidate)):
        for arm in arms:
            staged[(side, arm)] = stage_resources(artifact, work, f"res-{side}-{arm}", args.model if arm == "model" else None, args.base_lock)

    fixture = must_outrank.load_fixture(EVAL / "must-outrank.tsv")
    shared_words = work / "offensive-union.txt"
    shared_count = write_shared_word_list(args.baseline, args.candidate, shared_words)
    extra_words = [shared_words, *args.extra_words]

    jobs: list = []
    v2_extra = COMMON_ARGUMENTS + (["--limit", str(args.limit)] if args.limit else [])
    for side in ("B", "C"):
        resources, lock = staged[(side, "model")]
        out = work / side
        out.mkdir(exist_ok=True)
        # 全拼曝光的输入最多、候选表最长，最先开跑。
        for keyboard in ("full", "nine"):
            jobs.append(Job(f"{side} exposure {keyboard}", resources, lock, vulgar_exposure.SETS[keyboard], keyboard, out / f"exposure-{keyboard}.jsonl", COMMON_ARGUMENTS))
        for arm in arms:
            arm_resources, arm_lock = staged[(side, arm)]
            for grid in GRIDS:
                jobs.append(Job(f"{side} v2 {arm} {grid}", arm_resources, arm_lock, V2_SET, grid, out / f"v2-{arm}-{grid}.jsonl", v2_extra))
        for keyboard in must_outrank.KEYBOARDS:
            subset = [row for row in fixture if row["keyboard"] == keyboard]
            if subset:
                case_set = out / f"must-outrank-{keyboard}.tsv"
                must_outrank.write_case_set(subset, case_set)
                jobs.append(Job(f"{side} must-outrank {keyboard}", resources, lock, case_set, keyboard, out / f"must-outrank-{keyboard}.jsonl", COMMON_ARGUMENTS))
    # 参考项排在最后：都是几十到一千来条的小集合，不拖慢门槛那几组最长的。
    for side in ("B", "C"):
        for arm in arms:
            arm_resources, arm_lock = staged[(side, arm)]
            for set_name, keyboard in REFERENCE_RUNS:
                jobs.append(Job(f"{side} reference {arm} {Path(set_name).stem} {keyboard}", arm_resources, arm_lock, EVAL / set_name, keyboard, work / side / reference_dump(arm, set_name, keyboard), COMMON_ARGUMENTS))
    run_jobs(jobs, args.binary, args.jobs)

    outrank_results = {}
    for side in ("B", "C"):
        dumps = {keyboard: eval_keys.load_dump(work / side / f"must-outrank-{keyboard}.jsonl") for keyboard in must_outrank.KEYBOARDS if (work / side / f"must-outrank-{keyboard}.jsonl").exists()}
        outrank_results[side] = must_outrank.check(fixture, dumps, FIRST_PAGE_ROWS)
    gates.append(must_outrank_gate(fixture, outrank_results["B"], outrank_results["C"], args.must_outrank_strict))

    exposure = {}
    for side, artifact in (("B", args.baseline), ("C", args.candidate)):
        dumps = {keyboard: eval_keys.load_dump(work / side / f"exposure-{keyboard}.jsonl") for keyboard in ("full", "nine")}
        exposure[side] = vulgar_exposure.report(dumps, artifact / "msime-pinyin.db", extra_words, FIRST_PAGE_ROWS)
        (work / f"exposure-{side}.json").write_text(json.dumps(exposure[side], ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    exposure_result = exposure_gate(exposure["B"], exposure["C"])
    exposure_result["word_list_rows_from_databases"] = shared_count
    gates.append(exposure_result)

    tags = eval_compare.load_tags(V2_SET)
    comparisons = {}
    for arm in arms:
        for grid in GRIDS:
            before = eval_compare.load_dump(work / "B" / f"v2-{arm}-{grid}.jsonl")
            after = eval_compare.load_dump(work / "C" / f"v2-{arm}-{grid}.jsonl")
            comparisons[(arm, grid)] = eval_compare.compare_convert(before, after, tags, grid, [1, 5])
    gates.append(v2_gate(comparisons, args.alpha))

    references = []
    for arm in arms:
        for set_name, keyboard in REFERENCE_RUNS:
            before = eval_compare.load_dump(work / "B" / reference_dump(arm, set_name, keyboard))
            after = eval_compare.load_dump(work / "C" / reference_dump(arm, set_name, keyboard))
            references.append({"set": set_name, "keyboard": keyboard, "arm": arm, "result": eval_compare.compare_convert(before, after, None, keyboard, [1, 5])})

    order = ("gold-pairs", "must-outrank", "vulgar-exposure", "quanpin-words-v2", "row-floor")
    gates.sort(key=lambda gate: order.index(gate["name"]))
    return {
        "passed": all(gate["passed"] for gate in gates),
        "arms": arms,
        "limit": args.limit,
        "baseline": manifest_identity(args.baseline),
        "candidate": manifest_identity(args.candidate),
        "gates": gates,
        "reference": {"sets": references, "targets": exposure_targets(exposure["B"], exposure["C"])},
        "timings": {job.label: round(job.seconds) for job in jobs},
        "seconds": round(time.monotonic() - started),
    }


def manifest_identity(artifact: Path) -> dict:
    manifest = json.loads((artifact / resource_lock_from_sums.MANIFEST).read_text(encoding="utf-8"))
    return {"builder": manifest.get("source", {}).get("commit"), "dictionary": manifest.get("custom_dictionary_commit")}


# ---- 报告 ----

GATE_TITLES = {
    "gold-pairs": "金标准对不丢",
    "must-outrank": "must-outrank",
    "vulgar-exposure": "粗俗首屏曝光 C ≤ B",
    "quanpin-words-v2": "quanpin-words-v2 配对比较",
    "row-floor": "全拼行数下限",
}
KEYBOARD_NAMES = {"full": "全拼", "nine": "九键", "fourteen": "14 键"}
EXPOSURE_NAMES = {
    "vulgar_first_page_inputs": "粗俗首屏输入",
    "vulgar_top1_inputs": "粗俗首选输入",
    "english_offensive_first_page_inputs": "英文冒犯词首屏输入",
}


def short(commit: str | None) -> str:
    return f"`{commit[:10]}`" if commit else "未知"


def cell(text: str) -> str:
    """表格单元格里的 `|` 和换行会拆坏表格。"""
    return str(text).replace("|", "\\|").replace("\n", " ")


def gate_line(gate: dict) -> str:
    name = gate["name"]
    if name == "gold-pairs":
        detail = f"评测集 {gate['pairs']} 对，B 有 {gate['before']} 对，C 有 {gate['after']} 对，丢了 {len(gate['lost'])} 对"
    elif name == "must-outrank":
        detail = f"{gate['rows']} 行：B 通过 {gate['before_passed']}，C 通过 {gate['after_passed']}；退步 {len(gate['regressed'])}，修好 {len(gate['fixed'])}，两边都失败 {len(gate['known_failures'])}" + ("（严格模式）" if gate["strict"] else "（已知问题，不卡）")
    elif name == "vulgar-exposure":
        parts = []
        for keyboard, counts in gate["counts"].items():
            parts.append(KEYBOARD_NAMES[keyboard] + " " + "，".join(f"{EXPOSURE_NAMES[key]} {b} → {c}" for key, (b, c) in counts.items()))
        detail = "；".join(parts)
    elif name == "quanpin-words-v2":
        parts = []
        for label, result in gate["results"].items():
            top1, top5 = result["main"]["top1"], result["main"]["top5"]
            arm, grid = label.split("/")
            parts.append(f"{arm} {KEYBOARD_NAMES[grid]} top-1 +{top1['fixed']}/−{top1['broken']}，top-5 +{top5['fixed']}/−{top5['broken']}")
        detail = "；".join(parts)
    else:
        detail = f"C {gate['after']} 行（B {gate['before']}），下限 {gate['floor']}"
    return f"| {GATE_TITLES[name]} | {'通过' if gate['passed'] else '**失败**'} | {cell(detail)} |"


def format_rank(rank) -> str:
    return "-" if rank is None else str(rank)


def outrank_rows(results: list) -> list:
    lines = ["| id | 键盘 | 输入 | 断言 | 名次（靠前/靠后） | 首屏 |", "| --- | --- | --- | --- | --- | --- |"]
    for result in results:
        claim = f"{result['should_rank_higher']} > {result['should_rank_lower']}" if result["kind"] == "outrank" else f"{result['should_rank_lower']} 不在首屏"
        lines.append(f"| {result['id']} | {KEYBOARD_NAMES[result['keyboard']]} | `{result['input']}` | {cell(claim)} | {format_rank(result['higher_rank'])} / {format_rank(result['lower_rank'])} | {cell(' '.join(result['first_page']))} |")
    return lines


def details(title: str, lines: list) -> list:
    return ["", f"<details><summary>{title}</summary>", "", *lines, "", "</details>"]


def render_markdown(result: dict, preamble: str, listed: int = 30) -> str:
    lines = ["## 词库评测把关", ""]
    if preamble.strip():
        lines += [preamble.rstrip(), ""]
    lines.append(f"- 基线 B：词库 {short(result['baseline']['dictionary'])}，构建器 {short(result['baseline']['builder'])}")
    lines.append(f"- 候选 C：词库 {short(result['candidate']['dictionary'])}，构建器 {short(result['candidate']['builder'])}")
    arms = "挂模型（产品默认）" + ("，另加不挂模型一臂（只跑 quanpin-words-v2）" if "engine" in result["arms"] else "")
    lines.append(f"- 评测臂：{arms}；混输英文开，每页 6 个")
    if result["limit"]:
        lines.append(f"- quanpin-words-v2 只跑了等距子集（`--limit {result['limit']}`），其余门槛全量")
    lines.append(f"- 结论：{'**全部通过**' if result['passed'] else '**有门槛失败**，合入前先看下面的失败项'}（用时 {result['seconds'] // 60} 分钟）")
    lines += ["", "| 门槛 | 结果 | 说明 |", "| --- | --- | --- |"]
    lines += [gate_line(gate) for gate in result["gates"]]
    for gate in result["gates"]:
        name = gate["name"]
        if name == "gold-pairs" and gate["lost"]:
            body = ["| 词 | 读音 | 出处 |", "| --- | --- | --- |"]
            body += [f"| {cell(item['word'])} | `{item['reading']}` | {cell('，'.join(item['sources']))} |" for item in gate["lost"][:listed]]
            if len(gate["lost"]) > listed:
                body.append(f"\n另有 {len(gate['lost']) - listed} 对，见 JSON 报告。")
            lines += details(f"C 丢掉的金标准对（{len(gate['lost'])}）", body)
        elif name == "must-outrank":
            if gate["regressed"]:
                lines += details(f"must-outrank 退步（{len(gate['regressed'])}）", outrank_rows(gate["regressed"]))
            if gate["fixed"]:
                lines += details(f"must-outrank 修好（{len(gate['fixed'])}）", outrank_rows(gate["fixed"]))
            if gate["known_failures"]:
                lines += details(f"must-outrank 两边都失败（{len(gate['known_failures'])}）", outrank_rows(gate["known_failures"]))
        elif name == "vulgar-exposure" and gate["problems"]:
            lines += details("粗俗曝光增加的项", [f"- {cell(problem)}" for problem in gate["problems"]])
        elif name == "quanpin-words-v2":
            body = ["| 臂 | 键盘 | 用例（非同码） | top-1 B → C | fixed / broken | p | top-5 B → C | fixed / broken | p |", "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
            for label, compared in gate["results"].items():
                arm, grid = label.split("/")
                main = compared["main"]
                top1, top5 = main["top1"], main["top5"]
                body.append(f"| {arm} | {KEYBOARD_NAMES[grid]} | {main['cases']} | {top1['before']:.4f} → {top1['after']:.4f} | {top1['fixed']} / {top1['broken']} | {top1['mcnemar_p']:.3g} | {top5['before']:.4f} → {top5['after']:.4f} | {top5['fixed']} / {top5['broken']} | {top5['mcnemar_p']:.3g} |")
            for label, compared in gate["results"].items():
                broken = compared["main"]["top1"]["broken_cases"]
                if broken:
                    shown = [f"| `{case['input']}` | {cell(case['gold'])} | {format_rank(case['before'])} → {format_rank(case['after'])} |" for case in broken[:listed]]
                    body += ["", f"{label} top-1 broken（{len(broken)}，列前 {min(len(broken), listed)}）：", "", "| 输入 | 金标准 | 名次 |", "| --- | --- | --- |", *shown]
            if gate["problems"]:
                body += ["", *[f"- 失败：{cell(problem)}" for problem in gate["problems"]]]
            lines += details("quanpin-words-v2 明细（同码桶只报告，不进门槛）", body)
        elif name == "row-floor" and gate["problems"]:
            lines += details("行数下限的问题", [f"- {cell(problem)}" for problem in gate["problems"]])
    lines += render_reference(result["reference"], listed)
    return "\n".join(lines) + "\n"


def render_reference(reference: dict, listed: int) -> list:
    """方案 4.3 的参考项：整句集的配对比较和粗俗目标码，只报告，不影响结论。"""
    lines = ["", "### 参考（只报告，不进门槛）", "", "| 集合 | 臂 | 键盘 | 用例 | top-1 B → C | fixed / broken | p | top-5 B → C | fixed / broken | p |", "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for item in reference["sets"]:
        main = item["result"]["main"]
        top1, top5 = main["top1"], main["top5"]
        lines.append(f"| {Path(item['set']).stem} | {item['arm']} | {KEYBOARD_NAMES[item['keyboard']]} | {main['cases']} | {top1['before']:.4f} → {top1['after']:.4f} | {top1['fixed']} / {top1['broken']} | {top1['mcnemar_p']:.3g} | {top5['before']:.4f} → {top5['after']:.4f} | {top5['fixed']} / {top5['broken']} | {top5['mcnemar_p']:.3g} |")
    changed = []
    for item in reference["sets"]:
        top1 = item["result"]["main"]["top1"]
        for kind, cases in (("broken", top1["broken_cases"]), ("fixed", top1["fixed_cases"])):
            if cases:
                shown = [f"| `{case['input']}` | {cell(case['gold'])} | {format_rank(case['before'])} → {format_rank(case['after'])} |" for case in cases[:listed]]
                changed += ["", f"{Path(item['set']).stem} {item['arm']} {KEYBOARD_NAMES[item['keyboard']]} top-1 {kind}（{len(cases)}，列前 {min(len(cases), listed)}）：", "", "| 输入 | 金标准 | 名次 |", "| --- | --- | --- |", *shown]
    if changed:
        lines += details("整句集 top-1 变了的用例", changed)
    targets = reference["targets"]
    if targets:
        cleared = sum(not row["after_offensive"] for row in targets)
        body = ["| 键盘 | 码 | 对应的词 | B 首屏的冒犯词 | C 首屏的冒犯词 | C 首屏 |", "| --- | --- | --- | --- | --- | --- |"]
        for row in targets:
            body.append(f"| {KEYBOARD_NAMES[row['keyboard']]} | `{row['code']}` | {cell(row['for'])} | {cell(' '.join(row['before_offensive']) or '无')} | {cell(' '.join(row['after_offensive']) or '无')} | {cell(' '.join(row['after_first_page']))} |")
        lines += ["", f"粗俗目标码（方案 4.3，针对性清洗批次要清零）：{len(targets)} 个里 C 首屏没有冒犯词的 {cleared} 个。"]
        lines += details("粗俗目标码的首屏", body)
    lines += ["", "ctx-v1 和 typing_sim 只能在 Studio 上跑，不在这个作业里。"]
    return lines


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("run", help="run every hard gate of plan 4.3 for a candidate against a baseline")
    command.add_argument("--baseline", type=Path, required=True, help="B: directory of the released assets (msime-SHA256SUMS.txt and its files)")
    command.add_argument("--candidate", type=Path, required=True, help="C: the builder's --out directory")
    command.add_argument("--model", type=Path, help="sentence-model.safetensors for the product arm")
    command.add_argument("--binary", type=Path, default=eval_keys.DEFAULT_BINARY)
    command.add_argument("--work", type=Path, default=ROOT / "target/dict-eval-gate")
    command.add_argument("--jobs", type=int, default=4, help="convert_eval processes at once (each uses one core)")
    command.add_argument("--limit", type=int, default=0, help="run quanpin-words-v2 on convert_eval's evenly spaced subset of this size (0 = all)")
    command.add_argument("--arms", default="model", help="model, or model,engine to add the arm without the model for quanpin-words-v2")
    command.add_argument("--alpha", type=float, default=0.05)
    command.add_argument("--extra-words", type=Path, action="append", default=[], help="another offensive word list for the exposure gate, both sides")
    command.add_argument("--must-outrank-strict", action="store_true", help="every must-outrank row must pass, not only those passing on the baseline")
    command.add_argument("--row-floor", type=int, help="override the floor read from crates/dict-builder/src/product.rs")
    command.add_argument("--base-lock", type=Path, default=BASE_LOCK, help="the lock whose file list the staged resource directories follow")
    command.add_argument("--preamble", type=Path, help="Markdown put under the report title (run links, refs)")
    command.add_argument("--markdown", type=Path)
    command.add_argument("--json", type=Path)
    args = parser.parse_args(argv)
    if args.limit < 0:
        parser.error("--limit must be 0 or more")

    # 退出码 1 只留给「门槛算出了结果且有一道失败」。被调用的评测脚本遇到不可比的输入（dump 跑错集合、产物缺文件）时 `raise SystemExit(说明)`，退出码会是 1；库文件损坏（`sqlite3.DatabaseError`）、文件缺失、脚本自己的错误默认也以 1 结束。这些都不是门槛结论，统一换成 2，写报告的步骤也算在内。
    try:
        result = run(args)
        preamble = args.preamble.read_text(encoding="utf-8") if args.preamble else ""
        markdown = render_markdown(result, preamble)
        report = json.dumps(result, ensure_ascii=False, indent=1) + "\n"
        if args.markdown:
            args.markdown.write_text(markdown, encoding="utf-8")
        if args.json:
            args.json.write_text(report, encoding="utf-8")
    except (GateError, SystemExit) as error:
        print(f"dict_eval_gate: could not run the gates: {error}", file=sys.stderr)
        return 2
    except Exception:
        traceback.print_exc()
        print("dict_eval_gate: could not run the gates: unexpected error above", file=sys.stderr)
        return 2
    print(markdown)
    for gate in result["gates"]:
        if not gate["passed"]:
            print(f"gate failed: {gate['name']}", file=sys.stderr)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
