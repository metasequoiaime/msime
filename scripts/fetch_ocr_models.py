#!/usr/bin/env python3
"""准备 Android 离线 OCR 模块打包用的模型资产，成功时只向 stdout 输出资产目录的绝对路径。

模型来自 `resources/ocr-models-android.lock.json` 固定的 Hugging Face 仓库提交，逐文件限长并校验 SHA-256；不匹配的下载在落盘前丢弃。CTC 字典取自识别模型同一提交的 `inference.yml`，抽出后再按锁文件核对条目数和 SHA-256，检测模型 `inference.yml` 的后处理参数也必须等于锁文件记录、`OcrPipeline.java` 写死的那组值。这份锁只供构建使用，运行时的信任依据仍是 `resources/ocr-module-android.lock.json` 固定的整个模块 APK 摘要。

输出目录（默认 `target/android/ocr-module-assets`）下只有 `ocr/det.onnx`、`ocr/rec.onnx` 和 `ocr/dict.txt`，由 `platforms/android/gradle-app/ocr-module` 作为 assets 目录打进模块；下载缓存（默认 `target/ocr-models`）不进 APK。缓存已按锁校验时不联网；`MSIME_OCR_MODELS_OFFLINE=1` 或 `--offline` 禁止下载，缺少或损坏的输入直接失败。

usage: fetch_ocr_models.py [--out <目录>] [--cache <目录>] [--offline]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile

# 以 `python3 -I` 运行时 sys.path 不含脚本目录；同目录的 download_retry 要显式加回来。
sys.path.insert(0, str(Path(__file__).resolve().parent))
import download_retry  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/ocr-models-android.lock.json"
CHUNK = 1 << 20
# 单个模型文件的上限；固定的 tiny 模型都在 5 MB 以内，上限只用来拒绝异常的锁和响应。
MAX_MODEL_BYTES = 64 * 1024 * 1024
MODEL_NAMES = {"det.onnx", "det.yml", "rec.onnx", "rec.yml"}
ASSETS = ("det.onnx", "rec.onnx")


def read_lock(path: Path = LOCK) -> dict:
    lock = json.loads(path.read_text(encoding="utf-8"))
    if lock.get("version") != 1:
        raise ValueError("OCR 模型锁的版本必须是 1")
    runtime = lock["runtime"]
    if (not re.fullmatch(r"com\.microsoft\.onnxruntime:onnxruntime-android:\d+\.\d+\.\d+", runtime["maven"])
            or not re.fullmatch(r"[0-9a-f]{40}", runtime["commit"])):
        raise ValueError("OCR 模型锁必须固定 onnxruntime-android 的版本和来源提交")
    names = set()
    for entry in lock["models"]:
        if (entry["name"] not in MODEL_NAMES or entry["name"] in names
                or not re.fullmatch(r"PaddlePaddle/[A-Za-z0-9_.-]+", entry["repository"])
                or not re.fullmatch(r"[0-9a-f]{40}", entry["commit"])
                or entry["path"] not in ("inference.onnx", "inference.yml")
                or type(entry["size"]) is not int or not 0 < entry["size"] <= MAX_MODEL_BYTES
                or not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"])
                or entry["license"] != "Apache-2.0"):
            raise ValueError(f"无效的 OCR 模型记录：{entry.get('name')}")
        names.add(entry["name"])
    if names != MODEL_NAMES:
        raise ValueError("OCR 模型锁必须且只能列出 det.onnx、det.yml、rec.onnx、rec.yml")
    dictionary = lock["dictionary"]
    if (dictionary["name"] != "dict.txt" or dictionary["source"] != "rec.yml"
            or type(dictionary["entries"]) is not int or dictionary["entries"] <= 0
            or not re.fullmatch(r"[0-9a-f]{64}", dictionary["sha256"])):
        raise ValueError("无效的 OCR 字典记录")
    detection = lock["detection"]
    if set(detection) != {"thresh", "box_thresh", "unclip_ratio"}:
        raise ValueError("OCR 检测参数必须是 thresh、box_thresh、unclip_ratio")
    for entry in lock["licenses"]:
        if (not entry["path"].startswith("resources/licenses/")
                or not entry["url"].startswith("https://raw.githubusercontent.com/")
                or not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"])):
            raise ValueError(f"无效的 OCR 许可证记录：{entry.get('path')}")
    return lock


def model_url(entry: dict) -> str:
    return f"https://huggingface.co/{entry['repository']}/resolve/{entry['commit']}/{entry['path']}"


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def matches(path: Path, entry: dict) -> bool:
    return path.is_file() and path.stat().st_size == entry["size"] and digest(path) == entry["sha256"]


def download(entry: dict, destination: Path) -> None:
    """限长下载到同目录的暂存文件，校验通过才原子替换；失败不留下暂存文件，也不动已有文件。"""
    url = model_url(entry)
    print(f"  fetching {entry['repository']}@{entry['commit'][:8]}/{entry['path']} ({entry['size'] / 1e6:.1f} MB)",
          file=sys.stderr)
    destination.parent.mkdir(parents=True, exist_ok=True)
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with download_retry.urlopen(url, timeout=300) as response:
                size = 0
                while block := response.read(CHUNK):
                    size += len(block)
                    if size > entry["size"]:
                        raise ValueError(f"{entry['name']}: 响应超过锁文件记录的长度")
                    staged.write(block)
        actual = digest(staged_path)
        size = staged_path.stat().st_size
        if actual != entry["sha256"] or size != entry["size"]:
            raise ValueError(f"{entry['name']}: 期望 {entry['sha256']}（{entry['size']} 字节），实际 {actual}（{size} 字节）")
        staged_path.replace(destination)
    except BaseException:
        # with 块已关闭文件后再删：Windows 拒绝删除打开的文件，PermissionError 会盖住真正的错误。
        staged_path.unlink(missing_ok=True)
        raise


def post_process(text: str) -> dict:
    """读出 `inference.yml` 顶层 `PostProcess:` 下缩进两格的标量键，只接受这种固定形状。"""
    lines = text.split("\n")
    try:
        start = lines.index("PostProcess:")
    except ValueError:
        raise ValueError("inference.yml 缺少顶层 PostProcess") from None
    values = {}
    for line in lines[start + 1:]:
        if not line.startswith("  "):
            break
        match = re.fullmatch(r"  ([a-z_]+): (\S.*)", line)
        if match:
            values[match.group(1)] = match.group(2)
    return values


def check_detection(text: str, expected: dict) -> None:
    values = post_process(text)
    if values.get("name") != "DBPostProcess":
        raise ValueError("检测模型的后处理不是 DBPostProcess")
    for key, value in expected.items():
        if key not in values or float(values[key]) != float(value):
            raise ValueError(f"检测模型的 {key} 是 {values.get(key)}，锁文件记录的是 {value}")


# 双引号标量只允许这些转义；遇到别的转义说明上游换了写法，宁可失败也不猜。
DOUBLE_QUOTED_ESCAPES = {"\\": "\\", '"': '"', "/": "/", "t": "\t", " ": " ", "_": " ", "0": "\0"}


def scalar(raw: str) -> str:
    if raw.startswith("'"):
        if len(raw) < 2 or not raw.endswith("'"):
            raise ValueError(f"未闭合的单引号标量：{raw!r}")
        inner = raw[1:-1]
        if "'" in inner.replace("''", ""):
            raise ValueError(f"单引号标量里有未转义的引号：{raw!r}")
        return inner.replace("''", "'")
    if raw.startswith('"'):
        if len(raw) < 2 or not raw.endswith('"'):
            raise ValueError(f"未闭合的双引号标量：{raw!r}")
        result, index, inner = [], 0, raw[1:-1]
        while index < len(inner):
            char = inner[index]
            if char == '"':
                raise ValueError(f"双引号标量里有未转义的引号：{raw!r}")
            if char != "\\":
                result.append(char)
                index += 1
                continue
            code = inner[index + 1:index + 2]
            if code in DOUBLE_QUOTED_ESCAPES:
                result.append(DOUBLE_QUOTED_ESCAPES[code])
                index += 2
            elif code in ("x", "u", "U"):
                width = {"x": 2, "u": 4, "U": 8}[code]
                digits = inner[index + 2:index + 2 + width]
                if not re.fullmatch(f"[0-9A-Fa-f]{{{width}}}", digits):
                    raise ValueError(f"无效的转义：{raw!r}")
                result.append(chr(int(digits, 16)))
                index += 2 + width
            else:
                raise ValueError(f"不支持的转义：{raw!r}")
        return "".join(result)
    # 普通标量：上游用 yaml.dump 生成，会被解析成非字符串（数字、`~`）或以指示符开头的值都带了引号。
    # YAML 只把 ASCII 空格和制表符当空白，全角空格 U+3000 这类字符可以是普通标量。
    if (len(raw) != 1 or raw in " \t0123456789~"
            or raw in "-?:,[]{}#&*!|>'\"%@`"):
        raise ValueError(f"意外的普通标量：{raw!r}")
    return raw


def dictionary(text: str) -> list:
    """抽出识别模型 `PostProcess.character_dict` 的条目，顺序即 CTC 类别序号减一。"""
    lines = text.split("\n")
    try:
        start = lines.index("PostProcess:")
    except ValueError:
        raise ValueError("inference.yml 缺少顶层 PostProcess") from None
    if post_process(text).get("name") != "CTCLabelDecode":
        raise ValueError("识别模型的后处理不是 CTCLabelDecode")
    try:
        begin = lines.index("  character_dict:", start + 1)
    except ValueError:
        raise ValueError("识别模型缺少 character_dict") from None
    entries = []
    for line in lines[begin + 1:]:
        if not line.startswith("  - "):
            if line.startswith("    "):
                raise ValueError(f"character_dict 里出现嵌套结构：{line!r}")
            break
        value = scalar(line[4:])
        # 每个条目都是一个字符；多字符条目会让 CTC 的类别数与 Java 侧按行读出的字典对不上。
        if len(value) != 1 or value == "\n" or value == "\r":
            raise ValueError(f"字典条目必须是单个字符：{value!r}")
        entries.append(value)
    if not entries:
        raise ValueError("character_dict 为空")
    return entries


def prepare(lock: dict, out: Path, cache: Path, *, offline: bool) -> Path:
    cache.mkdir(parents=True, exist_ok=True)
    files = {}
    for entry in lock["models"]:
        target = cache / entry["commit"] / entry["name"]
        if not matches(target, entry):
            if offline:
                error = ValueError if target.exists() else FileNotFoundError
                raise error(f"OCR 模型缺少或校验失败：{target}；请联网运行 scripts/fetch_ocr_models.py 准备缓存")
            download(entry, target)
        files[entry["name"]] = target
    check_detection(files["det.yml"].read_text(encoding="utf-8"), lock["detection"])
    entries = dictionary(files["rec.yml"].read_text(encoding="utf-8"))
    data = ("\n".join(entries) + "\n").encode("utf-8")
    expected = lock["dictionary"]
    if len(entries) != expected["entries"] or hashlib.sha256(data).hexdigest() != expected["sha256"]:
        raise ValueError(f"抽出的字典有 {len(entries)} 条、SHA-256 {hashlib.sha256(data).hexdigest()}，"
                         f"锁文件记录的是 {expected['entries']} 条、{expected['sha256']}")
    assets = out / "ocr"
    assets.mkdir(parents=True, exist_ok=True)
    # 资产目录整个进 APK，只保留这三份文件，旧版本留下的其他文件一律删掉。
    for stale in assets.iterdir():
        if stale.name not in (*ASSETS, expected["name"]):
            if stale.is_dir() and not stale.is_symlink():
                shutil.rmtree(stale)
            else:
                stale.unlink()
    for name in ASSETS:
        entry = next(model for model in lock["models"] if model["name"] == name)
        if not matches(assets / name, entry):
            with tempfile.NamedTemporaryFile(dir=assets, delete=False) as handle:
                staged = Path(handle.name)
            try:
                shutil.copyfile(files[name], staged)
                staged.replace(assets / name)
            except BaseException:
                staged.unlink(missing_ok=True)
                raise
    target = assets / expected["name"]
    if not target.is_file() or target.read_bytes() != data:
        with tempfile.NamedTemporaryFile(dir=assets, delete=False) as handle:
            handle.write(data)
            staged = Path(handle.name)
        staged.replace(target)
    return out.resolve()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=ROOT / "target/android/ocr-module-assets")
    parser.add_argument("--cache", type=Path, default=ROOT / "target/ocr-models")
    parser.add_argument("--offline", action="store_true")
    arguments = parser.parse_args()
    offline = arguments.offline or os.environ.get("MSIME_OCR_MODELS_OFFLINE") == "1"
    print(prepare(read_lock(), arguments.out, arguments.cache, offline=offline))


if __name__ == "__main__":
    main()
