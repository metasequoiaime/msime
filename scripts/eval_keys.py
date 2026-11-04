"""评测脚本共用的键码换算和 `convert_eval` 调用。

码表照抄 `crates/engine/src/nine_key.rs` 的 `KEYPAD`（九键数字）和 `FOURTEEN_KEYPAD`（全拼 14 键组码），`scripts/test-build-quanpin-words-v2.py` 核对两边逐字相同。`convert_eval --grid nine|fourteen` 的用例输入写字母，由它自己换成组码再打；要在九键上打一串数字时，用 `nine_digits_to_letters` 换成一串编码相同的字母交给它。
"""
from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# crates/engine/src/nine_key.rs 的 `KEYPAD`：a..=z 各自所在的九键数字。
NINE_KEYPAD = "22233344455566677778889999"
# crates/engine/src/nine_key.rs 的 `FOURTEEN_KEYPAD`：a..=z 各自所在的 14 键组码（每组首字母）。
FOURTEEN_KEYPAD = "abcdedggujjlmbooqeatucqztz"
KEYPADS = {"nine": NINE_KEYPAD, "fourteen": FOURTEEN_KEYPAD}
# 每个九键数字取键上第一个字母：换回来的字母串在九键上的编码就是原来的数字。
NINE_DIGIT_LETTER = {digit: chr(97 + NINE_KEYPAD.index(digit)) for digit in "23456789"}
DEFAULT_BINARY = ROOT / "target/release/examples/convert_eval"
ENGLISH_SOURCE = 4  # msime_engine::CandidateSource::English，见 convert_eval/metrics.rs 的 source_name。
# 冻结评测集（quanpin-words-v2、exposure-*-v1）的参照词库。生成脚本只认这一份，不读当前的资源锁：锁会随词库升级前进，冻结集要能在锁变了以后照样逐字节重新生成。换参照词库要发新版本的评测集，同时改这里。
REFERENCE_RELEASE = "dict-v2.0.41"
REFERENCE_PINYIN_SHA256 = "5973347e707b4654b1c97eae8d99dc113a0915c461cd4b14fe5f438741dc3725"


def encode(keyboard: str, text: str) -> str:
    """字母串在某种键盘上的码：全键盘原样；九键、14 键与 `KeyGrid::encode` 相同，只取 a..=z，其余字节（`'`）跳过。"""
    if keyboard == "full":
        return text
    keypad = KEYPADS[keyboard]
    return "".join(keypad[ord(ch) - 97] for ch in text if "a" <= ch <= "z")


def nine_digits_to_letters(digits: str) -> str:
    """一串九键数字换成编码相同的字母串。0、1 和非数字没有字母，报错。"""
    try:
        return "".join(NINE_DIGIT_LETTER[digit] for digit in digits)
    except KeyError as error:
        raise ValueError(f"{digits}: only 2-9 can be typed as nine-key letters") from error


def canonical_reading(reading: str) -> str:
    """去掉音节分隔，并把 `lue`、`nue` 写成词库用的 `lve`、`nve`（引擎把用户打的 `lue` 规整成 `lve`）。没有以 `ue` 开头的音节，按子串替换不会误伤。"""
    return reading.replace("'", "").replace("lue", "lve").replace("nue", "nve")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def check_reference_pinyin(database: Path) -> None:
    """冻结评测集只能从参照词库生成：`msime-pinyin.db` 的 SHA-256 不是 `REFERENCE_PINYIN_SHA256` 就报错。"""
    actual = sha256(database)
    if actual != REFERENCE_PINYIN_SHA256:
        raise SystemExit(f"{database} has SHA-256 {actual}, but the frozen eval sets are derived from {REFERENCE_RELEASE} msime-pinyin.db ({REFERENCE_PINYIN_SHA256})")


def grid_arguments(keyboard: str) -> list:
    return [] if keyboard == "full" else ["--grid", keyboard]


def run_convert_eval(binary: Path, resources: Path, resource_lock: Path | None, case_set: Path, dump: Path, report: Path, keyboard: str, extra: list) -> None:
    """跑一次 `convert_eval`，写 dump 和报告。stdout 是报告本身，丢掉；失败时把 stderr 原样带出来。"""
    if not binary.exists():
        raise SystemExit(f"{binary} does not exist; build it with `cargo build --release -p msime-input-runtime --example convert_eval` or pass --binary")
    command = [str(binary), "--resources", str(resources), "--set", str(case_set), "--dump", str(dump), "--report", str(report), *grid_arguments(keyboard), *extra]
    if resource_lock is not None:
        command += ["--resource-lock", str(resource_lock)]
    result = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    if result.returncode != 0:
        raise SystemExit(f"convert_eval failed ({result.returncode}): {' '.join(command)}\n{result.stderr}")


def load_dump(path: Path) -> dict:
    """`convert_eval --dump` 按 id 返回整行。"""
    rows = {}
    with path.open(encoding="utf-8") as file:
        for line in file:
            if line.strip():
                row = json.loads(line)
                rows[row["id"]] = row
    return rows
