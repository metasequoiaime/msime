#!/usr/bin/env python3
"""Locked downloads retry transient failures (5xx, 429, network errors) and fail at once on any other HTTP status."""

import socket
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


def attempts(outcomes):
    """Runs download_retry.urlopen against outcomes (an exception to raise or a value to return), returning (result or exception, calls, sleeps)."""
    queue = list(outcomes)
    calls = []
    sleeps = []

    def urlopen(url, timeout):
        calls.append((url, timeout))
        outcome = queue.pop(0)
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

    for transient in (http_error(500), http_error(503), http_error(429), urllib.error.URLError("reset"), socket.timeout("timed out"), ConnectionResetError()):
        result, calls, sleeps = attempts([transient, "response"])
        if result != "response" or len(calls) != 2 or sleeps != delays[:1]:
            failures.append(f"{transient!r} was not retried once: result={result!r} calls={len(calls)} sleeps={sleeps}")

    result, calls, sleeps = attempts([http_error(502)] * (len(delays) + 1))
    if not (isinstance(result, urllib.error.HTTPError) and result.code == 502) or len(calls) != len(delays) + 1 or sleeps != delays:
        failures.append(f"a persistent 502 was not retried {len(delays)} times and then raised: result={result!r} calls={len(calls)} sleeps={sleeps}")

    for permanent in (404, 403, 410):
        result, calls, sleeps = attempts([http_error(permanent), "response"])
        if not (isinstance(result, urllib.error.HTTPError) and result.code == permanent) or len(calls) != 1 or sleeps:
            failures.append(f"HTTP {permanent} was retried: result={result!r} calls={len(calls)} sleeps={sleeps}")

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
