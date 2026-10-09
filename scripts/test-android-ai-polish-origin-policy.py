#!/usr/bin/env python3
"""AI 润色配置直接复用共享端点来源策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/voice/AiPolishConfiguration.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static String credentialOrigin(URI uri)" in source:
        errors.append(f"{SOURCE}: 不应保留 credentialOrigin URI 转发重载")
    if "credentialOrigin(this.endpoint)" in source or "credentialOrigin(validatedEndpoint(endpoint))" in source:
        errors.append(f"{SOURCE}: 不应调用私有 credentialOrigin 转发重载")
    if source.count("AiEndpointPolicy.origin(") < 2:
        errors.append(f"{SOURCE}: AI 润色配置应直接复用 AiEndpointPolicy.origin")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android AI polish configuration uses the shared endpoint-origin policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
