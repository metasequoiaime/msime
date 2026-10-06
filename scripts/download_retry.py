"""Open a pinned download, retrying the failures a CDN serves transiently.

The ``fetch_*.py`` scripts download release assets from GitHub, whose CDN now and then answers a single request with a 5xx; one such answer used to fail a whole release build (Release Linux run 37259338700, ``HTTP Error 500`` on ``msime-cantonese.db``). Server errors, 429, timeouts and connection failures are retried after ``RETRY_DELAYS``; any other HTTP status (404, 403, ...) fails at once, since asking again cannot change it. Only opening the response is retried: the callers stream into a staged file and verify the size and SHA-256 afterwards, so a body cut off mid-way is still rejected rather than retried.
"""
import sys
import time
import urllib.error
import urllib.request

# Seconds to wait before each retry, so four attempts in all.
RETRY_DELAYS = (2, 5, 15)


def transient(error: Exception) -> bool:
    if isinstance(error, urllib.error.HTTPError):
        return error.code == 429 or error.code >= 500
    return isinstance(error, (urllib.error.URLError, TimeoutError, ConnectionError))


def urlopen(url, timeout: float):
    """``urllib.request.urlopen`` with retries; ``url`` is a URL string or a ``urllib.request.Request``."""
    for delay in (*RETRY_DELAYS, None):
        try:
            return urllib.request.urlopen(url, timeout=timeout)
        except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
            if not transient(error):
                raise
            target = url.full_url if isinstance(url, urllib.request.Request) else url
            detail = ""
            if isinstance(error, urllib.error.HTTPError):
                # GitHub's release downloads redirect to release-assets.githubusercontent.com, so the hop that failed is named; GitHub Support can trace a failure by its X-GitHub-Request-Id.
                request_id = error.headers.get("X-GitHub-Request-Id") if error.headers else None
                detail = f" (from {error.url}" + (f", X-GitHub-Request-Id {request_id}" if request_id else "") + ")"
            outcome = "giving up" if delay is None else f"retrying in {delay}s"
            print(f"  {target}: {type(error).__name__}: {error}{detail}; {outcome}", file=sys.stderr)
            if delay is None:
                raise
            if isinstance(error, urllib.error.HTTPError):
                error.close()
            time.sleep(delay)
