#!/usr/bin/env python3
"""各宿主使用共享云候选默认时限与描述符预算。

云候选等多久是产品的预算，不归某个宿主：参考实现给请求的每个阶段都是 2000 ms（WinHTTP 的 `WinHttpSetTimeouts(2000, 2000, 2000, 2000)`），时限内返回的结果就是用户该看到的候选。这里每个宿主用自己的网络库发请求——Apple 上是 NSURLSession，Windows 上是 libcurl，Android 是 HttpURLConnection，HarmonyOS 是系统 http 模块——编译器拦不住哪一个悄悄换成别的时限，而丢掉的云候选和本来就没有云结果看起来一模一样。

默认时限在 `client-core` 声明，C 头文件及旧宿主兼容常量继续核对默认值。自定义服务由共享描述符给出时限，各宿主读取描述符，不重新指定私有预算。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHARED = ROOT / "crates/client-core/src/cloud/candidates.rs"
HEADER = ROOT / "crates/host-api/include/msime_client.h"
HOSTS = [
    (
        ROOT / "platforms/macos/src/cloud/CloudCandidateRequest.mm",
        ["MSIME_CLOUD_REQUEST_TIMEOUT_MS"],
        # Only the cloud-candidate initialiser. The translation and AI initialisers in the same
        # file set their own deadlines from the descriptors they validate a few lines earlier, so
        # a literal there is the host holding a contract rather than inventing one.
        (r"- \(instancetype\)initWithURL:", r"\n- \(", r"_timeout = ([0-9.]+)\s*;"),
    ),
    (
        ROOT / "platforms/windows/src/candidate/CloudCandidateWorker.cpp",
        ["MSIME_CLOUD_CONNECT_TIMEOUT_MS", "MSIME_CLOUD_REQUEST_TIMEOUT_MS"],
        (None, None, r"CURLOPT_\w*TIMEOUT\w*_MS,\s*([0-9]+)L"),
    ),
    (
        ROOT / "platforms/android/java/app/msime/android/candidate/OnlineCandidateTransport.java",
        [
            "HttpConnectionPolicy.setTimeouts(connection,",
            "HttpConnectionPolicy.setTimeouts(connection, connectTimeout, timeout)",
            'descriptor, "timeout_ms"',
            'descriptor, "connect_timeout_ms"',
        ],
        (r"public static String cloud\(JSONObject descriptor\)", r"\n    (?:public|private|protected) ", r"set(?:Connect|Read)Timeout\((\d+)"),
    ),
    (
        ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets",
        [
            "connectTimeout: descriptor.connect_timeout_ms",
            "readTimeout: descriptor.timeout_ms",
        ],
        (r"private async fetchCloud\(", r"\n  (?:private|public|protected) ", r"(?:connectTimeout|readTimeout):\s*(\d+)"),
    ),
]
# Android 和 HarmonyOS 的云候选时限常量。它们连接和整体共用一个数，所以要同时等于 client-core 的 CONNECT 与 REQUEST。
MOBILE_CONSTANTS = [
    (
        ROOT / "platforms/android/java/app/msime/android/candidate/OnlineCandidatePolicy.java",
        r"CLOUD_TIMEOUT_MILLIS = ([0-9_]+);",
    ),
    (
        ROOT / "platforms/harmony/entry/src/main/ets/keyboard/candidate/OnlineCandidatePolicy.ts",
        r"CLOUD_TIMEOUT_MS: number = ([0-9_]+);",
    ),
]
# What the reference asks for, so a change here is a change against it rather than a typo. Its cloud
# worker fetches over WinHTTP and gives resolve, connect, send and receive 2000 ms each.
REFERENCE = {"CONNECT": 2000, "REQUEST": 2000}

# AI 候选有自己的时间预算。client-providers 的 AI 描述符沿用参考实现的 2500 ms 连接和 8000 ms 总时长，Windows 的 `AiAssistant` 在输入空闲 650 ms 后才发请求（云候选等 500 ms）。Linux 宿主不读这个描述符：在线提供方用它自己的 Python 常量发请求，两个引擎也各自持有空闲计时器，所以这里把那些字面量对照描述符固定下来。
AI_DESCRIPTOR = ROOT / "crates/client-providers/src/ai.rs"
LINUX_PROVIDER = ROOT / "platforms/linux/scripts/msime-linux-online-provider"
LINUX_TRANSPORT = ROOT / "crates/input-runtime/src/providers.rs"
LINUX_IBUS = ROOT / "platforms/linux/src/core/ClientEngine.cpp"
LINUX_FCITX = ROOT / "platforms/linux/fcitx5/FcitxEngine.cpp"
AI_IDLE_DELAY_MS = 650
CLOUD_IDLE_DELAY_MS = 500


def linux_ai_budget(failures: list[str]) -> bool:
    paths = [AI_DESCRIPTOR, LINUX_PROVIDER, LINUX_TRANSPORT, LINUX_IBUS, LINUX_FCITX]
    if not all(path.is_file() for path in paths):
        return False
    descriptor = AI_DESCRIPTOR.read_text(encoding="utf-8")
    # DeepSeek 思考开着时描述符给 30 秒预算（`if thinking {30000} else {8000}`）；Linux 的 provider 脚本恒发 disabled，只会产生普通档，这里取普通档数字核它的预算。
    match = re.search(
        r'"timeout_ms":if thinking \{\d+\} else \{(\d+)\},"connect_timeout_ms":(\d+)'
        r'|"timeout_ms":(\d+),"connect_timeout_ms":(\d+)',
        descriptor,
    )
    if not match:
        failures.append("client-providers' AI descriptor no longer declares timeout_ms and connect_timeout_ms")
        return True
    total_ms, connect_ms = int(match.group(1) or match.group(3)), int(match.group(2) or match.group(4))

    provider = LINUX_PROVIDER.read_text(encoding="utf-8")
    for name, expected_ms in (("AI_REQUEST_TIMEOUT", total_ms), ("AI_CONNECT_TIMEOUT", connect_ms)):
        found = re.search(rf"^{name} = ([0-9.]+)$", provider, re.M)
        if not found or round(float(found.group(1)) * 1000) != expected_ms:
            failures.append(
                f"the Linux online provider's {name} is {found.group(1) if found else None} s, and client-providers' AI descriptor asks for {expected_ms} ms"
            )
    # worker 会拒绝超过上限的期限；上限低于调用方预算会让每个请求在发出前静默失败（ai_polish_test 曾以 8 秒请求撞上 7 秒上限）。账号注册与刷新已经由 Rust host-api 负责，不再占用 Python worker 的预算。
    found = re.search(r"^HTTP_WORKER_MAX_TIMEOUT = ([0-9.]+)$", provider, re.M)
    if not found or float(found.group(1)) * 1000 < max(total_ms, 10000):
        failures.append("the Linux HTTP worker cannot cover the AI and configurable cloud budgets")
    if "not 0 < timeout <= HTTP_WORKER_MAX_TIMEOUT" not in provider:
        failures.append("the Linux HTTP worker no longer validates requests against its declared max timeout")
    # 「水杉账号」释义整条经 msime_client_account 的 translate（会话、令牌、请求和逐项规范化都在 Rust），provider 不再自己取令牌、发 /v1/translate。
    if any(symbol not in provider for symbol in (
            '"msime_client_ensure_anonymous_account_with_agent"',
            '"msime_client_account"')):
        failures.append("the Linux online provider no longer routes account registration and translation through host-api")
    if "msime_client_account_access_token" in provider or "msime_client_account_gloss" in provider:
        failures.append("the Linux online provider takes account tokens again instead of letting msime_client_account translate send the request")
    if "/v1/auth/" in provider or "anonymous-account.json" in provider or "anonymous-session.json" in provider:
        failures.append("the Linux online provider still duplicates Rust account authentication or storage")
    if not re.search(r"fetch\(config\[\"endpoint\"\], AI_REQUEST_TIMEOUT,[^)]*connect_timeout=AI_CONNECT_TIMEOUT\)", provider):
        failures.append("the Linux AI candidate request does not use AI_REQUEST_TIMEOUT and AI_CONNECT_TIMEOUT")

    # socket 总时限至少覆盖 AI 预算，并随较长的云请求预算延长；缓存探测仍不等网络。
    transport = LINUX_TRANSPORT.read_text(encoding="utf-8")
    found = re.search(r"let timeout_ms\s*=\s*if query\.ai_eligible[^{}]*\{\s*(\d+)", transport)
    if not found or int(found.group(1)) <= total_ms:
        failures.append(
            f"the Linux online transport waits {found.group(1) if found else None} ms for an AI reply, which does not outlast the {total_ms} ms budget"
        )
    if "Duration::from_millis(if query.ai_cache_only" not in transport or "request.timeout_ms + 500" not in transport:
        failures.append("the Linux socket deadline no longer handles cache probes and cloud request budgets")

    ibus = LINUX_IBUS.read_text(encoding="utf-8")
    for name, expected in (("kAiIdleDelayMs", AI_IDLE_DELAY_MS), ("kCloudIdleDelayMs", CLOUD_IDLE_DELAY_MS)):
        found = re.search(rf"constexpr guint {name} = (\d+);", ibus)
        if not found or int(found.group(1)) != expected:
            failures.append(
                f"the IBus engine's {name} is {found.group(1) if found else None}, and the reference waits {expected} ms"
            )
    fcitx = LINUX_FCITX.read_text(encoding="utf-8")
    for name, expected in (("ai_due_", AI_IDLE_DELAY_MS), ("online_due_", CLOUD_IDLE_DELAY_MS)):
        found = re.search(rf"{name} = now \+ std::chrono::milliseconds\((\d+)\);", fcitx)
        if not found or int(found.group(1)) != expected:
            failures.append(
                f"the Fcitx5 engine's {name} delay is {found.group(1) if found else None} ms, and the reference waits {expected} ms"
            )
    return True


def declared(path: pathlib.Path, pattern: str) -> dict[str, int]:
    text = path.read_text(encoding="utf-8")
    return {name: int(value) for name, value in re.findall(pattern, text)}


def main() -> int:
    if not SHARED.is_file() or not HEADER.is_file():
        print("skipped: the cloud candidate contract is not present")
        return 0
    failures: list[str] = []

    shared = declared(SHARED, r"pub const (CONNECT|REQUEST)_TIMEOUT_MS: u64 = (\d+)")
    header = declared(HEADER, r"#define MSIME_CLOUD_(CONNECT|REQUEST)_TIMEOUT_MS (\d+)")
    for name, expected in REFERENCE.items():
        if shared.get(name) != expected:
            failures.append(
                f"client-core's {name}_TIMEOUT_MS is {shared.get(name)}, and the reference asks for {expected}"
            )
        if header.get(name) != expected:
            failures.append(
                f"the C header's MSIME_CLOUD_{name}_TIMEOUT_MS is {header.get(name)}, and the reference asks for {expected}"
            )

    for path, pattern in MOBILE_CONSTANTS:
        if not path.is_file():
            continue
        found = re.search(pattern, path.read_text(encoding="utf-8"))
        value = int(found.group(1).replace("_", "")) if found else None
        for name in ("CONNECT", "REQUEST"):
            if value != shared.get(name):
                failures.append(
                    f"{path.relative_to(ROOT)} sets the cloud timeout to {value}, and client-core's {name}_TIMEOUT_MS is {shared.get(name)}"
                )

    for path, required, (scope, scope_end, literal) in HOSTS:
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for name in required:
            if name not in text:
                failures.append(f"{path.relative_to(ROOT)} does not read {name}")
        if scope:
            match = re.search(scope, text)
            if not match:
                failures.append(f"{path.relative_to(ROOT)} no longer has the request this checks")
                continue
            # 范围到下一个方法为止，各语言的方法开头不同，所以结束标记按宿主给。
            rest = text[match.end() :]
            end = re.search(scope_end, rest)
            text = rest[: end.start()] if end else rest
        for value in re.findall(literal, text):
            failures.append(
                f"{path.relative_to(ROOT)} sets this request's deadline to a literal {value}"
            )
        # A literal beside the call is how the two hosts drifted in the first place. Only lines that
        # *set* this request's deadline count: a line that checks a value the shared layer declared
        # in a descriptor is the host holding the contract, which is the opposite of drift.

    linux_ai = linux_ai_budget(failures)

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(
        f"cloud request budget: connect {REFERENCE['CONNECT']} ms, total {REFERENCE['REQUEST']} ms, "
        f"macOS and Windows read the shared declaration, Android and HarmonyOS constants match it"
    )
    if linux_ai:
        print(
            f"linux AI candidate budget: idle {AI_IDLE_DELAY_MS} ms (cloud {CLOUD_IDLE_DELAY_MS} ms), matching client-providers' AI descriptor"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
