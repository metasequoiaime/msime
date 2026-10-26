#!/usr/bin/env python3
"""用 Python 的 fractions 和 ast 独立核对 calc 插件的 msime-pack run 输出，由 scripts/test-script-plugins.sh 在 check_run.py 之后调用。

expected.jsonl 只能说明输出和上次提交的一样；这里换一种做法把每个算式重新算一遍，说明它是对的：

- 数按正则切出来（整数部分可以按三位用 `,` 分组），换成 `Fraction`，算式交给 Python 的 `ast` 解析：运算符优先级、结合性和一元正负号都按 Python 的语法，`%` 按插件的规则取截断余数（符号跟被除数），不用 Python 自己的向下取整余数；
- 有结果时，第 1 行是结果（精确时注释「结果」，否则四舍五入到 10 位小数、注释「约等于」），有二元运算符时第 2 行是「算式=结果」（不精确时用 `≈`，超过 64 个 UTF-16 码元时没有这一行），整数部分 5 位以上时最后一行是千分位写法；
- 空算式、写错、除以 0 时没有任何行；插件报 `skip: overflow` 时，这里要看到确实有超过 10^9 的中间值，免得小数也被当成溢出放过去。

usage: check_output.py <fixture.jsonl> <granted.jsonl>
"""
import ast
import json
import re
import sys
from fractions import Fraction
from pathlib import Path

NUMBER = re.compile(r"\d{1,3}(?:,\d{3})+(?:\.\d*)?|\d+(?:\.\d*)?|\.\d+")
DIGITS = 10
LARGE = 10**9


class NoResult(Exception):
    pass


def read(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def to_fraction(token: str) -> Fraction:
    whole, _, fraction = token.replace(",", "").partition(".")
    return Fraction(int(whole or "0") * 10 ** len(fraction) + int(fraction or "0"), 10 ** len(fraction))


def evaluate(expression: str) -> tuple[Fraction, int, int]:
    """返回结果、二元运算符个数和中间值里最大的分子或分母。写错、除以 0 时抛 NoResult。"""
    values: list[Fraction] = []

    def substitute(match: re.Match) -> str:
        values.append(to_fraction(match.group(0)))
        return f"v{len(values) - 1}"

    source = NUMBER.sub(substitute, expression)
    # 切完数以后只能剩下运算符、括号和占位名；别的（`=`、多余的 `,` 和 `.`）都是写错。
    if re.search(r"[^v0-9+\-*/%() ]", source) or re.search(r"v\d+v|\)v|v\(", source):
        raise NoResult("syntax")
    try:
        tree = ast.parse(source, mode="eval")
    except SyntaxError:
        raise NoResult("syntax") from None
    largest = 0
    operators = 0

    def walk(node: ast.AST) -> Fraction:
        nonlocal largest, operators
        if isinstance(node, ast.Name):
            value = values[int(node.id[1:])]
        elif isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.USub, ast.UAdd)):
            value = walk(node.operand)
            value = -value if isinstance(node.op, ast.USub) else value
        elif isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub, ast.Mult, ast.Div, ast.Mod)):
            left, right = walk(node.left), walk(node.right)
            operators += 1
            if isinstance(node.op, (ast.Div, ast.Mod)) and right == 0:
                raise NoResult("divide by zero")
            if isinstance(node.op, ast.Add):
                value = left + right
            elif isinstance(node.op, ast.Sub):
                value = left - right
            elif isinstance(node.op, ast.Mult):
                value = left * right
            elif isinstance(node.op, ast.Div):
                value = left / right
            else:
                # int() 对 Fraction 向 0 截断。
                value = left - right * int(left / right)
        else:
            raise NoResult("syntax")
        largest = max(largest, abs(value.numerator), value.denominator)
        return value

    result = walk(tree.body)
    return result, operators, largest


def decimal(value: Fraction) -> tuple[str, bool]:
    scaled = abs(value) * 10**DIGITS
    exact = scaled.denominator == 1
    rounded = int(scaled + Fraction(1, 2))
    whole, fraction = divmod(rounded, 10**DIGITS)
    text = str(whole)
    if fraction:
        text += "." + f"{fraction:0{DIGITS}d}".rstrip("0")
    if value < 0 and rounded:
        text = "-" + text
    return text, exact


def grouped(text: str) -> str:
    sign = "-" if text.startswith("-") else ""
    whole, dot, fraction = text.lstrip("-").partition(".")
    return f"{sign}{int(whole):,}{dot}{fraction}"


def expected_rows(argument: str) -> tuple[list[list[str]], str | None, int]:
    expression = argument.rstrip("=")
    if not expression:
        return [], "empty", 0
    try:
        value, operators, largest = evaluate(expression)
    except NoResult as reason:
        return [], str(reason), 0
    text, exact = decimal(value)
    rows = [[text, "结果" if exact else "约等于"]]
    if operators:
        equation = expression + ("=" if exact else "≈") + text
        if len(equation) <= 64:
            rows.append([equation, "算式"])
    if len(str(abs(value.numerator) // value.denominator)) >= 5:
        rows.append([grouped(text), "千分位"])
    return rows, None, largest


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    events, outputs = read(Path(argv[0])), read(Path(argv[1]))
    problems: list[str] = []
    if len(events) != len(outputs):
        problems.append(f"夹具有 {len(events)} 个事件，输出有 {len(outputs)} 行")
    for event, output in zip(events, outputs):
        want, failure, largest = expected_rows(event.get("argument", ""))
        got = output.get("rows")
        logs = [line["text"] for line in output.get("log", [])]
        if got == want:
            continue
        if not got and failure is None and logs == ["skip: overflow"] and largest > LARGE:
            continue
        problems.append(f"seq {output['seq']} {event.get('argument', '')!r}：期望 {want}（{failure or '有结果'}），实际 {got}，日志 {logs}")
    for problem in problems:
        print(f"FAIL calc (fractions): {problem}")
    if not problems:
        print(f"ok calc: {len(outputs)} events match Python fractions")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
