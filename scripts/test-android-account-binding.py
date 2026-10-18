#!/usr/bin/env python3
"""检查 Android 的登录、退出与云同步的账号绑定在同一把绑定锁里衔接。

云同步的令牌和账号对齐从 #8201 迁移计划 5d 起由 Rust 的 msime_client_cloud_sync 按会话处理，Java 的同步接口已经删掉，这里只剩登录一侧。
"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
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
    sign_in = SIGN_IN.read_text(encoding="utf-8")
    ok = True
    for expression, description in (
        ("new BackendAccount(context).login(started, idToken)", "Google 登录"),
        ("AppleWebSignIn.complete(context, pending, grant)", "Apple 登录"),
        ("new BackendAccount(context).verifyEmailCode(challenge, credential)", "邮箱登录"),
        ("new BackendAccount(context).signOut()", "退出登录"),
    ):
        if not inside_lock(sign_in, expression):
            print(f"{description} 没有与同步绑定锁原子衔接", file=sys.stderr)
            ok = False
    if ok:
        print("Android sign-in, sign-out and the sync account binding share the binding lock")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
