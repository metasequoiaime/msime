#!/usr/bin/env python3
"""鸿蒙两个沙箱的会话采用顺序（#8201 第 2 步，迁移差异 M2）。

服务端每次刷新都轮换刷新令牌，拿用过的那枚再刷新会撤销整个会话族。设置应用和键盘各有各的 `files/state`，另一边刷新过的会话要先经 Rust 的 `session_adopt` 采用过来，本沙箱才能发认证请求。`platforms/harmony/tests/account-requests.test.ts` 只测得到 `AccountSessionAdoptionPolicy.ts` 里的纯函数（`SerialGate`、`AdoptionTracker`、`adoptedByRust`）；把这些规则接到调用点的是 .ets 文件，那里改回去 tsc、node 测试都看不见，HAP 也没人在这里打包。这个检查盯住调用点：

1. 每个认证请求（`client.account`、`accountThread`、`community`、`cloudDictionary`、`cloudDictionarySnapshot`、`accountCloudClipboard`、`aiSkinGenerate`、`ensureAnonymousAccountWithAgent`）在 `AccountSessionAdoption.ets` 以外的调用，都排在同一个方法里、包住它的代码块里的 `await AccountSessionAdoption.beforeRequest()` 之后；调用在私有方法里时，改为要求本文件里这个私有方法的每个调用点都满足同一条（递归）。
2. `beforeRequest()` 每次都再排一次进程启动时登记的采用（`AdoptionGate.beforeRequest`），而不是只等已经排进来的：回到前台时网页在 `visibilitychange` 里发的请求可能早于窗口的 ACTIVE 事件，只等已排进来的就会漏掉前台那次采用。`AccountSessionAdoption.ets` 里的 `gate` 必须是 `AdoptionGate`，`adoptOnStart` 经 `gate.register` 登记，`beforeRequest` 原样转给 `gate.beforeRequest()`；`AdoptionGate` 本身的语义由 `account-requests.test.ts` 测。
3. 两个 ability 的 `onCreate` 先 `AccountSessionAdoption.adoptOnStart`、再 `HarmonyAccountIdentity.ensureAnonymous`，采用方向分别是 `keyboard`（设置应用）和 `settings`（键盘）；没有登记过的 `beforeRequest()` 只等已经排进来的采用，顺序反了匿名注册就不等。
4. 回到前台时两边都再排一次采用：`Settings.syncShared` 采用键盘的，`KeyboardSession.syncSharedState` 采用设置应用的。它让本地显示的账号状态尽早跟上，认证请求的顺序不靠它。
5. 两份会话和匿名账号的身份都不按文件复制导入：`SharedStateSync.accountFileDecision` 交给 `SharedStateMirrorPolicy.importDecision`（规则由 `shared-state-mirror-policy.test.ts` 测），两个 `beforeFile` 都落到它；身份随匿名会话由 Rust 采用（迁移差异 M40），所以 `adoptOne` 必须把身份文件的路径 `identityPath` 交给 `sessionAdoptRequest`，否则 Rust 不动身份、它就再也没人导入。

文本检查不是编译器：它不认识动态调用和别名，所以第 6 条要求所有 `libmsimeclient.so` 的导入都叫 `client`，不能换个名字绕过第 1 条。
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ETS = ROOT / "platforms/harmony/entry/src/main/ets"

# 会用到本沙箱会话（可能刷新令牌）的 NAPI 入口。`accountStatus` 只读本地文件，`aiSkinCancel` 只取消，不在其中。
AUTHENTICATED = (
    "account",
    "accountThread",
    "community",
    "cloudDictionary",
    "cloudDictionarySnapshot",
    "accountCloudClipboard",
    "aiSkinGenerate",
    "ensureAnonymousAccountWithAgent",
)
CALL = re.compile(r"\bclient\.(" + "|".join(AUTHENTICATED) + r")\s*\(")
SETTLED = re.compile(r"\bawait\s+AccountSessionAdoption\.beforeRequest\s*\(\s*\)")
IMPORT = re.compile(r"^\s*import\s+(.+?)\s+from\s+['\"]libmsimeclient\.so['\"]", re.M)
METHOD = re.compile(
    r"^(?:@\w+(?:\([^)]*\))?\s+)*"
    r"(?:(?:public|private|protected|static|async|override)\s+)*"
    r"(\w+)\s*(?:<[^>]*>)?\s*\("
)
NOT_METHODS = {"if", "for", "while", "switch", "catch", "function", "return", "await", "new", "else"}
ADOPTION_FILE = "account/AccountSessionAdoption.ets"


def mask(text: str) -> tuple[str, str]:
    """返回两份与原文等长的文本：去掉注释的（字符串留着），和注释、字符串内容都换成空格的（用来配括号）。换行一律保留，位置与原文一一对应。"""
    no_comments = list(text)
    bare = list(text)
    index = 0
    length = len(text)
    while index < length:
        char = text[index]
        pair = text[index:index + 2]
        if pair == "//":
            end = text.find("\n", index)
            end = length if end < 0 else end
            for position in range(index, end):
                no_comments[position] = " "
                bare[position] = " "
            index = end
        elif pair == "/*":
            end = text.find("*/", index + 2)
            end = length if end < 0 else end + 2
            for position in range(index, end):
                if text[position] != "\n":
                    no_comments[position] = " "
                    bare[position] = " "
            index = end
        elif char in "'\"`":
            position = index + 1
            while position < length and text[position] != char:
                if text[position] == "\\":
                    position += 1
                elif char != "`" and text[position] == "\n":
                    break
                position += 1
            for inner in range(index + 1, min(position, length)):
                if text[inner] != "\n":
                    bare[inner] = " "
            index = position + 1
        elif char == "/" and regex_may_start(text, index):
            # 正则字面量（`/\r$/`、`/['"]/`）里的引号和括号不算数。
            position = index + 1
            in_class = False
            while position < length and text[position] != "\n":
                current = text[position]
                if current == "\\":
                    position += 2
                    continue
                if current == "[":
                    in_class = True
                elif current == "]":
                    in_class = False
                elif current == "/" and not in_class:
                    break
                position += 1
            for inner in range(index + 1, min(position, length)):
                bare[inner] = " "
            index = position + 1
        else:
            index += 1
    return "".join(no_comments), "".join(bare)


def regex_may_start(text: str, index: int) -> bool:
    """`/` 前面最近的非空白字符是运算符或开括号时，它开始的是正则字面量而不是除号。"""
    position = index - 1
    while position >= 0 and text[position] in " \t\r\n":
        position -= 1
    if position < 0:
        return True
    previous = text[position]
    if previous in "(,=:[!&|?{};+-*%<>~^":
        return True
    word = re.search(r"(\w+)$", text[:position + 1])
    return word is not None and word.group(1) in {"return", "typeof", "case", "in", "of"}


@dataclass
class Block:
    start: int  # `{` 的位置
    end: int  # 配对的 `}` 的位置
    header: str
    parent: "Block | None"

    def method(self) -> str | None:
        """这个代码块是不是方法体：是就返回方法名。箭头函数、if、try 等都不算，采用的等待可以在它们外面。"""
        header = self.header.strip()
        if header.endswith("=>"):
            return None
        match = METHOD.match(header)
        if match is None or match.group(1) in NOT_METHODS:
            return None
        return match.group(1)

    def contains(self, position: int) -> bool:
        return self.start < position < self.end


class Source:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.relative = path.relative_to(ETS).as_posix()
        self.text = path.read_text(encoding="utf-8")
        self.code, self.bare = mask(self.text)
        self.blocks: list[Block] = []
        self.balanced = self._parse()

    def _parse(self) -> bool:
        stack: list[Block] = []
        boundary = 0
        for position, char in enumerate(self.bare):
            if char == "{":
                parent = stack[-1] if stack else None
                block = Block(position, -1, self.bare[boundary:position], parent)
                stack.append(block)
                self.blocks.append(block)
                boundary = position + 1
            elif char == "}":
                if not stack:
                    return False
                stack.pop().end = position
                boundary = position + 1
            elif char == ";":
                boundary = position + 1
        return not stack

    def line(self, position: int) -> int:
        return self.text.count("\n", 0, position) + 1

    def innermost(self, position: int) -> Block | None:
        found: Block | None = None
        for block in self.blocks:
            if block.contains(position) and (found is None or block.start > found.start):
                found = block
        return found

    def enclosing_method(self, position: int) -> Block | None:
        block = self.innermost(position)
        while block is not None and block.method() is None:
            block = block.parent
        return block

    def dominated(self, earlier: int, later: int) -> bool:
        """`earlier` 在 `later` 之前，并且 `earlier` 所在的代码块也包住 `later`：不在另一个分支里。"""
        if earlier >= later:
            return False
        block = self.innermost(earlier)
        return block is None or block.contains(later)


def covered(source: Source, position: int, visiting: set[tuple[str, int]]) -> str | None:
    """这个位置是否排在采用之后。是就返回 None，不是就返回原因。"""
    method = source.enclosing_method(position)
    if method is None:
        return "not inside a method"
    for match in SETTLED.finditer(source.code, method.start, position):
        if source.dominated(match.start(), position):
            return None
    name = method.method()
    if "private" not in method.header.split():
        return f"{name}() does not await AccountSessionAdoption.beforeRequest() before it and is not private"
    key = (name, method.start)
    if key in visiting:
        return f"{name}() is reached recursively"
    callers = [
        match.start()
        for match in re.finditer(r"\b(?:this|[A-Z]\w*)\." + re.escape(name) + r"\s*\(", source.code)
        if not method.contains(match.start())
    ]
    if not callers:
        return f"private {name}() awaits nothing and has no caller in this file"
    for caller in callers:
        reason = covered(source, caller, visiting | {key})
        if reason is not None:
            return f"{name}() called at line {source.line(caller)}: {reason}"
    return None


def collapse(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def method_body(source: Source, name: str) -> tuple[Block, str] | None:
    for block in source.blocks:
        if block.method() == name:
            return block, source.code[block.start + 1:block.end]
    return None


def check_calls(sources: list[Source], problems: list[str]) -> int:
    count = 0
    for source in sources:
        if source.relative == ADOPTION_FILE:
            continue
        for match in CALL.finditer(source.code):
            count += 1
            reason = covered(source, match.start(), set())
            if reason is not None:
                problems.append(
                    f"{source.relative}:{source.line(match.start())}: client.{match.group(1)} "
                    f"is not ordered after the session adoption ({reason})"
                )
    if count == 0:
        problems.append("no authenticated client call found: the pattern no longer matches the bridge")
    return count


def check_imports(sources: list[Source], problems: list[str]) -> None:
    for source in sources:
        for match in IMPORT.finditer(source.code):
            default = match.group(1).split(",")[0].strip()
            if default != "client":
                problems.append(
                    f"{source.relative}:{source.line(match.start())}: libmsimeclient.so must be imported as "
                    f"`client` so this check sees its calls (got `{match.group(1)}`)"
                )


def check_start(source: Source, publisher: str, problems: list[str]) -> None:
    found = method_body(source, "onCreate")
    if found is None:
        problems.append(f"{source.relative}: onCreate not found")
        return
    block, _ = found
    adopt = re.compile(r"\bAccountSessionAdoption\.adoptOnStart\s*\(([^;]*?)\)\s*;", re.S)
    anonymous = re.compile(r"\bHarmonyAccountIdentity\.ensureAnonymous\s*\(")
    starts = [m for m in adopt.finditer(source.code, block.start, block.end)]
    ensures = [m.start() for m in anonymous.finditer(source.code, block.start, block.end)]
    if not starts:
        problems.append(f"{source.relative}: onCreate does not queue AccountSessionAdoption.adoptOnStart")
        return
    if not ensures:
        problems.append(f"{source.relative}: onCreate no longer registers the anonymous account")
    first = starts[0]
    if f"'{publisher}'" not in first.group(1):
        problems.append(f"{source.relative}: adoptOnStart must adopt what '{publisher}' publishes")
    for position in ensures:
        if not source.dominated(first.start(), position):
            problems.append(
                f"{source.relative}:{source.line(position)}: ensureAnonymous runs before adoptOnStart is queued"
            )
    # 别处的匿名注册不受 onCreate 的顺序保护。
    for other in anonymous.finditer(source.code):
        if not block.contains(other.start()):
            problems.append(f"{source.relative}:{source.line(other.start())}: ensureAnonymous outside onCreate")


def check_foreground(source: Source, method: str, publisher: str, problems: list[str]) -> None:
    found = method_body(source, method)
    if found is None:
        problems.append(f"{source.relative}: {method}() not found")
        return
    block, body = found
    pattern = re.compile(
        r"\bAccountSessionAdoption\.adopt\s*\(\s*this\.sharedMirror\s*,\s*this\.stateDirectory\s*,\s*'"
        + publisher + r"'\s*\)"
    )
    if pattern.search(body) is None:
        problems.append(f"{source.relative}: {method}() does not queue an adoption from '{publisher}'")
    callers = [
        m for m in re.finditer(r"\bthis(?:\.bridge)?\." + method + r"\s*\(", source.code)
        if not block.contains(m.start())
    ]
    if not callers:
        problems.append(f"{source.relative}: nothing calls {method}() any more")


def check_adoption_wiring(source: Source, problems: list[str]) -> None:
    """`beforeRequest()` 必须每次都再排一次登记过的采用：只等已经排进来的，回到前台时比 ACTIVE 事件先到的请求就不等前台那次采用。"""
    if re.search(r"\bprivate\s+static\s+readonly\s+gate\s*:\s*AdoptionGate\s*=\s*new\s+AdoptionGate\s*\(\s*\)\s*;", source.code) is None:
        problems.append(f"{source.relative}: the adoption queue must be an AdoptionGate")
    found = method_body(source, "beforeRequest")
    if found is None:
        problems.append(f"{source.relative}: beforeRequest() not found")
    elif collapse(found[1]) != "return AccountSessionAdoption.gate.beforeRequest();":
        problems.append(
            f"{source.relative}: beforeRequest() must queue the registered adoption via gate.beforeRequest() (got `{collapse(found[1])}`)"
        )
    start = method_body(source, "adoptOnStart")
    if start is None:
        problems.append(f"{source.relative}: adoptOnStart() not found")
    elif re.search(r"\bAccountSessionAdoption\.gate\.register\s*\(", start[1]) is None:
        problems.append(f"{source.relative}: adoptOnStart() must register the adoption so every request re-runs it")


def check_import_decision(source: Source, problems: list[str]) -> None:
    found = method_body(source, "accountFileDecision")
    if found is None:
        problems.append(f"{source.relative}: accountFileDecision() not found")
        return
    _, body = found
    expected = "return SharedStateMirrorPolicy.importDecision(name);"
    if collapse(body) != expected:
        problems.append(
            f"{source.relative}: accountFileDecision() must hand sessions and the anonymous identity to importDecision (got `{collapse(body)}`)"
        )
    for side, importer in (("settingsSync", "settings"), ("keyboardSync", "keyboard")):
        side_found = method_body(source, side)
        if side_found is None:
            problems.append(f"{source.relative}: {side}() not found")
            continue
        _, side_body = side_found
        decision = re.search(r"beforeFile\s*:.*?\n\s*\},", side_body, re.S)
        call = "return SharedStateSync.accountFileDecision(name);"
        if decision is None or call not in decision.group(0):
            problems.append(f"{source.relative}: {side}() beforeFile no longer falls through to accountFileDecision ({importer} side)")


def check_identity_path(source: Source, problems: list[str]) -> None:
    """身份不再按文件复制，只随匿名会话由 Rust 采用：`adoptOne` 不把身份文件的路径交给 `session_adopt` 时 Rust 不动身份，设置应用采用了键盘的匿名会话却留着自己的身份。"""
    found = method_body(source, "adoptOne")
    if found is None:
        problems.append(f"{source.relative}: adoptOne() not found")
        return
    _, body = found
    if re.search(r"\bsessionAdoptRequest\s*\(\s*stateDirectory\s*,\s*path\s*,\s*session\.item\s*,\s*publisher\s*,\s*identityPath\s*\)", body) is None:
        problems.append(f"{source.relative}: adoptOne() must pass the published identity path to sessionAdoptRequest")
    if re.search(r"\bmirror\.publishedFile\s*\(\s*publisher\s*,\s*identityEntry\s*\)", body) is None:
        problems.append(f"{source.relative}: adoptOne() must take the identity path from the publisher's mirror entry")


def main() -> int:
    if not ETS.is_dir():
        print("harmony session adoption order: skipped, no Harmony sources")
        return 0
    paths = sorted(list(ETS.rglob("*.ets")) + list(ETS.rglob("*.ts")))
    sources = [Source(path) for path in paths]
    problems: list[str] = [f"{s.relative}: braces do not balance after masking" for s in sources if not s.balanced]
    by_name = {source.relative: source for source in sources}

    count = check_calls(sources, problems)
    check_imports(sources, problems)
    check_adoption_wiring(by_name[ADOPTION_FILE], problems)
    check_identity_path(by_name[ADOPTION_FILE], problems)
    check_start(by_name["entryability/EntryAbility.ets"], "keyboard", problems)
    check_start(by_name["inputmethodextability/KeyboardExtensionAbility.ets"], "settings", problems)
    check_foreground(by_name["pages/Settings.ets"], "syncShared", "keyboard", problems)
    check_foreground(by_name["keyboard/KeyboardSession.ets"], "syncSharedState", "settings", problems)
    check_import_decision(by_name["keyboard/SharedStateSync.ets"], problems)

    if problems:
        for problem in problems:
            print(f"harmony session adoption order: {problem}")
        return 1
    print(f"harmony session adoption order: {count} authenticated calls queue and wait for their own adoption; both sandboxes register it at start and queue it on foreground")
    return 0


if __name__ == "__main__":
    sys.exit(main())
