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
            if delay is None or not transient(error):
                raise
            if isinstance(error, urllib.error.HTTPError):
                error.close()
            target = url.full_url if isinstance(url, urllib.request.Request) else url
            print(f"  {target}: {error}; retrying in {delay}s", file=sys.stderr)
            time.sleep(delay)
