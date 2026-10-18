#!/usr/bin/env python3
"""确保 Android 的对话页（底部「对话」tab，也是试用键盘）默认就能和 AI 对话：有字就能发，编辑草稿时仍能取消请求；用户选不了模型，页面用 Rust 的 chat_models 给的 defaultModel（Rust 的流式对话要求请求带 `model`，#8201 迁移计划 5c）；聊天请求绑定发起它的那个账号会话。"""
from pathlib import Path
import ast
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/ChatFragment.java"
ACCOUNT = ROOT / "platforms/android/java/app/msime/android/account/BackendAccount.java"


class KeyboardTryoutSourceContract(unittest.TestCase):
    def test_editing_during_chat_keeps_stop_enabled(self):
        source = SOURCE.read_text()
        watcher = re.search(
            r"ViewPolicy\.afterTextChanged\(field,\s*text\s*->\s*\{(?P<body>.*?)\n\s*\}\);",
            source,
            re.S,
        )
        self.assertIsNotNone(watcher, "could not locate the tryout draft watcher")
        enabled = re.search(r"ViewPolicy\.setEnabled\(sendAi,\s*(.*?)\);", watcher.group("body"))
        self.assertIsNotNone(enabled, "could not locate the draft action state")
        expression = enabled.group(1).replace("text.length()", "text_length")
        expression = expression.replace("&&", " and ").replace("||", " or ")
        expression = expression.replace("!", " not ").strip()
        tree = ast.parse(expression, mode="eval")
        allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp,
                   ast.Not, ast.Compare, ast.Gt, ast.Name, ast.Load, ast.Constant)
        self.assertTrue(all(isinstance(node, allowed) for node in ast.walk(tree)))
        self.assertTrue(all(node.id in {"sending", "text_length"}
                            for node in ast.walk(tree) if isinstance(node, ast.Name)))
        condition = compile(tree, str(SOURCE), "eval")
        for sending in (False, True):
            for text_length in (0, 12):
                with self.subTest(sending=sending, text_length=text_length):
                    actual = eval(condition, {"__builtins__": {}}, {
                        "sending": sending, "text_length": text_length,
                    })
                    if sending:
                        self.assertTrue(actual, "draft edits must keep Stop enabled")
                    else:
                        # 默认就能和 AI 对话：发送键只看有没有字。
                        self.assertEqual(actual, text_length > 0)

    def test_page_uses_the_default_model(self):
        # 产品决定：用户不能选对话模型。Rust 要求请求带模型，页面就用 chat_models 的 defaultModel，不给选择界面，也不自己挑。
        source = SOURCE.read_text()
        for pattern in (r"ChatModel\b", r"loadModels", r"selectedModel", r"modelPicker"):
            self.assertNotRegex(source, pattern)
        self.assertIn('put("operation", "chat_models")', source)
        self.assertIn('optString("defaultModel"', source)
        self.assertRegex(source, r"RustAccount\.chatStream\(app, body, chosen, reply\.call, reply::append\)")

    def test_chat_is_bound_to_the_page_session(self):
        # tab 关不掉：换账号时页面改绑新会话。发出去的那一次带着发起时的会话，旧账号的请求不会落到新账号上。
        source = SOURCE.read_text()
        dispatch = re.search(
            r"private void dispatchChat\(String text, MaterialButton send\) \{(?P<body>.*?)\n    \}\n",
            source,
            re.S,
        )
        self.assertIsNotNone(dispatch, "could not locate the chat dispatcher")
        body = dispatch.group("body")
        self.assertRegex(body, r"final String session = pageSessionId;")
        guards = re.findall(r"if \(token != generation \|\| !alive\(\)\) return;", body)
        self.assertEqual(len(guards), 2, "both the success and the failure callback drop a stale reply")


if __name__ == "__main__":
    unittest.main()
