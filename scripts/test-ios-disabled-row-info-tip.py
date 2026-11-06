#!/usr/bin/env python3
"""iOS 设置里变灰的行仍要能点开「?」。

SwiftUI 的 `.disabled` 沿视图树向下传，加在带说明的行（或包着这些行的分区）上时，标题旁的「?」也一起被禁用，点了没反应。而这类说明往往正是在讲这一项为什么现在不能用（真机验收：iPhone XR 上「自动添加标点」「中英混输」等行变灰后「?」点不开）。带说明的行要变灰时用行自己的 `enabled:` 参数（`DesignToggleRow`、`DesignSelectRow`、候选页的 `stepperRow`、标点页的 `smartRule`），只禁用开关或选择本身。

这里扫描 App 的 Swift 源码：找到每个 `.disabled(...)` 修饰的那一段视图表达式，表达式里只要出现不是直接写 nil 的 `info:` 就算违规。`ALLOWED` 里是忙碌时短暂整块禁用的地方（导出、同步、录音进行中），以及禁用时 `info` 本来就是 nil 的行，各自写明原因。
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / "platforms/ios/App/Sources"

# (相对 SOURCES 的路径, `.disabled(` 括号里的条件) -> 允许的原因
ALLOWED = {
    ("settings/DeveloperOptionsView.swift", "exporting || resetting"): "导出或重置进行中短暂禁用，结束后恢复",
    ("settings/FeatureSettingsViews.swift", "busy || recorder.isRecording"): "请求或录音进行中短暂禁用整个分区",
    ("settings/LocalSpeechModelsView.swift", "disabled"): "调用方只在识别或录音进行中传 true",
    ("settings/SettingsSyncView.swift", "busy"): "上传或下载进行中短暂禁用整页",
    ("dictionary/CloudDictionaryFilesView.swift", "busy"): "传输进行中短暂禁用整页",
    ("dictionary/DictionarySyncView.swift", "working"): "同步进行中短暂禁用",
    ("dictionary/CloudDictionaryView.swift", "busy"): "云词库请求进行中短暂禁用整页",
    ("keyboard/CloudClipboardSyncModeView.swift", "disabled"): "调用方只在切换同步方式的请求进行中传 true",
    ("plugins/PluginsSettingsView.swift", "library.working"): "安装或导入进行中短暂禁用",
    ("account/AccountSettingsView.swift", "!signedIn"): "未登录时这几行的 info 是 nil，没有「?」",
}

STRING = re.compile(r'"(?:\\.|[^"\\\n])*"')
INFO = re.compile(r"\binfo:\s*(?!nil\b)")
DISABLED = ".disabled("


def code_only(line: str) -> str:
    """去掉字符串字面量和行注释，只留下用来数括号的代码。"""
    line = STRING.sub('""', line)
    return line.split("//", 1)[0]


def balance(text: str) -> int:
    """闭括号比开括号多几个。"""
    code = code_only(text)
    return sum(code.count(c) for c in ")]}") - sum(code.count(c) for c in "([{")


def indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def condition(line: str, start: int) -> str:
    """`.disabled(` 括号里的条件。"""
    depth = 0
    for index in range(start + len(DISABLED) - 1, len(line)):
        if line[index] == "(":
            depth += 1
        elif line[index] == ")":
            depth -= 1
            if depth == 0:
                return line[start + len(DISABLED):index].strip()
    return line[start + len(DISABLED):].strip()


def modified_expression(lines: list[str], row: int, column: int) -> str:
    """`.disabled` 修饰的那一段视图表达式：从它往回找，括号配平、不是以 `.` 开头的修饰器行、缩进不比它深的那一行就是表达式的开头。只有注释或空白的行跳过，不当作开头，否则行和 `.disabled` 之间夹一句注释就看不到上面的 `info:`。"""
    line = lines[row]
    prefix = line[:column]
    depth = balance(prefix)
    if depth <= 0 and prefix.strip() and not prefix.lstrip().startswith("."):
        return prefix
    parts = [prefix]
    for index in range(row - 1, -1, -1):
        text = lines[index]
        parts.append(text)
        depth += balance(text)
        stripped = code_only(text).strip()
        if depth <= 0 and stripped and not stripped.startswith(".") and indent(text) <= indent(line):
            break
    return "\n".join(reversed(parts))


def violations() -> list[str]:
    found = []
    for path in sorted(SOURCES.rglob("*.swift")):
        relative = path.relative_to(SOURCES).as_posix()
        lines = path.read_text(encoding="utf-8").split("\n")
        for row, line in enumerate(lines):
            start = code_only(line).find(DISABLED)
            if start < 0:
                continue
            # code_only 不改变字符串之前的列位置，但字符串里的内容被清空了，所以在原行里重新定位。
            start = line.find(DISABLED)
            expression = modified_expression(lines, row, start)
            if not INFO.search(expression):
                continue
            if (relative, condition(line, start)) in ALLOWED:
                continue
            found.append(f"{relative}:{row + 1}: {line.strip()}")
    return found


class IOSDisabledRowInfoTip(unittest.TestCase):
    def test_disabled_rows_keep_their_info_tip_reachable(self):
        found = violations()
        self.assertEqual(
            found,
            [],
            "these rows carry an info「?」but are disabled with `.disabled`, which disables the「?」too; "
            "use the row's `enabled:` parameter instead:\n" + "\n".join(found),
        )

    def test_allowlist_entries_still_exist(self):
        for (relative, guard), reason in ALLOWED.items():
            text = (SOURCES / relative).read_text(encoding="utf-8")
            self.assertIn(f".disabled({guard})", text, f"stale allowlist entry ({reason}): {relative}")

    def test_scanner_sees_through_multiline_rows_and_sections(self):
        row = [
            '          DesignToggleRow(title: "中英混输", info: "中文输入时在候选中补充英文单词",',
            '                          isOn: stored("mixed_input", "english", $english))',
            '            .accessibilityIdentifier("mixedEnglish")',
            "            .disabled(!mixedEnabled)",
        ]
        self.assertIn("info:", modified_expression(row, 3, row[3].find(DISABLED)))
        section = [
            "      Section {",
            "        Toggle(isOn: stored(\"a\", $a)) {",
            '          SettingsOptionLabel(title: "数字后直出", info: "数字后保留英文标点")',
            '        }.accessibilityIdentifier("a")',
            "      } footer: {",
            '        Text("以上规则在智能标点开启时生效。")',
            "      }",
            "      .disabled(!smart)",
        ]
        self.assertTrue(INFO.search(modified_expression(section, 7, section[7].find(DISABLED))))
        sibling = [
            '          DesignToggleRow(title: "云候选", info: "说明", isOn: $cloud)',
            "          DesignDivider()",
            '          DesignToggleRow(title: "连续输入", isOn: $on)',
            "            .disabled(!enabled)",
        ]
        self.assertNotIn("info:", modified_expression(sibling, 3, sibling[3].find(DISABLED)))
        # 行和 `.disabled` 之间隔着注释或空行，照样往上找到带 `info:` 的那一行。
        commented = [
            '          DesignToggleRow(title: "符号混输", info: "说明", isOn: $symbols)',
            "            // 关着时变灰",
            "",
            "            .disabled(!on)",
        ]
        self.assertIn("info:", modified_expression(commented, 3, commented[3].find(DISABLED)))
        commented_group = [
            '        DesignGroup(title: "智能标点", info: "说明") {',
            "          rows",
            "        }",
            "        // 智能标点关着时整组变灰",
            "        .disabled(hidden)",
        ]
        self.assertIn("info:", modified_expression(commented_group, 4, commented_group[4].find(DISABLED)))
        nil_info = ['        MeRow(title: "手动同步", info: signedIn ? "说明" : nil)', "        .disabled(!signedIn)"]
        self.assertTrue(INFO.search(modified_expression(nil_info, 1, nil_info[1].find(DISABLED))))


if __name__ == "__main__":
    unittest.main()
