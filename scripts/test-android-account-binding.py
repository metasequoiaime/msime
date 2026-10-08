#!/usr/bin/env python3
"""检查 Android 登录会话替换与云同步账号代数使用同一把绑定锁。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SYNC_API = ROOT / "platforms/android/java/app/msime/android/account/SyncApi.java"
SIGN_IN = ROOT / "platforms/android/java/app/msime/android/home/SignIn.java"


def inside_lock(source: str, expression: str) -> bool:
    """判断一次会话读写是否位于 bindingLock 的 synchronized 块内。"""
    position = source.find(expression)
    if position < 0:
        return False
    lock = source.rfind("synchronized (SyncSwitch.bindingLock()) {", 0, position)
    if lock < 0:
        return False
    opening = source.find("{", lock)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return position < index
    return False


def main() -> int:
    sync_api = SYNC_API.read_text(encoding="utf-8")
    sign_in = SIGN_IN.read_text(encoding="utf-8")
    ok = True
    if not inside_lock(sync_api, "String token = new BackendAccount(application).currentAccessToken(rejected)"):
        print("SyncApi 的同步令牌读取没有绑定账号锁", file=sys.stderr)
        ok = False
    for expression, description in (
        ("new BackendAccount(context).login(started, idToken, userAgent(context))", "Google 登录"),
        ("AppleWebSignIn.complete(context, pending, grant, userAgent(context))", "Apple 登录"),
        ("new BackendAccount(context).verifyEmailCode(challenge, credential, userAgent(context))", "邮箱登录"),
        ("new BackendAccount(context).signOut()", "退出登录"),
    ):
        if not inside_lock(sign_in, expression):
            print(f"{description} 没有与同步绑定锁原子衔接", file=sys.stderr)
            ok = False
    if ok:
        print("Android account session changes and sync token reads share the binding lock")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
