#!/usr/bin/env python3
"""Linux IBus 宿主里，直接挂在 glib 主循环上的回调读 Engine 视图前必须先确认它是对象。

`State::view` 在会话打开之前是 JSON null，`State::close()` 也会把它置回 null：英文模式、密码框这类被拦截的输入框、偏好保存触发的会话重建、`guarded()` 的兜底都会走到。nlohmann 的 `value()` / `at()` 对 null 直接抛 `type_error`。按键、焦点这些入口都包在 `guarded()` 里，异常最多让这次操作失败；而 `g_timeout_add*` / `g_idle_add*` 注册的回调外面什么都没有，异常会一路冒到 `entrypoints/ibus_main.cpp` 的 `std::set_terminate`，整个宿主进程 abort。

#6675 就是这样：输入模式提示的 1.2 秒定时器在这段时间里遇到会话被关掉，读 null 视图抛 `type_error.306`，Ubuntu 上两天崩了 65 次。`test-linux-rendered-view-guard.py` 只看 `rendered_view`，这次崩的是另一个成员。

这里静态检查每个以 lambda 注册的 glib 回调体：其中对 `s.view` / `state(...).view` 的 `value(` 与 `at(` 读取，要么在同一个回调体里、读取之前出现过 `s.view.is_object()`，要么位于回调体内的 `guarded(...)` 调用之中。以命名函数注册的回调（如 `apply_candidate_hide`、`reload_preferences`）和回调间接调用的函数不在检查范围内，它们的读取要靠代码评审；复现这类时序需要真实的 IBus 会话，本地门禁里没有。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "platforms/linux/src/core/ClientEngine.cpp"

text = SOURCE.read_text()


def blank_literals(source: str) -> str:
    """把注释和字符串、字符字面量换成等长空白，保留换行，便于按偏移数括号。"""
    out = []
    i = 0
    n = len(source)
    while i < n:
        c = source[i]
        if source.startswith("//", i):
            j = source.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif source.startswith("/*", i):
            j = source.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(re.sub(r"[^\n]", " ", source[i:j]))
            i = j
        elif c == '"' or c == "'":
            j = i + 1
            while j < n and source[j] != c:
                j += 2 if source[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(c + " " * (j - i - 2) + c if j - i >= 2 else " " * (j - i))
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


code = blank_literals(text)
assert len(code) == len(text)


def matching(open_index: int, opener: str, closer: str) -> int:
    depth = 0
    for index in range(open_index, len(code)):
        if code[index] == opener:
            depth += 1
        elif code[index] == closer:
            depth -= 1
            if depth == 0:
                return index
    raise SystemExit(f"{SOURCE}: unbalanced {opener}{closer} at offset {open_index}")


def line_of(offset: int) -> int:
    return text.count("\n", 0, offset) + 1


REGISTRATION = re.compile(r"\bg_(?:timeout|idle)_add(?:_full|_seconds)?\s*\(")
LAMBDA = re.compile(r"\[[^\]]*\]\s*\([^)]*\)[^{;]*\{")
READ = re.compile(r"(?<![\w.])(?:s|state\([^()]*\))\.view\.(?:value|at)\(")
GUARD = re.compile(r"(?<![\w.])(?:s|state\([^()]*\))\.view\.is_object\(\)")
GUARDED_CALL = re.compile(r"(?<![\w.])guarded\s*\(")

bodies = []
for registration in REGISTRATION.finditer(code):
    call_open = registration.end() - 1
    call_close = matching(call_open, "(", ")")
    lam = LAMBDA.search(code, call_open, call_close)
    if not lam:
        continue
    body_open = lam.end() - 1
    bodies.append((body_open, matching(body_open, "{", "}")))

if len(bodies) < 8:
    sys.exit(f"expected the IBus host to register many glib lambda callbacks, found {len(bodies)}")

violations = []
reads = 0
for start, end in bodies:
    body = code[start:end]
    guarded_spans = []
    for call in GUARDED_CALL.finditer(body):
        open_index = start + call.end() - 1
        guarded_spans.append((open_index, matching(open_index, "(", ")")))
    for read in READ.finditer(body):
        reads += 1
        offset = start + read.start()
        if any(a < offset < b for a, b in guarded_spans):
            continue
        if GUARD.search(code, start, offset):
            continue
        line = line_of(offset)
        violations.append((line, text.split("\n")[line - 1].strip()))

if violations:
    for number, line in violations:
        print(f"{SOURCE}:{number}: Engine view read in a glib callback without s.view.is_object(): {line}")
    sys.exit(
        "State::view is null before a session opens and after close(); value()/at() on it "
        "throws, and a glib callback has no guarded() to catch it, so the host aborts (#6675)"
    )
print(f"linux glib callback view guard: {len(bodies)} callbacks, {reads} guarded view reads")
