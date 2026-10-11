#!/usr/bin/env python3
"""确保 Android 的对话页（底部「对话」tab，也是试用键盘）默认就能和 AI 对话：有字就能发，编辑草稿时仍能取消请求；用哪个模型由服务端决定，页面不加载模型目录、请求里不带 `model`；聊天请求绑定发起它的那个账号会话。"""
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

    def test_page_does_not_choose_a_model(self):
        # 产品决定：用户不能选对话模型，客户端也不替用户挑。页面不加载 `/v1/models`，发送时不传模型。
        source = SOURCE.read_text()
        for pattern in (r"chatModels", r"ChatModel\b", r"\bmodels\b", r"loadModels", r"selectedModel"):
            self.assertNotRegex(source, pattern)
        self.assertRegex(source, r"\.chatStream\(request, reply\.call, reply::append,\s*session\)")

    def test_chat_request_leaves_the_model_to_the_server(self):
        # 后端在请求省略 `model` 时用它配置的默认模型，所以请求体里不放这个字段，也不为挑模型去拉目录。
        account = ACCOUNT.read_text()
        body = re.search(
            r"private static JSONObject chatBody\((?P<params>[^)]*)\) throws Exception \{(?P<body>.*?)\n    \}\n",
            account,
            re.S,
        )
        self.assertIsNotNone(body, "could not locate the chat request body builder")
        self.assertNotIn("model", body.group("params"))
        self.assertNotIn('"model"', body.group("body"))
        self.assertNotIn('"/v1/models"', account)
        self.assertNotRegex(account, r"\bchatModels\b|\bChatModel\b|MAX_CHAT_MODELS")

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
