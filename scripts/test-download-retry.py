#!/usr/bin/env python3
"""Locked downloads retry transient failures (5xx, 429, network errors) and fail at once on any other HTTP status."""

import contextlib
import email.message
import io
import socket
import ssl
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import download_retry  # noqa: E402

URL = "https://example.invalid/synthetic.bin"


def http_error(code):
    return urllib.error.HTTPError(URL, code, "synthetic", {}, None)


def attempts(outcomes, on_call=lambda outcome: outcome):
    """对 outcomes（要抛出的异常或要返回的值）运行 download_retry.urlopen，返回 (结果或异常, 调用记录, 等待记录)；on_call 在每次请求时拿到这次的 outcome，可用来推进假时钟。"""
    queue = list(outcomes)
    calls = []
    sleeps = []

    def urlopen(url, timeout):
        calls.append((url, timeout))
        outcome = on_call(queue.pop(0))
        if isinstance(outcome, BaseException):
            raise outcome
        return outcome

    original_urlopen, original_sleep = urllib.request.urlopen, download_retry.time.sleep
    urllib.request.urlopen, download_retry.time.sleep = urlopen, sleeps.append
    try:
        try:
            result = download_retry.urlopen(URL, timeout=300)
        except Exception as error:
            result = error
    finally:
        urllib.request.urlopen, download_retry.time.sleep = original_urlopen, original_sleep
    return result, calls, sleeps


def main():
    failures = []
    delays = list(download_retry.RETRY_DELAYS)

    for transient in (http_error(500), http_error(503), http_error(504), http_error(429), urllib.error.URLError("reset"), socket.timeout("timed out"), ConnectionResetError()):
        result, calls, sleeps = attempts([transient, "response"])
        if result != "response" or len(calls) != 2 or sleeps != delays[:1]:
            failures.append(f"{transient!r} was not retried once: result={result!r} calls={len(calls)} sleeps={sleeps}")

    result, calls, sleeps = attempts([http_error(502)] * (len(delays) + 1))
    if not (isinstance(result, urllib.error.HTTPError) and result.code == 502) or len(calls) != len(delays) + 1 or sleeps != delays:
        failures.append(f"a persistent 502 was not retried {len(delays)} times and then raised: result={result!r} calls={len(calls)} sleeps={sleeps}")

    # 指数退避：从 2 秒起每次翻倍，单次不超过上限；重试之间合计的等待要盖得住 2026-11-05 那段至少 34 秒的 504，又不能长到拖住作业。
    if delays[0] != 2 or any(later != min(download_retry.MAX_DELAY, earlier * 2) for earlier, later in zip(delays, delays[1:])):
        failures.append(f"the retry delays are not a doubling backoff from 2s capped at {download_retry.MAX_DELAY}s: {delays}")
    if max(delays) != download_retry.MAX_DELAY or not 120 <= sum(delays) <= download_retry.RETRY_BUDGET:
        failures.append(f"the retry delays do not reach the cap or wait between 120s and the budget in all: {delays}")

    # 重放 2026-11-05 iOS CI 的失败：连续四次 504 之后服务恢复，下载成功。
    result, calls, sleeps = attempts([http_error(504)] * 4 + ["response"])
    if result != "response" or len(calls) != 5 or sleeps != delays[:4]:
        failures.append(f"four 504s in a row were not ridden out: result={result!r} calls={len(calls)} sleeps={sleeps}")

    # 证书校验失败不是瞬时故障，重试只会多等。
    certificate = urllib.error.URLError(ssl.SSLCertVerificationError(1, "certificate verify failed"))
    result, calls, sleeps = attempts([certificate, "response"])
    if result is not certificate or len(calls) != 1 or sleeps:
        failures.append(f"a certificate verification failure was retried: result={result!r} calls={len(calls)} sleeps={sleeps}")

    # 每次请求都挂满 300 秒超时时，总时长封顶在 RETRY_BUDGET：第二次请求结束时已用掉 600 秒，再等一次就超出预算，于是放弃，不把八次都试完。
    clock = [0.0]
    original_monotonic = download_retry.time.monotonic
    download_retry.time.monotonic = lambda: clock[0]

    def hang(outcome):
        clock[0] += 300
        return outcome

    try:
        result, calls, sleeps = attempts([socket.timeout("timed out")] * (len(delays) + 1), on_call=hang)
    finally:
        download_retry.time.monotonic = original_monotonic
    if not isinstance(result, socket.timeout) or len(calls) != 2 or sleeps != delays[:1]:
        failures.append(f"hanging attempts were not cut off by the {download_retry.RETRY_BUDGET}s budget: result={result!r} calls={len(calls)} sleeps={sleeps}")

    for permanent in (404, 403, 410):
        result, calls, sleeps = attempts([http_error(permanent), "response"])
        if not (isinstance(result, urllib.error.HTTPError) and result.code == permanent) or len(calls) != 1 or sleeps:
            failures.append(f"HTTP {permanent} was retried: result={result!r} calls={len(calls)} sleeps={sleeps}")

    headers = email.message.Message()
    headers["X-GitHub-Request-Id"] = "ABCD:1234"
    redirected = urllib.error.HTTPError("https://release-assets.example.invalid/blob", 500, "synthetic", headers, None)
    log = io.StringIO()
    with contextlib.redirect_stderr(log):
        attempts([redirected, "response"])
    if "release-assets.example.invalid/blob" not in log.getvalue() or "ABCD:1234" not in log.getvalue():
        failures.append(f"a retried HTTP error did not name the failing hop and its request id: {log.getvalue()!r}")
    log = io.StringIO()
    with contextlib.redirect_stderr(log):
        attempts([redirected] * (len(delays) + 1))
    if "giving up" not in log.getvalue() or log.getvalue().count("ABCD:1234") != len(delays) + 1:
        failures.append(f"the final failure was not logged with its request id: {log.getvalue()!r}")

    request = urllib.request.Request(URL, headers={"User-Agent": "synthetic"})
    queue = [http_error(500), "response"]
    seen = []

    def urlopen(url, timeout):
        seen.append(url)
        outcome = queue.pop(0)
        if isinstance(outcome, BaseException):
            raise outcome
        return outcome

    original_urlopen, original_sleep = urllib.request.urlopen, download_retry.time.sleep
    urllib.request.urlopen, download_retry.time.sleep = urlopen, lambda _seconds: None
    try:
        if download_retry.urlopen(request, timeout=300) != "response" or seen != [request, request]:
            failures.append(f"a Request was not passed through on retry: {seen}")
    finally:
        urllib.request.urlopen, download_retry.time.sleep = original_urlopen, original_sleep

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("download retry ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
