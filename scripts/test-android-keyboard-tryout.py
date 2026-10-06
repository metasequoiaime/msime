#!/usr/bin/env python3
"""确保 Android 试用页默认就能和 AI 对话：有字就能发，编辑草稿时仍能取消请求，模型目录到了会刷新发送键并发出等着的那句。"""
from pathlib import Path
import ast
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/KeyboardTryoutActivity.java"


class KeyboardTryoutSourceContract(unittest.TestCase):
    def test_editing_during_chat_keeps_stop_enabled(self):
        source = SOURCE.read_text()
        watcher = re.search(
            r"void afterTextChanged\(@NonNull Editable text\) \{(?P<body>.*?)\n\s*\}",
            source,
            re.S,
        )
        self.assertIsNotNone(watcher, "could not locate the tryout draft watcher")
        enabled = re.search(r"sendAi\.setEnabled\((.*?)\);", watcher.group("body"))
        self.assertIsNotNone(enabled, "could not locate the draft action state")
        expression = enabled.group(1).replace("models.isEmpty()", "models_empty")
        expression = expression.replace("text.length()", "text_length")
        expression = expression.replace("&&", " and ").replace("||", " or ")
        expression = expression.replace("!", " not ").strip()
        tree = ast.parse(expression, mode="eval")
        allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp,
                   ast.Not, ast.Compare, ast.Gt, ast.Name, ast.Load, ast.Constant)
        self.assertTrue(all(isinstance(node, allowed) for node in ast.walk(tree)))
        self.assertTrue(all(node.id in {"sending", "models_empty", "text_length"}
                            for node in ast.walk(tree) if isinstance(node, ast.Name)))
        condition = compile(tree, str(SOURCE), "eval")
        for sending in (False, True):
            for models_empty in (False, True):
                for text_length in (0, 12):
                    with self.subTest(sending=sending, models_empty=models_empty,
                                      text_length=text_length):
                        actual = eval(condition, {"__builtins__": {}}, {
                            "sending": sending, "models_empty": models_empty,
                            "text_length": text_length,
                        })
                        if sending:
                            self.assertTrue(actual, "draft edits must keep Stop enabled")
                        else:
                            # 默认就能和 AI 对话：发送键只看有没有字，模型目录还没到时发出的那句等目录到了再发。
                            self.assertEqual(actual, text_length > 0)

    def test_model_loader_restores_controls_after_success(self):
        source = SOURCE.read_text()
        success = re.search(
            r"models\.clear\(\);(?P<body>.*?)\n\s*\}\);\n\s*\}\s*catch",
            source,
            re.S,
        )
        self.assertIsNotNone(success, "could not locate the model-load success callback")
        body = success.group("body")
        self.assertRegex(body, r"send\.setEnabled\([^;]*field\.length\(\)\s*>\s*0")
        self.assertRegex(body, r"flushPendingSend\(send\)")


if __name__ == "__main__":
    unittest.main()
