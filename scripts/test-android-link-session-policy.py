#!/usr/bin/env python3
"""检查 Android 关联登录的 provider、挑战和会话绑定保持同一条链路。"""
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BACKEND = ROOT / "platforms/android/java/app/msime/android/account/BackendAccount.java"
APPLE = ROOT / "platforms/android/java/app/msime/android/account/AppleWebSignIn.java"
SIGN_IN = ROOT / "platforms/android/java/app/msime/android/home/SignIn.java"
LOGIN_SHEET = ROOT / "platforms/android/java/app/msime/android/home/LoginSheet.java"


def main() -> int:
    backend = BACKEND.read_text(encoding="utf-8")
    apple = APPLE.read_text(encoding="utf-8")
    sign_in = SIGN_IN.read_text(encoding="utf-8")
    login_sheet = LOGIN_SHEET.read_text(encoding="utf-8")
    checks = (
        ("public Challenge challenge(String provider, String target, String purpose)",
         "关联挑战必须显式携带用途"),
        ("new Challenge(id, nonce, purpose, session.sessionId())", "提供商挑战必须记录发起会话"),
        ("requireSession(session, \"link\".equals(challenge.purpose()) ? challenge.sessionId() : null)",
         "提供商兑换必须检查发起会话"),
        ("public record EmailChallenge(String id, String purpose, long expiresIn, String sessionId)",
         "邮箱挑战必须记录发起会话"),
        ("KEY_SESSION_ID", "Apple pending 必须保存关联会话"),
        ("pending.sessionId()", "Apple 兑换必须使用 pending 会话"),
        ("challenge(\"google\", null, purpose)", "Google 必须沿用 LoginSheet 的关联用途"),
        ("if (!\"link\".equals(purpose)) bind(context, \"google\")", "关联 Google 不能重绑同步账号"),
        ("SignIn.startGoogle(activity, purpose, this::finishWith)", "登录面板必须把用途传给 Google 流程"),
    )
    failed = [description for source, description in checks
              if source not in (backend + apple + sign_in + login_sheet)]
    if failed:
        for description in failed:
            print(f"缺少关联登录会话约束：{description}")
        return 1
    print("Android link sign-in flows preserve provider purpose and account session")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
