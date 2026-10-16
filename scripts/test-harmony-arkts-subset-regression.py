#!/usr/bin/env python3
"""回归检查接口内联类型漏检，以及正常对象值被误报的边界。"""

import importlib.util
import pathlib
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).with_name("test-harmony-arkts-subset.py")
SPEC = importlib.util.spec_from_file_location("harmony_arkts_subset", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SUBSET = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUBSET)
REASON = "object literal used as a type (arkts-no-obj-literals-as-types)"


class InterfaceTypeRegression(unittest.TestCase):
    def scan(self, source: str) -> list[tuple[int, str]]:
        with tempfile.TemporaryDirectory(prefix="msime-arkts-regression-") as directory:
            path = pathlib.Path(directory) / "Synthetic.ets"
            path.write_text(source, encoding="utf-8")
            return SUBSET.findings(path)

    def test_nightly_reply_type(self):
        source = """interface NineKeyShuangpinLayoutReply {
  ok: boolean;
  value?: { keys?: NineKeyShuangpinRawFace[] };
}
"""
        self.assertEqual(self.scan(source), [(3, REASON)])

    def test_named_layout_type(self):
        self.assertEqual(self.scan("""interface RawLayout {
  keys?: NineKeyShuangpinRawFace[];
}
interface LayoutReply {
  ok: boolean;
  value?: RawLayout;
}
"""), [])

    def test_multiline_nested_and_empty_types(self):
        self.assertEqual(self.scan("""interface Reply<T> extends BaseReply<T> {
  value?:
    {
      child: {};
    };
}
"""), [(2, REASON), (4, REASON)])

    def test_values_outside_interface_are_not_types(self):
        self.assertEqual(self.scan("""interface Reply { ok: boolean; }
const reply: Reply = { ok: true };
const serialized: string = JSON.stringify({ value: { keys: [] } });
"""), [])

    def test_comments_and_strings_do_not_change_interface_bounds(self):
        self.assertEqual(self.scan('''// interface Fake { value: {} }
const text: string = "interface Fake { value: {} }";
/* interface Fake { value: {} } */
interface Reply {
  // } interface Fake { value: {} }
  value?: { keys: string[] };
}
const serialized: string = JSON.stringify({ value: { keys: [] } });
'''), [(6, REASON)])

    def test_existing_parameter_and_return_rules(self):
        self.assertEqual(self.scan("""function read(value: { key: string }): void {}
function write(): { key: string } { return { key: '' }; }
"""), [(1, REASON), (2, REASON)])


if __name__ == "__main__":
    unittest.main()
