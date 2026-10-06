#!/usr/bin/env python3
"""Every host waits the same amount of time for a cloud candidate.

云候选等多久是产品的预算，不归某个宿主：参考实现给请求的每个阶段都是 2000 ms（WinHTTP 的 `WinHttpSetTimeouts(2000, 2000, 2000, 2000)`），时限内返回的结果就是用户该看到的候选。这里每个宿主用自己的网络库发请求——Apple 上是 NSURLSession，Windows 上是 libcurl，Android 是 HttpURLConnection，HarmonyOS 是系统 http 模块——编译器拦不住哪一个悄悄换成别的时限，而丢掉的云候选和本来就没有云结果看起来一模一样。

所以数字在 `client-core` 里声明一次，C 头文件为 C++ 与 Objective-C 宿主重复同一对常量；Android 和 HarmonyOS 读不到这两处，各有一个常量，这里核对它们的值与 client-core 相同。同时检查每个宿主发云候选请求的那一段代码只读常量，不另写字面量时限。
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
            "setConnectTimeout(OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS)",
            "setReadTimeout(OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS)",
        ],
        (r"public static String cloud\(String url\)", r"\n    (?:public|private|protected) ", r"set(?:Connect|Read)Timeout\((\d+)"),
    ),
    (
        ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets",
        [
            "connectTimeout: OnlineCandidatePolicy.CLOUD_TIMEOUT_MS",
            "readTimeout: OnlineCandidatePolicy.CLOUD_TIMEOUT_MS",
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

# The AI candidate has its own budget. client-core's AI descriptor carries the reference's 2500 ms connect and 8000 ms total, and Windows AiAssistant waits 650 ms of idle input before asking (the cloud candidate waits 500 ms). The Linux hosts do not read the descriptor: the online provider fetches with its own Python constants and both engines hold their own idle timers, so those literals are pinned here against the descriptor.
AI_DESCRIPTOR = ROOT / "crates/client-core/src/ai.rs"
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
    match = re.search(r'"timeout_ms":(\d+),"connect_timeout_ms":(\d+)', descriptor)
    if not match:
        failures.append("client-core's AI descriptor no longer declares timeout_ms and connect_timeout_ms")
        return True
    total_ms, connect_ms = int(match.group(1)), int(match.group(2))

    provider = LINUX_PROVIDER.read_text(encoding="utf-8")
    for name, expected_ms in (("AI_REQUEST_TIMEOUT", total_ms), ("AI_CONNECT_TIMEOUT", connect_ms)):
        found = re.search(rf"^{name} = ([0-9.]+)$", provider, re.M)
        if not found or round(float(found.group(1)) * 1000) != expected_ms:
            failures.append(
                f"the Linux online provider's {name} is {found.group(1) if found else None} s, and client-core's AI descriptor asks for {expected_ms} ms"
            )
    anonymous = re.search(r"^ANONYMOUS_ACCOUNT_TIMEOUT = ([0-9.]+)$", provider, re.M)
    if not anonymous:
        failures.append("the Linux online provider no longer declares an anonymous account timeout")
    # worker 会拒绝超过上限的期限；上限低于调用方预算会让每个请求在发出前静默失败（ai_polish_test 曾以 8 秒请求撞上 7 秒上限）。
    if "HTTP_WORKER_MAX_TIMEOUT = max(AI_REQUEST_TIMEOUT, ANONYMOUS_ACCOUNT_TIMEOUT)" not in provider:
        failures.append("the Linux HTTP worker max timeout no longer covers both AI and anonymous account budgets")
    if "not 0 < timeout <= HTTP_WORKER_MAX_TIMEOUT" not in provider:
        failures.append("the Linux HTTP worker no longer validates requests against its declared max timeout")
    auth_timeouts = re.findall(
        r'ANONYMOUS_ACCOUNT_ORIGIN \+ "/v1/auth/(?:refresh|challenges|login)",\s*([A-Z_]+)', provider)
    if auth_timeouts != ["ANONYMOUS_ACCOUNT_TIMEOUT"] * 3:
        failures.append("the Linux anonymous account requests do not all use ANONYMOUS_ACCOUNT_TIMEOUT")
    if not re.search(r"fetch\(config\[\"endpoint\"\], AI_REQUEST_TIMEOUT,[^)]*connect_timeout=AI_CONNECT_TIMEOUT\)", provider):
        failures.append("the Linux AI candidate request does not use AI_REQUEST_TIMEOUT and AI_CONNECT_TIMEOUT")

    # The engine's socket deadline must outlast the provider's budget, or a reply the provider accepts at the edge is dropped on the way back.
    transport = LINUX_TRANSPORT.read_text(encoding="utf-8")
    found = re.search(r"ai_cache_only[^{]*\{[^}]*Duration::from_secs\((\d+)\)", transport)
    if not found or int(found.group(1)) * 1000 <= total_ms:
        failures.append(
            f"the Linux online transport waits {found.group(1) if found else None} s for an AI reply, which does not outlast the {total_ms} ms budget"
        )

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
            f"linux AI candidate budget: idle {AI_IDLE_DELAY_MS} ms (cloud {CLOUD_IDLE_DELAY_MS} ms), matching client-core's AI descriptor"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
