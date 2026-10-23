from pathlib import Path
import hashlib
import json
import re
import subprocess

folder = Path("target/collocation-tail-evidence")
binary = str(folder / "paired-system")
expected = json.loads((folder / "system-binary.json").read_text())["sha256"]
assert hashlib.sha256(Path(binary).read_bytes()).hexdigest() == expected
symbols = subprocess.check_output(["nm", "-n", binary], text=True)
report = {}
for label, ending in [
    ("current", "12msime_engine7lattice6decode12decode_graph"),
    ("old", "22collocation_tail_tests6frozen19frozen_decode_graph"),
]:
    matches = [line.split()[-1] for line in symbols.splitlines() if line.endswith(ending)]
    assert len(matches) == 1
    assembly = subprocess.check_output(
        ["xcrun", "llvm-objdump", "-d", "--disassemble-symbols=" + matches[0], binary], text=True,
    )
    (folder / (label + "-decode.s")).write_text(assembly)
    instructions = []
    for line in assembly.splitlines():
        match = re.match(r"^([0-9a-f]+):\s+[0-9a-f]{8}\s+(\S+)\s*(.*)$", line)
        if match:
            instructions.append((int(match[1], 16), match[2], match[3]))
    assert not any(item[1] == "br" for item in instructions), "不支持未解析的间接跳转"
    addresses = {item[0]: index for index, item in enumerate(instructions)}
    successors = [set() for _ in instructions]
    predecessors = [set() for _ in instructions]
    for index, (_, operation, operand) in enumerate(instructions):
        conditional = operation.startswith("b.") or operation in {"cbz", "cbnz", "tbz", "tbnz"}
        if operation == "b" or conditional:
            targets = re.findall(r"0x[0-9a-f]+", operand.split(" <")[0])
            assert targets
            target = addresses.get(int(targets[-1], 16))
            if target is not None:
                successors[index].add(target)
        if operation not in {"b", "ret", "br", "brk", "udf"} and index + 1 < len(instructions):
            successors[index].add(index + 1)
        for target in successors[index]:
            predecessors[target].add(index)
    # 在指令级控制流上求支配关系，以自然循环区分内层出边和外层假设循环。
    reachable = set()
    pending = [0]
    while pending:
        index = pending.pop()
        if index not in reachable:
            reachable.add(index)
            pending.extend(successors[index])
    all_bits = sum(1 << index for index in reachable)
    dominators = [all_bits] * len(instructions)
    dominators[0] = 1
    changed = True
    while changed:
        changed = False
        for index in sorted(reachable - {0}):
            bits = all_bits
            for previous in predecessors[index] & reachable:
                bits &= dominators[previous]
            bits |= 1 << index
            if bits != dominators[index]:
                dominators[index] = bits
                changed = True
    loops = {}
    for source in reachable:
        for header in successors[source]:
            if dominators[source] & (1 << header):
                members = {header, source}
                pending = [source] if source != header else []
                while pending:
                    for previous in predecessors[pending.pop()] & reachable:
                        if previous not in members:
                            members.add(previous)
                            pending.append(previous)
                loops.setdefault(header, set()).update(members)
    tail_calls = [i for i, (_, op, operand) in enumerate(instructions) if op == "bl" and "16collocation_tail>" in operand]
    assert len(tail_calls) == 2, "默认与宽末列各有一个回溯调用点"
    results = []
    for tail in tail_calls:
        callback = next(i for i in range(tail + 1, len(instructions)) if instructions[i][1] == "blr")
        assert instructions[callback][0] - instructions[tail][0] < 512
        header, members = min(
            ((header, members) for header, members in loops.items() if callback in members),
            key=lambda item: len(item[1]),
        )
        inside = tail in members
        assert inside == (label == "old")
        utf8 = [i for i in range(tail + 1, callback) if instructions[i][1] == "bl" and "9from_utf8>" in instructions[i][2]]
        assert len(utf8) == 1, "尾窗到评分器之间应有一次最终 UTF-8 校验"
        assert (utf8[0] in members) == (label == "old")
        results.append({
            "tail_call": hex(instructions[tail][0]),
            "scorer_call": hex(instructions[callback][0]),
            "innermost_scorer_loop_header": hex(instructions[header][0]),
            "loop_instruction_count": len(members),
            "tail_in_scorer_loop": inside,
            "utf8_call": hex(instructions[utf8[0]][0]),
            "utf8_in_scorer_loop": utf8[0] in members,
        })
    report[label] = results
(folder / "assembly-analysis.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
