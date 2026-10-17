#!/usr/bin/env python3
"""本地语音识别的准确率评测：用公开测试集跑已安装的模型，按混合错误率（MER）打分。

MER 把参考文本和识别结果切成同一种单位后算编辑距离：汉字按字，拉丁字母和数字按词，标点和大小写不计。中文评测普遍按字算 CER，英文按词算 WER，中英混说时各算各的会让一种语言的错被另一种稀释，所以合成一个数。

先把测试集准备成统一格式（16 kHz 单声道 16 位 WAV 和一份 `set.tsv`：id、相对路径、参考文本），数据本身不入库：

    python3 scripts/voice_eval.py prepare-fleurs <test.tsv> <test.tar.gz> <out dir>
    uv run --with pyarrow scripts/voice_eval.py prepare-ascend <test.parquet> <out dir>

FLEURS（CC-BY-4.0）的普通话测试集在 https://huggingface.co/datasets/google/fleurs 的 `data/cmn_hans_cn/`，ASCEND（CC-BY-SA-4.0，中英混说）在 https://huggingface.co/datasets/CAiRE/ASCEND 的 test parquet。

再用 shared/voice 构建出的 msime-voice-local 逐条识别，结果按条写进 JSON 行文件。模型在同一个进程里只加载一次，按 shared/voice/README.md 的 stdin/stdout 协议送音频：

    python3 scripts/voice_eval.py run --helper build/shared-voice/msime-voice-local \\
        --runtime target/voice-runtime/macos/libsherpa-onnx-c-api.dylib \\
        --model <installed model dir> --set <out dir>/set.tsv [--set ...] --hypotheses <model>.jsonl [--limit N]

最后打分，可以一次给多个模型的结果：

    python3 scripts/voice_eval.py score <model>.jsonl [...] [--host-library target/debug/libmsime_host_api.dylib] [--json result.json]

给了 `--host-library` 时，另外给出参考文本和识别结果都经过宿主同样调用的 `msime_client_voice_normalize_numbers`（按模型清单的 kind，只改写不自带 ITN 的模型）之后的分数。各测试集写数字的习惯不同（FLEURS 写阿拉伯数字，ASCEND 写汉字），原样打分会把写法差异算成识别错误；两边都规整后，这一项只看听对没有，适合跨测试集、跨模型比较。识别结果存在文件里，改数字规则或打分规则后只需重新打分，不必重新识别。

基线记在 resources/eval/voice-baseline.json，换模型、改解码参数或热词分数前后各跑一次比较。
"""
import argparse
import base64
import ctypes
import json
import subprocess
import sys
import tarfile
import time
import unicodedata
import wave
from pathlib import Path

SAMPLE_RATE = 16000


# ---- 打分 ----


def is_cjk(character):
    code = ord(character)
    return (
        0x3400 <= code <= 0x4DBF
        or 0x4E00 <= code <= 0x9FFF
        or 0xF900 <= code <= 0xFAFF
        or 0x20000 <= code <= 0x3134F
        or 0x3040 <= code <= 0x30FF
    )


def tokens(text):
    """汉字（及假名）一字一个单位，其余字母数字连成的串一个单位；标点、符号和空白只作分隔。"""
    out = []
    word = []
    for character in unicodedata.normalize("NFKC", text).lower():
        if is_cjk(character):
            if word:
                out.append("".join(word))
                word = []
            out.append(character)
        elif character.isalnum() or character == "'":
            word.append(character)
        else:
            if word:
                out.append("".join(word))
                word = []
    if word:
        out.append("".join(word))
    return [token for token in out if token.strip("'")]


def edit_distance(reference, hypothesis):
    previous = list(range(len(hypothesis) + 1))
    for i, expected in enumerate(reference, 1):
        current = [i] + [0] * len(hypothesis)
        for j, heard in enumerate(hypothesis, 1):
            current[j] = min(previous[j] + 1, current[j - 1] + 1, previous[j - 1] + (expected != heard))
        previous = current
    return previous[-1]


def score(pairs):
    """(参考, 识别) 对的总错误数与参考单位数；MER = errors / units。"""
    errors = 0
    units = 0
    for reference, hypothesis in pairs:
        expected = tokens(reference)
        errors += edit_distance(expected, tokens(hypothesis))
        units += len(expected)
    return errors, units


# ---- 数据准备 ----


def write_wav(path, pcm):
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(SAMPLE_RATE)
        out.writeframes(pcm)


def to_pcm16(data):
    """任意音频字节经 ffmpeg 转成 16 kHz 单声道 16 位小端 PCM。"""
    result = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", "pipe:0", "-f", "s16le", "-ac", "1", "-ar", str(SAMPLE_RATE), "pipe:1"],
        input=data,
        capture_output=True,
        check=True,
        timeout=120,
    )
    return result.stdout


def prepare_fleurs(tsv, archive, out):
    # FLEURS 的 test.tsv：id、文件名、原始转写、规范化转写、字符序列、采样数、性别。参考文本用原始转写，它保留了数字和大小写，和用户想要的上屏结果一致。
    references = {}
    for line in Path(tsv).read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        if len(fields) >= 3:
            references[fields[1]] = fields[2]
    rows = []
    with tarfile.open(archive) as tar:
        for member in tar:
            name = Path(member.name).name
            if not member.isfile() or name not in references:
                continue
            target = Path(out) / "wavs" / name
            write_wav(target, to_pcm16(tar.extractfile(member).read()))
            rows.append((Path(name).stem, f"wavs/{name}", references[name]))
    write_set(out, rows)


def prepare_ascend(parquet, out):
    import pyarrow.parquet as pq

    table = pq.read_table(parquet).to_pylist()
    rows = []
    for row in table:
        identifier = str(row["id"])
        name = f"{identifier}.wav"
        write_wav(Path(out) / "wavs" / name, to_pcm16(row["audio"]["bytes"]))
        rows.append((identifier, f"wavs/{name}", row["transcription"]))
    write_set(out, rows)


def write_set(out, rows):
    rows.sort()
    lines = [f"{identifier}\t{path}\t{' '.join(text.split())}" for identifier, path, text in rows]
    (Path(out) / "set.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(rows)} utterances -> {Path(out) / 'set.tsv'}")


def read_set(path, limit):
    rows = []
    base = Path(path).parent
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        identifier, relative, reference = line.split("\t", 2)
        rows.append((identifier, base / relative, reference))
    return rows[:limit] if limit else rows


# ---- 识别 ----


class Helper:
    """msime-voice-local 的一个进程，模型加载一次，逐条识别。"""

    def __init__(self, helper, runtime):
        self.process = subprocess.Popen(
            [helper, "--runtime", runtime, "--idle-exit", "0"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        hello = self.receive()
        if not hello.get("available"):
            raise SystemExit(f"runtime unavailable: {hello.get('error')}")
        self.next_id = 1

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def receive(self):
        line = self.process.stdout.readline()
        if not line:
            raise SystemExit("msime-voice-local exited")
        return json.loads(line)

    def recognize(self, model, pcm):
        identifier = self.next_id
        self.next_id += 1
        self.send({"op": "start", "id": identifier, "model": model, "language": "zh-CN"})
        started = self.receive()
        if started.get("type") != "started":
            raise SystemExit(f"start failed: {started}")
        # 每次 100 毫秒，与移动端采集的块大小一致。
        chunk = SAMPLE_RATE // 10 * 2
        for offset in range(0, len(pcm), chunk):
            self.send({"op": "audio", "pcm16": base64.b64encode(pcm[offset : offset + chunk]).decode()})
        finished_at = time.monotonic()
        self.send({"op": "finish"})
        while True:
            message = self.receive()
            if message.get("type") == "partial":
                continue
            if message.get("type") != "final":
                raise SystemExit(f"recognition failed: {message}")
            return message["text"], time.monotonic() - finished_at

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=30)


class Numbers:
    """宿主库的 msime_client_voice_normalize_numbers。"""

    def __init__(self, library, kind):
        self.library = ctypes.CDLL(library)
        self.library.msime_client_voice_normalize_numbers.argtypes = (ctypes.c_char_p, ctypes.c_size_t)
        self.library.msime_client_voice_normalize_numbers.restype = ctypes.c_void_p
        self.library.msime_client_string_free.argtypes = (ctypes.c_void_p,)
        self.kind = kind

    def __call__(self, text):
        request = json.dumps({"text": text, "kind": self.kind}).encode()
        pointer = self.library.msime_client_voice_normalize_numbers(request, len(request))
        try:
            response = json.loads(ctypes.string_at(pointer).decode())
        finally:
            self.library.msime_client_string_free(pointer)
        return response["value"]["text"] if response.get("ok") else text


def run(arguments):
    """逐条识别，每条的参考文本、识别结果和松手到出结果的时间写进 --hypotheses（JSON 行）。"""
    model = Path(arguments.model).resolve()
    manifest = json.loads((model / "msime-model.json").read_text(encoding="utf-8"))
    helper = Helper(arguments.helper, arguments.runtime)
    try:
        with open(arguments.hypotheses, "w", encoding="utf-8") as out:
            for set_path in arguments.set:
                name = Path(set_path).parent.name
                for identifier, wav_path, reference in read_set(set_path, arguments.limit):
                    with wave.open(str(wav_path)) as recording:
                        pcm = recording.readframes(recording.getnframes())
                    text, finish = helper.recognize(str(model), pcm)
                    row = {
                        "model": manifest["id"],
                        "kind": manifest["kind"],
                        "set": name,
                        "id": identifier,
                        "seconds": round(len(pcm) / 2 / SAMPLE_RATE, 3),
                        "finish_seconds": round(finish, 3),
                        "reference": reference,
                        "hypothesis": text,
                    }
                    out.write(json.dumps(row, ensure_ascii=False) + "\n")
                    out.flush()
    finally:
        helper.close()


def summarize(rows, numbers):
    """按模型和测试集汇总：原样打分，以及给了宿主库时数字规整后再打分。"""
    groups = {}
    for row in rows:
        groups.setdefault((row["model"], row["kind"], row["set"]), []).append(row)
    runs = {}
    for (model, kind, name), items in sorted(groups.items()):
        errors, units = score((row["reference"], row["hypothesis"]) for row in items)
        result = {
            "utterances": len(items),
            "audio_seconds": round(sum(row["seconds"] for row in items), 1),
            "units": units,
            "errors": errors,
            "mer": round(100 * errors / units, 2) if units else None,
        }
        if numbers:
            normalize = Numbers(numbers, kind)
            # 参考文本也过同一个规整：各测试集写数字的习惯不同（FLEURS 写阿拉伯数字，ASCEND 写汉字），两边都规整后比较的只是听对没有。
            errors_numbers, units_numbers = score((normalize(row["reference"]), normalize(row["hypothesis"])) for row in items)
            result["errors_numbers"] = errors_numbers
            result["units_numbers"] = units_numbers
            result["mer_numbers"] = round(100 * errors_numbers / units_numbers, 2) if units_numbers else None
        finish = sorted(row["finish_seconds"] for row in items)
        # 松手到出最终结果的时间，在跑评测的这台机器上；只适合同机前后比较。
        result["finish_seconds_median"] = finish[len(finish) // 2]
        runs.setdefault(model, {"model": model, "kind": kind, "sets": {}})["sets"][name] = result
    return list(runs.values())


def score_files(arguments):
    rows = []
    for path in arguments.hypotheses:
        for line in Path(path).read_text(encoding="utf-8").splitlines():
            if line.strip():
                rows.append(json.loads(line))
    runs = summarize(rows, arguments.host_library)
    for run_result in runs:
        for name, result in run_result["sets"].items():
            numbers = f"\t数字规整后 {result['mer_numbers']}%" if "mer_numbers" in result else ""
            print(f"{run_result['model']}\t{name}\tMER {result['mer']}%{numbers}\t({result['utterances']} 条)")
    if arguments.json:
        document = {"scoring": "voice_eval.py MER：汉字按字，拉丁字母与数字按词，不计标点和大小写", "runs": runs}
        Path(arguments.json).write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    commands = parser.add_subparsers(dest="command", required=True)
    fleurs = commands.add_parser("prepare-fleurs")
    fleurs.add_argument("tsv")
    fleurs.add_argument("archive")
    fleurs.add_argument("out")
    ascend = commands.add_parser("prepare-ascend")
    ascend.add_argument("parquet")
    ascend.add_argument("out")
    recognize = commands.add_parser("run")
    recognize.add_argument("--helper", required=True)
    recognize.add_argument("--runtime", required=True)
    recognize.add_argument("--model", required=True)
    recognize.add_argument("--set", action="append", required=True)
    recognize.add_argument("--hypotheses", required=True)
    recognize.add_argument("--limit", type=int, default=0)
    rescore = commands.add_parser("score")
    rescore.add_argument("hypotheses", nargs="+")
    rescore.add_argument("--host-library")
    rescore.add_argument("--json")
    arguments = parser.parse_args()
    if arguments.command == "prepare-fleurs":
        prepare_fleurs(arguments.tsv, arguments.archive, arguments.out)
    elif arguments.command == "prepare-ascend":
        prepare_ascend(arguments.parquet, arguments.out)
    elif arguments.command == "run":
        run(arguments)
    else:
        score_files(arguments)


if __name__ == "__main__":
    main()
