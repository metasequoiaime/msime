#!/usr/bin/env python3
"""真实编译器区分独立翻译单元的链接依赖、源码错误和运行失败。"""

import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


def load_runner():
    spec = importlib.util.spec_from_file_location(
        "native_runner", ROOT / "scripts/test-windows-native-run.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class NativeDiagnostics(unittest.TestCase):
    def setUp(self):
        self.runner = load_runner()
        if not shutil.which("git") or self.runner.compiler() is None:
            self.skipTest("缺少 git 或支持 -fdeclspec 的宿主 C++ 编译器")
        self.directory = tempfile.TemporaryDirectory(prefix="native diagnostics ")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.runner.ROOT = self.root
        self.runner.TESTS = self.root / "tests"
        self.runner.TSF_TESTS = self.root / "tsf-tests"
        self.runner.TESTS.mkdir()
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)

    def build(self, code):
        source = self.runner.TESTS / "synthetic.cpp"
        source.write_text(code, encoding="utf-8")
        binary, reason = self.runner.build(source, [], self.root)
        return source, binary, reason

    def test_unlinked_external_belongs_to_native_build(self):
        _, binary, reason = self.build(
            "extern int synthetic_dependency();\n"
            "int main() { return synthetic_dependency(); }\n"
        )
        self.assertIsNone(binary)
        self.assertEqual(reason, "", reason)

    def test_syntax_error_remains_a_failure(self):
        _, binary, reason = self.build("int main() { synthetic syntax error; }\n")
        self.assertIsNone(binary)
        self.assertTrue(reason, "语法错误不能被当成缺少链接依赖")

    def test_linker_words_in_source_error_remain_a_failure(self):
        for message in ("unresolved external symbol", "undefined symbol:"):
            with self.subTest(message=message):
                _, binary, reason = self.build(
                    f'static_assert(false, "{message}");\n'
                    "int main() { return 0; }\n"
                )
                self.assertIsNone(binary)
                self.assertTrue(reason, "源码诊断中的字串不能被当成缺少链接依赖")

    def test_runtime_failure_remains_a_failure(self):
        source, binary, reason = self.build("int main() { return 7; }\n")
        self.assertIsNotNone(binary, reason)
        outcome, _ = self.runner.execute(source, binary)
        self.assertEqual(outcome, "failed")

    def test_standalone_program_runs(self):
        source, binary, reason = self.build("int main() { return 0; }\n")
        self.assertIsNotNone(binary, reason)
        outcome, _ = self.runner.execute(source, binary)
        self.assertEqual(outcome, "passed")

    def test_repository_header_without_include_path_remains_a_failure(self):
        header = self.root / "owned" / "synthetic_owned.h"
        header.parent.mkdir()
        header.write_text("#pragma once\n", encoding="utf-8")
        subprocess.run(
            ["git", "-C", str(self.root), "add", "owned/synthetic_owned.h"], check=True
        )
        _, binary, reason = self.build(
            '#include "synthetic_owned.h"\nint main() { return 0; }\n'
        )
        self.assertIsNone(binary)
        self.assertIn("owned/synthetic_owned.h", reason)


if __name__ == "__main__":
    unittest.main()
